//! Marcado dividido y repetido (M40) y rotativo galvo (M30): la secuencia de piezas.
//!
//! Un diseño más grande que el campo del galvo se parte en piezas del tamaño del campo (o del
//! `piece_mm` del ajuste, si es menor). Entre pieza y pieza la mesa se mueve (marcado dividido) o
//! el rotativo gira (rotativo galvo: las piezas son franjas angulares de la pieza cilíndrica).
//! El marcado repetido hace toda la secuencia `count` veces, con un avance (`increment_mm`) y
//! una pausa entre repeticiones.
//!
//! Este módulo **solo genera la secuencia**: no envía nada. Para cada paso, la integración:
//! 1. mueve la mesa o el rotativo según [`MarkStep::stage`] y espera [`MarkStep::pause_ms`];
//! 2. calcula el trabajo con `JobOptions::tile = MarkStep::tile` (el recorte por tramos de
//!    `tile` deja solo esa zona y la desplaza al centro del campo) y lo marca.
//!
//! Convenciones:
//! - El **primer paso** no lleva movimiento: se supone la mesa ya colocada con la primera pieza
//!   bajo el campo.
//! - `StageMove::{dx, dy}` es cuánto debe **moverse el material** (la mesa) respecto del campo
//!   para llevar la zona del paso siguiente bajo el centro del campo: `centro_anterior -
//!   centro_nuevo`. `StageMove::rotary_deg` es el giro del rotativo, positivo en el sentido en
//!   que avanza la Y de superficie del diseño (negado si `RotarySettings::mirror`); como las
//!   piezas van de arriba abajo, suele ser negativo. La interfaz ajusta el signo a su montaje.
//! - El desplazamiento `TileSpec::offset` lleva el centro de la pieza al centro del campo (el
//!   centro de la cama), no su esquina al origen.
//! - Con rotativo, una vuelta son `pi * object_diameter` mm de superficie.
//!
//! Por omisión el recorte es el del tramo: una forma que cruza el borde de una pieza se marca en
//! dos partes. Con `SplitMarking::whole_shapes` las piezas se cortan por la frontera de las formas
//! (ver `split_shapes`): cada paso lleva entonces los ids de los objetos que marca
//! ([`MarkStep::objects`]), que la integración pasa como `JobOptions::selection`. El marcado al
//! vuelo (encoder de banda) no existe: ver `tlaku_galvo`, «Lo que no se hace».
//!
//! # Rotativo del galvo (M30)
//!
//! Lo define `GalvoDevice::rotary` (diámetro, pasos por vuelta, eje, sentido); si está apagado se
//! lee el rotativo general del perfil, para que los perfiles antiguos sigan igual (ver
//! [`galvo_rotary`]). Con el eje `x` la superficie avanza en Y (franjas horizontales); con el eje
//! `y`, en X (franjas verticales).

use crate::job::ComputedJob;
use crate::rotary::circumference_mm;
use crate::split_shapes::ShapeUnit;
use serde::{Deserialize, Serialize};
use tlaku_core::{Bounds, Point};
use tlaku_doc::{CylinderAxis, DeviceProfile, SplitAxes, SplitOrder, TileSpec};
use tlaku_i18n::tr;

/// Tope de piezas de un plan (más es casi seguro un tamaño mal escrito).
pub const MAX_PIECES: usize = 10_000;
/// Tope de repeticiones.
pub const MAX_REPEATS: u32 = 100_000;
/// Tolerancia al comparar tamaños (mm).
const EPS: f64 = 1e-9;

/// Movimiento de la mesa y/o del rotativo antes de marcar un paso.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct StageMove {
    /// Movimiento del material en X e Y, mm.
    pub dx: f64,
    pub dy: f64,
    /// Giro del rotativo, grados.
    pub rotary_deg: f64,
}

impl StageMove {
    pub fn is_zero(&self) -> bool {
        self.dx == 0.0 && self.dy == 0.0 && self.rotary_deg == 0.0
    }
}

/// El rotativo que usa un perfil galvo.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GalvoRotarySetup {
    /// Mm de superficie por vuelta completa.
    pub circumference: f64,
    /// Eje de la pieza (con `X` la superficie avanza en Y).
    pub axis: CylinderAxis,
    /// El giro positivo va contra el avance de la superficie.
    pub reverse: bool,
    /// Pasos del motor por vuelta; 0 = no se sabe.
    pub steps_per_rev: u32,
}

impl GalvoRotarySetup {
    /// Pasos del motor que equivalen a `deg` grados; `None` si no se conocen los pasos por vuelta.
    pub fn steps_for(&self, deg: f64) -> Option<i64> {
        (self.steps_per_rev > 0)
            .then(|| (deg / 360.0 * f64::from(self.steps_per_rev)).round() as i64)
    }
}

/// El rotativo de un perfil galvo, o `None` si no hay. Manda `galvo.rotary`; si está apagado se
/// mira el rotativo general (`DeviceProfile.rotary`, eje `x`, `mirror` como sentido). Falla si el
/// diámetro no es un número mayor que 0.
pub fn galvo_rotary(device: &DeviceProfile) -> Result<Option<GalvoRotarySetup>, String> {
    let r = &device.galvo.rotary;
    if r.enabled {
        if !(r.diameter_mm.is_finite() && r.diameter_mm > 0.0) {
            return Err(tr!(
                "pipeline.split.rotary_needs_diameter",
                machine = device.name
            ));
        }
        return Ok(Some(GalvoRotarySetup {
            circumference: std::f64::consts::PI * r.diameter_mm,
            axis: r.axis,
            reverse: r.reverse,
            steps_per_rev: r.steps_per_rev,
        }));
    }
    if !device.rotary.enabled {
        return Ok(None);
    }
    let circumference = circumference_mm(device).ok_or_else(|| {
        tr!(
            "pipeline.split.rotary_needs_diameter",
            machine = device.name
        )
    })?;
    Ok(Some(GalvoRotarySetup {
        circumference,
        axis: CylinderAxis::X,
        reverse: device.rotary.mirror,
        steps_per_rev: 0,
    }))
}

/// Un paso de la secuencia de marcado.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MarkStep {
    /// Repetición (desde 0) y pieza dentro de ella (desde 0).
    pub repeat: u32,
    pub piece: usize,
    /// Zona del diseño y su desplazamiento al campo; `None` = el diseño entero, sin recorte.
    pub tile: Option<TileSpec>,
    /// Con piezas por formas enteras: ids de los objetos que se marcan en este paso (va a
    /// `JobOptions::selection`); `None` = los que caen en `tile`.
    #[serde(default)]
    pub objects: Option<Vec<String>>,
    /// Movimiento previo (el primer paso lo lleva en cero).
    pub stage: StageMove,
    /// Pausa tras el movimiento, ms.
    pub pause_ms: f64,
}

/// La secuencia y los avisos que dio al calcularla.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct MarkPlan {
    pub steps: Vec<MarkStep>,
    pub warnings: Vec<String>,
}

/// Rangos `(min, max)` de las piezas a lo largo de `[lo, hi]`: de a lo sumo `size` mm con
/// `overlap` mm de solape; la última llega justo hasta `hi`.
fn ranges(lo: f64, hi: f64, size: f64, overlap: f64) -> Vec<(f64, f64)> {
    let len = hi - lo;
    if len <= size + EPS {
        return vec![(lo, hi)];
    }
    let stride = size - overlap;
    let n = ((len - size) / stride - EPS).ceil().max(0.0) as usize + 1;
    (0..n)
        .map(|k| {
            let start = lo + k as f64 * stride;
            (start, (start + size).min(hi))
        })
        .collect()
}

fn center(r: &[[f64; 2]; 2]) -> Point {
    Point::new((r[0][0] + r[1][0]) / 2.0, (r[0][1] + r[1][1]) / 2.0)
}

/// Rectángulos de las piezas de `design`, de arriba abajo, en el orden pedido.
fn pieces(
    design: Bounds,
    size: [f64; 2],
    overlap: f64,
    order: SplitOrder,
) -> Result<Vec<[[f64; 2]; 2]>, String> {
    let cols = ranges(design.min.x, design.max.x, size[0], overlap);
    // Filas de arriba hacia abajo: se reparte -y y se vuelve a invertir.
    let rows: Vec<(f64, f64)> = ranges(-design.max.y, -design.min.y, size[1], overlap)
        .into_iter()
        .map(|(a, b)| (-b, -a))
        .collect();
    if cols.len().saturating_mul(rows.len()) > MAX_PIECES {
        return Err(tr!(
            "pipeline.split.too_many_pieces",
            cols = cols.len(),
            rows = rows.len(),
            max = MAX_PIECES
        ));
    }
    let mut out = Vec::with_capacity(cols.len() * rows.len());
    for (r, &(y0, y1)) in rows.iter().enumerate() {
        let mut row: Vec<[[f64; 2]; 2]> =
            cols.iter().map(|&(x0, x1)| [[x0, y0], [x1, y1]]).collect();
        if order == SplitOrder::Snake && r % 2 == 1 {
            row.reverse();
        }
        out.extend(row);
    }
    Ok(out)
}

fn check_positive(name: &str, v: f64) -> Result<(), String> {
    if v.is_finite() && v > 0.0 {
        Ok(())
    } else {
        Err(tr!(
            "pipeline.split.must_be_positive",
            name = name,
            value = v
        ))
    }
}

/// Una pieza del plan: su rectángulo del diseño y, con formas enteras, los objetos que marca.
pub(crate) struct Piece {
    pub rect: [[f64; 2]; 2],
    pub objects: Option<Vec<String>>,
}

/// La secuencia de marcado para un diseño de caja `design` (en mm del documento) en `device`.
///
/// - Sin `galvo.split.enabled` hay una sola pieza (el diseño entero, `tile = None`).
/// - Con él y sin rotativo: cuadrícula de piezas con la mesa moviéndose en `split.axes`.
/// - Con él y con rotativo (`galvo_rotary`): franjas a lo largo de la superficie; el otro lado
///   debe caber en una pieza porque el rotativo no mueve ese eje.
/// - `galvo.repeat` repite toda la secuencia.
pub fn mark_sequence(design: Bounds, device: &DeviceProfile) -> Result<MarkPlan, String> {
    plan(design, device, None)
}

/// Como [`mark_sequence`] pero con la lista de formas del diseño: si `split.whole_shapes` está
/// activo, las piezas se cortan por la frontera de las formas (ver `split_shapes`).
pub fn mark_sequence_shapes(
    design: Bounds,
    shapes: &[ShapeUnit],
    device: &DeviceProfile,
) -> Result<MarkPlan, String> {
    plan(design, device, Some(shapes))
}

fn plan(
    design: Bounds,
    device: &DeviceProfile,
    shapes: Option<&[ShapeUnit]>,
) -> Result<MarkPlan, String> {
    let g = &device.galvo;
    let sp = &g.split;
    let rep = &g.repeat;
    let mut warnings = Vec::new();
    for (n, v) in [
        (tr!("pipeline.split.param_overlap"), sp.overlap_mm),
        (
            tr!("pipeline.split.param_pause_move"),
            sp.pause_after_move_ms,
        ),
        (tr!("pipeline.split.param_piece"), sp.piece_mm),
        (tr!("pipeline.split.param_pause_repeat"), rep.pause_ms),
    ] {
        if !(v.is_finite() && v >= 0.0) {
            return Err(tr!("pipeline.split.bad_param", name = n, value = v));
        }
    }
    if !(rep.increment_mm[0].is_finite() && rep.increment_mm[1].is_finite()) {
        return Err(tr!("pipeline.split.increment_not_number"));
    }
    if rep.count == 0 || rep.count > MAX_REPEATS {
        return Err(tr!(
            "pipeline.split.bad_repeats",
            max = MAX_REPEATS,
            n = rep.count
        ));
    }
    let rot = galvo_rotary(device)?;
    let circumference = rot.map(|r| r.circumference);
    // Eje de la superficie del rotativo: `X` = la pieza va a lo largo de X y la superficie
    // avanza en Y.
    let rot_axis = rot.map_or(CylinderAxis::X, |r| r.axis);
    // Cada pieza: rectángulo del diseño y su centro en el campo.
    let field_center = Point::new(device.bed[0] / 2.0, device.bed[1] / 2.0);
    let pieces_list: Vec<Piece> = if sp.enabled {
        check_positive(&tr!("pipeline.split.param_field"), g.field_mm)?;
        let side = if sp.piece_mm > 0.0 {
            sp.piece_mm.min(g.field_mm)
        } else {
            g.field_mm
        };
        if sp.overlap_mm >= side {
            return Err(tr!(
                "pipeline.split.overlap_too_big",
                overlap = sp.overlap_mm,
                side = side
            ));
        }
        let (w, h) = (design.max.x - design.min.x, design.max.y - design.min.y);
        // Qué ejes se pueden partir: (columnas = partir X, filas = partir Y).
        let (cut_cols, cut_rows) = if rot.is_some() {
            match rot_axis {
                CylinderAxis::X => {
                    if w > side + EPS {
                        return Err(tr!(
                            "pipeline.split.rotary_too_wide",
                            w = format!("{w:.2}"),
                            side = format!("{side:.2}")
                        ));
                    }
                    (false, true)
                }
                CylinderAxis::Y => {
                    if h > side + EPS {
                        return Err(tr!(
                            "pipeline.split.rotary_too_tall",
                            h = format!("{h:.2}"),
                            side = format!("{side:.2}")
                        ));
                    }
                    (true, false)
                }
            }
        } else {
            match sp.axes {
                SplitAxes::Xy => (true, true),
                SplitAxes::X => {
                    if h > side + EPS {
                        return Err(tr!(
                            "pipeline.split.too_tall_for_x",
                            h = format!("{h:.2}"),
                            side = format!("{side:.2}")
                        ));
                    }
                    (true, false)
                }
                SplitAxes::Y => {
                    if w > side + EPS {
                        return Err(tr!(
                            "pipeline.split.too_wide_for_y",
                            w = format!("{w:.2}"),
                            side = format!("{side:.2}")
                        ));
                    }
                    (false, true)
                }
            }
        };
        // Un eje que no se mueve ya cabe en una sola pieza (comprobado arriba).
        let whole = match shapes {
            Some(list) if sp.whole_shapes => crate::split_shapes::whole_pieces(
                list,
                side,
                (cut_cols, cut_rows),
                sp.order,
                &mut warnings,
            ),
            _ => None,
        };
        let pieces_list = match whole {
            Some(p) => p,
            None => pieces(design, [side, side], sp.overlap_mm, sp.order)?
                .into_iter()
                .map(|rect| Piece {
                    rect,
                    objects: None,
                })
                .collect(),
        };
        if let Some(circ) = circumference {
            let span = side / circ * 360.0;
            if span > 120.0 {
                warnings.push(tr!(
                    "pipeline.split.rotary_curvature",
                    span = format!("{span:.0}"),
                    circ = format!("{circ:.1}")
                ));
            }
            let along = match rot_axis {
                CylinderAxis::X => h,
                CylinderAxis::Y => w,
            };
            if along / circ > 1.0 + 1e-9 {
                warnings.push(tr!(
                    "pipeline.split.rotary_overlap",
                    h = format!("{along:.2}"),
                    circ = format!("{circ:.2}")
                ));
            }
        }
        pieces_list
    } else {
        Vec::new()
    };
    let rects: Vec<&[[f64; 2]; 2]> = pieces_list.iter().map(|p| &p.rect).collect();
    // Movimiento entre dos centros (de `a` a `b`, ambos en mm del diseño).
    let travel = |a: Point, b: Point| -> StageMove {
        match (circumference, rot) {
            (Some(circ), Some(r)) => {
                let sign = if r.reverse { -1.0 } else { 1.0 };
                let along = match rot_axis {
                    CylinderAxis::X => b.y - a.y,
                    CylinderAxis::Y => b.x - a.x,
                };
                StageMove {
                    dx: 0.0,
                    dy: 0.0,
                    rotary_deg: sign * along / circ * 360.0,
                }
            }
            _ => StageMove {
                dx: a.x - b.x,
                dy: a.y - b.y,
                rotary_deg: 0.0,
            },
        }
    };
    // Avance entre repeticiones: la mesa en X e Y, o con rotativo el giro de la superficie (la
    // Y con el eje `x`, la X con el eje `y`); el otro eje sigue siendo de la mesa.
    let increment = match (circumference, rot) {
        (Some(circ), Some(r)) => {
            let sign = if r.reverse { -1.0 } else { 1.0 };
            match rot_axis {
                CylinderAxis::X => StageMove {
                    dx: rep.increment_mm[0],
                    dy: 0.0,
                    rotary_deg: sign * rep.increment_mm[1] / circ * 360.0,
                },
                CylinderAxis::Y => StageMove {
                    dx: 0.0,
                    dy: rep.increment_mm[1],
                    rotary_deg: sign * rep.increment_mm[0] / circ * 360.0,
                },
            }
        }
        _ => StageMove {
            dx: rep.increment_mm[0],
            dy: rep.increment_mm[1],
            rotary_deg: 0.0,
        },
    };
    let n_pieces = rects.len().max(1);
    if n_pieces.saturating_mul(rep.count as usize) > MAX_PIECES {
        return Err(tr!(
            "pipeline.split.too_many_steps",
            pieces = n_pieces,
            repeats = rep.count,
            max = MAX_PIECES
        ));
    }
    let tile_of = |r: &[[f64; 2]; 2]| {
        let c = center(r);
        TileSpec {
            rect: *r,
            offset: [field_center.x - c.x, field_center.y - c.y],
        }
    };
    let mut steps = Vec::with_capacity(n_pieces * rep.count as usize);
    for repeat in 0..rep.count {
        for piece in 0..n_pieces {
            let (tile, stage, pause_ms) = if rects.is_empty() {
                // Sin partir, la única "pieza" no se mueve; entre repeticiones avanza el incremento.
                let stage = if repeat > 0 {
                    increment
                } else {
                    StageMove::default()
                };
                (None, stage, rep.pause_ms)
            } else {
                let here = center(rects[piece]);
                if piece > 0 {
                    let from = center(rects[piece - 1]);
                    (
                        Some(tile_of(rects[piece])),
                        travel(from, here),
                        sp.pause_after_move_ms,
                    )
                } else if repeat > 0 {
                    // Vuelta a la primera pieza más el avance de la repetición.
                    let from = center(rects[rects.len() - 1]);
                    let back = travel(from, here);
                    (
                        Some(tile_of(rects[piece])),
                        StageMove {
                            dx: back.dx + increment.dx,
                            dy: back.dy + increment.dy,
                            rotary_deg: back.rotary_deg + increment.rotary_deg,
                        },
                        rep.pause_ms,
                    )
                } else {
                    (Some(tile_of(rects[piece])), StageMove::default(), 0.0)
                }
            };
            let pause_ms = if stage.is_zero() { 0.0 } else { pause_ms };
            steps.push(MarkStep {
                repeat,
                piece,
                tile,
                objects: pieces_list.get(piece).and_then(|p| p.objects.clone()),
                stage,
                pause_ms,
            });
        }
    }
    Ok(MarkPlan { steps, warnings })
}

/// Como [`mark_sequence`] con la caja de un trabajo ya calculado (`build`/`compute` sin tramo).
/// Falla si el trabajo está vacío.
pub fn job_mark_sequence(job: &ComputedJob, device: &DeviceProfile) -> Result<MarkPlan, String> {
    let bounds = job
        .bounds()
        .ok_or_else(|| tr!("pipeline.split.no_motion"))?;
    mark_sequence(bounds, device)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tlaku_doc::{
        DeviceKind, GalvoDevice, RepeatMarking, RotaryKind, RotarySettings, SplitMarking,
    };

    fn bounds(x0: f64, y0: f64, x1: f64, y1: f64) -> Bounds {
        Bounds {
            min: Point::new(x0, y0),
            max: Point::new(x1, y1),
        }
    }

    fn galvo_device(split: SplitMarking, repeat: RepeatMarking) -> DeviceProfile {
        DeviceProfile {
            kind: DeviceKind::Galvo,
            bed: [100.0, 100.0],
            galvo: GalvoDevice {
                field_mm: 100.0,
                split,
                repeat,
                ..GalvoDevice::default()
            },
            ..DeviceProfile::default()
        }
    }

    fn enabled() -> SplitMarking {
        SplitMarking {
            enabled: true,
            ..SplitMarking::default()
        }
    }

    #[test]
    fn without_split_there_is_a_single_whole_step() {
        let d = galvo_device(SplitMarking::default(), RepeatMarking::default());
        let plan = mark_sequence(bounds(0.0, 0.0, 500.0, 500.0), &d).unwrap();
        assert_eq!(plan.steps.len(), 1);
        assert_eq!(plan.steps[0].tile, None);
        assert!(plan.steps[0].stage.is_zero());
    }

    #[test]
    fn small_design_is_one_piece_centered_in_the_field() {
        let d = galvo_device(enabled(), RepeatMarking::default());
        let plan = mark_sequence(bounds(10.0, 10.0, 30.0, 50.0), &d).unwrap();
        assert_eq!(plan.steps.len(), 1);
        let t = plan.steps[0].tile.as_ref().unwrap();
        assert_eq!(t.rect, [[10.0, 10.0], [30.0, 50.0]]);
        // Centro (20, 30) al centro de la cama (50, 50).
        assert_eq!(t.offset, [30.0, 20.0]);
    }

    #[test]
    fn large_design_splits_into_a_snake_grid() {
        let d = galvo_device(enabled(), RepeatMarking::default());
        // 250 x 150 con piezas de 100: 3 columnas y 2 filas.
        let plan = mark_sequence(bounds(0.0, 0.0, 250.0, 150.0), &d).unwrap();
        assert_eq!(plan.steps.len(), 6);
        let rects: Vec<_> = plan
            .steps
            .iter()
            .map(|s| s.tile.as_ref().unwrap().rect)
            .collect();
        // Fila de arriba (y 50..150) de izquierda a derecha, luego la de abajo al revés.
        assert_eq!(rects[0], [[0.0, 50.0], [100.0, 150.0]]);
        assert_eq!(rects[1], [[100.0, 50.0], [200.0, 150.0]]);
        assert_eq!(rects[2], [[200.0, 50.0], [250.0, 150.0]]);
        assert_eq!(rects[3], [[200.0, 0.0], [250.0, 50.0]]);
        assert_eq!(rects[5], [[0.0, 0.0], [100.0, 50.0]]);
        // Ninguna pieza pasa del campo y juntas cubren el diseño.
        for r in &rects {
            assert!(r[1][0] - r[0][0] <= 100.0 + 1e-9 && r[1][1] - r[0][1] <= 100.0 + 1e-9);
        }
        let area: f64 = rects
            .iter()
            .map(|r| (r[1][0] - r[0][0]) * (r[1][1] - r[0][1]))
            .sum();
        assert!((area - 250.0 * 150.0).abs() < 1e-6);
    }

    #[test]
    fn stage_moves_bring_the_next_center_under_the_field() {
        let d = galvo_device(
            SplitMarking {
                pause_after_move_ms: 250.0,
                ..enabled()
            },
            RepeatMarking::default(),
        );
        let plan = mark_sequence(bounds(0.0, 0.0, 200.0, 100.0), &d).unwrap();
        assert_eq!(plan.steps.len(), 2);
        assert!(plan.steps[0].stage.is_zero() && plan.steps[0].pause_ms == 0.0);
        // El material avanza -100 en X: la zona derecha pasa a estar bajo el campo.
        assert_eq!(plan.steps[1].stage.dx, -100.0);
        assert_eq!(plan.steps[1].stage.dy, 0.0);
        assert_eq!(plan.steps[1].pause_ms, 250.0);
        // Suma de los movimientos = centro de la primera - centro de la última.
        let total: f64 = plan.steps.iter().map(|s| s.stage.dx).sum();
        assert_eq!(total, 50.0 - 150.0);
    }

    #[test]
    fn overlap_shifts_the_stride() {
        let d = galvo_device(
            SplitMarking {
                overlap_mm: 10.0,
                ..enabled()
            },
            RepeatMarking::default(),
        );
        let plan = mark_sequence(bounds(0.0, 0.0, 190.0, 100.0), &d).unwrap();
        let xs: Vec<_> = plan
            .steps
            .iter()
            .map(|s| s.tile.as_ref().unwrap().rect)
            .collect();
        assert_eq!(xs.len(), 2);
        assert_eq!(xs[1][0][0], 90.0);
        assert_eq!(xs[1][1][0], 190.0);
        // Solape de 10 mm entre las dos.
        assert_eq!(xs[0][1][0] - xs[1][0][0], 10.0);
    }

    #[test]
    fn restricted_axes_refuse_what_they_cannot_move() {
        let d = galvo_device(
            SplitMarking {
                axes: SplitAxes::X,
                ..enabled()
            },
            RepeatMarking::default(),
        );
        let e = mark_sequence(bounds(0.0, 0.0, 250.0, 150.0), &d).unwrap_err();
        assert!(e.contains("solo se mueve en X"), "{e}");
        let ok = mark_sequence(bounds(0.0, 0.0, 250.0, 80.0), &d).unwrap();
        assert_eq!(ok.steps.len(), 3);
        assert!(ok.steps.iter().all(|s| s.stage.dy == 0.0));
    }

    #[test]
    fn repeat_marking_repeats_with_increment_and_pause() {
        let d = galvo_device(
            SplitMarking::default(),
            RepeatMarking {
                count: 3,
                increment_mm: [0.0, 40.0],
                pause_ms: 500.0,
            },
        );
        let plan = mark_sequence(bounds(0.0, 0.0, 20.0, 20.0), &d).unwrap();
        assert_eq!(plan.steps.len(), 3);
        assert!(plan.steps[0].stage.is_zero());
        for s in &plan.steps[1..] {
            assert_eq!((s.stage.dx, s.stage.dy), (0.0, 40.0));
            assert_eq!(s.pause_ms, 500.0);
        }
        assert_eq!(
            plan.steps.iter().map(|s| s.repeat).collect::<Vec<_>>(),
            vec![0, 1, 2]
        );
    }

    #[test]
    fn repeat_over_split_returns_to_the_first_piece() {
        let d = galvo_device(
            enabled(),
            RepeatMarking {
                count: 2,
                increment_mm: [0.0, 5.0],
                pause_ms: 100.0,
            },
        );
        let plan = mark_sequence(bounds(0.0, 0.0, 200.0, 100.0), &d).unwrap();
        assert_eq!(plan.steps.len(), 4);
        let s = &plan.steps[2];
        assert_eq!((s.repeat, s.piece), (1, 0));
        // De la pieza derecha (centro 150) a la izquierda (50): +100, más el avance.
        assert_eq!((s.stage.dx, s.stage.dy), (100.0, 5.0));
        assert_eq!(s.pause_ms, 100.0);
    }

    fn rotary_device(piece_mm: f64) -> DeviceProfile {
        let mut d = galvo_device(
            SplitMarking {
                piece_mm,
                ..enabled()
            },
            RepeatMarking::default(),
        );
        d.rotary = RotarySettings {
            enabled: true,
            kind: RotaryKind::Chuck,
            object_diameter: 50.0,
            ..RotarySettings::default()
        };
        d
    }

    #[test]
    fn rotary_galvo_splits_into_angular_strips() {
        // Circunferencia de pi * 50 = 157.08 mm; franjas de 40 mm en Y.
        let d = rotary_device(40.0);
        let plan = mark_sequence(bounds(0.0, 0.0, 30.0, 120.0), &d).unwrap();
        assert_eq!(plan.steps.len(), 3);
        let circ = std::f64::consts::PI * 50.0;
        // De arriba abajo: la Y baja 40 mm entre franjas, el rotativo gira en negativo.
        for s in &plan.steps[1..] {
            assert!((s.stage.rotary_deg + 40.0 / circ * 360.0).abs() < 1e-9);
            assert_eq!((s.stage.dx, s.stage.dy), (0.0, 0.0));
        }
        let total: f64 = plan.steps.iter().map(|s| s.stage.rotary_deg).sum();
        assert!((total + 80.0 / circ * 360.0).abs() < 1e-9);
        assert!(plan.warnings.is_empty(), "{:?}", plan.warnings);
    }

    #[test]
    fn rotary_mirror_flips_the_turn_and_wide_designs_fail() {
        let mut d = rotary_device(40.0);
        d.rotary.mirror = true;
        let plan = mark_sequence(bounds(0.0, 0.0, 30.0, 80.0), &d).unwrap();
        assert!(plan.steps[1].stage.rotary_deg > 0.0);
        let e = mark_sequence(bounds(0.0, 0.0, 130.0, 80.0), &d).unwrap_err();
        assert!(e.contains("no mueve X"), "{e}");
    }

    #[test]
    fn rotary_warns_about_wide_strips_and_overlap_of_ends() {
        let d = rotary_device(0.0);
        // Franjas de 100 mm en una circunferencia de 157 mm: 229 grados; y 300 mm > vuelta.
        let plan = mark_sequence(bounds(0.0, 0.0, 30.0, 300.0), &d).unwrap();
        assert!(plan.warnings.iter().any(|w| w.contains("grados")));
        assert!(plan.warnings.iter().any(|w| w.contains("encimarán")));
    }

    #[test]
    fn rotary_repeat_converts_the_y_increment_to_degrees() {
        let mut d = rotary_device(0.0);
        d.galvo.split.enabled = false;
        d.galvo.repeat = RepeatMarking {
            count: 2,
            increment_mm: [0.0, std::f64::consts::PI * 50.0 / 4.0],
            pause_ms: 0.0,
        };
        let plan = mark_sequence(bounds(0.0, 0.0, 30.0, 30.0), &d).unwrap();
        assert!((plan.steps[1].stage.rotary_deg - 90.0).abs() < 1e-9);
    }

    #[test]
    fn invalid_settings_are_errors() {
        let mut d = galvo_device(enabled(), RepeatMarking::default());
        d.galvo.split.overlap_mm = 100.0;
        assert!(mark_sequence(bounds(0.0, 0.0, 300.0, 50.0), &d).is_err());
        let mut d = galvo_device(enabled(), RepeatMarking::default());
        d.galvo.repeat.count = 0;
        assert!(mark_sequence(bounds(0.0, 0.0, 30.0, 50.0), &d).is_err());
        let mut d = galvo_device(enabled(), RepeatMarking::default());
        d.galvo.split.pause_after_move_ms = f64::NAN;
        assert!(mark_sequence(bounds(0.0, 0.0, 30.0, 50.0), &d).is_err());
        let mut d = galvo_device(enabled(), RepeatMarking::default());
        d.galvo.field_mm = 0.0;
        assert!(mark_sequence(bounds(0.0, 0.0, 30.0, 50.0), &d).is_err());
    }

    #[test]
    fn too_many_pieces_are_refused() {
        let mut d = galvo_device(enabled(), RepeatMarking::default());
        d.galvo.field_mm = 1.0;
        let e = mark_sequence(bounds(0.0, 0.0, 1000.0, 1000.0), &d).unwrap_err();
        assert!(e.contains("demasiado"), "{e}");
    }
    fn galvo_rotary_device(
        piece_mm: f64,
        axis: CylinderAxis,
        reverse: bool,
        steps: u32,
    ) -> DeviceProfile {
        let mut d = galvo_device(
            SplitMarking {
                piece_mm,
                ..enabled()
            },
            RepeatMarking::default(),
        );
        d.galvo.rotary = tlaku_doc::GalvoRotary {
            enabled: true,
            diameter_mm: 100.0 / std::f64::consts::PI, // 100 mm por vuelta
            steps_per_rev: steps,
            axis,
            reverse,
        };
        d
    }

    /// Cálculo a mano. Una vuelta son 100 mm. Franjas de 25 mm en Y (de arriba abajo): entre dos
    /// franjas el centro baja 25 mm = un cuarto de vuelta = -90 grados; con 1200 pasos por vuelta
    /// son -300 pasos. Con el sentido invertido, +90 grados y +300 pasos.
    #[test]
    fn galvo_rotary_gives_degrees_and_motor_steps() {
        let d = galvo_rotary_device(25.0, CylinderAxis::X, false, 1200);
        let plan = mark_sequence(bounds(0.0, 0.0, 20.0, 100.0), &d).unwrap();
        assert_eq!(plan.steps.len(), 4);
        for s in &plan.steps[1..] {
            assert!((s.stage.rotary_deg + 90.0).abs() < 1e-9, "{:?}", s.stage);
            assert_eq!((s.stage.dx, s.stage.dy), (0.0, 0.0));
        }
        let r = galvo_rotary(&d).unwrap().unwrap();
        assert!((r.circumference - 100.0).abs() < 1e-9);
        assert_eq!(r.steps_for(-90.0), Some(-300));
        assert_eq!(r.steps_for(45.0), Some(150));
        let d = galvo_rotary_device(25.0, CylinderAxis::X, true, 1200);
        let plan = mark_sequence(bounds(0.0, 0.0, 20.0, 100.0), &d).unwrap();
        assert!((plan.steps[1].stage.rotary_deg - 90.0).abs() < 1e-9);
        // Sin pasos por vuelta solo hay grados.
        let d = galvo_rotary_device(25.0, CylinderAxis::X, false, 0);
        assert_eq!(galvo_rotary(&d).unwrap().unwrap().steps_for(90.0), None);
    }

    /// Con el eje en Y la superficie avanza en X: las franjas son verticales (de izquierda a
    /// derecha, el centro sube 25 mm en X = +90 grados) y lo que no se mueve es Y.
    #[test]
    fn galvo_rotary_on_the_y_axis_strips_along_x() {
        let d = galvo_rotary_device(25.0, CylinderAxis::Y, false, 0);
        let plan = mark_sequence(bounds(0.0, 0.0, 100.0, 20.0), &d).unwrap();
        assert_eq!(plan.steps.len(), 4);
        let rects: Vec<_> = plan
            .steps
            .iter()
            .map(|s| s.tile.as_ref().unwrap().rect)
            .collect();
        assert_eq!(rects[0], [[0.0, 0.0], [25.0, 20.0]]);
        assert_eq!(rects[3], [[75.0, 0.0], [100.0, 20.0]]);
        for s in &plan.steps[1..] {
            assert!((s.stage.rotary_deg - 90.0).abs() < 1e-9, "{:?}", s.stage);
            assert_eq!((s.stage.dx, s.stage.dy), (0.0, 0.0));
        }
        // Un diseño más alto que la pieza no cabe: el rotativo no mueve Y.
        let e = mark_sequence(bounds(0.0, 0.0, 100.0, 40.0), &d).unwrap_err();
        assert!(e.contains("no mueve Y"), "{e}");
        // Y el avance entre repeticiones convierte la X en giro y deja la Y a la mesa.
        let mut d = galvo_rotary_device(0.0, CylinderAxis::Y, false, 0);
        d.galvo.split.enabled = false;
        d.galvo.repeat = RepeatMarking {
            count: 2,
            increment_mm: [50.0, 3.0],
            pause_ms: 0.0,
        };
        let plan = mark_sequence(bounds(0.0, 0.0, 10.0, 10.0), &d).unwrap();
        let s = plan.steps[1].stage;
        assert!(
            (s.rotary_deg - 180.0).abs() < 1e-9 && s.dx == 0.0 && s.dy == 3.0,
            "{s:?}"
        );
    }

    /// El rotativo del galvo manda sobre el general; con el suyo apagado se lee el general (los
    /// perfiles anteriores a la ola 6E siguen igual) y sin ninguno no hay rotativo.
    #[test]
    fn the_galvo_rotary_wins_and_the_general_one_is_the_fallback() {
        let mut d = rotary_device(40.0);
        let legacy = galvo_rotary(&d).unwrap().unwrap();
        assert!((legacy.circumference - std::f64::consts::PI * 50.0).abs() < 1e-9);
        assert_eq!(
            (legacy.axis, legacy.reverse, legacy.steps_per_rev),
            (CylinderAxis::X, false, 0)
        );
        d.galvo.rotary.enabled = true;
        d.galvo.rotary.diameter_mm = 20.0;
        d.galvo.rotary.reverse = true;
        let mine = galvo_rotary(&d).unwrap().unwrap();
        assert!((mine.circumference - std::f64::consts::PI * 20.0).abs() < 1e-9);
        assert!(mine.reverse);
        // Sin ninguno.
        let none = galvo_device(enabled(), RepeatMarking::default());
        assert_eq!(galvo_rotary(&none).unwrap(), None);
        // Un diámetro inválido es un error, no un rotativo sin vuelta.
        d.galvo.rotary.diameter_mm = 0.0;
        assert!(galvo_rotary(&d).is_err());
    }
}
