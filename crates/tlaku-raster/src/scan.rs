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
/// y así. Con `angle_deg` distinto de 0 las líneas van giradas ese ángulo (ver
/// `rotated_moves`); con 0 el resultado es el barrido por filas de siempre. Un tramo más
/// corto que `dot_width_mm` queda como un pulso mínimo centrado.
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

    let angle = normalized_angle(settings.angle_deg);
    let moves = if angle == 0.0 {
        horizontal_moves(bitmap, origin, settings, min_pct, max_pct)
    } else {
        rotated_moves(bitmap, origin, settings, angle, min_pct, max_pct)
    };

    ToolpathBlock {
        label: format!(
            "Imagen {}x{} puntos, {:.3}x{:.3} mm, velocidad={} potencia={}-{}%{}",
            bitmap.width,
            bitmap.height,
            bitmap.width_mm(),
            bitmap.height_mm(),
            settings.speed_mm_min,
            min_pct,
            max_pct,
            if angle == 0.0 {
                String::new()
            } else {
                format!(" ángulo={angle}")
            }
        ),
        speed_mm_min: settings.speed_mm_min,
        air_assist: settings.air_assist,
        power_mode: settings.power_mode,
        moves,
        z_mm: None,
        galvo: None,
    }
}

/// Ángulo en grados dentro de [0, 360); los valores no finitos o casi múltiplos de 360
/// valen 0 (barrido horizontal clásico).
fn normalized_angle(deg: f64) -> f64 {
    if !deg.is_finite() {
        return 0.0;
    }
    let a = deg.rem_euclid(360.0);
    if a < 1e-9 || 360.0 - a < 1e-9 { 0.0 } else { a }
}

/// Barrido horizontal: una línea por fila del bitmap.
fn horizontal_moves(
    bitmap: &RasterBitmap,
    origin: Point,
    settings: &RasterSettings,
    min_pct: f64,
    max_pct: f64,
) -> Vec<Move> {
    let overscan = settings.overscan_mm.max(0.0);
    let half_dot = settings.dot_width_mm.max(0.0) / 2.0;
    let mut moves = Vec::new();
    let mut scanned = 0usize;
    for row in 0..bitmap.height {
        let y = origin.y
            + (bitmap.height - 1 - row) as f64 * bitmap.pixel_size_y_mm
            + bitmap.pixel_size_y_mm / 2.0;
        let runs = row_runs(
            bitmap.row(row),
            origin.x,
            bitmap.pixel_size_x_mm,
            origin.x + bitmap.width as f64 * bitmap.pixel_size_x_mm,
            min_pct,
            max_pct,
            half_dot,
        );
        let (first, last) = match (runs.first(), runs.last()) {
            (Some(f), Some(l)) => (f.x0, l.x1),
            _ if settings.skip_blank_rows => continue,
            _ => (origin.x, origin.x + bitmap.width_mm()),
        };
        let reverse = settings.bidirectional && scanned % 2 == 1;
        scanned += 1;
        emit_row(
            &mut moves,
            &|x| Point::new(x, y),
            &runs,
            (first, last),
            overscan,
            reverse,
        );
    }
    moves
}

/// Barrido con líneas giradas `angle` grados (antihorario) alrededor del centro de la
/// imagen colocada. Las líneas, separadas `pixel_size_y_mm`, se recortan al rectángulo
/// de la imagen y se muestrean con vecino más cercano cada `pixel_size_x_mm` (el
/// bitmap no se gira ni se interpola, para no inventar niveles de potencia). La
/// primera línea es la de más "arriba" respecto de la dirección girada; con 0 grados
/// sería la fila 0. Dentro de cada línea la coordenada `s` va medida desde el centro
/// de la imagen a lo largo de la dirección de barrido.
fn rotated_moves(
    bitmap: &RasterBitmap,
    origin: Point,
    settings: &RasterSettings,
    angle: f64,
    min_pct: f64,
    max_pct: f64,
) -> Vec<Move> {
    let overscan = settings.overscan_mm.max(0.0);
    let half_dot = settings.dot_width_mm.max(0.0) / 2.0;
    let (w_mm, h_mm) = (bitmap.width_mm(), bitmap.height_mm());
    let (px, py) = (bitmap.pixel_size_x_mm, bitmap.pixel_size_y_mm);
    let (hw, hh) = (w_mm / 2.0, h_mm / 2.0);
    let (cx, cy) = (origin.x + hw, origin.y + hh);
    let (sin, cos) = angle.to_radians().sin_cos();
    // Mitad de la extensión del rectángulo proyectada sobre la perpendicular.
    let extent = hw * sin.abs() + hh * cos.abs();
    let lines = ((2.0 * extent / py) - 1e-9).ceil().max(1.0) as usize;

    let mut moves = Vec::new();
    let mut scanned = 0usize;
    let mut values = Vec::new();
    for k in (0..lines).rev() {
        let t = (k as f64 + 0.5 - lines as f64 / 2.0) * py;
        // Recorte de la recta al rectángulo: |s*cos - t*sin| <= hw y |s*sin + t*cos| <= hh.
        let Some((lo_x, hi_x)) = slab(cos, -t * sin, hw) else {
            continue;
        };
        let Some((lo_y, hi_y)) = slab(sin, t * cos, hh) else {
            continue;
        };
        let (s0, s1) = (lo_x.max(lo_y), hi_x.min(hi_y));
        if s1 - s0 < 1e-9 {
            continue;
        }
        let count = (((s1 - s0) / px) - 1e-9).ceil().max(1.0) as usize;
        let ds = (s1 - s0) / count as f64;
        values.clear();
        for j in 0..count {
            let s = s0 + (j as f64 + 0.5) * ds;
            let lx = s * cos - t * sin;
            let ly = s * sin + t * cos;
            let col = (((lx + hw) / px).floor().max(0.0) as usize).min(bitmap.width - 1);
            let row = (((hh - ly) / py).floor().max(0.0) as usize).min(bitmap.height - 1);
            values.push(bitmap.get(col, row));
        }
        let runs = row_runs(&values, s0, ds, s1, min_pct, max_pct, half_dot);
        let (first, last) = match (runs.first(), runs.last()) {
            (Some(f), Some(l)) => (f.x0, l.x1),
            _ if settings.skip_blank_rows => continue,
            _ => (s0, s1),
        };
        let reverse = settings.bidirectional && scanned % 2 == 1;
        scanned += 1;
        emit_row(
            &mut moves,
            &|s| Point::new(cx + s * cos - t * sin, cy + s * sin + t * cos),
            &runs,
            (first, last),
            overscan,
            reverse,
        );
    }
    moves
}

/// Valores de `s` con `|a*s + b| <= half`. `None` si no hay ninguno.
fn slab(a: f64, b: f64, half: f64) -> Option<(f64, f64)> {
    if a.abs() < 1e-12 {
        return (b.abs() <= half).then_some((f64::NEG_INFINITY, f64::INFINITY));
    }
    let (p, q) = ((-half - b) / a, (half - b) / a);
    Some((p.min(q), p.max(q)))
}

/// Longitud mínima de un tramo encendido tras la corrección de ancho de punto.
const MIN_RUN_MM: f64 = 0.01;

/// Une muestras consecutivas de igual valor y aplica la corrección de ancho de punto.
/// La muestra `i` cubre `[s0 + i*ds, s0 + (i+1)*ds)`, recortada a `s_end`.
fn row_runs(
    data: &[f32],
    s0: f64,
    ds: f64,
    s_end: f64,
    min_pct: f64,
    max_pct: f64,
    half_dot: f64,
) -> Vec<Run> {
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
        let (left, right) = (s0 + start as f64 * ds, (s0 + i as f64 * ds).min(s_end));
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

/// Emite una línea. `at` convierte la coordenada a lo largo de la línea en un punto
/// de la máquina; los tramos (`runs`) y `first`/`last` usan esa misma coordenada.
fn emit_row(
    moves: &mut Vec<Move>,
    at: &dyn Fn(f64) -> Point,
    runs: &[Run],
    (first, last): (f64, f64),
    overscan: f64,
    reverse: bool,
) {
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

    // --- Barrido girado ---

    fn full(w: usize, h: usize, px: f64, py: f64, v: f32) -> RasterBitmap {
        RasterBitmap::new(w, h, px, py, vec![v; w * h]).unwrap()
    }

    /// Longitud total (mm) de los tramos con potencia > 0, y si todos los puntos del
    /// bloque están dentro del rectángulo `[x0, x1] x [y0, y1]`.
    fn lit_length(blk: &ToolpathBlock, rect: (f64, f64, f64, f64)) -> (f64, bool) {
        let (x0, y0, x1, y1) = rect;
        let eps = 1e-6;
        let mut len = 0.0;
        let mut inside = true;
        let mut prev: Option<Point> = None;
        for m in &blk.moves {
            let to = m.to();
            inside &= to.x >= x0 - eps && to.x <= x1 + eps && to.y >= y0 - eps && to.y <= y1 + eps;
            if let (Move::Cut { power_pct, .. }, Some(p)) = (m, prev)
                && *power_pct > 0.0
            {
                len += (to.x - p.x).hypot(to.y - p.y);
            }
            prev = Some(to);
        }
        (len, inside)
    }

    #[test]
    fn zero_angle_is_the_classic_scan() {
        let b = bitmap(&[".##..#.", "#..#.##"], 0.7, 0.4);
        let base = RasterSettings {
            overscan_mm: 1.0,
            bidirectional: true,
            dot_width_mm: 0.2,
            ..plain()
        };
        let classic = raster_to_block(&b, Point::new(3.0, 4.0), &base);
        for angle in [0.0, 360.0, -360.0, 720.0, f64::NAN, f64::INFINITY] {
            let s = RasterSettings {
                angle_deg: angle,
                ..base.clone()
            };
            assert_eq!(
                raster_to_block(&b, Point::new(3.0, 4.0), &s),
                classic,
                "ángulo {angle}"
            );
        }
        assert!(!classic.label.contains("ángulo"));
    }

    #[test]
    fn scan_at_90_degrees_runs_along_y() {
        // 4 columnas x 2 filas de 1 mm, en (10, 20): rectángulo [10, 14] x [20, 22].
        let b = full(4, 2, 1.0, 1.0, 1.0);
        let s = RasterSettings {
            angle_deg: 90.0,
            ..plain()
        };
        let blk = raster_to_block(&b, Point::new(10.0, 20.0), &s);
        let (len, inside) = lit_length(&blk, (10.0, 20.0, 14.0, 22.0));
        assert!(inside);
        assert!((len - 8.0).abs() < 1e-6, "{len}");
        // 4 líneas, una por columna, de x = 10.5 a 13.5, todas verticales de 2 mm.
        let travels: Vec<Point> = blk
            .moves
            .iter()
            .filter(|m| matches!(m, Move::Travel { .. }))
            .map(|m| m.to())
            .collect();
        assert_eq!(travels.len(), 4);
        let mut xs: Vec<f64> = travels.iter().map(|p| p.x).collect();
        xs.sort_by(f64::total_cmp);
        for (x, e) in xs.iter().zip([10.5, 11.5, 12.5, 13.5]) {
            assert!((x - e).abs() < 1e-9, "{xs:?}");
        }
        for m in &blk.moves {
            if let Move::Cut { to, .. } = m {
                assert!((to.y - 20.0).abs() < 1e-9 || (to.y - 22.0).abs() < 1e-9);
            }
        }
    }

    #[test]
    fn scan_at_90_degrees_follows_the_image() {
        // Solo la columna 2 (de 0 a 3) encendida: un único tramo en x = 12.5.
        let b = bitmap(&["..#.", "..#."], 1.0, 1.0);
        let s = RasterSettings {
            angle_deg: 90.0,
            ..plain()
        };
        let blk = raster_to_block(&b, Point::new(10.0, 20.0), &s);
        assert_eq!(blk.moves.len(), 2);
        let (a, z) = (blk.moves[0].to(), blk.moves[1].to());
        assert!((a.x - 12.5).abs() < 1e-9 && (z.x - 12.5).abs() < 1e-9);
        assert!((a.y - 20.0).abs() < 1e-9 && (z.y - 22.0).abs() < 1e-9);
    }

    #[test]
    fn scan_at_45_degrees_stays_inside_and_keeps_coverage() {
        let (w, h, px, py) = (40, 20, 0.5, 0.5);
        let origin = Point::new(5.0, -3.0);
        let rect = (5.0, -3.0, 5.0 + 20.0, -3.0 + 10.0);
        let all = full(w, h, px, py, 1.0);
        for angle in [45.0, 30.0, 135.0, 200.0, -45.0] {
            let s = RasterSettings {
                angle_deg: angle,
                ..plain()
            };
            let blk = raster_to_block(&all, origin, &s);
            let (len, inside) = lit_length(&blk, rect);
            assert!(inside, "ángulo {angle}");
            // Área = longitud barrida x separación de líneas.
            let area = len * py;
            assert!(
                (area - 200.0).abs() / 200.0 < 0.02,
                "ángulo {angle}: {area}"
            );
        }
        // Mitad izquierda encendida: la mitad del área.
        let half = RasterBitmap::new(
            w,
            h,
            px,
            py,
            (0..w * h).map(|i| ((i % w) < w / 2) as u8 as f32).collect(),
        )
        .unwrap();
        let s = RasterSettings {
            angle_deg: 45.0,
            ..plain()
        };
        let blk = raster_to_block(&half, origin, &s);
        let (len, inside) = lit_length(&blk, rect);
        assert!(inside);
        assert!((len * py - 100.0).abs() / 100.0 < 0.05, "{}", len * py);
    }

    #[test]
    fn rotated_scan_keeps_gray_power_and_alternates_direction() {
        let b = full(10, 10, 1.0, 1.0, 0.5);
        let s = RasterSettings {
            angle_deg: 45.0,
            bidirectional: true,
            min_power_pct: 10.0,
            max_power_pct: 30.0,
            ..plain()
        };
        let blk = raster_to_block(&b, Point::new(0.0, 0.0), &s);
        assert!(blk.label.contains("ángulo=45"));
        let mut dirs = Vec::new();
        let mut last_travel = None;
        for m in &blk.moves {
            match m {
                Move::Travel { to } => last_travel = Some(*to),
                Move::Cut { to, power_pct } => {
                    assert!((power_pct - 20.0).abs() < 1e-9);
                    let from = last_travel.take().unwrap();
                    dirs.push((to.x - from.x) + (to.y - from.y));
                }
                Move::Dwell { .. } => unreachable!("el raster no emite pausas"),
            }
        }
        assert!(dirs.len() > 4);
        // Dirección +u (arriba a la derecha) y -u alternadas.
        for (i, d) in dirs.iter().enumerate() {
            assert_eq!(*d > 0.0, i % 2 == 0, "línea {i}: {d}");
        }
    }

    #[test]
    fn rotated_overscan_runs_along_the_line() {
        let b = full(4, 2, 1.0, 1.0, 1.0);
        let s = RasterSettings {
            angle_deg: 90.0,
            overscan_mm: 1.5,
            ..plain()
        };
        let blk = raster_to_block(&b, Point::new(10.0, 20.0), &s);
        let first = blk.moves[0].to();
        assert!((first.y - 18.5).abs() < 1e-9, "{first:?}");
        let last = blk.moves.last().unwrap().to();
        assert!(
            blk.moves
                .iter()
                .all(|m| m.to().y >= 18.5 - 1e-9 && m.to().y <= 23.5 + 1e-9),
            "{last:?}"
        );
    }

    #[test]
    fn rotated_blank_lines_and_dot_width() {
        // Mitad derecha vacía: a 90 grados, las columnas vacías son líneas en blanco.
        let b = bitmap(&["##..", "##.."], 1.0, 1.0);
        let skip = RasterSettings {
            angle_deg: 90.0,
            ..plain()
        };
        assert_eq!(
            raster_to_block(&b, Point::new(0.0, 0.0), &skip)
                .moves
                .iter()
                .filter(|m| matches!(m, Move::Travel { .. }))
                .count(),
            2
        );
        let all = RasterSettings {
            skip_blank_rows: false,
            ..skip.clone()
        };
        assert_eq!(
            raster_to_block(&b, Point::new(0.0, 0.0), &all)
                .moves
                .iter()
                .filter(|m| matches!(m, Move::Travel { .. }))
                .count(),
            4
        );
        // Corrección de ancho de punto a lo largo de la línea: tramo [0, 2] -> [0.2, 1.8].
        let dot = RasterSettings {
            dot_width_mm: 0.4,
            ..skip
        };
        let blk = raster_to_block(&b, Point::new(0.0, 0.0), &dot);
        let ys: Vec<f64> = blk.moves.iter().map(|m| m.to().y).collect();
        assert!(
            (ys[0] - 0.2).abs() < 1e-9 && (ys[1] - 1.8).abs() < 1e-9,
            "{ys:?}"
        );
    }
}
