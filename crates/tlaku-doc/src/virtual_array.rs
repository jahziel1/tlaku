//! Copias de un arreglo virtual (`DesignObject::array`): la regla única que usan el motor
//! (`tlaku-pipeline`, al expandirlas en el trabajo) y la caja del diseño (`Document::bounds`).
//! La interfaz tiene su espejo en `apps/desktop/src/lib/virtualArray.ts` (`virtualCopies`).

use crate::affine::{self, Transform};
use crate::cut::VirtualArray;
use crate::document::{DesignObject, union};
use tlaku_core::{Bounds, Point};
use tlaku_i18n::tr;

/// Tope de copias virtuales por objeto (el mismo que el de los arreglos reales de la interfaz).
pub const MAX_VIRTUAL_COPIES: usize = 2000;

impl DesignObject {
    /// Transformaciones de las copias de un arreglo virtual, **sin** la original, en coordenadas
    /// del documento: la copia final es `compose(copia, objeto.transform)`.
    ///
    /// - `Grid`: la original es la esquina inferior izquierda; las columnas van hacia la derecha y
    ///   las filas hacia arriba, separadas `dx` y `dy` mm entre bordes de la caja del objeto
    ///   (negativo = solapadas).
    /// - `Circular`: `count` piezas en total, contando la original, alrededor de `center`. Con
    ///   `sweep_deg` de 360 o más se reparte sin repetir la original; con menos, incluye ambos
    ///   extremos del arco. Con `rotate` cada copia gira con el arreglo; sin él, conserva la
    ///   orientación y solo su centro recorre el arco.
    ///
    /// Es la misma regla que `virtualCopies` de la interfaz. Error con parámetros inválidos o más
    /// de [`MAX_VIRTUAL_COPIES`] copias; vacío si no hay arreglo o el objeto no dibuja nada.
    pub fn virtual_copies(&self) -> Result<Vec<Transform>, String> {
        let Some(array) = &self.array else {
            return Ok(Vec::new());
        };
        match *array {
            VirtualArray::Grid { cols, rows, dx, dy } => {
                if cols == 0 || rows == 0 {
                    return Err(tr!("doc.virtual_array.grid_size", id = self.id));
                }
                if !(dx.is_finite() && dy.is_finite()) {
                    return Err(tr!("doc.virtual_array.grid_gap", id = self.id));
                }
                if (cols as usize).saturating_mul(rows as usize) > MAX_VIRTUAL_COPIES {
                    return Err(tr!(
                        "doc.virtual_array.too_many",
                        id = self.id,
                        max = MAX_VIRTUAL_COPIES
                    ));
                }
                let Some(b) = self.bounds() else {
                    return Ok(Vec::new());
                };
                let (sx, sy) = (b.max.x - b.min.x + dx, b.max.y - b.min.y + dy);
                let mut out = Vec::with_capacity((cols * rows) as usize - 1);
                for j in 0..rows {
                    for i in 0..cols {
                        if i != 0 || j != 0 {
                            out.push(affine::translate(f64::from(i) * sx, f64::from(j) * sy));
                        }
                    }
                }
                Ok(out)
            }
            VirtualArray::Circular {
                count,
                center,
                sweep_deg,
                rotate,
            } => {
                if count == 0 {
                    return Err(tr!("doc.virtual_array.circular_count", id = self.id));
                }
                if !(center.iter().all(|v| v.is_finite()) && sweep_deg.is_finite()) {
                    return Err(tr!("doc.virtual_array.circular_center", id = self.id));
                }
                if count as usize > MAX_VIRTUAL_COPIES {
                    return Err(tr!(
                        "doc.virtual_array.too_many",
                        id = self.id,
                        max = MAX_VIRTUAL_COPIES
                    ));
                }
                let Some(b) = self.bounds() else {
                    return Ok(Vec::new());
                };
                let n = f64::from(count);
                let step = if sweep_deg.abs() >= 360.0 - 1e-9 {
                    sweep_deg / n
                } else if count > 1 {
                    sweep_deg / (n - 1.0)
                } else {
                    0.0
                };
                let (cx, cy) = (center[0], center[1]);
                let (mx, my) = ((b.min.x + b.max.x) / 2.0, (b.min.y + b.max.y) / 2.0);
                Ok((1..count)
                    .map(|k| {
                        let rot = affine::rotate((f64::from(k) * step).to_radians());
                        // Giro alrededor de `center`: T(c) · R · T(-c).
                        let about = affine::compose(
                            &affine::translate(cx, cy),
                            &affine::compose(&rot, &affine::translate(-cx, -cy)),
                        );
                        if rotate {
                            about
                        } else {
                            let [px, py] = affine::apply(&about, [mx, my]);
                            affine::translate(px - mx, py - my)
                        }
                    })
                    .collect())
            }
        }
    }

    /// Caja envolvente del objeto **y de sus copias virtuales**, en coordenadas de máquina. Igual
    /// que [`bounds`](Self::bounds) si no hay arreglo; un arreglo inválido se ignora (el motor
    /// avisa y tampoco genera las copias).
    pub fn bounds_with_copies(&self) -> Option<Bounds> {
        let own = self.bounds()?;
        let Ok(copies) = self.virtual_copies() else {
            return Some(own);
        };
        let curves = (!copies.is_empty()).then(|| self.curves());
        let mut all = own;
        for t in &copies {
            let moved = if t[..4] == [1.0, 0.0, 0.0, 1.0] {
                // Traslación pura (rejilla): basta mover la caja.
                Bounds {
                    min: Point::new(own.min.x + t[4], own.min.y + t[5]),
                    max: Point::new(own.max.x + t[4], own.max.y + t[5]),
                }
            } else {
                let place = |p: Point| {
                    let [x, y] = affine::apply(t, [p.x, p.y]);
                    Point::new(x, y)
                };
                // La afín de una Bézier es otra Bézier: la caja sigue siendo exacta.
                let Some(b) = curves
                    .iter()
                    .flatten()
                    .filter_map(|c| c.map_points(place).bounds())
                    .reduce(union)
                else {
                    continue;
                };
                b
            };
            all = union(all, moved);
        }
        Some(all)
    }
}
