//! Tipos de la configuración persistente: alineación, preferencias, cámara del cabezal y
//! [`CameraSetup`].

use super::*;

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
    /// Posiciones del cabezal con el láser apagado (cámara fija, sin grabar): una sola muestra
    /// a altura 0 con los puntos (píxel, mm de cama) ya corregidos de paralaje y desfase.
    HeadSweep,
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

/// De dónde viene el video: una cámara local (`getUserMedia`) o una cámara de red (URL).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CameraSource {
    #[default]
    Local,
    /// Cámara IP: snapshot HTTP o MJPEG, pedida por el motor (`fetch_camera_frame`).
    Network,
}

/// Preferencias de captura y de vista que conviene recordar.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CameraPrefs {
    /// Fuente del video. Por defecto, la cámara local.
    #[serde(default)]
    pub source: CameraSource,
    /// URL de la cámara de red (`http://...`). Puede llevar `usuario:clave@`: se guarda solo en
    /// este perfil local (`camera.json`) y nunca entra en un paquete `.tlakubundle`.
    #[serde(default)]
    pub network_url: Option<String>,
    /// Resolución del fondo (puntos por mm) fijada a mano; `None` = la que sale de la alineación.
    #[serde(default)]
    pub dpm: Option<f64>,
    /// Desplazamiento X del fondo (mm) para afinar la alineación a mano.
    #[serde(default)]
    pub offset_x_mm: f64,
    /// Desplazamiento Y del fondo (mm).
    #[serde(default)]
    pub offset_y_mm: f64,
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
    /// El fondo se muestra en escala de grises (se ven mejor los trazos del diseño encima).
    #[serde(default)]
    pub grayscale: bool,
    /// Refrescar el fondo al terminar un trabajo.
    #[serde(default)]
    pub update_on_job_finish: bool,
}

impl Default for CameraPrefs {
    fn default() -> Self {
        Self {
            source: CameraSource::Local,
            network_url: None,
            dpm: None,
            offset_x_mm: 0.0,
            offset_y_mm: 0.0,
            device_id: None,
            device_label: None,
            width: None,
            height: None,
            opacity: 0.6,
            visible: false,
            material_height_mm: 0.0,
            grayscale: false,
            update_on_job_finish: false,
        }
    }
}

/// Calibración de una cámara montada en el cabezal: cómo se relaciona la imagen corregida con
/// la posición del láser. El punto de la cama que ve el píxel `p` cuando el láser está en `L` es
/// `L + offset_mm + px_to_mm * (p - ref_px)` (mm de documento, Y hacia arriba).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HeadCalibration {
    /// Tamaño (px) de la imagen corregida en que se midió.
    pub image_size: [u32; 2],
    /// Píxel de referencia (el centro óptico de la imagen corregida).
    pub ref_px: [f64; 2],
    /// Matriz 2x2, por filas, de píxeles a mm (incluye la inversión del eje Y de la imagen).
    pub px_to_mm: [[f64; 2]; 2],
    /// Desfase del centro de la imagen respecto del láser (mm).
    pub offset_mm: [f64; 2],
    /// Error rms (mm) del ajuste sobre las observaciones.
    #[serde(default)]
    pub rms_mm: f64,
    #[serde(default)]
    pub observations: u32,
}

/// Cámara en el cabezal: modo y calibración. Falta la calibración hasta medirla.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct HeadCamera {
    /// La cámara está montada en el cabezal (la imagen se mueve con la máquina).
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub calibration: Option<HeadCalibration>,
}

pub(super) fn check_head(h: &HeadCamera) -> Result<(), String> {
    let Some(c) = &h.calibration else {
        return Ok(());
    };
    let vals = c
        .ref_px
        .iter()
        .chain(c.px_to_mm.iter().flatten())
        .chain(c.offset_mm.iter())
        .chain(std::iter::once(&c.rms_mm));
    if c.image_size[0] == 0 || c.image_size[1] == 0 || vals.into_iter().any(|v| !v.is_finite()) {
        return Err(tr!("desktop.edit.camera.head.bad_calibration"));
    }
    let det = c.px_to_mm[0][0] * c.px_to_mm[1][1] - c.px_to_mm[0][1] * c.px_to_mm[1][0];
    if det.abs() < 1e-12 {
        return Err(tr!("desktop.edit.camera.head.zero_scale"));
    }
    Ok(())
}

/// Configuración de cámara con nombre: la fuente y los ajustes del fondo.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CameraPreset {
    pub name: String,
    #[serde(default)]
    pub source: CameraSource,
    #[serde(default)]
    pub device_id: Option<String>,
    #[serde(default)]
    pub device_label: Option<String>,
    #[serde(default)]
    pub width: Option<u32>,
    #[serde(default)]
    pub height: Option<u32>,
    /// URL de la cámara de red (puede llevar credenciales; ver [`CameraPrefs::network_url`]).
    #[serde(default)]
    pub network_url: Option<String>,
    #[serde(default)]
    pub dpm: Option<f64>,
    #[serde(default)]
    pub offset_x_mm: f64,
    #[serde(default)]
    pub offset_y_mm: f64,
    #[serde(default)]
    pub grayscale: bool,
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
    /// Cámara montada en el cabezal (ola 4).
    #[serde(default)]
    pub head: HeadCamera,
    /// Configuraciones de cámara con nombre.
    #[serde(default)]
    pub presets: Vec<CameraPreset>,
}

impl Default for CameraSetup {
    fn default() -> Self {
        Self {
            version: SETUP_VERSION,
            calibration: None,
            alignment: None,
            prefs: CameraPrefs::default(),
            head: HeadCamera::default(),
            presets: Vec::new(),
        }
    }
}
