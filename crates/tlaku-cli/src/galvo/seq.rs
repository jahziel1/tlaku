//! Marcado en varias piezas desde la línea de comandos: marcado dividido (M40), repetido (M40) y
//! rotativo galvo (M30). La tarjeta no tiene ejes: entre piezas el programa se detiene, dice cuánto
//! mover la mesa o girar el rotativo y espera la confirmación de la persona.
//!
//! Las piezas salen del mismo cálculo que usa la aplicación (`tlaku_pipeline::doc_mark_sequence`, un
//! `JobOptions::tile` por pieza y, con piezas por formas enteras, la selección de objetos). Sin verificar con hardware, como todo el galvo.

use super::*;
use clap::ValueEnum;
use tlaku_i18n::tr;
use tlaku_pipeline::doc::{CylinderAxis, JobOptions, SplitAxes, SplitOrder};
use tlaku_pipeline::{StageMove, doc_mark_sequence};

/// Orden de las piezas del marcado dividido.
#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum OrderArg {
    /// Filas de arriba abajo, cada una en zigzag
    Snake,
    /// Filas de arriba abajo, siempre de izquierda a derecha
    Rows,
}

/// Ejes de la mesa que se pueden mover entre piezas.
#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum AxesArg {
    Xy,
    X,
    Y,
}

/// Marcado dividido, repetido y rotativo. Sin ninguna de estas banderas se marca una sola pieza.
#[derive(Args, Clone, Debug)]
pub struct SequenceArgs {
    /// Marcado dividido: lado de las piezas en mm (0 = el campo de la lente). Activa el marcado dividido
    #[arg(long, value_name = "MM", value_parser = parse_f64)]
    split_piece: Option<f64>,
    /// Solape entre piezas contiguas en mm
    #[arg(long, value_name = "MM", default_value_t = 0.0, value_parser = parse_f64)]
    split_overlap: f64,
    /// Orden de las piezas
    #[arg(long, value_enum, default_value_t = OrderArg::Snake)]
    split_order: OrderArg,
    /// Ejes de la mesa que se mueven entre piezas (sin rotativo)
    #[arg(long, value_enum, default_value_t = AxesArg::Xy)]
    split_axes: AxesArg,
    /// Pausa tras cada movimiento de la mesa o del rotativo, en ms
    #[arg(long, value_name = "MS", default_value_t = 0.0, value_parser = parse_f64)]
    stage_pause: f64,
    /// Marcado repetido: veces que se marca el trabajo (o la secuencia de piezas)
    #[arg(long, value_name = "N", default_value_t = 1)]
    repeat_count: u32,
    /// Avance de la mesa entre repeticiones, DX,DY en mm (con rotativo la Y se vuelve giro)
    #[arg(long, value_name = "DX,DY", value_parser = parse_pair, allow_hyphen_values = true)]
    repeat_step: Option<(f64, f64)>,
    /// Pausa entre repeticiones en ms
    #[arg(long, value_name = "MS", default_value_t = 0.0, value_parser = parse_f64)]
    repeat_pause: f64,
    /// Partir por formas enteras: las piezas se cortan por la frontera de las formas y cada forma
    /// se marca completa en una sola pieza (con --split-piece; el solape no se usa)
    #[arg(long)]
    split_whole_shapes: bool,
    /// Rotativo galvo: diámetro de la pieza en mm (activa el rotativo: las piezas son franjas
    /// angulares y entre ellas se pide girar la pieza)
    #[arg(long, value_name = "MM", value_parser = parse_positive)]
    rotary_diameter: Option<f64>,
    /// Eje de la pieza del rotativo: con x la superficie avanza en Y, con y avanza en X
    #[arg(long, value_enum, default_value_t = RotaryAxisArg::X)]
    rotary_axis: RotaryAxisArg,
    /// Invierte el sentido del giro del rotativo
    #[arg(long)]
    rotary_reverse: bool,
    /// Pasos del motor del rotativo por vuelta (0 = solo se dan grados)
    #[arg(long, value_name = "PASOS", default_value_t = 0)]
    rotary_steps: u32,
}

/// Eje de la pieza del rotativo.
#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum RotaryAxisArg {
    X,
    Y,
}

impl SequenceArgs {
    /// ¿Se pidió algo que parta el trabajo en varios pasos?
    pub fn active(&self) -> bool {
        self.split_piece.is_some() || self.repeat_count > 1 || self.rotary_diameter.is_some()
    }

    /// Pone el marcado dividido, el repetido y el rotativo en la máquina con la que se calcula.
    pub fn apply(&self, machine: &mut DeviceProfile) {
        let g = &mut machine.galvo;
        if let Some(piece) = self.split_piece {
            g.split.enabled = true;
            g.split.piece_mm = piece;
            g.split.overlap_mm = self.split_overlap;
            g.split.order = match self.split_order {
                OrderArg::Snake => SplitOrder::Snake,
                OrderArg::Rows => SplitOrder::Rows,
            };
            g.split.axes = match self.split_axes {
                AxesArg::Xy => SplitAxes::Xy,
                AxesArg::X => SplitAxes::X,
                AxesArg::Y => SplitAxes::Y,
            };
            g.split.pause_after_move_ms = self.stage_pause;
        }
        g.repeat.count = self.repeat_count;
        if let Some((dx, dy)) = self.repeat_step {
            g.repeat.increment_mm = [dx, dy];
        }
        g.repeat.pause_ms = self.repeat_pause;
        g.split.whole_shapes = self.split_whole_shapes;
        if let Some(d) = self.rotary_diameter {
            g.rotary.enabled = true;
            g.rotary.diameter_mm = d;
            g.rotary.axis = match self.rotary_axis {
                RotaryAxisArg::X => CylinderAxis::X,
                RotaryAxisArg::Y => CylinderAxis::Y,
            };
            g.rotary.reverse = self.rotary_reverse;
            g.rotary.steps_per_rev = self.rotary_steps;
        }
    }
}

/// Un paso de la secuencia: el recorrido de la pieza y lo que hay que mover antes.
pub struct SeqStep {
    pub toolpath: Toolpath,
    pub stage: StageMove,
    pub pause_ms: f64,
    pub repeat: u32,
    pub repeats: u32,
    pub piece: usize,
    pub pieces: usize,
}

impl SeqStep {
    /// ¿Hay que mover el material o el rotativo antes de este paso?
    pub fn needs_move(&self) -> bool {
        !self.stage.is_zero()
    }

    /// «Paso 2 de 6 · pieza 2 de 3 · repetición 1 de 2».
    pub fn label(&self, n: usize, total: usize) -> String {
        let mut parts = vec![tr!("cli.galvo.seq_step", n = n, total = total)];
        if self.pieces > 1 {
            parts.push(tr!(
                "cli.galvo.seq_piece",
                piece = self.piece,
                pieces = self.pieces
            ));
        }
        if self.repeats > 1 {
            parts.push(tr!(
                "cli.galvo.seq_repeat",
                repeat = self.repeat,
                repeats = self.repeats
            ));
        }
        parts.join(" · ")
    }

    /// Qué mover antes del paso: «mesa X +100,00 mm · Y 0,00 mm · rotativo +45,0°».
    pub fn move_text(&self) -> String {
        let signed = |v: f64, d: usize| format!("{}{v:.d$}", if v > 0.0 { "+" } else { "" });
        let mut parts = Vec::new();
        if self.stage.dx != 0.0 || self.stage.dy != 0.0 {
            parts.push(tr!(
                "cli.galvo.seq_stage_move",
                dx = signed(self.stage.dx, 2),
                dy = signed(self.stage.dy, 2)
            ));
        }
        if self.stage.rotary_deg != 0.0 {
            parts.push(tr!(
                "cli.galvo.seq_rotary_move",
                deg = signed(self.stage.rotary_deg, 1)
            ));
        }
        parts.join(" · ")
    }
}

/// Calcula los pasos del trabajo: uno por pieza y repetición, cada uno con su zona del diseño. Una
/// pieza sin contenido se omite y su movimiento se suma al del paso siguiente (el material no se mueve
/// dos veces). Falla en español si el diseño no cabe en el campo o no hay nada que marcar.
pub fn steps(planned: &Planned) -> Result<(Vec<SeqStep>, Vec<String>), String> {
    let request = planned
        .request
        .as_ref()
        .ok_or_else(|| tr!("cli.galvo.seq_no_design"))?;
    let designed = job::design(request)?;
    let machine = &planned.machine;
    let mut warnings = designed.warnings.clone();
    // El diseño entero (sin zona) da la caja que se reparte en piezas.
    let whole_options = JobOptions {
        tile: None,
        ..designed.options.clone()
    };
    let whole = tlaku_pipeline::compute_at(&designed.doc, machine, &whole_options, None)?;
    warnings.extend(whole.preview.warnings.iter().cloned());
    let plan = doc_mark_sequence(&designed.doc, &whole, machine, &whole_options)?;
    warnings.extend(plan.warnings.iter().cloned());
    let repeats = machine.galvo.repeat.count.max(1);
    let pieces = plan
        .steps
        .iter()
        .map(|s| s.piece)
        .max()
        .map_or(1, |p| p + 1);
    let mut out: Vec<SeqStep> = Vec::new();
    let mut carried = StageMove::default();
    let mut carried_pause = 0.0_f64;
    for s in &plan.steps {
        let options = JobOptions {
            tile: s.tile.clone(),
            // Con piezas por formas enteras, solo los objetos de la pieza (ya sin recorte).
            selection: s
                .objects
                .clone()
                .or_else(|| designed.options.selection.clone()),
            ..designed.options.clone()
        };
        let job = match tlaku_pipeline::compute_at(&designed.doc, machine, &options, None) {
            Ok(j) => j,
            Err(e)
                if tlaku_i18n::msg_is(&e, "pipeline.build_plan.nothing_to_produce")
                    && s.tile.is_some() =>
            {
                warnings.push(tr!(
                    "cli.galvo.seq_piece_empty",
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
        let mut toolpath = job.toolpath;
        if let Some(p) = &planned.params {
            apply_mark_params(&mut toolpath, &planned.device, p);
        }
        out.push(SeqStep {
            toolpath,
            stage: StageMove {
                dx: carried.dx + s.stage.dx,
                dy: carried.dy + s.stage.dy,
                rotary_deg: carried.rotary_deg + s.stage.rotary_deg,
            },
            pause_ms: carried_pause.max(s.pause_ms),
            repeat: s.repeat + 1,
            repeats,
            piece: s.piece + 1,
            pieces,
        });
        carried = StageMove::default();
        carried_pause = 0.0;
    }
    if out.is_empty() {
        return Err(tr!("cli.galvo.seq_nothing"));
    }
    // El primer paso se marca con la mesa ya colocada: no lleva movimiento.
    if let Some(first) = out.first_mut() {
        first.stage = StageMove::default();
        first.pause_ms = 0.0;
    }
    Ok((out, warnings))
}
