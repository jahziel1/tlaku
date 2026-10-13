//! Conversión de un `RasterBitmap` a movimientos de barrido.

use crate::bitmap::RasterBitmap;
use tlaku_core::Point;
use tlaku_core::toolpath::{Move, PowerMode, ToolpathBlock};

#[derive(Debug, Clone, PartialEq)]
pub struct RasterSettings {
    pub speed_mm_min: f64,
    /// Potencia (%) de un punto con fracción mínima > 0; el gris 0..1 se mapea a
    /// `min_power_pct..max_power_pct`. Los modos binarios usan siempre la máxima.
    pub min_power_pct: f64,
    pub max_power_pct: f64,
    /// Filas alternas en sentido contrario.
    pub bidirectional: bool,
    /// Distancia (mm) de aceleración fuera del primer y último punto encendido de cada fila.
    pub overscan_mm: f64,
    /// Omite las filas sin contenido con un viaje rápido directo a la siguiente. Si
    /// es falso, se barren completas con potencia 0 a la velocidad de grabado.
    pub skip_blank_rows: bool,
    /// Corrección de ancho de punto: cada tramo encendido se acorta en la mitad por lado.
    pub dot_width_mm: f64,
    pub air_assist: bool,
    pub power_mode: PowerMode,
    /// Ángulo de las líneas de barrido en grados, antihorario (0 = horizontal).
    pub angle_deg: f64,
}

impl Default for RasterSettings {
    fn default() -> Self {
        Self {
            speed_mm_min: 3000.0,
            min_power_pct: 0.0,
            max_power_pct: 20.0,
            bidirectional: true,
            overscan_mm: 2.0,
            skip_blank_rows: true,
            dot_width_mm: 0.0,
            air_assist: false,
            power_mode: PowerMode::Dynamic,
            angle_deg: 0.0,
        }
    }
}

impl RasterSettings {
    pub fn validate(&self) -> Result<(), String> {
        if self.speed_mm_min.is_nan() || self.speed_mm_min <= 0.0 {
            return Err(format!("velocidad inválida: {}", self.speed_mm_min));
        }
        for (name, v) in [
            ("mínima", self.min_power_pct),
            ("máxima", self.max_power_pct),
        ] {
            if !(0.0..=100.0).contains(&v) {
                return Err(format!("potencia {name} fuera de 0-100 %: {v}"));
            }
        }
        if self.min_power_pct > self.max_power_pct {
            return Err("la potencia mínima supera a la máxima".into());
        }
        if !(self.overscan_mm >= 0.0 && self.overscan_mm.is_finite()) {
            return Err(format!("overscan inválido: {}", self.overscan_mm));
        }
        if !(self.dot_width_mm >= 0.0 && self.dot_width_mm.is_finite()) {
            return Err(format!("ancho de punto inválido: {}", self.dot_width_mm));
        }
        Ok(())
    }
}

/// Tramo encendido de una fila: columnas `[start, end)` y fracción de potencia.
struct Run {
    x0: f64,
    x1: f64,
    power_pct: f64,
}

/// Genera el barrido. `origin` es la esquina inferior izquierda de la imagen en la
/// máquina. La fila 0 del bitmap queda arriba (Y mayor); cada fila se recorre en su
/// centro. Los ajustes fuera de rango se acotan (usa `RasterSettings::validate` para
/// avisar al usuario antes); el overscan no se recorta a los límites de la máquina.
///
/// Por fila con contenido: Travel a `inicio - overscan`, Cut a potencia 0 hasta el primer
/// tramo, tramos encendidos, Cut a potencia 0 por los huecos y overscan al final. Con
/// `bidirectional`, la 1.ª fila con contenido va de izquierda a derecha, la 2.ª al revés,
/// y así. Un tramo más corto que `dot_width_mm` queda como un pulso mínimo centrado.
pub fn raster_to_block(
    bitmap: &RasterBitmap,
    origin: Point,
    settings: &RasterSettings,
) -> ToolpathBlock {
    assert_eq!(
        bitmap.data.len(),
        bitmap.width * bitmap.height,
        "datos del bitmap inconsistentes"
    );
    let max_pct = settings.max_power_pct.clamp(0.0, 100.0);
    let min_pct = settings.min_power_pct.clamp(0.0, max_pct);
    let overscan = settings.overscan_mm.max(0.0);
    let half_dot = settings.dot_width_mm.max(0.0) / 2.0;

    let mut moves = Vec::new();
    let mut scanned = 0usize;
    for row in 0..bitmap.height {
        let y = origin.y
            + (bitmap.height - 1 - row) as f64 * bitmap.pixel_size_y_mm
            + bitmap.pixel_size_y_mm / 2.0;
        let runs = row_runs(bitmap, row, origin.x, min_pct, max_pct, half_dot);
        let (first, last) = match (runs.first(), runs.last()) {
            (Some(f), Some(l)) => (f.x0, l.x1),
            _ if settings.skip_blank_rows => continue,
            _ => (origin.x, origin.x + bitmap.width_mm()),
        };
        let reverse = settings.bidirectional && scanned % 2 == 1;
        scanned += 1;
        emit_row(&mut moves, y, &runs, (first, last), overscan, reverse);
    }

    ToolpathBlock {
        label: format!(
            "Imagen {}x{} puntos, {:.3}x{:.3} mm, velocidad={} potencia={}-{}%",
            bitmap.width,
            bitmap.height,
            bitmap.width_mm(),
            bitmap.height_mm(),
            settings.speed_mm_min,
            min_pct,
            max_pct
        ),
        speed_mm_min: settings.speed_mm_min,
        air_assist: settings.air_assist,
        power_mode: settings.power_mode,
        moves,
        z_mm: None,
    }
}

/// Longitud mínima de un tramo encendido tras la corrección de ancho de punto.
const MIN_RUN_MM: f64 = 0.01;

/// Une píxeles consecutivos de igual valor y aplica la corrección de ancho de punto.
fn row_runs(
    bitmap: &RasterBitmap,
    row: usize,
    origin_x: f64,
    min_pct: f64,
    max_pct: f64,
    half_dot: f64,
) -> Vec<Run> {
    let data = bitmap.row(row);
    let px = bitmap.pixel_size_x_mm;
    let mut runs = Vec::new();
    let mut i = 0;
    while i < data.len() {
        let v = data[i];
        // También descarta NaN.
        if v.is_nan() || v <= 0.0 {
            i += 1;
            continue;
        }
        let start = i;
        while i < data.len() && data[i] == v {
            i += 1;
        }
        let (left, right) = (origin_x + start as f64 * px, origin_x + i as f64 * px);
        let (mut x0, mut x1) = (left + half_dot, right - half_dot);
        if x1 - x0 < MIN_RUN_MM {
            // El haz ya es más ancho que el tramo: un pulso corto centrado deja el punto
            // en vez de perder el detalle fino del dithering.
            let center = (left + right) / 2.0;
            (x0, x1) = (center - MIN_RUN_MM / 2.0, center + MIN_RUN_MM / 2.0);
        }
        let frac = f64::from(v).clamp(0.0, 1.0);
        runs.push(Run {
            x0,
            x1,
            power_pct: min_pct + frac * (max_pct - min_pct),
        });
    }
    runs
}

fn emit_row(
    moves: &mut Vec<Move>,
    y: f64,
    runs: &[Run],
    (first, last): (f64, f64),
    overscan: f64,
    reverse: bool,
) {
    let at = |x: f64| Point::new(x, y);
    let off = |x: f64| Move::Cut {
        to: at(x),
        power_pct: 0.0,
    };
    let (start, end, sign) = if reverse {
        (last, first, -1.0)
    } else {
        (first, last, 1.0)
    };
    moves.push(Move::Travel {
        to: at(start - sign * overscan),
    });
    if overscan > 0.0 {
        moves.push(off(start));
    }
    let ordered: Vec<&Run> = if reverse {
        runs.iter().rev().collect()
    } else {
        runs.iter().collect()
    };
    let mut pos = start;
    for run in ordered {
        let (from, to) = if reverse {
            (run.x1, run.x0)
        } else {
            (run.x0, run.x1)
        };
        if (from - pos).abs() > 0.0 {
            moves.push(off(from));
        }
        moves.push(Move::Cut {
            to: at(to),
            power_pct: run.power_pct,
        });
        pos = to;
    }
    if pos != end {
        moves.push(off(end));
    }
    if overscan > 0.0 {
        moves.push(off(end + sign * overscan));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bitmap(rows: &[&str], px: f64, py: f64) -> RasterBitmap {
        let h = rows.len();
        let w = rows[0].len();
        let data = rows
            .iter()
            .flat_map(|r| r.chars())
            .map(|c| match c {
                '#' => 1.0,
                '+' => 0.5,
                _ => 0.0,
            })
            .collect();
        RasterBitmap::new(w, h, px, py, data).unwrap()
    }

    fn plain() -> RasterSettings {
        RasterSettings {
            speed_mm_min: 1000.0,
            min_power_pct: 0.0,
            max_power_pct: 50.0,
            bidirectional: false,
            overscan_mm: 0.0,
            ..Default::default()
        }
    }

    fn travel(x: f64, y: f64) -> Move {
        Move::Travel {
            to: Point::new(x, y),
        }
    }

    fn cut(x: f64, y: f64, p: f64) -> Move {
        Move::Cut {
            to: Point::new(x, y),
            power_pct: p,
        }
    }

    #[test]
    fn simple_row_with_gap() {
        let b = bitmap(&[".##..#."], 1.0, 1.0);
        let blk = raster_to_block(&b, Point::new(10.0, 20.0), &plain());
        assert_eq!(
            blk.moves,
            vec![
                travel(11.0, 20.5),
                cut(13.0, 20.5, 50.0),
                cut(15.0, 20.5, 0.0),
                cut(16.0, 20.5, 50.0),
            ]
        );
        assert_eq!(blk.speed_mm_min, 1000.0);
    }

    #[test]
    fn rows_top_down_and_blank_rows_skipped() {
        let b = bitmap(&["#.", "..", "..", ".#"], 1.0, 0.5);
        let blk = raster_to_block(&b, Point::new(0.0, 0.0), &plain());
        assert_eq!(
            blk.moves,
            vec![
                travel(0.0, 1.75),
                cut(1.0, 1.75, 50.0),
                travel(1.0, 0.25),
                cut(2.0, 0.25, 50.0),
            ]
        );
    }

    #[test]
    fn blank_rows_scanned_when_not_skipping() {
        let b = bitmap(&["#.", ".."], 1.0, 1.0);
        let s = RasterSettings {
            skip_blank_rows: false,
            ..plain()
        };
        let blk = raster_to_block(&b, Point::new(0.0, 0.0), &s);
        assert_eq!(
            blk.moves,
            vec![
                travel(0.0, 1.5),
                cut(1.0, 1.5, 50.0),
                travel(0.0, 0.5),
                cut(2.0, 0.5, 0.0),
            ]
        );
    }

    #[test]
    fn fully_blank_bitmap_is_empty() {
        let b = bitmap(&["...", "..."], 1.0, 1.0);
        assert!(
            raster_to_block(&b, Point::new(0.0, 0.0), &plain())
                .moves
                .is_empty()
        );
    }

    #[test]
    fn overscan_both_directions() {
        let b = bitmap(&[".#.#.", ".#.#."], 1.0, 1.0);
        let s = RasterSettings {
            overscan_mm: 2.0,
            bidirectional: true,
            ..plain()
        };
        let blk = raster_to_block(&b, Point::new(0.0, 0.0), &s);
        assert_eq!(
            blk.moves,
            vec![
                // fila de arriba, izquierda a derecha
                travel(-1.0, 1.5),
                cut(1.0, 1.5, 0.0),
                cut(2.0, 1.5, 50.0),
                cut(3.0, 1.5, 0.0),
                cut(4.0, 1.5, 50.0),
                cut(6.0, 1.5, 0.0),
                // fila de abajo, derecha a izquierda
                travel(6.0, 0.5),
                cut(4.0, 0.5, 0.0),
                cut(3.0, 0.5, 50.0),
                cut(2.0, 0.5, 0.0),
                cut(1.0, 0.5, 50.0),
                cut(-1.0, 0.5, 0.0),
            ]
        );
    }

    #[test]
    fn unidirectional_always_left_to_right() {
        let b = bitmap(&["#", "#"], 1.0, 1.0);
        let s = RasterSettings {
            overscan_mm: 1.0,
            ..plain()
        };
        let blk = raster_to_block(&b, Point::new(0.0, 0.0), &s);
        let travels: Vec<f64> = blk
            .moves
            .iter()
            .filter(|m| matches!(m, Move::Travel { .. }))
            .map(|m| m.to().x)
            .collect();
        assert_eq!(travels, vec![-1.0, -1.0]);
    }

    #[test]
    fn bidirectional_alternates_over_emitted_rows() {
        let b = bitmap(&["#", ".", "#"], 1.0, 1.0);
        let s = RasterSettings {
            bidirectional: true,
            ..plain()
        };
        let blk = raster_to_block(&b, Point::new(0.0, 0.0), &s);
        // La 2.ª fila con contenido va de derecha a izquierda aunque haya una en blanco en medio.
        assert_eq!(
            blk.moves,
            vec![
                travel(0.0, 2.5),
                cut(1.0, 2.5, 50.0),
                travel(1.0, 0.5),
                cut(0.0, 0.5, 50.0),
            ]
        );
    }

    #[test]
    fn grayscale_maps_levels_between_min_and_max() {
        let b = bitmap(&["+#."], 1.0, 1.0);
        let s = RasterSettings {
            min_power_pct: 10.0,
            max_power_pct: 30.0,
            ..plain()
        };
        let blk = raster_to_block(&b, Point::new(0.0, 0.0), &s);
        assert_eq!(
            blk.moves,
            vec![travel(0.0, 0.5), cut(1.0, 0.5, 20.0), cut(2.0, 0.5, 30.0)]
        );
    }

    #[test]
    fn equal_neighbors_merge_different_split() {
        let b = bitmap(&["++##"], 0.5, 1.0);
        let blk = raster_to_block(&b, Point::new(0.0, 0.0), &plain());
        assert_eq!(
            blk.moves,
            vec![travel(0.0, 0.5), cut(1.0, 0.5, 25.0), cut(2.0, 0.5, 50.0)]
        );
    }

    #[test]
    fn dot_width_shortens_runs_symmetrically() {
        let b = bitmap(&[".###..#."], 1.0, 1.0);
        let s = RasterSettings {
            dot_width_mm: 0.4,
            overscan_mm: 1.0,
            ..plain()
        };
        let blk = raster_to_block(&b, Point::new(0.0, 0.0), &s);
        // Tramos [1, 4) y [6, 7) acortados 0.2 por lado.
        let expect = [
            travel(0.2, 0.5),
            cut(1.2, 0.5, 0.0),
            cut(3.8, 0.5, 50.0),
            cut(6.2, 0.5, 0.0),
            cut(6.8, 0.5, 50.0),
            cut(7.8, 0.5, 0.0),
        ];
        assert_eq!(blk.moves.len(), expect.len());
        for (m, e) in blk.moves.iter().zip(&expect) {
            assert!((m.to().x - e.to().x).abs() < 1e-9, "{m:?} vs {e:?}");
            assert_eq!(std::mem::discriminant(m), std::mem::discriminant(e));
        }
    }

    #[test]
    fn dot_width_keeps_isolated_dots_as_short_pulses() {
        let b = bitmap(&["#.", ".#"], 1.0, 1.0);
        let s = RasterSettings {
            dot_width_mm: 1.0,
            ..plain()
        };
        let lit: Vec<(f64, f64)> = raster_to_block(&b, Point::new(0.0, 0.0), &s)
            .moves
            .windows(2)
            .filter_map(|w| match w[1] {
                Move::Cut { to, power_pct } if power_pct > 0.0 => Some((w[0].to().x, to.x)),
                _ => None,
            })
            .collect();
        assert_eq!(lit.len(), 2);
        for ((a, b), center) in lit.iter().zip([0.5, 1.5]) {
            assert!(((a + b) / 2.0 - center).abs() < 1e-9);
            assert!(((b - a).abs() - MIN_RUN_MM).abs() < 1e-9);
        }
    }

    #[test]
    fn settings_are_clamped_and_validated() {
        let bad = RasterSettings {
            max_power_pct: 150.0,
            overscan_mm: -3.0,
            ..plain()
        };
        assert!(bad.validate().is_err());
        let b = bitmap(&["#"], 1.0, 1.0);
        let blk = raster_to_block(&b, Point::new(0.0, 0.0), &bad);
        assert_eq!(blk.moves, vec![travel(0.0, 0.5), cut(1.0, 0.5, 100.0)]);
        assert!(plain().validate().is_ok());
        let inverted = RasterSettings {
            min_power_pct: 60.0,
            ..plain()
        };
        assert!(inverted.validate().is_err());
        assert!(
            RasterSettings {
                speed_mm_min: 0.0,
                ..plain()
            }
            .validate()
            .is_err()
        );
    }
}
