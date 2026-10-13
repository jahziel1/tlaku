use crate::nesting::analyze;
use crate::util::{MIN_AREA, clean_closed, signed_area};
use i_overlay::core::fill_rule::FillRule;
use i_overlay::core::overlay_rule::OverlayRule;
use i_overlay::float::overlay::FloatOverlay;
use tlaku_core::{Point, Polyline};

type Contour = Vec<[f64; 2]>;
type Shapes = Vec<Vec<Contour>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoolOp {
    /// A o B.
    Union,
    /// A menos B (lo que está en A y no en B). No es simétrica: el orden importa.
    Difference,
    /// A y B.
    Intersection,
    /// A o B pero no ambos.
    Xor,
}

impl BoolOp {
    fn rule(self) -> OverlayRule {
        match self {
            BoolOp::Union => OverlayRule::Union,
            BoolOp::Difference => OverlayRule::Difference,
            BoolOp::Intersection => OverlayRule::Intersect,
            BoolOp::Xor => OverlayRule::Xor,
        }
    }
}

/// Contornos limpios con el sentido que impone su profundidad: pares
/// antihorarios (material) e impares horarios (huecos). Con la regla de
/// relleno no-cero, así se suman las piezas solapadas y los agujeros solo
/// restan sobre su propia pieza.
fn oriented(paths: &[Polyline]) -> Vec<Contour> {
    let clean = clean_closed(paths);
    let (depth, _) = analyze(&clean);
    clean
        .into_iter()
        .zip(depth)
        .map(|(p, d)| {
            let mut pts: Contour = p.points.iter().map(|q| [q.x, q.y]).collect();
            if (signed_area(&p.points) > 0.0) != (d % 2 == 0) {
                pts.reverse();
            }
            pts
        })
        .collect()
}

/// Aplana las formas del motor y fija el sentido: exterior antihorario, agujeros horarios.
pub(crate) fn shapes_to_polylines(shapes: &Shapes) -> Vec<Polyline> {
    let mut out = Vec::new();
    for shape in shapes {
        for (i, contour) in shape.iter().enumerate() {
            let mut points: Vec<Point> = contour.iter().map(|p| Point::new(p[0], p[1])).collect();
            let area = signed_area(&points);
            if points.len() < 3 || area.abs() <= MIN_AREA {
                continue;
            }
            if (area > 0.0) != (i == 0) {
                points.reverse();
            }
            out.push(Polyline {
                points,
                closed: true,
            });
        }
    }
    out
}

/// Región resultante de unir todas las piezas (con sus agujeros) de `paths`.
pub(crate) fn weld_shapes(paths: &[Polyline]) -> Shapes {
    let subject = oriented(paths);
    if subject.is_empty() {
        return Vec::new();
    }
    FloatOverlay::<[f64; 2]>::with_subj(&subject).overlay(OverlayRule::Subject, FillRule::NonZero)
}

/// Operación booleana entre dos conjuntos de trayectorias cerradas. Cada
/// conjunto es una región según la anidamiento par-impar (ver
/// [`nesting_depth`](crate::nesting_depth)): dentro de un conjunto, las piezas
/// que se solapan se unen y los agujeros se respetan. Las abiertas se ignoran.
///
/// `Difference` resta B de A. El resultado sigue las convenciones del crate:
/// exteriores antihorarios, cada uno seguido de sus agujeros horarios.
pub fn boolean(a: &[Polyline], b: &[Polyline], op: BoolOp) -> Vec<Polyline> {
    let (sa, sb) = (oriented(a), oriented(b));
    let shapes = match (sa.is_empty(), sb.is_empty()) {
        (true, true) => return Vec::new(),
        (true, false) => match op {
            BoolOp::Union | BoolOp::Xor => weld_shapes(b),
            BoolOp::Difference | BoolOp::Intersection => return Vec::new(),
        },
        (false, true) => match op {
            BoolOp::Union | BoolOp::Xor | BoolOp::Difference => weld_shapes(a),
            BoolOp::Intersection => return Vec::new(),
        },
        (false, false) => FloatOverlay::<[f64; 2]>::with_subj_and_clip(&sa, &sb)
            .overlay(op.rule(), FillRule::NonZero),
    };
    shapes_to_polylines(&shapes)
}

/// Une todas las trayectorias cerradas en una sola región (Weld). Las piezas
/// que se solapan se funden y los agujeros se conservan, incluidas las islas
/// dentro de ellos (el texto dentro de un anillo no desaparece).
///
/// Ojo: una forma totalmente contenida en otra de su mismo conjunto cuenta como
/// agujero (regla par-impar), igual que en `nesting_depth`. Para unir una forma
/// con otra que la contiene usa [`boolean`] con `Union` y dos conjuntos.
pub fn weld(paths: &[Polyline]) -> Vec<Polyline> {
    shapes_to_polylines(&weld_shapes(paths))
}
