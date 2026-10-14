//! Del perfil a la sesión, aviso previo al envío y estado publicado.

use super::*;

// ---------------------------------------------------------------------------
// Del perfil a la sesión
// ---------------------------------------------------------------------------

/// Esquina del home a partir del origen del perfil: el home de un K40 de serie es arriba a la
/// izquierda. El origen del perfil es donde la máquina pone su (0, 0), o sea, el home.
pub fn home_for_origin(origin: Origin) -> HomeCorner {
    match origin {
        Origin::TopLeft => HomeCorner::TopLeft,
        Origin::TopRight => HomeCorner::TopRight,
        Origin::BottomLeft => HomeCorner::BottomLeft,
        Origin::BottomRight => HomeCorner::BottomRight,
    }
}

/// Modelo de placa del perfil.
pub fn board_of(device: &DeviceProfile) -> Result<Board, String> {
    Board::parse(&device.lhymicro.board).ok_or_else(|| {
        format!(
            "El modelo de placa «{}» no existe. Usa A, B, B1, B2, M, M1, M2 (la de serie) o M3.",
            device.lhymicro.board
        )
    })
}

/// Velocidad máxima de corte (mm/s) a partir del tope del perfil (mm/min); 140 mm/s si el perfil
/// no tiene uno utilizable.
pub(super) fn max_speed_mm_s(device: &DeviceProfile) -> f64 {
    let rate = device.max_rate[0].max(device.max_rate[1]);
    if rate.is_finite() && rate > 0.0 {
        rate / 60.0
    } else {
        EncodeConfig::default().max_speed_mm_s
    }
}

/// Configuración de la sesión a partir del perfil: placa, cama, home y tope de velocidad.
pub fn session_config(device: &DeviceProfile) -> Result<SessionConfig, String> {
    Ok(SessionConfig {
        board: board_of(device)?,
        bed_mm: device.bed,
        home: home_for_origin(device.origin),
        max_speed_mm_s: max_speed_mm_s(device),
        ..SessionConfig::default()
    })
}

/// Configuración del codificador con la que se calcula el aviso previo; `start_mm` es dónde está
/// el cabezal (en mm de la máquina) o `None` si se hará home primero.
pub(super) fn encode_config(
    device: &DeviceProfile,
    start_mm: Option<[f64; 2]>,
) -> Result<EncodeConfig, String> {
    let cfg = session_config(device)?;
    let mil = |v: f64| tlaku_lhymicro::encode::mm_to_mil(v).max(0);
    Ok(EncodeConfig {
        board: cfg.board,
        bed_mm: cfg.bed_mm,
        home: cfg.home,
        autolock: cfg.autolock,
        max_speed_mm_s: cfg.max_speed_mm_s,
        constant_speed: cfg.constant_speed,
        fix_speeds: cfg.fix_speeds,
        start_mil: start_mm.map_or((0, 0), |[x, y]| (mil(x), mil(y))),
        ..EncodeConfig::default()
    })
}

// ---------------------------------------------------------------------------
// Aviso previo al envío
// ---------------------------------------------------------------------------

/// Un aviso del codificador, con lo justo para que la interfaz lo escriba en su idioma.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct LhyWarning {
    /// `power_ignored`, `gray_thresholded`, `air_assist_ignored`, `z_ignored` o `dwell_skipped`.
    pub kind: &'static str,
    /// Número del bloque del trabajo (desde 0); es el mismo orden de las capas calculadas.
    pub block: usize,
    /// Nombre de la capa del bloque, si se conoce.
    pub layer: Option<String>,
    /// Potencia pedida (%) en `power_ignored`; milisegundos en `dwell_skipped`.
    pub value: Option<f64>,
    /// Solo en `dwell_skipped`: era un pulso (el láser disparaba) y no una pausa.
    pub fires: bool,
}

/// Lo que se sabe de un trabajo antes de enviarlo a un K40.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct LhyPreview {
    pub board: String,
    pub warnings: Vec<LhyWarning>,
    pub packets: usize,
    pub bytes: usize,
    /// Tiempo estimado por exceso (s), con saltos rápidos a 40 mm/s.
    pub est_seconds: f64,
    pub burn_mm: f64,
    pub travel_mm: f64,
    /// La posición del cabezal se desconoce: el envío hará home primero.
    pub homes_first: bool,
}

pub(super) fn to_warning(w: &Warning, layers: &[String]) -> LhyWarning {
    let (kind, block, value, fires) = match *w {
        Warning::PowerIgnored { block, pct } => ("power_ignored", block, Some(pct), false),
        Warning::GrayThresholded { block } => ("gray_thresholded", block, None, false),
        Warning::AirAssistIgnored { block } => ("air_assist_ignored", block, None, false),
        Warning::ZIgnored { block } => ("z_ignored", block, None, false),
        Warning::DwellSkipped { block, ms, fires } => ("dwell_skipped", block, Some(ms), fires),
    };
    LhyWarning {
        kind,
        block,
        layer: layers.get(block).cloned(),
        value,
        fires,
    }
}

/// Calcula el trabajo, lo codifica (sin enviar nada) y devuelve sus avisos. `head` es la posición
/// del cabezal en mm de la máquina si se conoce.
pub fn preview(
    doc: &tlaku_doc::Document,
    device: &DeviceProfile,
    options: &tlaku_doc::JobOptions,
    head: Option<[f64; 2]>,
) -> Result<LhyPreview, String> {
    let at = head.map(|[x, y]| Point::new(x, y));
    let job = tlaku_pipeline::compute_at(doc, device, options, at)?;
    let cfg = encode_config(device, head)?;
    let program = encode(&job.toolpath, &cfg).map_err(|e| e.to_string())?;
    let frames = program.frames().map_err(|e| e.to_string())?;
    let layers: Vec<String> = job.layers.iter().map(|l| l.name.clone()).collect();
    Ok(LhyPreview {
        board: program.board.name().to_string(),
        warnings: program
            .warnings
            .iter()
            .map(|w| to_warning(w, &layers))
            .collect(),
        packets: frames.len(),
        bytes: program.stream.len(),
        est_seconds: program.stats.est_seconds,
        burn_mm: program.stats.burn_mm,
        travel_mm: program.stats.travel_mm,
        homes_first: head.is_none(),
    })
}

/// Estado de la conexión para el panel (lo que la interfaz no puede deducir de los eventos).
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct LhyState {
    pub board: String,
    pub virtual_machine: bool,
    /// Se conoce la posición del cabezal (se cuenta desde el último home).
    pub position_known: bool,
    /// Último código de estado de la placa (206 lista, 238 ocupada, 236 terminó, 239 sin
    /// energía...); 0 si aún no hay lectura.
    pub status_code: u8,
}
