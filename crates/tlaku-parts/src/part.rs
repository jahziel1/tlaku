//! Una pieza generada: contornos de corte y trazos de grabado en milímetros.
//!
//! Coordenadas locales de la pieza: la esquina inferior izquierda de su caja envolvente está en
//! (0, 0) y Y crece hacia arriba (convención del proyecto, `docs/arquitectura.md#convenciones`).
//! Los cortes son la geometría **nominal**: ni kerf ni holgura de la máquina; el kerf lo compensa
//! la capa (`tlaku_geom::kerf_compensate`).

use tlaku_core::{Point, Polyline};

/// Pieza lista para insertar: un objeto de corte y, si hay rótulos, uno de grabado.
#[derive(Debug, Clone, PartialEq)]
pub struct Part {
    /// Identificador estable (`front`, `plate`...), no traducido: sirve de id de objeto.
    pub key: String,
    /// Nombre para mostrar, en el idioma activo.
    pub name: String,
    /// Contornos de corte. Los cerrados siguen la regla par-impar (un agujero está dentro de un
    /// contorno); los abiertos son cortes sueltos (los de una bisagra flexible).
    pub cuts: Vec<Polyline>,
    /// Trazos de grabado (rótulos); normalmente abiertos.
    pub engraves: Vec<Polyline>,
    /// Ancho de la caja envolvente de los cortes y los trazos.
    pub width: f64,
    /// Alto de la caja envolvente.
    pub height: f64,
}

/// Resultado de un generador: piezas y avisos (textos ya traducidos).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Generated {
    pub parts: Vec<Part>,
    pub warnings: Vec<String>,
}

/// Caja envolvente `(min_x, min_y, max_x, max_y)` de un conjunto de trayectorias.
pub fn bounds_of<'a>(
    paths: impl IntoIterator<Item = &'a Polyline>,
) -> Option<(f64, f64, f64, f64)> {
    let mut out: Option<(f64, f64, f64, f64)> = None;
    for p in paths.into_iter().flat_map(|p| p.points.iter()) {
        let b = out.get_or_insert((p.x, p.y, p.x, p.y));
        b.0 = b.0.min(p.x);
        b.1 = b.1.min(p.y);
        b.2 = b.2.max(p.x);
        b.3 = b.3.max(p.y);
    }
    out
}

impl Part {
    /// Crea la pieza y la lleva a su origen (esquina inferior izquierda en (0, 0)).
    pub fn new(
        key: &str,
        name: String,
        mut cuts: Vec<Polyline>,
        mut engraves: Vec<Polyline>,
    ) -> Self {
        let (x0, y0, x1, y1) =
            bounds_of(cuts.iter().chain(engraves.iter())).unwrap_or((0.0, 0.0, 0.0, 0.0));
        for p in cuts.iter_mut().chain(engraves.iter_mut()) {
            for pt in &mut p.points {
                pt.x -= x0;
                pt.y -= y0;
            }
        }
        Self {
            key: key.to_string(),
            name,
            cuts,
            engraves,
            width: x1 - x0,
            height: y1 - y0,
        }
    }

    /// Los cortes y el grabado desplazados `(dx, dy)`.
    pub fn placed(&self, dx: f64, dy: f64) -> (Vec<Polyline>, Vec<Polyline>) {
        let shift = |paths: &[Polyline]| {
            paths
                .iter()
                .map(|p| Polyline {
                    points: p
                        .points
                        .iter()
                        .map(|q| Point::new(q.x + dx, q.y + dy))
                        .collect(),
                    closed: p.closed,
                })
                .collect()
        };
        (shift(&self.cuts), shift(&self.engraves))
    }
}
