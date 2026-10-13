use crate::boolean::{shapes_to_polylines, weld_shapes};
use crate::util::MAX_COORD;
use i_overlay::mesh::float::outline::offset::OutlineOffset;
use i_overlay::mesh::float::style::{LineJoin, OutlineStyle};
use std::f64::consts::PI;
use tlaku_core::Polyline;

/// Estilo de las esquinas en un offset.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Join {
    /// Arco circular; la `tolerance` de [`offset`] acota su desviación.
    Round,
    /// Esquina en punta. `limit` es la razón máxima entre el largo de la punta
    /// y la distancia del offset (como el miter limit de SVG, >= 1). Las
    /// esquinas más agudas se recortan en lugar de extenderse sin fin.
    Miter { limit: f64 },
    /// Esquina cortada en recta.
    Bevel,
}

/// Tolerancia (mm) cuando el llamador pasa una inválida.
const DEFAULT_TOLERANCE: f64 = 0.01;

fn line_join(join: Join, distance: f64, tolerance: f64) -> LineJoin<f64> {
    match join {
        Join::Bevel => LineJoin::Bevel,
        Join::Miter { limit } => {
            let limit = if limit.is_finite() {
                limit.max(1.0)
            } else {
                1.0
            };
            // Ángulo interior mínimo que todavía se extiende en punta.
            LineJoin::Miter(2.0 * (1.0 / limit).asin())
        }
        Join::Round => {
            let tol = if tolerance.is_finite() && tolerance > 0.0 {
                tolerance
            } else {
                DEFAULT_TOLERANCE
            };
            // Flecha del arco: r (1 - cos(paso / 2)) <= tol.
            let ratio = (tol / distance.abs()).min(1.0);
            LineJoin::Round(2.0 * (1.0 - ratio).acos().clamp(1e-3, PI))
        }
    }
}

/// Desplaza el contorno de una región `distance` mm. Positivo = hacia afuera
/// de la pieza (la pieza crece); negativo = hacia adentro. Los agujeros se
/// mueven en sentido opuesto: con una distancia positiva se encogen, porque
/// también es la pieza la que crece. Esto sigue la regla par-impar de
/// [`nesting_depth`](crate::nesting_depth).
///
/// - Solo opera sobre cerradas; las abiertas y las degeneradas se ignoran.
/// - Las piezas solapadas se unen primero (como [`weld`](crate::weld)).
/// - Las formas que desaparecen (un círculo de radio 5 con `distance` = -6, o un
///   agujero que se cierra) simplemente no aparecen en el resultado, y no hay
///   bucles: el coste no depende del tamaño de `distance`.
/// - Un `NaN` o infinito devuelve vacío; la distancia se limita a 1e7 mm.
/// - `tolerance` (mm) es la desviación máxima de los arcos de `Join::Round`
///   respecto al círculo ideal; el motor impone un mínimo de 1,8 grados por
///   segmento, así que con radios muy grandes la tolerancia real puede ser
///   algo mayor. Con 0, negativo o NaN se usa 0,01 mm. Se ignora en las demás
///   uniones.
/// - Con `distance` = 0 devuelve la región limpia (útil para reparar vectores).
pub fn offset(paths: &[Polyline], distance: f64, join: Join, tolerance: f64) -> Vec<Polyline> {
    if !distance.is_finite() {
        return Vec::new();
    }
    let distance = distance.clamp(-MAX_COORD, MAX_COORD);
    let region = weld_shapes(paths);
    if region.is_empty() {
        return Vec::new();
    }
    if distance.abs() < 1e-9 {
        return shapes_to_polylines(&region);
    }

    if distance < 0.0 {
        // Un disco de radio |d| no cabe en la caja de las piezas: desaparece todo.
        let (mut min, mut max) = ([f64::INFINITY; 2], [f64::NEG_INFINITY; 2]);
        for p in region.iter().filter_map(|s| s.first()).flatten() {
            for k in 0..2 {
                min[k] = min[k].min(p[k]);
                max[k] = max[k].max(p[k]);
            }
        }
        let smallest_side = (max[0] - min[0]).min(max[1] - min[1]);
        if -distance * 2.0 >= smallest_side {
            return Vec::new();
        }
    }

    let style = OutlineStyle::new(distance).line_join(line_join(join, distance, tolerance));
    shapes_to_polylines(&region.outline(&style))
}
