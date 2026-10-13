//! Comandos del área `camera` (fase 3): detección de tablero, calibración de lente y la
//! configuración persistente (calibración + alineación con la cama).
//!
//! El video nunca pasa por aquí: la interfaz corrige cada cuadro con WebGL. Estos comandos
//! reciben fotogramas sueltos (PNG en base64) y guardan lo que no debe perderse al reiniciar.

use crate::engine::write_atomic;
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use image::GrayImage;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};
use tlaku_camera::{CalibrationReport, ImageToBed, LensCalibration, find_chessboard};
use tlaku_core::Point;

/// Archivo de la configuración de cámara dentro de la carpeta de configuración de la app.
const SETUP_FILE: &str = "camera.json";
/// Versión del formato de `camera.json`.
const SETUP_VERSION: u32 = 1;

fn blocking_error(e: tauri::Error) -> String {
    format!("La tarea se interrumpió: {e}")
}

/// Decodifica un PNG (o JPEG/BMP) en base64, con o sin prefijo `data:...;base64,`, a gris.
fn decode_gray(b64: &str) -> Result<GrayImage, String> {
    let raw = b64.split_once(',').map_or(b64, |(_, rest)| rest).trim();
    let bytes = STANDARD
        .decode(raw)
        .map_err(|e| format!("La imagen no es base64 válido: {e}"))?;
    let img = image::load_from_memory(&bytes)
        .map_err(|e| format!("No se pudo decodificar la imagen: {e}"))?;
    Ok(img.to_luma8())
}

fn check_board(board: [u32; 2]) -> Result<(), String> {
    if board[0] < 2 || board[1] < 2 || board[0].max(board[1]) < 3 {
        return Err(format!(
            "El tablero debe tener al menos 2x3 esquinas internas (se pidió {}x{}).",
            board[0], board[1]
        ));
    }
    Ok(())
}

/// Esquinas internas del tablero (fila por fila, en píxeles) o `None` si no se ve completo.
pub fn run_detect_board(image_b64: &str, board: [u32; 2]) -> Result<Option<Vec<[f64; 2]>>, String> {
    check_board(board)?;
    let img = decode_gray(image_b64)?;
    Ok(find_chessboard(&img, board)
        .ok()
        .map(|pts| pts.iter().map(|p| [p.x, p.y]).collect()))
}

/// Calibra la lente con varias vistas del tablero (todas del mismo tamaño).
pub fn run_calibrate(
    images_b64: &[String],
    board: [u32; 2],
    square_mm: f64,
) -> Result<CalibrationReport, String> {
    check_board(board)?;
    if !square_mm.is_finite() || square_mm <= 0.0 {
        return Err("El tamaño del cuadro debe ser mayor que 0 mm.".into());
    }
    if images_b64.is_empty() {
        return Err("Captura al menos 3 vistas del tablero antes de calcular.".into());
    }
    let images = images_b64
        .iter()
        .enumerate()
        .map(|(i, b)| decode_gray(b).map_err(|e| format!("Vista {}: {e}", i + 1)))
        .collect::<Result<Vec<_>, _>>()?;
    tlaku_camera::calibrate_lens(&images, board, square_mm)
}

// La interfaz manda los argumentos en snake_case (`image_png_base64`), como el resto del contrato.
#[tauri::command(rename_all = "snake_case")]
pub async fn detect_board(
    image_png_base64: String,
    board: [u32; 2],
) -> Result<Option<Vec<[f64; 2]>>, String> {
    tauri::async_runtime::spawn_blocking(move || run_detect_board(&image_png_base64, board))
        .await
        .map_err(blocking_error)?
}

#[tauri::command(rename_all = "snake_case")]
pub async fn calibrate_lens(
    images_png_base64: Vec<String>,
    board: [u32; 2],
    square_mm: f64,
) -> Result<CalibrationReport, String> {
    tauri::async_runtime::spawn_blocking(move || {
        run_calibrate(&images_png_base64, board, square_mm)
    })
    .await
    .map_err(blocking_error)?
}

// ---------------------------------------------------------------------------
// Configuración persistente
// ---------------------------------------------------------------------------

/// Alineación manual de 4 puntos entre la imagen ya corregida y la cama.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CameraAlignment {
    /// Tamaño (px) de la imagen corregida sobre la que se marcaron los puntos.
    pub image_size: [u32; 2],
    /// Los 4 puntos marcados, en píxeles de la imagen corregida.
    pub image_px: [[f64; 2]; 4],
    /// Sus coordenadas en la cama (mm de documento), en el mismo orden.
    pub bed_mm: [[f64; 2]; 4],
    /// Homografía píxel -> mm (la recalcula el motor al guardar).
    #[serde(default)]
    pub to_bed: [[f64; 3]; 3],
    /// Error de transferencia (rms) en mm sobre los 4 puntos; ~0 con 4 puntos exactos.
    #[serde(default)]
    pub rms_mm: f64,
}

/// Preferencias de captura y de vista que conviene recordar.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CameraPrefs {
    #[serde(default)]
    pub device_id: Option<String>,
    #[serde(default)]
    pub device_label: Option<String>,
    #[serde(default)]
    pub width: Option<u32>,
    #[serde(default)]
    pub height: Option<u32>,
    /// Opacidad del fondo en el lienzo (0..1).
    pub opacity: f64,
    /// El fondo estaba visible al cerrar.
    pub visible: bool,
}

impl Default for CameraPrefs {
    fn default() -> Self {
        Self {
            device_id: None,
            device_label: None,
            width: None,
            height: None,
            opacity: 0.6,
            visible: false,
        }
    }
}

/// Todo lo que sobrevive al reinicio: calibración de lente, alineación y preferencias.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CameraSetup {
    pub version: u32,
    #[serde(default)]
    pub calibration: Option<LensCalibration>,
    #[serde(default)]
    pub alignment: Option<CameraAlignment>,
    #[serde(default)]
    pub prefs: CameraPrefs,
}

impl Default for CameraSetup {
    fn default() -> Self {
        Self {
            version: SETUP_VERSION,
            calibration: None,
            alignment: None,
            prefs: CameraPrefs::default(),
        }
    }
}

fn pt(p: [f64; 2]) -> Point {
    Point::new(p[0], p[1])
}

/// Valida la configuración y recalcula la homografía de la alineación con tlaku-camera.
pub fn normalize_setup(mut setup: CameraSetup) -> Result<CameraSetup, String> {
    setup.version = SETUP_VERSION;
    if let Some(c) = &setup.calibration {
        c.validate()?;
    }
    if let Some(a) = &mut setup.alignment {
        if a.image_size[0] == 0 || a.image_size[1] == 0 {
            return Err("La alineación tiene un tamaño de imagen nulo.".into());
        }
        let all = a.image_px.iter().chain(a.bed_mm.iter()).flatten();
        if all.clone().any(|v| !v.is_finite()) {
            return Err("La alineación contiene valores no finitos.".into());
        }
        let img: Vec<Point> = a.image_px.iter().copied().map(pt).collect();
        let bed: Vec<Point> = a.bed_mm.iter().copied().map(pt).collect();
        let fit = ImageToBed::fit(&img, &bed, None)
            .map_err(|e| format!("Los 4 puntos no definen una alineación válida: {e}"))?;
        a.to_bed = fit.alignment.to_bed.m;
        a.rms_mm = fit.rms_mm;
        // Una homografía con valores no finitos (puntos casi alineados) no sirve.
        if a.to_bed.iter().flatten().any(|v| !v.is_finite()) {
            return Err(
                "Los 4 puntos están casi alineados; marca las esquinas de un rectángulo.".into(),
            );
        }
    }
    if !(0.0..=1.0).contains(&setup.prefs.opacity) {
        setup.prefs.opacity = setup.prefs.opacity.clamp(0.0, 1.0);
    }
    Ok(setup)
}

pub fn setup_file(config_dir: &Path) -> PathBuf {
    config_dir.join(SETUP_FILE)
}

/// Lee la configuración. Sin archivo, devuelve una vacía. Si el archivo está dañado lo aparta
/// como `camera.json.dañado` (no se pierde) y devuelve una vacía.
pub fn load_setup(config_dir: &Path) -> Result<CameraSetup, String> {
    let file = setup_file(config_dir);
    let text = match fs::read_to_string(&file) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(CameraSetup::default()),
        Err(e) => return Err(format!("{}: {e}", file.display())),
    };
    match serde_json::from_str::<CameraSetup>(&text)
        .map_err(|e| e.to_string())
        .and_then(normalize_setup)
    {
        Ok(setup) => Ok(setup),
        Err(_) => {
            let aside = file.with_file_name(format!("{SETUP_FILE}.dañado"));
            let _ = fs::rename(&file, &aside);
            Ok(CameraSetup::default())
        }
    }
}

/// Valida y guarda; devuelve lo guardado (con la homografía recalculada).
pub fn store_setup(config_dir: &Path, setup: CameraSetup) -> Result<CameraSetup, String> {
    let setup = normalize_setup(setup)?;
    fs::create_dir_all(config_dir).map_err(|e| format!("{}: {e}", config_dir.display()))?;
    let text = serde_json::to_string_pretty(&setup)
        .map_err(|e| format!("No se pudo serializar la configuración de cámara: {e}"))?;
    write_atomic(&setup_file(config_dir), &text)?;
    Ok(setup)
}

fn config_dir(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_config_dir()
        .map_err(|e| format!("No se encontró la carpeta de configuración: {e}"))
}

#[tauri::command]
pub async fn load_camera_setup(app: AppHandle) -> Result<CameraSetup, String> {
    let dir = config_dir(&app)?;
    tauri::async_runtime::spawn_blocking(move || load_setup(&dir))
        .await
        .map_err(blocking_error)?
}

#[tauri::command]
pub async fn save_camera_setup(app: AppHandle, setup: CameraSetup) -> Result<CameraSetup, String> {
    let dir = config_dir(&app)?;
    tauri::async_runtime::spawn_blocking(move || store_setup(&dir, setup))
        .await
        .map_err(blocking_error)?
}

/// Guarda el tablero de calibración (SVG o PDF) que eligió el usuario en el diálogo de guardar.
pub fn write_board_file(path: &str, content_b64: &str) -> Result<(), String> {
    let p = Path::new(path);
    let ext = p
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase);
    if !matches!(ext.as_deref(), Some("svg" | "pdf")) {
        return Err("El tablero solo se guarda como .svg o .pdf.".into());
    }
    let bytes = STANDARD
        .decode(content_b64)
        .map_err(|e| format!("El contenido no es base64 válido: {e}"))?;
    fs::write(p, bytes).map_err(|e| format!("{}: {e}", p.display()))
}

#[tauri::command(rename_all = "snake_case")]
pub async fn save_board_file(path: String, content_base64: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || write_board_file(&path, &content_base64))
        .await
        .map_err(blocking_error)?
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::ImageFormat;
    use std::io::Cursor;
    use std::sync::atomic::{AtomicU32, Ordering};

    const BOARD: [u32; 2] = [9, 6];
    const SQ: f64 = 25.0;

    type Mat = [[f64; 3]; 3];

    fn mul(a: &Mat, b: &Mat) -> Mat {
        let mut m = [[0.0; 3]; 3];
        for (i, row) in m.iter_mut().enumerate() {
            for (j, v) in row.iter_mut().enumerate() {
                *v = (0..3).map(|k| a[i][k] * b[k][j]).sum();
            }
        }
        m
    }

    fn rot(rx: f64, ry: f64, rz: f64) -> Mat {
        let (sx, cx) = rx.to_radians().sin_cos();
        let (sy, cy) = ry.to_radians().sin_cos();
        let (sz, cz) = rz.to_radians().sin_cos();
        let x = [[1.0, 0.0, 0.0], [0.0, cx, -sx], [0.0, sx, cx]];
        let y = [[cy, 0.0, sy], [0.0, 1.0, 0.0], [-sy, 0.0, cy]];
        let z = [[cz, -sz, 0.0], [sz, cz, 0.0], [0.0, 0.0, 1.0]];
        mul(&mul(&z, &y), &x)
    }

    /// Cámara de la prueba: 640x480 con distorsión radial moderada.
    struct Cam {
        fx: f64,
        fy: f64,
        cx: f64,
        cy: f64,
        k1: f64,
    }

    const CAM: Cam = Cam {
        fx: 620.0,
        fy: 625.0,
        cx: 320.0,
        cy: 240.0,
        k1: -0.12,
    };
    const W: u32 = 640;
    const H: u32 = 480;

    /// Pose del tablero: `r` columnas = ejes del tablero en la cámara, `t` = origen del tablero.
    struct Pose {
        r: Mat,
        t: [f64; 3],
    }

    fn pose(deg: (f64, f64, f64), center: (f64, f64, f64)) -> Pose {
        let r = rot(deg.0, deg.1, deg.2);
        // El centro del patrón de esquinas internas queda en `center`.
        let c = [
            f64::from(BOARD[0] - 1) * SQ / 2.0,
            f64::from(BOARD[1] - 1) * SQ / 2.0,
            0.0,
        ];
        let t = [
            center.0 - (r[0][0] * c[0] + r[0][1] * c[1]),
            center.1 - (r[1][0] * c[0] + r[1][1] * c[1]),
            center.2 - (r[2][0] * c[0] + r[2][1] * c[1]),
        ];
        Pose { r, t }
    }

    fn distort(x: f64, y: f64) -> (f64, f64) {
        let q = 1.0 + CAM.k1 * (x * x + y * y);
        (x * q, y * q)
    }

    /// Inversa de `distort` por punto fijo (basta con una distorsión moderada).
    fn undistort(xd: f64, yd: f64) -> (f64, f64) {
        let (mut x, mut y) = (xd, yd);
        for _ in 0..30 {
            let q = 1.0 + CAM.k1 * (x * x + y * y);
            x = xd / q;
            y = yd / q;
        }
        (x, y)
    }

    /// Píxel donde cae el punto (x, y) del plano del tablero (mm).
    fn project(p: &Pose, x: f64, y: f64) -> [f64; 2] {
        let c = |i: usize| p.r[i][0] * x + p.r[i][1] * y + p.t[i];
        let (xd, yd) = distort(c(0) / c(2), c(1) / c(2));
        [CAM.fx * xd + CAM.cx, CAM.fy * yd + CAM.cy]
    }

    fn truth_corners(p: &Pose) -> Vec<[f64; 2]> {
        let mut v = Vec::new();
        for j in 0..BOARD[1] {
            for i in 0..BOARD[0] {
                v.push(project(p, f64::from(i) * SQ, f64::from(j) * SQ));
            }
        }
        v
    }

    /// Dibuja el tablero (10x7 cuadros, margen blanco) por trazado inverso con 3x3 muestras por píxel.
    fn render(p: &Pose) -> GrayImage {
        let normal = [p.r[0][2], p.r[1][2], p.r[2][2]];
        let dist = normal[0] * p.t[0] + normal[1] * p.t[1] + normal[2] * p.t[2];
        let shade = |u: f64, v: f64| -> f64 {
            let (x, y) = undistort((u - CAM.cx) / CAM.fx, (v - CAM.cy) / CAM.fy);
            let ray = [x, y, 1.0];
            let denom = normal[0] * ray[0] + normal[1] * ray[1] + normal[2];
            let s = dist / denom;
            let q = [ray[0] * s - p.t[0], ray[1] * s - p.t[1], s - p.t[2]];
            // Coordenadas en el plano del tablero: proyección sobre sus ejes.
            let bx = p.r[0][0] * q[0] + p.r[1][0] * q[1] + p.r[2][0] * q[2];
            let by = p.r[0][1] * q[0] + p.r[1][1] * q[1] + p.r[2][1] * q[2];
            let (ci, cj) = ((bx / SQ).floor(), (by / SQ).floor());
            let inside = ci >= -1.0
                && ci <= f64::from(BOARD[0]) - 1.0
                && cj >= -1.0
                && cj <= f64::from(BOARD[1]) - 1.0;
            if inside && ((ci + cj) as i64).rem_euclid(2) == 0 {
                30.0
            } else {
                225.0
            }
        };
        let n = 3;
        GrayImage::from_fn(W, H, |i, j| {
            let mut acc = 0.0;
            for sj in 0..n {
                for si in 0..n {
                    let u = f64::from(i) - 0.5 + (f64::from(si) + 0.5) / f64::from(n);
                    let v = f64::from(j) - 0.5 + (f64::from(sj) + 0.5) / f64::from(n);
                    acc += shade(u, v);
                }
            }
            image::Luma([(acc / f64::from(n * n)).round() as u8])
        })
    }

    fn to_b64(img: &GrayImage) -> String {
        let mut buf = Cursor::new(Vec::new());
        img.write_to(&mut buf, ImageFormat::Png).unwrap();
        STANDARD.encode(buf.into_inner())
    }

    fn views() -> Vec<Pose> {
        [
            ((20.0, -15.0, 8.0), (-50.0, -30.0, 480.0)),
            ((-25.0, 12.0, -10.0), (60.0, 40.0, 450.0)),
            ((8.0, 30.0, 20.0), (0.0, 0.0, 520.0)),
            ((-15.0, -25.0, 95.0), (-70.0, 50.0, 540.0)),
            ((30.0, 5.0, -35.0), (40.0, -40.0, 470.0)),
            ((-30.0, 25.0, 10.0), (45.0, 55.0, 500.0)),
            ((12.0, -30.0, 4.0), (-40.0, -45.0, 460.0)),
            ((0.0, 8.0, -6.0), (0.0, 10.0, 430.0)),
        ]
        .into_iter()
        .map(|(d, c)| pose(d, c))
        .collect()
    }

    #[test]
    fn detecta_las_esquinas_de_un_tablero_sintetico() {
        let p = pose((15.0, -20.0, 10.0), (-20.0, 10.0, 480.0));
        let found = run_detect_board(&to_b64(&render(&p)), BOARD)
            .unwrap()
            .expect("el tablero se debe encontrar");
        let truth = truth_corners(&p);
        assert_eq!(found.len(), 54);
        // El orden puede venir girado 180 grados; se compara con la mejor de las dos.
        let err = |order: &dyn Fn(usize) -> usize| {
            let sum: f64 = found
                .iter()
                .enumerate()
                .map(|(i, f)| {
                    let t = truth[order(i)];
                    (f[0] - t[0]).hypot(f[1] - t[1])
                })
                .sum();
            sum / found.len() as f64
        };
        let direct = err(&|i| i);
        let flipped = err(&|i| truth.len() - 1 - i);
        assert!(
            direct.min(flipped) < 0.5,
            "error medio {direct:.3}/{flipped:.3} px"
        );
    }

    #[test]
    fn sin_tablero_devuelve_none() {
        let blank = GrayImage::from_pixel(W, H, image::Luma([128]));
        assert_eq!(run_detect_board(&to_b64(&blank), BOARD).unwrap(), None);
    }

    #[test]
    fn acepta_prefijo_data_url() {
        let blank = GrayImage::from_pixel(W, H, image::Luma([200]));
        let url = format!("data:image/png;base64,{}", to_b64(&blank));
        assert_eq!(run_detect_board(&url, BOARD).unwrap(), None);
    }

    #[test]
    fn rechaza_entradas_invalidas() {
        assert!(
            run_detect_board("###", BOARD)
                .unwrap_err()
                .contains("base64")
        );
        let png_roto = STANDARD.encode(b"no soy una imagen");
        assert!(
            run_detect_board(&png_roto, BOARD)
                .unwrap_err()
                .contains("decodificar")
        );
        assert!(
            run_detect_board("", [1, 1])
                .unwrap_err()
                .contains("al menos")
        );
        assert!(run_calibrate(&[], BOARD, SQ).is_err());
        let img = to_b64(&GrayImage::new(W, H));
        assert!(
            run_calibrate(&[img], BOARD, 0.0)
                .unwrap_err()
                .contains("mayor que 0")
        );
    }

    #[test]
    fn calibra_con_vistas_sinteticas() {
        let imgs: Vec<String> = views().iter().map(|p| to_b64(&render(p))).collect();
        let report = run_calibrate(&imgs, BOARD, SQ).unwrap();
        let c = &report.calibration;
        assert_eq!((c.width, c.height), (W, H));
        assert!(
            report.used_views.len() >= 6,
            "vistas usadas: {:?}",
            report.used_views
        );
        assert!(c.rms_error_px < 0.5, "rms {}", c.rms_error_px);
        assert!((c.fx - CAM.fx).abs() / CAM.fx < 0.02, "fx {}", c.fx);
        assert!((c.fy - CAM.fy).abs() / CAM.fy < 0.02, "fy {}", c.fy);
        assert!((c.k1 - CAM.k1).abs() < 0.05, "k1 {}", c.k1);
        // El informe mantiene la forma de CalibrationReport en types.ts.
        let json = serde_json::to_value(&report).unwrap();
        for key in ["calibration", "warnings", "used_views", "view_rms_px"] {
            assert!(json.get(key).is_some(), "falta {key}");
        }
        assert!(json["calibration"].get("rms_error_px").is_some());
    }

    #[test]
    fn calibrar_exige_vistas_con_tablero() {
        let blank = to_b64(&GrayImage::from_pixel(W, H, image::Luma([128])));
        let err = run_calibrate(&vec![blank; 4], BOARD, SQ).unwrap_err();
        assert!(err.contains("al menos 3"), "{err}");
    }

    static COUNTER: AtomicU32 = AtomicU32::new(0);

    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            let n = COUNTER.fetch_add(1, Ordering::Relaxed);
            let dir = std::env::temp_dir().join(format!("tlaku-camera-{}-{n}", std::process::id()));
            let _ = fs::remove_dir_all(&dir);
            Self(dir)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn sample_setup() -> CameraSetup {
        CameraSetup {
            calibration: Some(LensCalibration {
                width: W,
                height: H,
                fx: 620.0,
                fy: 625.0,
                cx: 320.0,
                cy: 240.0,
                k1: -0.12,
                k2: 0.01,
                p1: 0.0,
                p2: 0.0,
                k3: 0.0,
                rms_error_px: 0.31,
                views_used: 8,
            }),
            // Hoja carta (279.4 x 215.9 mm) vista casi de frente en 640x480.
            alignment: Some(CameraAlignment {
                image_size: [W, H],
                image_px: [[100.0, 400.0], [540.0, 410.0], [530.0, 60.0], [110.0, 70.0]],
                bed_mm: [[10.0, 10.0], [289.4, 10.0], [289.4, 225.9], [10.0, 225.9]],
                to_bed: [[0.0; 3]; 3],
                rms_mm: 0.0,
            }),
            ..CameraSetup::default()
        }
    }

    #[test]
    fn la_configuracion_sobrevive_al_reinicio() {
        let tmp = TempDir::new();
        assert_eq!(load_setup(&tmp.0).unwrap(), CameraSetup::default());
        let saved = store_setup(&tmp.0, sample_setup()).unwrap();
        // "Reiniciar": se lee otra vez desde el disco.
        let loaded = load_setup(&tmp.0).unwrap();
        assert_eq!(loaded, saved);
        let a = loaded.alignment.unwrap();
        assert!(a.rms_mm < 1e-6);
        // La homografía recalculada lleva cada punto marcado a su coordenada de cama.
        let h = tlaku_camera::Homography { m: a.to_bed };
        for (img, bed) in a.image_px.iter().zip(&a.bed_mm) {
            let q = h.apply(Point::new(img[0], img[1]));
            assert!((q.x - bed[0]).hypot(q.y - bed[1]) < 1e-6);
        }
        assert_eq!(loaded.calibration.unwrap().views_used, 8);
    }

    #[test]
    fn rechaza_alineaciones_degeneradas() {
        let tmp = TempDir::new();
        let mut s = sample_setup();
        s.alignment.as_mut().unwrap().image_px =
            [[0.0, 0.0], [10.0, 10.0], [20.0, 20.0], [30.0, 30.0]];
        assert!(store_setup(&tmp.0, s).is_err());
        let mut s = sample_setup();
        s.alignment.as_mut().unwrap().bed_mm[0][0] = f64::NAN;
        assert!(store_setup(&tmp.0, s).is_err());
        let mut s = sample_setup();
        s.calibration.as_mut().unwrap().fx = -1.0;
        assert!(store_setup(&tmp.0, s).is_err());
        assert!(!setup_file(&tmp.0).exists());
    }

    #[test]
    fn un_archivo_danado_se_aparta_sin_perder_nada() {
        let tmp = TempDir::new();
        fs::create_dir_all(&tmp.0).unwrap();
        fs::write(setup_file(&tmp.0), "{ esto no es json").unwrap();
        assert_eq!(load_setup(&tmp.0).unwrap(), CameraSetup::default());
        assert!(tmp.0.join("camera.json.dañado").exists());
        assert!(!setup_file(&tmp.0).exists());
    }

    #[test]
    fn la_opacidad_se_limita() {
        let tmp = TempDir::new();
        let mut s = CameraSetup::default();
        s.prefs.opacity = 7.0;
        assert_eq!(store_setup(&tmp.0, s).unwrap().prefs.opacity, 1.0);
    }

    #[test]
    fn guarda_el_tablero_solo_como_svg_o_pdf() {
        let tmp = TempDir::new();
        fs::create_dir_all(&tmp.0).unwrap();
        let ok = tmp.0.join("tablero.svg");
        write_board_file(ok.to_str().unwrap(), &STANDARD.encode(b"<svg/>")).unwrap();
        assert_eq!(fs::read(&ok).unwrap(), b"<svg/>");
        let bad = tmp.0.join("tablero.sh");
        assert!(write_board_file(bad.to_str().unwrap(), &STANDARD.encode(b"x")).is_err());
        assert!(!bad.exists());
    }
}
