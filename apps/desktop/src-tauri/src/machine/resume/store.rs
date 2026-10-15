//! El punto de control en disco: tres archivos en una carpeta propia de la app.
//!
//! - `job.gcode`: el G-code exacto que se envió (se escribe una vez, al empezar);
//! - `meta.json`: quién, cuándo, con qué perfil y origen (se escribe una vez, después del G-code:
//!   sin `meta.json` no hay punto de control);
//! - `progress.json`: el avance (se reescribe cada ~1 s).
//!
//! Todo se escribe a un temporal junto al archivo y se renombra (`engine::write_atomic`): un
//! corte de luz a mitad de la escritura deja el archivo anterior intacto o el temporal suelto, que
//! se ignora. Un archivo que aun así quede ilegible (disco dañado) nunca rompe nada: `load`
//! devuelve lo que se pueda leer.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use tlaku_doc::{DeviceProfile, ResumeKind, ResumeModal, StartFrom};

pub const VERSION: u32 = 1;
const META: &str = "meta.json";
const PROGRESS: &str = "progress.json";
const GCODE: &str = "job.gcode";

/// Milisegundos desde 1970.
pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

/// El origen del trabajo: de dónde salieron las coordenadas del G-code.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OriginInfo {
    pub start_from: StartFrom,
    pub job_origin: u32,
    pub user_origin: Option<[f64; 2]>,
    /// Posición de trabajo de la cabeza al iniciar.
    pub head: Option<[f64; 2]>,
}

/// Cómo se envió un trabajo que es la reanudación de otro. El G-code guardado sigue siendo el
/// original; esto dice qué parte del enviado es preámbulo y desde qué línea del original sigue.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResumeRun {
    /// Primera línea (desde 0) del original que se envió tras el preámbulo.
    pub tail_first: u32,
    /// Líneas del preámbulo (con comentarios).
    pub preamble_len: u32,
    /// Líneas enviables del preámbulo.
    pub preamble_sendable: u32,
    /// Líneas enviables del original antes de `tail_first`.
    pub tail_sendable_before: u32,
    /// Punto de control del que viene.
    pub from_id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CheckpointMeta {
    pub version: u32,
    pub id: String,
    pub kind: ResumeKind,
    pub started_at_ms: u64,
    pub document: Option<String>,
    pub device: DeviceProfile,
    pub port: String,
    pub origin: Option<OriginInfo>,
    /// Líneas enviables del G-code original.
    pub total_lines: u32,
    pub resume: Option<ResumeRun>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CheckpointProgress {
    pub version: u32,
    pub seq: u64,
    /// `ok` recibidos del programa enviado (preámbulo incluido).
    pub acked: u32,
    /// Línea (desde 0) del programa enviado a la que corresponde el último `ok`.
    pub sent_line: Option<u32>,
    /// Líneas entregadas a la máquina, con o sin `ok` (preámbulo incluido). Lo que va entre
    /// `acked` y `sent` está en su memoria y pudo seguir ejecutándose sin la app.
    #[serde(default)]
    pub sent: u32,
    /// Línea (desde 0) del programa enviado de la última línea entregada a la máquina.
    #[serde(default)]
    pub sent_to_line: Option<u32>,
    pub mpos: Option<[f64; 2]>,
    pub wpos: Option<[f64; 2]>,
    pub wco: Option<[f64; 2]>,
    /// Bloques libres del planificador y bytes libres del búfer en el último reporte.
    pub planner_free: Option<u32>,
    pub rx_free: Option<u32>,
    /// Mayores valores vistos de los dos (sirven de capacidad).
    pub planner_cap: Option<u32>,
    pub rx_cap: Option<u32>,
    pub modal: ResumeModal,
    pub t_ms: u64,
}

#[derive(Debug, Clone)]
pub struct Loaded {
    pub meta: CheckpointMeta,
    /// `None` si no hay `progress.json` o no se pudo leer.
    pub progress: Option<CheckpointProgress>,
}

#[derive(Debug)]
pub struct CheckpointStore {
    dir: PathBuf,
}

impl CheckpointStore {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    fn file(&self, name: &str) -> PathBuf {
        self.dir.join(name)
    }

    /// Borra el punto de control (y temporales sueltos). No falla si no hay nada.
    pub fn clear(&self) {
        for name in [META, PROGRESS, GCODE] {
            let _ = fs::remove_file(self.file(name));
            let _ = fs::remove_file(self.file(&format!("{name}.tmp")));
        }
    }

    /// Empieza un punto de control nuevo: borra el anterior, guarda el G-code exacto y después
    /// `meta.json`. `gcode` es `None` para los marcadores de las máquinas que no se reanudan.
    pub fn begin(&self, meta: &CheckpointMeta, gcode: Option<&[String]>) -> Result<(), String> {
        self.clear();
        fs::create_dir_all(&self.dir).map_err(|e| format!("{}: {e}", self.dir.display()))?;
        if let Some(lines) = gcode {
            let mut text = lines.join("\n");
            text.push('\n');
            crate::engine::write_atomic(&self.file(GCODE), &text)?;
        }
        let json = serde_json::to_string(meta).map_err(|e| e.to_string())?;
        crate::engine::write_atomic(&self.file(META), &json)
    }

    pub fn write_progress(&self, p: &CheckpointProgress) -> Result<(), String> {
        let json = serde_json::to_string(p).map_err(|e| e.to_string())?;
        crate::engine::write_atomic(&self.file(PROGRESS), &json)
    }

    /// Lee el punto de control. `None` si no hay, si `meta.json` no se puede leer o si falta el
    /// G-code de un trabajo de G-code. Un `progress.json` ilegible no lo invalida.
    pub fn load(&self) -> Option<Loaded> {
        let meta: CheckpointMeta = serde_json::from_slice(&fs::read(self.file(META)).ok()?).ok()?;
        if meta.version != VERSION {
            return None;
        }
        if meta.kind == ResumeKind::Gcode && !self.file(GCODE).is_file() {
            return None;
        }
        let progress = fs::read(self.file(PROGRESS))
            .ok()
            .and_then(|b| serde_json::from_slice::<CheckpointProgress>(&b).ok())
            .filter(|p| p.version == VERSION);
        Some(Loaded { meta, progress })
    }

    /// El G-code guardado, una línea por elemento.
    pub fn read_gcode(&self) -> Result<Vec<String>, String> {
        let text = fs::read_to_string(self.file(GCODE))
            .map_err(|e| format!("{}: {e}", self.file(GCODE).display()))?;
        Ok(text.lines().map(str::to_string).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    pub fn sample_meta(id: &str) -> CheckpointMeta {
        CheckpointMeta {
            version: VERSION,
            id: id.into(),
            kind: ResumeKind::Gcode,
            started_at_ms: 1,
            document: Some("a.tlaku".into()),
            device: DeviceProfile::for_kind(tlaku_doc::DeviceKind::Grbl),
            port: "virtual:falcon2".into(),
            origin: None,
            total_lines: 3,
            resume: None,
        }
    }

    fn progress(seq: u64, acked: u32) -> CheckpointProgress {
        CheckpointProgress {
            version: VERSION,
            seq,
            acked,
            sent_line: Some(acked),
            sent: acked,
            sent_to_line: Some(acked),
            mpos: Some([1.0, 2.0]),
            wpos: Some([1.0, 2.0]),
            wco: Some([0.0, 0.0]),
            planner_free: Some(15),
            rx_free: Some(127),
            planner_cap: Some(15),
            rx_cap: Some(127),
            modal: ResumeModal::default(),
            t_ms: 5,
        }
    }

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("tlaku-ckpt-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn round_trip_and_clear() {
        let dir = tmp("rt");
        let s = CheckpointStore::new(&dir);
        assert!(s.load().is_none());
        let gcode: Vec<String> = ["G90", "G0 X1", "G1 X2"].map(String::from).to_vec();
        s.begin(&sample_meta("a"), Some(&gcode)).unwrap();
        s.write_progress(&progress(1, 2)).unwrap();
        let l = s.load().unwrap();
        assert_eq!(l.meta.id, "a");
        assert_eq!(l.progress.unwrap().acked, 2);
        assert_eq!(s.read_gcode().unwrap(), gcode);
        s.clear();
        assert!(s.load().is_none());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn a_new_checkpoint_replaces_the_old_one_completely() {
        let dir = tmp("replace");
        let s = CheckpointStore::new(&dir);
        s.begin(&sample_meta("a"), Some(&["G0 X1".to_string()]))
            .unwrap();
        s.write_progress(&progress(9, 1)).unwrap();
        s.begin(&sample_meta("b"), Some(&["G0 X2".to_string()]))
            .unwrap();
        let l = s.load().unwrap();
        assert_eq!(l.meta.id, "b");
        assert!(
            l.progress.is_none(),
            "el avance del trabajo anterior no sobrevive"
        );
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn half_written_files_do_not_break_loading() {
        let dir = tmp("half");
        let s = CheckpointStore::new(&dir);
        s.begin(&sample_meta("a"), Some(&["G0 X1".to_string()]))
            .unwrap();
        s.write_progress(&progress(1, 1)).unwrap();
        // Un corte a mitad de la escritura: temporal truncado junto al archivo bueno.
        fs::write(
            dir.join("progress.json.tmp"),
            br#"{"version":1,"seq":2,"ac"#,
        )
        .unwrap();
        assert_eq!(s.load().unwrap().progress.unwrap().acked, 1);
        // Un progress.json truncado (disco dañado): el punto de control sigue, sin avance.
        fs::write(dir.join("progress.json"), br#"{"version":1,"se"#).unwrap();
        let l = s.load().unwrap();
        assert_eq!(l.meta.id, "a");
        assert!(l.progress.is_none());
        // meta.json truncado: no hay punto de control (y no hay pánico).
        fs::write(dir.join("meta.json"), br#"{"version":1,"id":"a","ki"#).unwrap();
        assert!(s.load().is_none());
        // Sin el G-code tampoco hay nada que reanudar.
        s.begin(&sample_meta("c"), Some(&["G0 X1".to_string()]))
            .unwrap();
        fs::remove_file(dir.join("job.gcode")).unwrap();
        assert!(s.load().is_none());
        let _ = fs::remove_dir_all(dir);
    }
}
