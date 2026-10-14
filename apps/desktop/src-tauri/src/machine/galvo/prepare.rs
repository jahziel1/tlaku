//! Preparación del trabajo (`prepare`), vista previa y forma del encuadre.

use super::*;
use tlaku_i18n::tr;

// ---------------------------------------------------------------------------
// Trabajo preparado (secuencia de pasos)
// ---------------------------------------------------------------------------

/// Un paso del marcado: un tramo del diseño, con el movimiento de material que lo precede.
#[derive(Debug, Clone)]
pub struct PreparedStep {
    pub info: StepInfo,
    pub toolpath: Toolpath,
}

/// Un trabajo listo para enviar: uno o varios pasos (marcado dividido, repetido o rotativo).
#[derive(Debug, Clone)]
pub struct PreparedJob {
    pub steps: Vec<PreparedStep>,
    /// Recorrido con el láser deshabilitado y el puntero rojo (nunca se arma la fuente).
    pub red_only: bool,
    pub warnings: Vec<String>,
    pub est_seconds: f64,
}

impl PreparedJob {
    /// Pasos que exigen que el usuario mueva el material (la tarjeta no tiene ejes).
    pub fn manual_moves(&self) -> usize {
        self.steps.iter().filter(|s| s.info.needs_move()).count()
    }
}

/// Qué paso es y qué hay que hacer antes de marcarlo. Viaja a la interfaz.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct StepInfo {
    /// Paso actual y total (desde 1).
    pub step: usize,
    pub steps: usize,
    /// Repetición y pieza dentro de ella (desde 1) y sus totales.
    pub repeat: u32,
    pub repeats: u32,
    pub piece: usize,
    pub pieces: usize,
    /// Cuánto debe moverse el material (la mesa) en X e Y, mm.
    pub dx: f64,
    pub dy: f64,
    /// Giro del rotativo, grados.
    pub rotary_deg: f64,
    /// Pausa tras confirmar el movimiento, ms.
    pub pause_ms: f64,
}

impl StepInfo {
    /// ¿Hay que mover el material o el rotativo antes de marcar este paso?
    pub fn needs_move(&self) -> bool {
        self.dx != 0.0 || self.dy != 0.0 || self.rotary_deg != 0.0
    }
}

/// ¿El trabajo se marca en varios pasos (dividido, repetido o rotativo)?
pub fn is_sequenced(device: &DeviceProfile) -> bool {
    let g = &device.galvo;
    g.split.enabled || g.repeat.count > 1 || device.rotary.enabled
}

/// Calcula el trabajo con el pipeline. Con marcado dividido, repetido o rotativo recorre
/// `job_mark_sequence`: un paso por pieza y repetición, cada uno con `JobOptions.tile`. Una pieza
/// sin contenido se omite y su movimiento se suma al del paso siguiente (el material no se mueve
/// dos veces). Falla en español si el diseño no cabe en el campo o no hay nada que marcar.
pub fn prepare(
    doc: &tlaku_doc::Document,
    device: &DeviceProfile,
    options: &tlaku_doc::JobOptions,
    red_only: bool,
) -> Result<PreparedJob, String> {
    if device.kind != DeviceKind::Galvo {
        return Err(tr!("desktop.galvo.profile_not_galvo"));
    }
    let mut warnings = Vec::new();
    if !is_sequenced(device) {
        let job = tlaku_pipeline::compute_at(doc, device, options, None)?;
        warnings.extend(job.preview.warnings.iter().cloned());
        return Ok(PreparedJob {
            est_seconds: job.preview.total_s,
            steps: vec![PreparedStep {
                info: StepInfo {
                    step: 1,
                    steps: 1,
                    repeat: 1,
                    repeats: 1,
                    piece: 1,
                    pieces: 1,
                    dx: 0.0,
                    dy: 0.0,
                    rotary_deg: 0.0,
                    pause_ms: 0.0,
                },
                toolpath: job.toolpath,
            }],
            red_only,
            warnings,
        });
    }
    // El diseño entero (sin tramo) da la caja que se reparte en piezas.
    let whole_options = tlaku_doc::JobOptions {
        tile: None,
        ..options.clone()
    };
    let whole = tlaku_pipeline::compute_at(doc, device, &whole_options, None)?;
    warnings.extend(whole.preview.warnings.iter().cloned());
    let plan = job_mark_sequence(&whole, device)?;
    warnings.extend(plan.warnings.iter().cloned());
    let repeats = device.galvo.repeat.count.max(1);
    let pieces = plan
        .steps
        .iter()
        .map(|s| s.piece)
        .max()
        .map_or(1, |p| p + 1);
    let mut steps: Vec<PreparedStep> = Vec::new();
    let mut est = 0.0;
    // Movimiento de piezas omitidas que se suma al del siguiente paso con contenido.
    let mut carried = StageMove::default();
    let mut carried_pause = 0.0_f64;
    for s in &plan.steps {
        let step_options = tlaku_doc::JobOptions {
            tile: s.tile.clone(),
            ..options.clone()
        };
        let job = match tlaku_pipeline::compute_at(doc, device, &step_options, None) {
            Ok(j) => j,
            // i18n-acoplado: texto de tlaku-pipeline (`msg_is` lo reconoce en cualquier idioma).
            Err(e)
                if tlaku_i18n::msg_is(&e, "pipeline.build_plan.nothing_to_produce")
                    && s.tile.is_some() =>
            {
                warnings.push(tr!(
                    "desktop.galvo.piece_skipped",
                    piece = s.piece + 1,
                    repeat = s.repeat + 1
                ));
                carried = StageMove {
                    dx: carried.dx + s.stage.dx,
                    dy: carried.dy + s.stage.dy,
                    rotary_deg: carried.rotary_deg + s.stage.rotary_deg,
                };
                carried_pause = carried_pause.max(s.pause_ms);
                continue;
            }
            Err(e) => return Err(e),
        };
        warnings.extend(job.preview.warnings.iter().cloned());
        est += job.preview.total_s;
        let stage = StageMove {
            dx: carried.dx + s.stage.dx,
            dy: carried.dy + s.stage.dy,
            rotary_deg: carried.rotary_deg + s.stage.rotary_deg,
        };
        let pause_ms = carried_pause.max(s.pause_ms);
        carried = StageMove::default();
        carried_pause = 0.0;
        steps.push(PreparedStep {
            info: StepInfo {
                step: 0,
                steps: 0,
                repeat: s.repeat + 1,
                repeats,
                piece: s.piece + 1,
                pieces,
                dx: stage.dx,
                dy: stage.dy,
                rotary_deg: stage.rotary_deg,
                pause_ms,
            },
            toolpath: job.toolpath,
        });
    }
    if steps.is_empty() {
        return Err(tr!("desktop.galvo.no_piece_content"));
    }
    // El primer paso se marca con la mesa ya colocada: no lleva movimiento (aunque lo hayan
    // arrastrado piezas vacías, que tampoco se marcan).
    let total = steps.len();
    for (i, st) in steps.iter_mut().enumerate() {
        st.info.step = i + 1;
        st.info.steps = total;
    }
    if let Some(first) = steps.first_mut() {
        first.info.dx = 0.0;
        first.info.dy = 0.0;
        first.info.rotary_deg = 0.0;
        first.info.pause_ms = 0.0;
    }
    Ok(PreparedJob {
        steps,
        red_only,
        warnings,
        est_seconds: est,
    })
}

/// Lo que se sabe de un trabajo galvo antes de enviarlo (`galvo_preview`); no envía nada.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct GalvoPreview {
    pub steps: usize,
    pub pieces: usize,
    pub repeats: u32,
    /// Veces que habrá que mover el material (o girar el rotativo) a mano.
    pub manual_moves: usize,
    pub est_seconds: f64,
    pub warnings: Vec<String>,
    pub field_mm: f64,
    pub source: LaserSource,
    /// Potencia máxima del perfil (%).
    pub max_power_pct: f64,
}

/// Calcula el trabajo (sin tarjeta) y devuelve su resumen para el diálogo de inicio.
pub fn preview(
    doc: &tlaku_doc::Document,
    device: &DeviceProfile,
    options: &tlaku_doc::JobOptions,
) -> Result<GalvoPreview, String> {
    let job = prepare(doc, device, options, false)?;
    let first = job.steps.first().map(|s| s.info.clone());
    Ok(GalvoPreview {
        steps: job.steps.len(),
        pieces: first.as_ref().map_or(1, |i| i.pieces),
        repeats: first.as_ref().map_or(1, |i| i.repeats),
        manual_moves: job.manual_moves(),
        est_seconds: job.est_seconds,
        warnings: job.warnings,
        field_mm: device.galvo.field_mm,
        source: device.galvo.source,
        max_power_pct: device.galvo.max_power_pct,
    })
}

/// Qué se dibuja con el puntero rojo al encuadrar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FrameKind {
    /// El rectángulo que ocupa el trabajo.
    #[default]
    Rect,
    /// Los trazos del trabajo.
    Contour,
    /// El casco convexo de lo que se marca.
    Hull,
    /// Un rectángulo por cada objeto suelto.
    Objects,
}

/// Holgura (mm) con la que dos trazos se consideran del mismo objeto al encuadrar objetos sueltos.
pub const DEFAULT_OBJECT_GAP_MM: f64 = 1.0;

/// La forma del encuadre con el puntero rojo. Con marcado dividido se encuadra la primera pieza.
/// `gap_mm` solo cuenta para [`FrameKind::Objects`].
pub fn light_shape(
    doc: &tlaku_doc::Document,
    device: &DeviceProfile,
    options: &tlaku_doc::JobOptions,
    kind: FrameKind,
    gap_mm: Option<f64>,
) -> Result<LightShape, String> {
    let job = prepare(doc, device, options, true)?;
    let tp = &job
        .steps
        .first()
        .ok_or_else(|| tr!("desktop.common.nothing_to_frame"))?
        .toolpath;
    let shape = match kind {
        FrameKind::Rect => LightShape::bounds_of(tp),
        FrameKind::Contour => LightShape::contour_of(tp),
        FrameKind::Hull => LightShape::hull_of(tp),
        FrameKind::Objects => {
            LightShape::objects_of(tp, gap_mm.unwrap_or(DEFAULT_OBJECT_GAP_MM)).map(|(s, _)| s)
        }
    };
    shape.ok_or_else(|| tr!("desktop.common.nothing_to_frame"))
}
