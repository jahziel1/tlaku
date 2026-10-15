//! Paso `headalign` de `hwtest`: «alinear la cámara con el cabezal» con la máquina y la cámara
//! reales, con el láser APAGADO (el único movimiento es `jog_to`, que apaga el láser antes de
//! mover). Corre el mismo flujo que las pruebas con la máquina virtual
//! (`edit::camera::run_head_align_flow`): rejilla, foto en cada punto, ajuste y, si la cámara
//! ve solo parte de la cama, rejillas más finas donde se vio el cabezal.
//!
//! Los movimientos pasan por el comando `jog_to`, la configuración por `load_camera_setup` (y
//! `save_camera_setup` con `TLAKU_HW_ALIGN_SAVE=1`) y las fotos por el comando `fetch_camera_frame`
//! (cámara de red) o por el programa de `TLAKU_HW_SNAP` (cámara USB), como el paso `snap:`.
//!
//! Uso: `headalign[:<lado>[,<velocidad mm/min>]]` (por defecto `3,6000`). Variables de entorno:
//!
//! - `TLAKU_HW_ALIGN_DIR` (obligatoria): carpeta donde se guardan las fotos (`foto_NN_x..._y....jpg`),
//!   `resultado.json` (informe completo), `alineacion.json` y `resumen.txt`.
//! - Cámara: `TLAKU_HW_CAMERA_URL` (red) o `TLAKU_HW_SNAP` + `TLAKU_HW_CAMERA` (nombre de la cámara).
//! - `TLAKU_HW_ALIGN_MARGIN` (mm de margen en la cama, 20), `TLAKU_HW_ALIGN_REGION` (`x0,y0,x1,y1`:
//!   zona que ve la cámara), `TLAKU_HW_SETTLE_MS` (espera tras cada movimiento antes de la foto, 1200).
//! - Paralaje: `TLAKU_HW_CAMERA_HEIGHT` y `TLAKU_HW_FEATURE_HEIGHT` (mm sobre el material) y
//!   `TLAKU_HW_ALIGN_OFFSET` (`dx,dy` mm entre el punto detectado y el láser).
//! - `TLAKU_HW_ALIGN_SAVE=1`: guarda la alineación (y la lente estimada, si no había calibración)
//!   en la configuración de cámara de la app. Sin ella solo se escriben los archivos de la carpeta.
//!
//! Si hay calibración de lente guardada, las fotos se corrigen con ella antes de detectar.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use serde_json::{Value, json};
use tlaku_camera::{LensCalibration, PlanOptions, UndistortMap};

use crate::edit::camera::{
    AlignRig, CameraSetup, FlowEvent, FlowOptions, HeadAlignConfig, HeadAlignReport,
    run_head_align_flow,
};
use crate::smoke::invoke_json;

/// Último reporte de estado de la máquina (`machine://status`).
#[derive(Debug, Clone)]
pub(super) struct Snap {
    pub at: Instant,
    pub state: String,
    pub work_pos: [f64; 2],
}

pub(super) type Shared = Arc<Mutex<Option<Snap>>>;

/// Lee un reporte `machine://status` (JSON) para guardarlo en [`Shared`].
pub(super) fn snap_from_payload(payload: &str) -> Option<Snap> {
    let v: Value = serde_json::from_str(payload).ok()?;
    let pos = v.get("work_pos")?.as_array()?;
    Some(Snap {
        at: Instant::now(),
        state: v.get("state")?.as_str()?.to_string(),
        work_pos: [pos.first()?.as_f64()?, pos.get(1)?.as_f64()?],
    })
}

fn env_f64(name: &str) -> Option<f64> {
    std::env::var(name).ok()?.trim().parse().ok()
}

fn env_nums(name: &str) -> Option<Vec<f64>> {
    let v: Vec<f64> = std::env::var(name)
        .ok()?
        .split(',')
        .filter_map(|x| x.trim().parse().ok())
        .collect();
    (!v.is_empty()).then_some(v)
}

enum Camera {
    Url(String),
    Snap { bin: String, name: String },
}

struct HwRig<'a> {
    handle: &'a tauri::AppHandle,
    wv: &'a tauri::Webview,
    status: Shared,
    speed: f64,
    settle: Duration,
    camera: Camera,
    dir: PathBuf,
    /// Calibración con que se corrigen las fotos (si hay).
    undistort: Option<(UndistortMap, (u32, u32))>,
    target: [f64; 2],
    shots: usize,
}

impl HwRig<'_> {
    fn invoke(&self, cmd: &str, args: Value) -> Result<Value, String> {
        invoke_json(self.handle, self.wv, cmd, args)
    }

    /// Espera a que la máquina esté en reposo en `target` (reportes posteriores al movimiento).
    fn wait_still(&self, since: Instant) -> Result<(), String> {
        let end = Instant::now() + Duration::from_secs(120);
        let mut stable = 0;
        while Instant::now() < end {
            std::thread::sleep(Duration::from_millis(150));
            let snap = self.status.lock().map_err(|e| e.to_string())?.clone();
            let Some(s) = snap.filter(|s| s.at > since) else {
                continue;
            };
            if s.state == "alarm" {
                return Err("la máquina entró en alarma: se detiene el flujo".into());
            }
            let d = (s.work_pos[0] - self.target[0]).hypot(s.work_pos[1] - self.target[1]);
            if s.state == "idle" && d < 0.3 {
                stable += 1;
                if stable >= 2 {
                    return Ok(());
                }
            } else {
                stable = 0;
            }
        }
        Err(format!(
            "el cabezal no llegó a {:?} en 120 s (¿sin homing?)",
            self.target
        ))
    }

    /// Foto cruda de la cámara (bytes de imagen).
    fn snap_bytes(&self) -> Result<Vec<u8>, String> {
        match &self.camera {
            Camera::Url(url) => {
                let r = self.invoke("fetch_camera_frame", json!({ "url": url }))?;
                let b64 = r
                    .get("data_base64")
                    .and_then(Value::as_str)
                    .ok_or("fetch_camera_frame: sin data_base64")?;
                STANDARD.decode(b64).map_err(|e| e.to_string())
            }
            Camera::Snap { bin, name } => {
                let tmp = self.dir.join(".captura.jpg");
                let _ = std::fs::remove_file(&tmp);
                let out = std::process::Command::new(bin)
                    .arg(name)
                    .arg(&tmp)
                    .output()
                    .map_err(|e| format!("{bin}: {e}"))?;
                if !out.status.success() {
                    return Err(String::from_utf8_lossy(&out.stdout).into_owned());
                }
                let bytes = std::fs::read(&tmp).map_err(|e| format!("{}: {e}", tmp.display()))?;
                let _ = std::fs::remove_file(&tmp);
                Ok(bytes)
            }
        }
    }
}

impl AlignRig for HwRig<'_> {
    fn move_to(&mut self, mm: [f64; 2]) -> Result<(), String> {
        self.target = mm;
        let since = Instant::now();
        // `jog_to` apaga el láser (M5) antes de mover; nunca se manda potencia.
        self.invoke(
            "jog_to",
            json!({ "x": mm[0], "y": mm[1], "speed": self.speed }),
        )?;
        self.wait_still(since)?;
        std::thread::sleep(self.settle);
        Ok(())
    }

    fn capture(&mut self) -> Result<String, String> {
        let bytes = self.snap_bytes()?;
        self.shots += 1;
        let name = format!(
            "foto_{:02}_x{:.0}_y{:.0}.jpg",
            self.shots, self.target[0], self.target[1]
        );
        std::fs::write(self.dir.join(&name), &bytes).map_err(|e| e.to_string())?;
        let Some((map, size)) = &self.undistort else {
            return Ok(STANDARD.encode(&bytes));
        };
        let img = image::load_from_memory(&bytes)
            .map_err(|e| e.to_string())?
            .to_rgb8();
        if (img.width(), img.height()) != *size {
            return Err(format!(
                "la foto mide {}x{} y la calibración de lente {}x{}: recalibra o apaga la lente",
                img.width(),
                img.height(),
                size.0,
                size.1
            ));
        }
        let fixed = map.undistort(&img)?;
        let mut png = std::io::Cursor::new(Vec::new());
        fixed
            .write_to(&mut png, image::ImageFormat::Png)
            .map_err(|e| e.to_string())?;
        Ok(STANDARD.encode(png.into_inner()))
    }

    fn event(&mut self, e: &FlowEvent) {
        match e {
            FlowEvent::Round { round, points } => println!(
                "  ronda {round}: {} posiciones (cabezal sin láser)",
                points.len()
            ),
            FlowEvent::Shot { index, total, mm } => {
                println!("  foto {index}/{total} en x={:.1} y={:.1}", mm[0], mm[1])
            }
            FlowEvent::Solved { round, detected } => {
                println!("  ronda {round}: cabezal localizado en {detected} fotos")
            }
        }
    }
}

fn summary(report: &HeadAlignReport) -> String {
    let mut s = format!(
        "método: {:?}; fotos con cabezal localizado: {}; sin él: {}\n",
        report.method, report.detected, report.failed
    );
    for f in &report.frames {
        let what = match (&f.px, &f.failure) {
            (Some(p), _) => format!("px ({:.1}, {:.1}) conf {:.2}", p[0], p[1], f.confidence),
            (None, Some(e)) => format!("NO se vio: {e:?}"),
            _ => String::new(),
        };
        let err = f.error_mm.map_or(String::new(), |e| format!(" error {e:.2} mm"));
        let used = if f.px.is_some() && !f.used { " (descartado)" } else { "" };
        s += &format!(
            "  x={:6.1} y={:6.1}  {what}{err}{used}\n",
            f.machine_mm[0], f.machine_mm[1]
        );
    }
    match (&report.fit, &report.fit_error) {
        (Some(f), _) => {
            s += &format!(
                "ajuste: rms {:.2} mm, validación cruzada rms {:.2} mm (máx {:.2}), calidad {:?}, usados {} de {}\n",
                f.rms_mm,
                f.loo_rms_mm,
                f.loo_max_mm,
                f.quality,
                f.used,
                f.used + f.dropped
            );
            let c = f.covered_mm;
            s += &format!(
                "zona cubierta por los puntos: x {:.0}..{:.0} mm, y {:.0}..{:.0} mm\n",
                c[0], c[2], c[1], c[3]
            );
        }
        (None, e) => s += &format!("SIN ajuste: {}\n", e.clone().unwrap_or_default()),
    }
    if report.lens.is_some() {
        s += "lente aproximada estimada (no había calibración)\n";
    }
    for i in &report.issues {
        s += &format!("aviso: {i:?}\n");
    }
    s
}

fn write(dir: &Path, name: &str, text: &str) -> Result<(), String> {
    std::fs::write(dir.join(name), text).map_err(|e| format!("{name}: {e}"))
}

/// Corre el flujo completo. `args` es lo que sigue a `headalign` (`:3,6000` o vacío).
pub(super) fn run(
    handle: &tauri::AppHandle,
    wv: &tauri::Webview,
    args: &str,
    status: Shared,
    bed: [f64; 2],
) -> Result<Value, String> {
    let nums: Vec<f64> = args
        .trim_start_matches(':')
        .split(',')
        .filter_map(|x| x.trim().parse().ok())
        .collect();
    let grid = nums.first().copied().unwrap_or(3.0) as usize;
    let speed = nums.get(1).copied().unwrap_or(6000.0);
    if !(100.0..=12000.0).contains(&speed) {
        return Err("headalign: velocidad fuera de 100..12000 mm/min".into());
    }
    let dir = PathBuf::from(
        std::env::var("TLAKU_HW_ALIGN_DIR").map_err(|_| "falta TLAKU_HW_ALIGN_DIR")?,
    );
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let camera = match (
        std::env::var("TLAKU_HW_CAMERA_URL"),
        std::env::var("TLAKU_HW_SNAP"),
    ) {
        (Ok(url), _) => Camera::Url(url),
        (_, Ok(bin)) => Camera::Snap {
            bin,
            name: std::env::var("TLAKU_HW_CAMERA")
                .unwrap_or_else(|_| "Creality Falcon Camera".into()),
        },
        _ => return Err("falta TLAKU_HW_CAMERA_URL o TLAKU_HW_SNAP".into()),
    };
    // Configuración de cámara guardada (la misma que lee la app).
    let setup: CameraSetup = serde_json::from_value(invoke_json(
        handle,
        wv,
        "load_camera_setup",
        json!({}),
    )?)
    .map_err(|e| format!("camera.json: {e}"))?;
    let lens: Option<LensCalibration> = setup.calibration.clone();
    let undistort = match &lens {
        Some(l) => Some((
            UndistortMap::new(l, (l.width, l.height))?,
            (l.width, l.height),
        )),
        None => None,
    };
    println!(
        "  lente: {}",
        lens.as_ref().map_or("sin calibración (se estima una)".to_string(), |l| format!(
            "calibrada {}x{}, las fotos se corrigen",
            l.width, l.height
        ))
    );

    let plan = PlanOptions {
        bed,
        grid,
        margin_mm: env_f64("TLAKU_HW_ALIGN_MARGIN").unwrap_or(20.0),
        region: env_nums("TLAKU_HW_ALIGN_REGION")
            .filter(|v| v.len() == 4)
            .map(|v| [v[0], v[1], v[2], v[3]]),
    };
    let mut config = HeadAlignConfig {
        bed,
        lens,
        ..HeadAlignConfig::default()
    };
    if let (Some(cam), Some(feat)) = (
        env_f64("TLAKU_HW_CAMERA_HEIGHT"),
        env_f64("TLAKU_HW_FEATURE_HEIGHT"),
    ) {
        config.parallax = Some(tlaku_camera::Parallax {
            camera_height_mm: cam,
            feature_height_mm: feat,
        });
    }
    if let Some(o) = env_nums("TLAKU_HW_ALIGN_OFFSET").filter(|v| v.len() == 2) {
        config.offset_mm = [o[0], o[1]];
    }
    let mut rig = HwRig {
        handle,
        wv,
        status,
        speed,
        settle: Duration::from_millis(env_f64("TLAKU_HW_SETTLE_MS").unwrap_or(1200.0) as u64),
        camera,
        dir: dir.clone(),
        undistort,
        target: [0.0, 0.0],
        shots: 0,
    };
    let result = run_head_align_flow(&mut rig, &FlowOptions::new(plan, config))?;
    let report = result.report;
    let text = summary(&report);
    println!("{text}");
    write(&dir, "resumen.txt", &text)?;
    write(
        &dir,
        "resultado.json",
        &serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?,
    )?;
    if let Some(a) = &report.alignment {
        write(
            &dir,
            "alineacion.json",
            &serde_json::to_string_pretty(a).map_err(|e| e.to_string())?,
        )?;
    }
    println!("  archivos en {}", dir.display());
    let Some(alignment) = report.alignment.clone() else {
        return Err(format!(
            "sin alineación: {}",
            report.fit_error.clone().unwrap_or_default()
        ));
    };
    if std::env::var("TLAKU_HW_ALIGN_SAVE").as_deref() == Ok("1") {
        let mut setup = setup;
        setup.alignment = Some(alignment);
        if let Some(l) = report.lens.clone() {
            setup.calibration = Some(l);
        }
        invoke_json(handle, wv, "save_camera_setup", json!({ "setup": setup }))?;
        println!("  alineación guardada en la configuración de cámara");
    }
    Ok(json!({
        "fotos": result.frames.len(),
        "rondas_de_afinado": result.rounds,
        "localizado_en": report.detected,
        "rms_mm": report.fit.as_ref().map(|f| f.rms_mm),
        "validacion_cruzada_rms_mm": report.fit.as_ref().map(|f| f.loo_rms_mm),
    }))
}
