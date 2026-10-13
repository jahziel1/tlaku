//! Puentes de las capas line: de los ajustes de la capa a los movimientos de la trayectoria.
//!
//! La pieza se planifica como forma cerrada (el planificador decide el orden y por dónde
//! entra) y recién entonces se parte con [`tlaku_geom::tab_segments`], así el orden de corte
//! no cambia por los puentes.
//!
//! El hueco entre dos tramos es un `Cut` con potencia 0 y no un `Travel`. Los dos dejan el
//! láser apagado en GRBL (G1 con S0 / G0), pero un `Travel` es un G0 a velocidad rápida: la
//! máquina frena al cambiar de tipo de movimiento (así lo modela `tlaku-sim`), con un hueco de
//! 0,5 mm se sale del régimen de corte a cada puente y la estimación de tiempo se infla. Con
//! `Cut` a potencia 0 la cabeza conserva la velocidad de corte, igual que el overscan, y la
//! vista previa lo dibuja como láser apagado. El hueco que cae justo al final del recorrido
//! (cuando cubre el punto de partida) no se cruza.

use tlaku_core::toolpath::Move;
use tlaku_core::{Point, Polyline};
use tlaku_doc::LayerSettings;
use tlaku_geom::{MAX_TABS_PER_SHAPE, TabOptions, TabResult, tab_segments};

/// Puentes ya validados de una capa line.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct TabSpec {
    pub options: TabOptions,
    pub skip_inner: bool,
}

/// Lee y valida los puentes de una capa line. `None` si no están activos.
pub(crate) fn resolve(l: &LayerSettings) -> Result<Option<TabSpec>, String> {
    let t = &l.tabs;
    if !t.enabled {
        return Ok(None);
    }
    let err = |m: String| format!("capa {}: puentes: {m}", l.color);
    if !(t.width.is_finite() && t.width > 0.0) {
        return Err(err(format!("el ancho debe ser mayor que 0: {}", t.width)));
    }
    if !(t.spacing.is_finite() && t.spacing >= 0.0) {
        return Err(err(format!(
            "la separación debe ser 0 o mayor: {}",
            t.spacing
        )));
    }
    if t.count as usize > MAX_TABS_PER_SHAPE {
        return Err(err(format!(
            "demasiados puentes por forma ({}); el máximo es {MAX_TABS_PER_SHAPE}",
            t.count
        )));
    }
    if t.count == 0 && t.spacing == 0.0 {
        return Err(err(
            "indica cuántos puentes (count) o cada cuántos mm (spacing)".into(),
        ));
    }
    Ok(Some(TabSpec {
        options: TabOptions {
            width_mm: t.width,
            count: t.count,
            spacing_mm: t.spacing,
        },
        skip_inner: t.skip_inner,
    }))
}

/// Resumen de los puentes de una capa, para el informe y las advertencias.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct TabStats {
    /// Puentes colocados en total.
    pub placed: usize,
    /// Formas donde no cupieron todos los puentes pedidos: (pedidos, colocados, perímetro mm).
    pub short: Vec<(usize, usize, f64)>,
    /// Formas cerradas con puentes activos que se procesaron (colocaran o no).
    pub shapes: usize,
}

impl TabStats {
    pub fn record(&mut self, r: &TabResult) {
        if r.requested == 0 {
            return;
        }
        self.shapes += 1;
        self.placed += r.placed;
        if !r.fits() {
            self.short.push((r.requested, r.placed, r.length_mm));
        }
    }

    /// Advertencia (o ninguna) con el estado de los puentes de la capa `name`.
    pub fn warning(&self, name: &str, width_mm: f64) -> Option<String> {
        if self.short.is_empty() {
            return None;
        }
        let none = self.short.iter().filter(|s| s.1 == 0).count();
        let shortest = self.short.iter().map(|s| s.2).fold(f64::INFINITY, f64::min);
        Some(format!(
            "capa {name}: los puentes de {width_mm} mm no caben en {} forma(s) (el contorno más \
             corto mide {shortest:.2} mm); {none} se cortan sin puentes y el resto con menos de los \
             pedidos. Los puentes no ocupan más de la mitad del contorno",
            self.short.len()
        ))
    }
}

/// Aviso del ancho de los puentes frente al kerf de la capa. `kerf_mm` es el desplazamiento por
/// lado (la mitad del ancho del corte), así que el haz se come `2 * |kerf|` del puente. Fuerte si
/// `width < 2 * |kerf|` (no queda puente: las piezas se sueltan); suave si `width < 4 * |kerf|`
/// (queda un puente de menos de un ancho de corte, que se rompe al manipular la placa).
pub(crate) fn kerf_warning(layer: &str, width_mm: f64, kerf_mm: f64) -> Option<String> {
    let k = kerf_mm.abs();
    if !(k.is_finite() && k > 0.0 && width_mm.is_finite()) {
        return None;
    }
    let cut = 2.0 * k;
    if width_mm < cut {
        Some(format!(
            "capa {layer}: los puentes de {width_mm} mm no sobreviven al corte: con kerf {kerf_mm} mm \
             por lado el haz quita {cut} mm y no queda puente (las piezas se soltarán); usa un ancho \
             de al menos {} mm",
            2.0 * cut
        ))
    } else if width_mm < 2.0 * cut {
        Some(format!(
            "capa {layer}: puentes frágiles: de {width_mm} mm quedan unos {:.2} mm tras el corte \
             (kerf {kerf_mm} mm por lado); se recomienda un ancho de al menos {} mm",
            width_mm - cut,
            2.0 * cut
        ))
    } else {
        None
    }
}

/// Agrega a `moves` el recorrido de una forma: `pieces` es el contorno entero (un solo
/// elemento) o los tramos que dejan los puentes. El primer tramo entra con un `Travel`; los
/// huecos entre tramos se cruzan con un `Cut` a potencia 0.
pub(crate) fn push_shape(moves: &mut Vec<Move>, pieces: &[Polyline], power_pct: f64) {
    let mut first = true;
    for piece in pieces.iter().filter(|p| !p.points.is_empty()) {
        let start: Point = piece.start();
        moves.push(if first {
            Move::Travel { to: start }
        } else {
            Move::Cut {
                to: start,
                power_pct: 0.0,
            }
        });
        first = false;
        let rest = piece.points[1..]
            .iter()
            .chain(piece.closed.then_some(&piece.points[0]));
        moves.extend(rest.map(|&to| Move::Cut { to, power_pct }));
    }
}

/// Tramos de `path` según los puentes; el contorno entero si no corresponde partirlo.
pub(crate) fn split(path: &Polyline, spec: &TabSpec, stats: &mut TabStats) -> Vec<Polyline> {
    let r = tab_segments(path, &spec.options);
    stats.record(&r);
    r.segments
}

#[cfg(test)]
mod tests {
    use super::*;
    use tlaku_core::Rgb;

    fn layer(edit: impl FnOnce(&mut LayerSettings)) -> LayerSettings {
        let mut l = LayerSettings::for_color(Rgb(255, 0, 0));
        l.tabs.enabled = true;
        edit(&mut l);
        l
    }

    #[test]
    fn disabled_tabs_resolve_to_none_even_with_bad_values() {
        let mut l = LayerSettings::for_color(Rgb(255, 0, 0));
        l.tabs.width = -3.0;
        assert_eq!(resolve(&l).unwrap(), None);
    }

    #[test]
    fn bad_values_name_the_layer() {
        for l in [
            layer(|l| l.tabs.width = 0.0),
            layer(|l| l.tabs.width = f64::NAN),
            layer(|l| l.tabs.spacing = -1.0),
            layer(|l| l.tabs.count = 5000),
            layer(|l| {
                l.tabs.count = 0;
                l.tabs.spacing = 0.0
            }),
        ] {
            let e = resolve(&l).unwrap_err();
            assert!(e.starts_with("capa #ff0000: puentes:"), "{e}");
        }
        let ok = resolve(&layer(|l| {
            l.tabs.count = 0;
            l.tabs.spacing = 20.0
        }))
        .unwrap()
        .unwrap();
        assert_eq!(ok.options.spacing_mm, 20.0);
    }

    #[test]
    fn push_shape_crosses_gaps_with_laser_off_cuts() {
        let piece = |pts: &[(f64, f64)], closed| Polyline {
            points: pts.iter().map(|&(x, y)| Point::new(x, y)).collect(),
            closed,
        };
        let mut moves = Vec::new();
        push_shape(
            &mut moves,
            &[
                piece(&[(0.0, 0.0), (4.0, 0.0)], false),
                piece(&[(6.0, 0.0), (10.0, 0.0)], false),
            ],
            30.0,
        );
        assert_eq!(
            moves,
            vec![
                Move::Travel {
                    to: Point::new(0.0, 0.0)
                },
                Move::Cut {
                    to: Point::new(4.0, 0.0),
                    power_pct: 30.0
                },
                Move::Cut {
                    to: Point::new(6.0, 0.0),
                    power_pct: 0.0
                },
                Move::Cut {
                    to: Point::new(10.0, 0.0),
                    power_pct: 30.0
                },
            ]
        );
        // Un contorno cerrado sin partir se cierra solo.
        let mut closed = Vec::new();
        push_shape(
            &mut closed,
            &[piece(&[(0.0, 0.0), (1.0, 0.0), (1.0, 1.0)], true)],
            10.0,
        );
        assert_eq!(closed.len(), 4);
        assert_eq!(closed[3].to(), Point::new(0.0, 0.0));
    }

    #[test]
    fn warning_counts_shapes_and_mentions_the_shortest() {
        let mut s = TabStats::default();
        assert_eq!(s.warning("#ff0000", 2.0), None);
        s.record(&TabResult {
            segments: vec![],
            requested: 4,
            placed: 0,
            length_mm: 3.0,
        });
        s.record(&TabResult {
            segments: vec![],
            requested: 4,
            placed: 2,
            length_mm: 9.0,
        });
        s.record(&TabResult {
            segments: vec![],
            requested: 4,
            placed: 4,
            length_mm: 90.0,
        });
        assert_eq!((s.shapes, s.placed), (3, 6));
        let w = s.warning("#ff0000", 2.0).unwrap();
        assert!(
            w.contains("2 forma(s)") && w.contains("3.00 mm") && w.contains("1 se cortan sin"),
            "{w}"
        );
    }
}
