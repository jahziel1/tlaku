//! Comandos del área `remnants` (mejora 7): inventario de retazos para el acomodo.
//!
//! Un retazo es lo que sobra de una hoja: su contorno real (o un rectángulo), el material y el
//! grosor. Se guardan en `remnants.json` dentro de la carpeta de configuración (escritura atómica;
//! un archivo dañado se aparta como `remnants.json.dañado`, uno de versión desconocida da error
//! sin tocarlo). La interfaz carga la lista entera y la guarda entera, como la biblioteca de
//! materiales; aquí solo se valida.
//!
//! El acomodo ofrece los retazos como hojas antes que una hoja nueva (`nest_start` con
//! `remnants`); el sobrante de cada hoja lo calcula `nest_poll` al terminar (`leftovers`).

use crate::engine::write_atomic;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};
use tlaku_i18n::tr;

const FILE: &str = "remnants.json";
pub const VERSION: u32 = 1;
/// Tope de retazos guardados (una biblioteca más grande ya no se puede hojear).
pub const MAX_REMNANTS: usize = 500;
const MAX_NAME_CHARS: usize = 80;
/// Vértices máximos de un retazo (el contorno de un sobrante real tiene pocos).
const MAX_POINTS: usize = 5000;

/// Un retazo guardado. Los contornos van con el mínimo de la caja en el origen (0, 0), en mm.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Remnant {
    pub id: String,
    pub name: String,
    /// Nombre del material (se compara sin distinguir mayúsculas).
    pub material: String,
    /// Grosor en mm (0 = no se indicó).
    pub thickness_mm: f64,
    pub width_mm: f64,
    pub height_mm: f64,
    /// Área neta en mm² (exterior menos huecos).
    pub area_mm2: f64,
    /// `true` si es un rectángulo (los contornos son sus cuatro esquinas).
    pub rect: bool,
    /// Contornos cerrados: el exterior y sus huecos.
    pub paths: Vec<Vec<[f64; 2]>>,
    /// Milisegundos desde 1970.
    pub created_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RemnantLibrary {
    pub version: u32,
    pub remnants: Vec<Remnant>,
}

impl Default for RemnantLibrary {
    fn default() -> Self {
        Self {
            version: VERSION,
            remnants: Vec::new(),
        }
    }
}

fn config_dir(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_config_dir()
        .map_err(crate::edit::config_dir_missing)
}

pub fn library_file(config_dir: &Path) -> PathBuf {
    config_dir.join(FILE)
}

fn positive(x: f64) -> bool {
    x.is_finite() && x > 0.0
}

/// Comprueba lo que el formato no garantiza: ids únicos, nombre, medidas y contornos válidos.
pub fn check(library: &RemnantLibrary) -> Result<(), String> {
    if library.version != VERSION {
        return Err(tr!(
            "desktop.remnants.unsupported_version",
            version = library.version
        ));
    }
    if library.remnants.len() > MAX_REMNANTS {
        return Err(tr!("desktop.remnants.too_many", max = MAX_REMNANTS));
    }
    let mut seen = HashSet::new();
    for (i, r) in library.remnants.iter().enumerate() {
        if r.id.trim().is_empty() {
            return Err(tr!("desktop.remnants.no_id", n = i + 1));
        }
        if !seen.insert(r.id.as_str()) {
            return Err(tr!("desktop.remnants.duplicate_id", id = r.id));
        }
        if r.name.trim().is_empty() || r.name.chars().count() > MAX_NAME_CHARS {
            return Err(tr!(
                "desktop.remnants.bad_name",
                id = r.id,
                max = MAX_NAME_CHARS
            ));
        }
        if !(r.thickness_mm.is_finite() && r.thickness_mm >= 0.0) {
            return Err(tr!("desktop.remnants.bad_thickness", id = r.id));
        }
        let points: usize = r.paths.iter().map(Vec::len).sum();
        let shape_ok = !r.paths.is_empty()
            && points <= MAX_POINTS
            && r.paths
                .iter()
                .all(|p| p.len() >= 3 && p.iter().all(|q| q[0].is_finite() && q[1].is_finite()));
        if !shape_ok || !positive(r.width_mm) || !positive(r.height_mm) || !positive(r.area_mm2) {
            return Err(tr!("desktop.remnants.bad_shape", id = r.id));
        }
    }
    Ok(())
}

pub fn parse(text: &str) -> Result<RemnantLibrary, String> {
    let value: serde_json::Value =
        serde_json::from_str(text).map_err(|e| tr!("desktop.remnants.invalid_file", err = e))?;
    match value.get("version").and_then(serde_json::Value::as_u64) {
        Some(v) if v == u64::from(VERSION) => {}
        Some(v) => return Err(tr!("desktop.remnants.unsupported_version", version = v)),
        None => return Err(tr!("desktop.remnants.invalid_file", err = "version")),
    }
    let library: RemnantLibrary =
        serde_json::from_value(value).map_err(|e| tr!("desktop.remnants.invalid_file", err = e))?;
    check(&library)?;
    Ok(library)
}

/// Lee el inventario. Sin archivo, uno vacío. Un archivo que no es JSON o no tiene la forma
/// esperada se aparta como `remnants.json.dañado` (no se pierde) y se devuelve uno vacío; uno
/// de una versión que esta app no conoce falla sin tocarlo.
pub fn load(config_dir: &Path) -> Result<RemnantLibrary, String> {
    let file = library_file(config_dir);
    let text = match fs::read_to_string(&file) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(RemnantLibrary::default());
        }
        Err(e) => return Err(format!("{}: {e}", file.display())),
    };
    match parse(&text) {
        Ok(library) => Ok(library),
        Err(e) if tlaku_i18n::msg_is(&e, "desktop.remnants.unsupported_version") => Err(e),
        Err(_) => {
            let aside = file.with_file_name(format!("{FILE}.dañado"));
            let _ = fs::rename(&file, &aside);
            Ok(RemnantLibrary::default())
        }
    }
}

/// Valida y guarda el inventario con escritura atómica.
pub fn store(config_dir: &Path, library: &RemnantLibrary) -> Result<(), String> {
    check(library)?;
    fs::create_dir_all(config_dir).map_err(|e| format!("{}: {e}", config_dir.display()))?;
    let json = serde_json::to_string_pretty(library)
        .map_err(|e| tr!("desktop.remnants.invalid_file", err = e))?;
    write_atomic(&library_file(config_dir), &json)
}

/// Inventario de retazos (vacío si no hay archivo).
#[tauri::command]
pub async fn load_remnants(app: AppHandle) -> Result<RemnantLibrary, String> {
    let dir = config_dir(&app)?;
    tauri::async_runtime::spawn_blocking(move || load(&dir))
        .await
        .map_err(crate::edit::task_interrupted)?
}

#[tauri::command]
pub async fn save_remnants(app: AppHandle, library: RemnantLibrary) -> Result<(), String> {
    let dir = config_dir(&app)?;
    tauri::async_runtime::spawn_blocking(move || store(&dir, &library))
        .await
        .map_err(crate::edit::task_interrupted)?
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Tmp(PathBuf);
    impl Tmp {
        fn new(tag: &str) -> Self {
            let dir = std::env::temp_dir().join(format!(
                "tlaku-remnants-{tag}-{}-{:?}",
                std::process::id(),
                std::thread::current().id()
            ));
            let _ = fs::remove_dir_all(&dir);
            fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }
    }
    impl Drop for Tmp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn remnant(id: &str) -> Remnant {
        Remnant {
            id: id.into(),
            name: format!("Retazo {id}"),
            material: "Triplay".into(),
            thickness_mm: 3.0,
            width_mm: 200.0,
            height_mm: 100.0,
            area_mm2: 20000.0,
            rect: true,
            paths: vec![vec![[0.0, 0.0], [200.0, 0.0], [200.0, 100.0], [0.0, 100.0]]],
            created_ms: 1_700_000_000_000,
        }
    }

    fn lib(ids: &[&str]) -> RemnantLibrary {
        RemnantLibrary {
            version: VERSION,
            remnants: ids.iter().map(|i| remnant(i)).collect(),
        }
    }

    #[test]
    fn el_inventario_sobrevive_al_reinicio() {
        let t = Tmp::new("roundtrip");
        assert_eq!(load(&t.0).unwrap(), RemnantLibrary::default());
        let l = lib(&["a", "b"]);
        store(&t.0, &l).unwrap();
        assert_eq!(load(&t.0).unwrap(), l);
    }

    #[test]
    fn un_archivo_danado_se_aparta_sin_perder_nada() {
        let t = Tmp::new("bad");
        fs::write(library_file(&t.0), "no es json").unwrap();
        assert_eq!(load(&t.0).unwrap(), RemnantLibrary::default());
        assert!(t.0.join("remnants.json.dañado").exists());
    }

    #[test]
    fn una_version_desconocida_falla_sin_tocar_el_archivo() {
        let t = Tmp::new("version");
        let text = r#"{"version": 99, "remnants": []}"#;
        fs::write(library_file(&t.0), text).unwrap();
        assert!(load(&t.0).is_err());
        assert_eq!(fs::read_to_string(library_file(&t.0)).unwrap(), text);
    }

    #[test]
    fn rechaza_datos_incompletos_en_espanol() {
        let mut l = lib(&["a", "a"]);
        assert!(check(&l).unwrap_err().contains("repetido"));
        l = lib(&["a"]);
        l.remnants[0].id = " ".into();
        assert!(check(&l).unwrap_err().contains("sin id"));
        l = lib(&["a"]);
        l.remnants[0].name = String::new();
        assert!(check(&l).unwrap_err().contains("nombre"));
        l = lib(&["a"]);
        l.remnants[0].thickness_mm = -1.0;
        assert!(check(&l).unwrap_err().contains("grosor"));
        l = lib(&["a"]);
        l.remnants[0].paths = vec![vec![[0.0, 0.0], [1.0, f64::NAN], [2.0, 2.0]]];
        assert!(check(&l).unwrap_err().contains("contorno"));
        l = lib(&["a"]);
        l.remnants[0].width_mm = 0.0;
        assert!(check(&l).unwrap_err().contains("contorno"));
    }

    #[test]
    fn el_tope_de_retazos_se_respeta() {
        let ids: Vec<String> = (0..=MAX_REMNANTS).map(|i| i.to_string()).collect();
        let refs: Vec<&str> = ids.iter().map(String::as_str).collect();
        assert!(check(&lib(&refs)).unwrap_err().contains("demasiados"));
    }

    #[test]
    fn los_argumentos_llegan_como_los_manda_la_interfaz() {
        let json = r#"{"version":1,"remnants":[{"id":"r1","name":"Sobrante","material":"MDF",
            "thickness_mm":3,"width_mm":100,"height_mm":50,"area_mm2":5000,"rect":true,
            "paths":[[[0,0],[100,0],[100,50],[0,50]]],"created_ms":1}]}"#;
        let l = parse(json).unwrap();
        assert_eq!(l.remnants[0].material, "MDF");
        let out = serde_json::to_value(&l.remnants[0]).unwrap();
        for k in [
            "id",
            "name",
            "material",
            "thickness_mm",
            "width_mm",
            "height_mm",
            "area_mm2",
            "rect",
            "paths",
            "created_ms",
        ] {
            assert!(out.get(k).is_some(), "falta {k}");
        }
    }
}
