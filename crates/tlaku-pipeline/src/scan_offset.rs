//! Corrección del desfase entre las líneas de ida y de vuelta de un barrido bidireccional.
//!
//! En un barrido bidireccional el láser tarda un poco en responder, así que las líneas que se
//! recorren al revés quedan corridas respecto de las de ida. El perfil de la máquina guarda ese
//! corrimiento medido a varias velocidades (`DeviceProfile.scan_offsets`); aquí se corrige como
//! posproceso del bloque ya generado, sin que el relleno ni el rasterizador lo sepan.
//!
//! - Una "línea" es un `Travel` más los movimientos que le siguen hasta el siguiente `Travel`
//!   (incluido el overscan, que se desplaza junto con ella).
//! - Una línea es de vuelta si su sentido va contra el de barrido. El sentido de barrido sale
//!   del ángulo de la capa (0° = +X, antihorario); con crosshatch hay un segundo barrido a 90°
//!   y cada línea se compara con el eje con el que más se alinea. Así no depende de cómo se
//!   reparten las líneas entre formas ni de cuántas hay.
//! - Las líneas de vuelta se desplazan `offset` mm **en el sentido de barrido** (el de las
//!   líneas de ida, no el suyo propio). Un offset positivo las adelanta; uno negativo las
//!   atrasa.
//! - El offset de un bloque sale de interpolar linealmente por su velocidad; fuera del rango
//!   medido se usa el extremo más cercano.

use tlaku_core::Point;
use tlaku_core::toolpath::{Move, ToolpathBlock};
use tlaku_doc::ScanOffset;

/// Offset (mm) para `speed_mm_min`: interpolación lineal entre las muestras ordenadas por
/// velocidad; por debajo de la primera o por encima de la última, el valor del extremo.
/// Ignora las muestras no finitas. Sin muestras válidas devuelve 0.
pub(crate) fn offset_for_speed(offsets: &[ScanOffset], speed_mm_min: f64) -> f64 {
    let mut table: Vec<&ScanOffset> = offsets
        .iter()
        .filter(|o| o.speed.is_finite() && o.offset.is_finite())
        .collect();
    table.sort_by(|a, b| a.speed.total_cmp(&b.speed));
    let (Some(first), Some(last)) = (table.first(), table.last()) else {
        return 0.0;
    };
    if !speed_mm_min.is_finite() || speed_mm_min <= first.speed {
        return first.offset;
    }
    if speed_mm_min >= last.speed {
        return last.offset;
    }
    for pair in table.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        if speed_mm_min <= b.speed {
            let span = b.speed - a.speed;
            if span <= 0.0 {
                return b.offset;
            }
            return a.offset + (b.offset - a.offset) * (speed_mm_min - a.speed) / span;
        }
    }
    last.offset
}

fn shift(mv: &mut Move, dx: f64, dy: f64) {
    match mv {
        Move::Travel { to } | Move::Cut { to, .. } => *to = Point::new(to.x + dx, to.y + dy),
    }
}

/// Desplaza las líneas de vuelta de `block` según `offsets`. `sweep_angles_deg` son los
/// ángulos de barrido de la capa (uno, o dos con crosshatch). No hace nada sin muestras
/// o con un offset de 0.
pub(crate) fn apply(block: &mut ToolpathBlock, offsets: &[ScanOffset], sweep_angles_deg: &[f64]) {
    let offset = offset_for_speed(offsets, block.speed_mm_min);
    if offset == 0.0 || sweep_angles_deg.is_empty() {
        return;
    }
    let axes: Vec<(f64, f64)> = sweep_angles_deg
        .iter()
        .map(|a| {
            let r = a.to_radians();
            (r.cos(), r.sin())
        })
        .collect();

    let moves = &mut block.moves;
    let mut i = 0;
    while i < moves.len() {
        // Una línea empieza en un Travel; lo que haya antes del primero (no lo genera nadie)
        // se deja como está.
        if !matches!(moves[i], Move::Travel { .. }) {
            i += 1;
            continue;
        }
        let end = moves[i + 1..]
            .iter()
            .position(|m| matches!(m, Move::Travel { .. }))
            .map_or(moves.len(), |k| i + 1 + k);
        let (from, to) = (moves[i].to(), moves[end - 1].to());
        let dir = (to.x - from.x, to.y - from.y);
        // El eje con el que más se alinea la línea; es de vuelta si va en contra de él.
        let best = axes
            .iter()
            .map(|&(ax, ay)| (ax, ay, dir.0 * ax + dir.1 * ay))
            .max_by(|a, b| a.2.abs().total_cmp(&b.2.abs()));
        if let Some((ax, ay, dot)) = best
            && dot < 0.0
        {
            let (dx, dy) = (ax * offset, ay * offset);
            for mv in &mut moves[i..end] {
                shift(mv, dx, dy);
            }
        }
        i = end;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tlaku_core::toolpath::PowerMode;

    fn so(speed: f64, offset: f64) -> ScanOffset {
        ScanOffset { speed, offset }
    }

    fn block(speed: f64, moves: Vec<Move>) -> ToolpathBlock {
        ToolpathBlock {
            label: "t".into(),
            speed_mm_min: speed,
            air_assist: false,
            power_mode: PowerMode::Dynamic,
            moves,
            z_mm: None,
        }
    }

    fn travel(x: f64, y: f64) -> Move {
        Move::Travel {
            to: Point::new(x, y),
        }
    }

    fn cut(x: f64, y: f64, power_pct: f64) -> Move {
        Move::Cut {
            to: Point::new(x, y),
            power_pct,
        }
    }

    fn assert_moves(actual: &[Move], expected: &[Move]) {
        assert_eq!(actual.len(), expected.len());
        for (a, e) in actual.iter().zip(expected) {
            let (pa, pe) = (a.to(), e.to());
            assert!(
                (pa.x - pe.x).abs() < 1e-9 && (pa.y - pe.y).abs() < 1e-9,
                "{a:?} != {e:?}"
            );
            assert_eq!(std::mem::discriminant(a), std::mem::discriminant(e));
        }
    }

    #[test]
    fn interpolates_by_speed_and_clamps_at_the_ends() {
        let t = [so(6000.0, 0.4), so(1000.0, 0.1), so(3000.0, 0.2)];
        assert_eq!(offset_for_speed(&t, 500.0), 0.1);
        assert_eq!(offset_for_speed(&t, 1000.0), 0.1);
        assert!((offset_for_speed(&t, 2000.0) - 0.15).abs() < 1e-12);
        assert!((offset_for_speed(&t, 3000.0) - 0.2).abs() < 1e-12);
        assert!((offset_for_speed(&t, 4500.0) - 0.3).abs() < 1e-12);
        assert_eq!(offset_for_speed(&t, 9000.0), 0.4);
        assert_eq!(offset_for_speed(&[so(2000.0, -0.25)], 100.0), -0.25);
        assert_eq!(offset_for_speed(&[], 1000.0), 0.0);
        assert_eq!(offset_for_speed(&[so(f64::NAN, 1.0)], 1000.0), 0.0);
    }

    #[test]
    fn only_return_lines_move_along_the_sweep_axis() {
        // Ida (+X), vuelta (-X), ida (+X); la vuelta lleva overscan que se mueve con ella.
        let mut b = block(
            3000.0,
            vec![
                travel(-1.0, 0.0),
                cut(0.0, 0.0, 0.0),
                cut(10.0, 0.0, 50.0),
                cut(11.0, 0.0, 0.0),
                travel(11.0, 1.0),
                cut(10.0, 1.0, 0.0),
                cut(0.0, 1.0, 50.0),
                cut(-1.0, 1.0, 0.0),
                travel(-1.0, 2.0),
                cut(10.0, 2.0, 50.0),
            ],
        );
        apply(&mut b, &[so(3000.0, 0.2)], &[0.0]);
        assert_moves(
            &b.moves,
            &[
                travel(-1.0, 0.0),
                cut(0.0, 0.0, 0.0),
                cut(10.0, 0.0, 50.0),
                cut(11.0, 0.0, 0.0),
                travel(11.2, 1.0),
                cut(10.2, 1.0, 0.0),
                cut(0.2, 1.0, 50.0),
                cut(-0.8, 1.0, 0.0),
                travel(-1.0, 2.0),
                cut(10.0, 2.0, 50.0),
            ],
        );
    }

    #[test]
    fn works_at_any_sweep_angle() {
        for angle in [0.0f64, 30.0, 90.0, 135.0, 200.0, -60.0] {
            let (c, s) = (angle.to_radians().cos(), angle.to_radians().sin());
            // Dos líneas paralelas al barrido, separadas en la dirección perpendicular.
            let line = |lateral: f64, forward: bool| {
                let at = |t: f64| (c * t - s * lateral, s * t + c * lateral);
                let (a, z) = if forward { (0.0, 10.0) } else { (10.0, 0.0) };
                vec![travel(at(a).0, at(a).1), cut(at(z).0, at(z).1, 40.0)]
            };
            let mut moves = line(0.0, true);
            moves.extend(line(1.0, false));
            let mut b = block(1000.0, moves.clone());
            apply(&mut b, &[so(1000.0, 0.3)], &[angle]);
            // La ida no cambia; la vuelta avanza 0.3 mm hacia el sentido del barrido.
            assert_moves(&b.moves[..2], &moves[..2]);
            for (after, before) in b.moves[2..].iter().zip(&moves[2..]) {
                let (pa, pb) = (after.to(), before.to());
                assert!((pa.x - pb.x - 0.3 * c).abs() < 1e-9, "{angle}");
                assert!((pa.y - pb.y - 0.3 * s).abs() < 1e-9, "{angle}");
            }
        }
    }

    #[test]
    fn negative_offset_moves_the_return_lines_back() {
        let mut b = block(1000.0, vec![travel(10.0, 0.0), cut(0.0, 0.0, 50.0)]);
        apply(&mut b, &[so(1000.0, -0.5)], &[0.0]);
        assert_moves(&b.moves, &[travel(9.5, 0.0), cut(-0.5, 0.0, 50.0)]);
    }

    #[test]
    fn crosshatch_uses_both_axes() {
        // Barrido a 0° (ida +X) y a 90° (ida +Y); una vuelta en cada uno.
        let mut b = block(
            1000.0,
            vec![
                travel(10.0, 0.0),
                cut(0.0, 0.0, 50.0),
                travel(0.0, 10.0),
                cut(0.0, 0.0, 50.0),
            ],
        );
        apply(&mut b, &[so(1000.0, 1.0)], &[0.0, 90.0]);
        assert_moves(
            &b.moves,
            &[
                travel(11.0, 0.0),
                cut(1.0, 0.0, 50.0),
                travel(0.0, 11.0),
                cut(0.0, 1.0, 50.0),
            ],
        );
    }

    #[test]
    fn nothing_to_do_leaves_the_block_untouched() {
        let moves = vec![travel(10.0, 0.0), cut(0.0, 0.0, 50.0)];
        let mut b = block(1000.0, moves.clone());
        apply(&mut b, &[], &[0.0]);
        apply(&mut b, &[so(1000.0, 0.0)], &[0.0]);
        apply(&mut b, &[so(1000.0, 0.5)], &[]);
        assert_eq!(b.moves, moves);
        // Una línea sin recorrido (punto) no tiene sentido y no se toca.
        let mut dot = block(1000.0, vec![travel(1.0, 1.0), cut(1.0, 1.0, 50.0)]);
        apply(&mut dot, &[so(1000.0, 0.5)], &[0.0]);
        assert_eq!(dot.moves[0].to(), Point::new(1.0, 1.0));
    }
}
