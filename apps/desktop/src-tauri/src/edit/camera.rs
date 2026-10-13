//! Comandos del área `camera` (fases 3 y 4): detección de tablero, calibración de lente, marcas
//! grabadas para alinear con la cama, corrección por altura del material y la configuración
//! persistente (calibración + alineación con la cama).
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
use tlaku_camera::{
    CalibrationReport, DetectOptions, HeightModel, HeightSample, Homography, ImageToBed,
    LensCalibration, MarkLayout, MarksReport, detect_marks, find_chessboard, fit_height_model,
};
use tlaku_core::bezpath::BezPath;
use tlaku_core::{Point, Polyline, Rgb};
use tlaku_doc::affine::IDENTITY;
use tlaku_doc::{
    DOC_VERSION, DesignObject, Document, LayerMode, LayerSettings, ObjectKind, VectorObject,
};

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
// Marcas grabadas
// ---------------------------------------------------------------------------

/// Color de la capa de las marcas (azul, que el diseño del usuario rara vez usa).
const MARKS_COLOR: Rgb = Rgb(0, 102, 255);
/// Velocidad (mm/min) y potencia (%) por defecto de las marcas: un trazo tenue que no corta.
const MARKS_SPEED: f64 = 3000.0;
const MARKS_POWER: f64 = 10.0;

fn marks_layout(bed: [f64; 2], count: u32) -> Result<MarkLayout, String> {
    MarkLayout::new(bed, count as usize)
}

/// Documento con 4 o 9 cruces en posiciones conocidas de la cama (coordenadas absolutas),
/// en una capa `line` de baja potencia. La marca de la esquina inferior izquierda lleva
/// además un círculo para fijar la orientación en la detección.
pub fn build_marks_document(bed: [f64; 2], count: u32) -> Result<Document, String> {
    let layout = marks_layout(bed, count)?;
    let mut layer = LayerSettings::for_color(MARKS_COLOR);
    layer.name = "Marcas de cámara".into();
    layer.mode = LayerMode::Line;
    layer.speed = MARKS_SPEED;
    layer.power = MARKS_POWER;
    layer.passes = 1;
    let mut objects: Vec<DesignObject> = Vec::new();
    for (i, _) in layout.marks.iter().enumerate() {
        let paths: Vec<BezPath> = layout
            .strokes()
            .into_iter()
            .filter(|st| st.mark == i)
            .map(|st| {
                BezPath::from(&Polyline {
                    points: st.points,
                    closed: st.closed,
                })
            })
            .collect();
        objects.push(DesignObject {
            id: format!("mark-{}", i + 1),
            layer: MARKS_COLOR,
            transform: IDENTITY,
            kind: ObjectKind::Vector(VectorObject { paths }),
            locked: false,
            hidden: false,
            group: None,
        });
    }
    Ok(Document {
        version: DOC_VERSION,
        objects,
        layers: vec![layer],
        device_id: None,
    })
}

/// Busca las marcas en una foto ya corregida de lente, las asigna a sus posiciones y ajusta la
/// alineación. `prior_to_bed` es una alineación provisional (imagen a mm) que acelera y afianza
/// la asignación; sin ella, el círculo de la marca de origen fija la orientación. Las
/// coordenadas de píxel que entran y salen tienen el origen en el borde de la imagen (la
/// esquina superior izquierda es `(0, 0)`), como el resto de la alineación.
pub fn run_detect_marks(
    image_b64: &str,
    bed: [f64; 2],
    count: u32,
    prior_to_bed: Option<[[f64; 3]; 3]>,
) -> Result<MarksReport, String> {
    let layout = marks_layout(bed, count)?;
    let img = decode_gray(image_b64)?;
    let prior = match prior_to_bed {
        Some(m) if m.iter().flatten().all(|v| v.is_finite()) => Some(Homography { m }),
        _ => None,
    };
    let opts = DetectOptions {
        prior_to_bed: prior,
        pixel_offset: 0.5,
        ..DetectOptions::default()
    };
    detect_marks(&img, &layout, &opts)
}

#[tauri::command(rename_all = "snake_case")]
pub async fn camera_marks(bed: [f64; 2], count: u32) -> Result<Document, String> {
    tauri::async_runtime::spawn_blocking(move || build_marks_document(bed, count))
        .await
        .map_err(blocking_error)?
}

#[tauri::command(rename_all = "snake_case")]
pub async fn detect_camera_marks(
    image_png_base64: String,
    bed: [f64; 2],
    count: u32,
    prior_to_bed: Option<[[f64; 3]; 3]>,
) -> Result<MarksReport, String> {
    tauri::async_runtime::spawn_blocking(move || {
        run_detect_marks(&image_png_base64, bed, count, prior_to_bed)
    })
    .await
    .map_err(blocking_error)?
}

// ---------------------------------------------------------------------------
// Configuración persistente
// ---------------------------------------------------------------------------

/// Cómo se obtuvo la alineación.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AlignmentMethod {
    /// 4 puntos marcados a mano sobre la imagen.
    #[default]
    Manual,
    /// Marcas grabadas, detectadas en la foto (una o varias alturas).
    Marks,
}

/// Marcas medidas a una altura del material: la base de la alineación con marcas.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AlignmentSample {
    /// Altura del plano de las marcas sobre la cama (mm).
    pub height_mm: f64,
    /// Marcas usadas, en píxeles de la imagen corregida.
    pub image_px: Vec<[f64; 2]>,
    /// Sus posiciones en la cama (mm de documento), en el mismo orden.
    pub bed_mm: Vec<[f64; 2]>,
    /// Error de validación cruzada medido al detectar (mm); informativo.
    #[serde(default)]
    pub loo_rms_mm: Option<f64>,
    #[serde(default)]
    pub loo_max_mm: Option<f64>,
}

/// Alineación entre la imagen ya corregida y la cama: manual de 4 puntos o con marcas grabadas.
/// Lo calculado (`to_bed`, `rms_mm`, `height`) lo recalcula el motor al guardar.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CameraAlignment {
    /// Tamaño (px) de la imagen corregida sobre la que se marcaron los puntos.
    pub image_size: [u32; 2],
    /// Los 4 puntos marcados, en píxeles de la imagen corregida. Con marcas grabadas son las
    /// esquinas del rectángulo de las marcas, proyectadas con la alineación.
    pub image_px: [[f64; 2]; 4],
    /// Sus coordenadas en la cama (mm de documento), en el mismo orden.
    pub bed_mm: [[f64; 2]; 4],
    /// Homografía píxel -> mm a la altura `reference_height_mm` (la recalcula el motor).
    #[serde(default)]
    pub to_bed: [[f64; 3]; 3],
    /// Error de transferencia (rms) en mm sobre los puntos; ~0 con 4 puntos exactos.
    #[serde(default)]
    pub rms_mm: f64,
    #[serde(default)]
    pub method: AlignmentMethod,
    /// Altura (mm sobre la cama) del plano al que corresponde `to_bed`.
    #[serde(default)]
    pub reference_height_mm: f64,
    /// Marcas por altura (vacío en la alineación manual).
    #[serde(default)]
    pub samples: Vec<AlignmentSample>,
    /// Modelo `H(h)` para el grosor del material (lo calcula el motor): medido con marcas a
    /// dos alturas, o estimado con la matriz de la cámara si solo hay una alineación.
    #[serde(default)]
    pub height: Option<HeightModel>,
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
    /// Grosor del material actual (mm sobre la cama): el fondo se corrige a esa altura.
    #[serde(default)]
    pub material_height_mm: f64,
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
            material_height_mm: 0.0,
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

/// Máximo de alturas distintas y de marcas por altura que se guardan.
const MAX_SAMPLES: usize = 6;
const MAX_POINTS: usize = 100;
/// Alturas dentro de este margen (mm) cuentan como la misma: la captura nueva reemplaza a la vieja.
const SAME_HEIGHT_MM: f64 = 0.25;

/// Matriz de la cámara de la imagen corregida, `[fx, fy, cx, cy]`, en las coordenadas de la
/// alineación (origen en el borde del píxel, así que el punto principal suma 0.5 antes de
/// escalar). Es la misma que usa el corrector de la interfaz.
fn corrected_camera(c: &LensCalibration, size: [u32; 2]) -> [f64; 4] {
    let sx = f64::from(size[0]) / f64::from(c.width);
    let sy = f64::from(size[1]) / f64::from(c.height);
    [c.fx * sx, c.fy * sy, sx * (c.cx + 0.5), sy * (c.cy + 0.5)]
}

/// Modelo de altura de respaldo con una sola alineación y la matriz de la cámara.
fn modelled_height(
    to_bed: &Homography,
    reference_mm: f64,
    calib: Option<&LensCalibration>,
    size: [u32; 2],
) -> Option<HeightModel> {
    let bed_to_image = to_bed.inverse().ok()?;
    let model =
        HeightModel::from_intrinsics(&bed_to_image, reference_mm, corrected_camera(calib?, size))
            .ok()?;
    let finite = model.gradient.iter().all(|v| v.is_finite())
        && model.bed_to_image.iter().flatten().all(|v| v.is_finite());
    finite.then_some(model)
}

fn points(v: &[[f64; 2]]) -> Vec<Point> {
    v.iter().copied().map(pt).collect()
}

/// Alineación manual: la homografía exacta de los 4 puntos.
fn normalize_manual(a: &mut CameraAlignment) -> Result<(), String> {
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
    a.method = AlignmentMethod::Manual;
    Ok(())
}

/// Alineación con marcas: ajusta a una altura (homografía) o a varias (modelo de altura).
fn normalize_samples(
    a: &mut CameraAlignment,
    calib: Option<&LensCalibration>,
) -> Result<(), String> {
    for s in &a.samples {
        if !s.height_mm.is_finite() || !(-10.0..=500.0).contains(&s.height_mm) {
            return Err("La altura de las marcas debe estar entre -10 y 500 mm.".into());
        }
        if s.image_px.len() != s.bed_mm.len() {
            return Err("Cada altura necesita tantos puntos de imagen como de cama.".into());
        }
        if s.image_px.len() < 4 || s.image_px.len() > MAX_POINTS {
            return Err(format!(
                "Cada altura necesita entre 4 y {MAX_POINTS} marcas (la de {:.1} mm tiene {}).",
                s.height_mm,
                s.image_px.len()
            ));
        }
        if s.image_px
            .iter()
            .chain(&s.bed_mm)
            .flatten()
            .any(|v| !v.is_finite())
        {
            return Err("Las marcas contienen valores no finitos.".into());
        }
    }
    // Ordenadas por altura; una captura a la misma altura reemplaza a la anterior.
    let mut sorted: Vec<AlignmentSample> = Vec::new();
    let mut input = std::mem::take(&mut a.samples);
    input.sort_by(|x, y| x.height_mm.total_cmp(&y.height_mm));
    for s in input {
        match sorted.last_mut() {
            Some(last) if (s.height_mm - last.height_mm).abs() < SAME_HEIGHT_MM => *last = s,
            _ => sorted.push(s),
        }
    }
    if sorted.len() > MAX_SAMPLES {
        return Err(format!("Se guardan como máximo {MAX_SAMPLES} alturas."));
    }
    let lowest = sorted[0].height_mm;
    a.reference_height_mm = lowest;
    a.method = AlignmentMethod::Marks;
    if sorted.len() == 1 {
        let s = &sorted[0];
        let fit = ImageToBed::fit(&points(&s.image_px), &points(&s.bed_mm), None)
            .map_err(|e| format!("Las marcas no definen una alineación válida: {e}"))?;
        a.to_bed = fit.alignment.to_bed.m;
        a.rms_mm = fit.rms_mm;
        a.height = modelled_height(&fit.alignment.to_bed, lowest, calib, a.image_size);
    } else {
        let samples: Vec<HeightSample> = sorted
            .iter()
            .map(|s| HeightSample {
                height_mm: s.height_mm,
                image_px: points(&s.image_px),
                bed_mm: points(&s.bed_mm),
            })
            .collect();
        let fit = fit_height_model(&samples)
            .map_err(|e| format!("Las marcas a varias alturas no definen un modelo: {e}"))?;
        let to_bed = fit.model.image_to_bed_at(lowest)?;
        a.to_bed = to_bed.m;
        a.rms_mm = fit.sample_rms_mm.iter().sum::<f64>() / fit.sample_rms_mm.len() as f64;
        a.height = Some(fit.model);
    }
    if a.to_bed.iter().flatten().any(|v| !v.is_finite()) || !a.rms_mm.is_finite() {
        return Err("Las marcas dan una alineación no finita; vuelve a capturarlas.".into());
    }
    // Las 4 esquinas del rectángulo de las marcas, proyectadas, para mostrarlas y para que las
    // versiones sin marcas sigan viendo una alineación de 4 puntos.
    let bed_pts = points(&sorted[0].bed_mm);
    let (x0, x1) = bed_pts.iter().fold((f64::MAX, f64::MIN), |(lo, hi), p| {
        (lo.min(p.x), hi.max(p.x))
    });
    let (y0, y1) = bed_pts.iter().fold((f64::MAX, f64::MIN), |(lo, hi), p| {
        (lo.min(p.y), hi.max(p.y))
    });
    let to_image = Homography { m: a.to_bed }.inverse()?;
    let corners = [(x0, y0), (x1, y0), (x1, y1), (x0, y1)];
    for (i, (x, y)) in corners.into_iter().enumerate() {
        let q = to_image.apply(Point::new(x, y));
        a.bed_mm[i] = [x, y];
        a.image_px[i] = [q.x, q.y];
    }
    a.samples = sorted;
    Ok(())
}

/// Valida la configuración y recalcula la homografía de la alineación con tlaku-camera.
pub fn normalize_setup(mut setup: CameraSetup) -> Result<CameraSetup, String> {
    setup.version = SETUP_VERSION;
    if let Some(c) = &setup.calibration {
        c.validate()?;
    }
    let calib = setup.calibration.clone();
    if let Some(a) = &mut setup.alignment {
        if a.image_size[0] == 0 || a.image_size[1] == 0 {
            return Err("La alineación tiene un tamaño de imagen nulo.".into());
        }
        if !a.reference_height_mm.is_finite() {
            a.reference_height_mm = 0.0;
        }
        if a.samples.is_empty() {
            let all = a.image_px.iter().chain(a.bed_mm.iter()).flatten();
            if all.clone().any(|v| !v.is_finite()) {
                return Err("La alineación contiene valores no finitos.".into());
            }
            normalize_manual(a)?;
            a.height = modelled_height(
                &Homography { m: a.to_bed },
                a.reference_height_mm,
                calib.as_ref(),
                a.image_size,
            );
        } else {
            normalize_samples(a, calib.as_ref())?;
        }
    }
    if !(0.0..=1.0).contains(&setup.prefs.opacity) {
        setup.prefs.opacity = setup.prefs.opacity.clamp(0.0, 1.0);
    }
    let h = setup.prefs.material_height_mm;
    setup.prefs.material_height_mm = if h.is_finite() {
        h.clamp(0.0, 300.0)
    } else {
        0.0
    };
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
                method: AlignmentMethod::Manual,
                reference_height_mm: 0.0,
                samples: Vec::new(),
                height: None,
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

    // ---- marcas grabadas y altura -------------------------------------------------------

    const BED: [f64; 2] = [300.0, 200.0];

    #[test]
    fn el_documento_de_marcas_tiene_una_capa_line_de_baja_potencia() {
        let doc = build_marks_document(BED, 9).unwrap();
        assert_eq!(doc.version, 2);
        assert_eq!(doc.objects.len(), 9);
        assert_eq!(doc.layers.len(), 1);
        let layer = &doc.layers[0];
        assert_eq!(layer.mode, LayerMode::Line);
        assert!(layer.power <= 15.0 && layer.output);
        for o in &doc.objects {
            assert_eq!(o.layer, layer.color);
            let b = o.bounds().expect("dibuja algo");
            assert!(b.min.x >= 0.0 && b.min.y >= 0.0 && b.max.x <= BED[0] && b.max.y <= BED[1]);
            // El centro de cada marca cae en la rejilla del 7.5 % al 92.5 %.
            let (cx, cy) = ((b.min.x + b.max.x) / 2.0, (b.min.y + b.max.y) / 2.0);
            assert!((22.4..=277.6).contains(&cx) && (14.9..=185.1).contains(&cy));
        }
        // La marca 1 lleva el círculo (3 trazos); las demás, solo la cruz.
        let paths = |i: usize| match &doc.objects[i].kind {
            ObjectKind::Vector(v) => v.paths.len(),
            _ => 0,
        };
        assert_eq!((paths(0), paths(1)), (3, 2));
        // Sobrevive a un viaje por JSON.
        let again = Document::from_json(&doc.to_json().unwrap()).unwrap();
        assert_eq!(again.objects.len(), doc.objects.len());
        assert_eq!(again.layers, doc.layers);
        assert_eq!(build_marks_document(BED, 4).unwrap().objects.len(), 4);
        assert!(build_marks_document(BED, 6).is_err());
        assert!(build_marks_document([20.0, 20.0], 4).is_err());
    }

    /// Cama a imagen (origen en el borde del píxel) de la cama de prueba, con algo de perspectiva.
    fn bed_to_image() -> Homography {
        Homography {
            m: [
                [1.85, 0.06, 40.0],
                [0.03, -1.8, 410.0],
                [2.0e-5, -1.0e-5, 1.0],
            ],
        }
    }

    fn dist_to_segment(p: Point, a: Point, b: Point) -> f64 {
        let (dx, dy) = (b.x - a.x, b.y - a.y);
        let len2 = dx * dx + dy * dy;
        let t = if len2 == 0.0 {
            0.0
        } else {
            (((p.x - a.x) * dx + (p.y - a.y) * dy) / len2).clamp(0.0, 1.0)
        };
        p.dist(Point::new(a.x + t * dx, a.y + t * dy))
    }

    /// Foto sintética (sin lente) de las marcas del documento, con desenfoque y ruido.
    fn render_marks(layout: &MarkLayout, h: &Homography, size: (u32, u32)) -> GrayImage {
        let inv = h.inverse().unwrap();
        let strokes = layout.strokes();
        let mut state = 0x1234_5678_9abc_def1u64;
        let mut noise = move || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            ((state >> 11) as f64 / (1u64 << 53) as f64 - 0.5) * 5.0
        };
        let img = GrayImage::from_fn(size.0, size.1, |i, j| {
            let n = 3;
            let mut dark = 0.0;
            for sj in 0..n {
                for si in 0..n {
                    let u = f64::from(i) + (f64::from(si) + 0.5) / f64::from(n);
                    let v = f64::from(j) + (f64::from(sj) + 0.5) / f64::from(n);
                    let bed = inv.apply(Point::new(u, v));
                    let hit = strokes.iter().any(|st| {
                        let m = layout.marks[st.mark];
                        if (bed.x - m.x).abs() > 9.0 || (bed.y - m.y).abs() > 9.0 {
                            return false;
                        }
                        let pts = &st.points;
                        let last = if st.closed { pts.len() } else { pts.len() - 1 };
                        (0..last)
                            .any(|k| dist_to_segment(bed, pts[k], pts[(k + 1) % pts.len()]) <= 0.2)
                    });
                    if hit {
                        dark += 1.0;
                    }
                }
            }
            let f = dark / f64::from(n * n);
            image::Luma([(205.0 * (1.0 - f) + 60.0 * f) as u8])
        });
        let blurred = image::imageops::blur(&img, 0.8);
        GrayImage::from_fn(size.0, size.1, |i, j| {
            let v = f64::from(blurred.get_pixel(i, j)[0]) + noise();
            image::Luma([v.clamp(0.0, 255.0) as u8])
        })
    }

    #[test]
    fn detecta_las_marcas_de_un_documento_y_mide_la_precision() {
        let layout = MarkLayout::new(BED, 9).unwrap();
        let truth = bed_to_image();
        let png = to_b64(&render_marks(&layout, &truth, (640, 480)));
        let report = run_detect_marks(&png, BED, 9, None).unwrap();
        assert_eq!((report.found, report.used), (9, 9), "{:?}", report.warnings);
        let to_bed = Homography { m: report.to_bed };
        // Compara con la verdad en el sistema de la alineación (origen en el borde del píxel).
        let mut worst: f64 = 0.0;
        for j in 0..=8 {
            for i in 0..=12 {
                let bed = Point::new(f64::from(i) * 25.0, f64::from(j) * 25.0);
                worst = worst.max(to_bed.apply(truth.apply(bed)).dist(bed));
            }
        }
        assert!(worst < 0.3, "error real máximo {worst:.3} mm");
        assert!(report.loo_rms_mm.unwrap() < 0.3);
        // Con una alineación provisional también se encuentra, y se avisa que se usó.
        let prior = truth.inverse().unwrap();
        let with_prior = run_detect_marks(&png, BED, 9, Some(prior.m)).unwrap();
        assert!(with_prior.used_prior && with_prior.used == 9);
        assert!(run_detect_marks(&png, BED, 5, None).is_err());
        let blank = to_b64(&GrayImage::from_pixel(640, 480, image::Luma([150])));
        assert!(
            run_detect_marks(&blank, BED, 9, None)
                .unwrap_err()
                .contains("marcas")
        );
    }

    // Cámara de prueba para el modelo de altura: pinhole inclinada, cama con Y arriba.
    const K: [f64; 4] = [620.0, 625.0, 322.0, 241.0];

    fn matrix_at(h: f64) -> Mat {
        let r = mul(
            &rot(12.0, -8.0, 5.0),
            &[[1.0, 0.0, 0.0], [0.0, -1.0, 0.0], [0.0, 0.0, -1.0]],
        );
        let t = [-140.0, 95.0, 380.0];
        let cols = |c: usize| -> [f64; 3] {
            if c < 2 {
                [r[0][c], r[1][c], r[2][c]]
            } else {
                [t[0] + h * r[0][2], t[1] + h * r[1][2], t[2] + h * r[2][2]]
            }
        };
        let k = [[K[0], 0.0, K[2]], [0.0, K[1], K[3]], [0.0, 0.0, 1.0]];
        let mut m = [[0.0; 3]; 3];
        for c in 0..3 {
            let col = cols(c);
            for (i, row) in m.iter_mut().enumerate() {
                row[c] = (0..3).map(|q| k[i][q] * col[q]).sum();
            }
        }
        m
    }

    fn sample_at(h: f64) -> AlignmentSample {
        let truth = Homography { m: matrix_at(h) };
        let bed: Vec<[f64; 2]> = (0..9)
            .map(|k| {
                [
                    25.0 + 125.0 * f64::from(k % 3),
                    20.0 + 80.0 * f64::from(k / 3),
                ]
            })
            .collect();
        let image_px = bed
            .iter()
            .map(|b| {
                let q = truth.apply(pt(*b));
                [q.x, q.y]
            })
            .collect();
        AlignmentSample {
            height_mm: h,
            image_px,
            bed_mm: bed,
            loo_rms_mm: None,
            loo_max_mm: None,
        }
    }

    fn marks_alignment(samples: Vec<AlignmentSample>) -> CameraAlignment {
        CameraAlignment {
            image_size: [W, H],
            image_px: [[0.0; 2]; 4],
            bed_mm: [[0.0; 2]; 4],
            to_bed: [[0.0; 3]; 3],
            rms_mm: 0.0,
            method: AlignmentMethod::Marks,
            reference_height_mm: 0.0,
            samples,
            height: None,
        }
    }

    /// Calibración cuya matriz corregida coincide con `K` (el punto principal va en el borde del píxel).
    fn lens_for_k() -> LensCalibration {
        LensCalibration {
            width: W,
            height: H,
            fx: K[0],
            fy: K[1],
            cx: K[2] - 0.5,
            cy: K[3] - 0.5,
            k1: 0.0,
            k2: 0.0,
            p1: 0.0,
            p2: 0.0,
            k3: 0.0,
            rms_error_px: 0.2,
            views_used: 8,
        }
    }

    fn bed_error_at(a: &CameraAlignment, h: f64) -> f64 {
        let model = a.height.expect("hay modelo de altura");
        let truth = Homography { m: matrix_at(h) };
        let mut worst: f64 = 0.0;
        for j in 0..=8 {
            for i in 0..=12 {
                let bed = Point::new(f64::from(i) * 25.0, f64::from(j) * 25.0);
                let est = model.pixel_to_bed(truth.apply(bed), h).unwrap();
                worst = worst.max(est.dist(bed));
            }
        }
        worst
    }

    #[test]
    fn dos_alturas_dan_un_modelo_medido_que_interpola() {
        let tmp = TempDir::new();
        let mut setup = sample_setup();
        setup.alignment = Some(marks_alignment(vec![sample_at(20.0), sample_at(0.0)]));
        setup.prefs.material_height_mm = 7.5;
        let saved = store_setup(&tmp.0, setup).unwrap();
        let a = saved.alignment.as_ref().unwrap();
        assert_eq!(a.method, AlignmentMethod::Marks);
        assert_eq!(a.samples.len(), 2);
        assert_eq!(a.samples[0].height_mm, 0.0, "ordenadas por altura");
        let model = a.height.expect("modelo");
        assert_eq!(model.source, tlaku_camera::HeightSource::Measured);
        assert!(a.rms_mm < 1e-6);
        for h in [0.0, 3.0, 7.5, 12.0, 20.0] {
            assert!(bed_error_at(a, h) < 1e-5, "h {h}");
        }
        // `to_bed` corresponde a la altura de referencia (la más baja).
        assert_eq!(a.reference_height_mm, 0.0);
        let truth = Homography { m: matrix_at(0.0) };
        let q = Homography { m: a.to_bed }.apply(truth.apply(Point::new(100.0, 60.0)));
        assert!(q.dist(Point::new(100.0, 60.0)) < 1e-5);
        // Las 4 esquinas de mostrar son las del rectángulo de las marcas.
        assert_eq!(a.bed_mm[0], [25.0, 20.0]);
        assert_eq!(a.bed_mm[2], [275.0, 180.0]);
        // Persiste y se recarga igual; el grosor del material también.
        // (el JSON puede redondear el último decimal: se compara con tolerancia)
        let loaded = load_setup(&tmp.0).unwrap();
        assert_eq!(loaded.prefs, saved.prefs);
        assert_eq!(loaded.prefs.material_height_mm, 7.5);
        let b = loaded.alignment.as_ref().unwrap();
        assert_eq!((b.method, b.samples.len()), (a.method, a.samples.len()));
        assert_eq!(b.height.map(|m| m.source), a.height.map(|m| m.source));
        for h in [0.0, 7.5, 20.0] {
            assert!(bed_error_at(b, h) < 1e-5, "recargado, h {h}");
        }
    }

    #[test]
    fn una_sola_altura_con_lente_usa_el_modelo_de_la_camara() {
        let mut setup = sample_setup();
        setup.calibration = Some(lens_for_k());
        setup.alignment = Some(marks_alignment(vec![sample_at(0.0)]));
        let tmp = TempDir::new();
        let saved = store_setup(&tmp.0, setup).unwrap();
        let a = saved.alignment.unwrap();
        assert_eq!(
            a.height.unwrap().source,
            tlaku_camera::HeightSource::Modelled
        );
        // Con la matriz exacta, el respaldo predice bien las demás alturas.
        for h in [5.0, 20.0] {
            assert!(bed_error_at(&a, h) < 1e-4, "h {h}: {}", bed_error_at(&a, h));
        }
        // Sin lente no hay respaldo: queda solo la homografía de esa altura.
        let mut no_lens = sample_setup();
        no_lens.calibration = None;
        no_lens.alignment = Some(marks_alignment(vec![sample_at(0.0)]));
        let a = store_setup(&tmp.0, no_lens).unwrap().alignment.unwrap();
        assert!(a.height.is_none());
        assert!(a.rms_mm < 1e-6);
    }

    #[test]
    fn la_alineacion_manual_tambien_recibe_el_respaldo_de_altura() {
        let mut setup = sample_setup();
        setup.calibration = Some(lens_for_k());
        let tmp = TempDir::new();
        let a = store_setup(&tmp.0, setup).unwrap().alignment.unwrap();
        assert_eq!(a.method, AlignmentMethod::Manual);
        assert_eq!(
            a.height.map(|m| m.source),
            Some(tlaku_camera::HeightSource::Modelled)
        );
    }

    #[test]
    fn una_captura_a_la_misma_altura_reemplaza_a_la_anterior() {
        let mut older = sample_at(0.0);
        older.image_px[0][0] += 50.0; // mala captura
        let mut setup = sample_setup();
        setup.alignment = Some(marks_alignment(vec![
            older,
            sample_at(0.1),
            sample_at(20.0),
        ]));
        let tmp = TempDir::new();
        let a = store_setup(&tmp.0, setup).unwrap().alignment.unwrap();
        assert_eq!(a.samples.len(), 2);
        assert!(a.rms_mm < 1e-6);
    }

    #[test]
    fn rechaza_marcas_invalidas() {
        let tmp = TempDir::new();
        let bad = |f: &dyn Fn(&mut AlignmentSample)| {
            let mut s = sample_at(0.0);
            f(&mut s);
            let mut setup = sample_setup();
            setup.alignment = Some(marks_alignment(vec![s]));
            store_setup(&tmp.0, setup)
        };
        assert!(bad(&|s| s.image_px.truncate(3)).is_err());
        assert!(bad(&|s| s.bed_mm.pop().map(|_| ()).unwrap_or(())).is_err());
        assert!(bad(&|s| s.image_px[2][1] = f64::NAN).is_err());
        assert!(bad(&|s| s.height_mm = 900.0).is_err());
        // Dos alturas casi iguales pero distintas: no hay modelo (menos de 2 mm).
        let mut setup = sample_setup();
        setup.alignment = Some(marks_alignment(vec![sample_at(0.0), sample_at(1.0)]));
        let e = store_setup(&tmp.0, setup).unwrap_err();
        assert!(e.contains("difieren"), "{e}");
        assert!(!setup_file(&tmp.0).exists());
    }

    #[test]
    fn un_camera_json_anterior_sin_los_campos_nuevos_sigue_cargando() {
        let tmp = TempDir::new();
        fs::create_dir_all(&tmp.0).unwrap();
        let old = r#"{
          "version": 1,
          "calibration": null,
          "alignment": {
            "image_size": [640, 480],
            "image_px": [[100.0, 400.0], [540.0, 410.0], [530.0, 60.0], [110.0, 70.0]],
            "bed_mm": [[10.0, 10.0], [289.4, 10.0], [289.4, 225.9], [10.0, 225.9]],
            "to_bed": [[0,0,0],[0,0,0],[0,0,0]],
            "rms_mm": 0.0
          },
          "prefs": { "opacity": 0.5, "visible": true }
        }"#;
        fs::write(setup_file(&tmp.0), old).unwrap();
        let s = load_setup(&tmp.0).unwrap();
        let a = s.alignment.expect("la alineación vieja se conserva");
        assert_eq!(a.method, AlignmentMethod::Manual);
        assert!(a.samples.is_empty() && a.height.is_none());
        assert_eq!(a.reference_height_mm, 0.0);
        assert_eq!(s.prefs.material_height_mm, 0.0);
        assert!(s.prefs.visible);
        assert!(!tmp.0.join("camera.json.dañado").exists());
    }
}
