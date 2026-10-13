//! Corrección del desfase entre las líneas de ida y de vuelta de un barrido bidireccional.
//!
//! En un barrido bidireccional el láser tarda un poco en responder, así que las líneas que se
//! recorren al revés quedan corridas respecto de las de ida. El perfil de la máquina guarda la
//! separación medida entre ambas a varias velocidades (`DeviceProfile.scan_offsets`); aquí se
//! corrige como posproceso del bloque ya generado, sin que el relleno ni el rasterizador lo
//! sepan.
//!
//! - Una "línea" es un `Travel` más los movimientos que le siguen hasta el siguiente `Travel`
//!   (incluido el overscan, que se desplaza junto con ella).
//! - Una línea es de vuelta si su sentido va contra el de barrido. El sentido de barrido sale
//!   del ángulo de la capa (0° = +X, antihorario); con crosshatch hay un segundo barrido a 90°
//!   y cada línea se compara con el eje con el que más se alinea. Así no depende de cómo se
//!   reparten las líneas entre formas ni de cuántas hay.
//! - La separación medida `offset` se **reparte**: las líneas de ida se corren `-offset/2` y las
//!   de vuelta `+offset/2` a lo largo del sentido de barrido (el de las líneas de ida). Antes
//!   solo se movían las de vuelta el valor completo y todo el grabado quedaba corrido la mitad
//!   respecto de los cortes vectoriales; repartido, el centro de cada par ida/vuelta coincide
//!   con el vector. (LightBurn pide anotar la mitad de lo medido porque desplaza cada línea
//!   ese valor; el resultado es el mismo.) Un `offset` positivo separa las de vuelta hacia
//!   delante; uno negativo, hacia atrás.
//! - `DeviceProfile.scan_initial_offset` mueve **todas** las líneas (también las de un barrido
//!   unidireccional) ese valor en el sentido de barrido, para alinear el grabado con los cortes.
//! - El offset de un bloque sale de interpolar linealmente por su velocidad; fuera del rango
//!   medido se usa el extremo más cercano.
//! - Las líneas corridas necesitan overscan para entrar ya a velocidad: ver [`overscan_warning`].

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
        Move::Travel { to } | Move::Cut { to, .. } | Move::Dwell { to, .. } => {
            *to = Point::new(to.x + dx, to.y + dy)
        }
    }
}

/// Aviso si el overscan es menor que la separación `offset` (en valor absoluto) a esa velocidad:
/// la línea corrida entra al grabado sin haber llegado a velocidad. Solo importa con barrido
/// bidireccional; el llamador lo comprueba.
pub(crate) fn overscan_warning(
    layer: &str,
    offsets: &[ScanOffset],
    speed_mm_min: f64,
    overscan_mm: f64,
) -> Option<String> {
    let offset = offset_for_speed(offsets, speed_mm_min).abs();
    (offset > 0.0 && overscan_mm + 1e-9 < offset).then(|| {
        format!(
            "{layer}: el overscan ({overscan_mm:.2} mm) es menor que la corrección de barrido \
             ({offset:.2} mm a {speed_mm_min} mm/min): las líneas corridas entran sin llegar a \
             velocidad; sube el overscan"
        )
    })
}

/// Corrige las líneas de `block` según `offsets` (reparto ±offset/2 si el barrido es
/// `bidirectional`) y `initial_mm` (todas las líneas). `sweep_angles_deg` son los ángulos de
/// barrido de la capa (uno, o dos con crosshatch). No hace nada sin muestras ni offset inicial.
pub(crate) fn apply(
    block: &mut ToolpathBlock,
    offsets: &[ScanOffset],
    initial_mm: f64,
    sweep_angles_deg: &[f64],
    bidirectional: bool,
) {
    let offset = if bidirectional {
        offset_for_speed(offsets, block.speed_mm_min)
    } else {
        0.0
    };
    let initial = if initial_mm.is_finite() {
        initial_mm
    } else {
        0.0
    };
    if (offset == 0.0 && initial == 0.0) || sweep_angles_deg.is_empty() {
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
        if let Some((mut ax, mut ay, dot)) = best {
            // Una línea sin recorrido no tiene sentido: solo recibe el offset inicial, a lo
            // largo del primer barrido.
            if dot == 0.0 {
                (ax, ay) = axes[0];
            }
            let along = if dot < 0.0 {
                initial + offset / 2.0
            } else if dot > 0.0 {
                initial - offset / 2.0
            } else {
                initial
            };
            if along != 0.0 {
                let (dx, dy) = (ax * along, ay * along);
                for mv in &mut moves[i..end] {
                    shift(mv, dx, dy);
                }
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
    fn the_offset_is_split_half_to_each_direction() {
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
        apply(&mut b, &[so(3000.0, 0.2)], 0.0, &[0.0], true);
        assert_moves(
            &b.moves,
            &[
                travel(-1.1, 0.0),
                cut(-0.1, 0.0, 0.0),
                cut(9.9, 0.0, 50.0),
                cut(10.9, 0.0, 0.0),
                travel(11.1, 1.0),
                cut(10.1, 1.0, 0.0),
                cut(0.1, 1.0, 50.0),
                cut(-0.9, 1.0, 0.0),
                travel(-1.1, 2.0),
                cut(9.9, 2.0, 50.0),
            ],
        );
    }

    #[test]
    fn the_centre_of_each_pair_stays_on_the_nominal_line_and_they_end_offset_apart() {
        // La separación entre los extremos de ida y de vuelta es el offset completo.
        let mut b = block(
            1000.0,
            vec![
                travel(0.0, 0.0),
                cut(10.0, 0.0, 50.0),
                travel(10.0, 1.0),
                cut(0.0, 1.0, 50.0),
            ],
        );
        apply(&mut b, &[so(1000.0, 0.3)], 0.0, &[0.0], true);
        let end_forward = b.moves[1].to().x;
        let end_return = b.moves[3].to().x;
        assert!((end_return - 0.0 - (end_forward - 10.0) - 0.3).abs() < 1e-12);
        assert!(((end_forward - 10.0) + (end_return - 0.0)).abs() < 1e-12);
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
            apply(&mut b, &[so(1000.0, 0.3)], 0.0, &[angle], true);
            // La ida retrocede 0.15 mm y la vuelta avanza 0.15 mm a lo largo del barrido.
            for (i, (after, before)) in b.moves.iter().zip(&moves).enumerate() {
                let k = if i < 2 { -0.15 } else { 0.15 };
                let (pa, pb) = (after.to(), before.to());
                assert!((pa.x - pb.x - k * c).abs() < 1e-9, "{angle}");
                assert!((pa.y - pb.y - k * s).abs() < 1e-9, "{angle}");
            }
        }
    }

    #[test]
    fn negative_offset_swaps_the_directions() {
        let mut b = block(1000.0, vec![travel(10.0, 0.0), cut(0.0, 0.0, 50.0)]);
        apply(&mut b, &[so(1000.0, -0.5)], 0.0, &[0.0], true);
        assert_moves(&b.moves, &[travel(9.75, 0.0), cut(-0.25, 0.0, 50.0)]);
    }

    #[test]
    fn initial_offset_moves_every_line_and_also_unidirectional_sweeps() {
        let lines = vec![
            travel(0.0, 0.0),
            cut(10.0, 0.0, 50.0),
            travel(10.0, 1.0),
            cut(0.0, 1.0, 50.0),
        ];
        // Con offset de tabla 0.2 (±0.1) y offset inicial 0.5: ida +0.4, vuelta +0.6.
        let mut b = block(1000.0, lines.clone());
        apply(&mut b, &[so(1000.0, 0.2)], 0.5, &[0.0], true);
        assert_moves(
            &b.moves,
            &[
                travel(0.4, 0.0),
                cut(10.4, 0.0, 50.0),
                travel(10.6, 1.0),
                cut(0.6, 1.0, 50.0),
            ],
        );
        // Sin bidireccional solo cuenta el offset inicial, aunque haya tabla.
        let mut uni = block(1000.0, lines);
        apply(&mut uni, &[so(1000.0, 0.2)], 0.5, &[0.0], false);
        assert_moves(
            &uni.moves,
            &[
                travel(0.5, 0.0),
                cut(10.5, 0.0, 50.0),
                travel(10.5, 1.0),
                cut(0.5, 1.0, 50.0),
            ],
        );
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
        apply(&mut b, &[so(1000.0, 2.0)], 0.0, &[0.0, 90.0], true);
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
        apply(&mut b, &[], 0.0, &[0.0], true);
        apply(&mut b, &[so(1000.0, 0.0)], 0.0, &[0.0], true);
        apply(&mut b, &[so(1000.0, 0.5)], 0.0, &[], true);
        apply(&mut b, &[so(1000.0, 0.5)], 0.0, &[0.0], false);
        apply(&mut b, &[], f64::NAN, &[0.0], true);
        assert_eq!(b.moves, moves);
        // Una línea sin recorrido no tiene sentido: no recibe el reparto, solo el inicial.
        let mut dot = block(1000.0, vec![travel(1.0, 1.0), cut(1.0, 1.0, 50.0)]);
        apply(&mut dot, &[so(1000.0, 0.5)], 0.0, &[0.0], true);
        assert_eq!(dot.moves[0].to(), Point::new(1.0, 1.0));
        apply(&mut dot, &[], 0.25, &[0.0], true);
        assert_eq!(dot.moves[0].to(), Point::new(1.25, 1.0));
    }

    #[test]
    fn warns_when_the_overscan_is_shorter_than_the_offset() {
        let t = [so(1000.0, 0.4)];
        let w = overscan_warning("capa #ff0000", &t, 1000.0, 0.2).unwrap();
        assert!(
            w.contains("overscan") && w.contains("0.20") && w.contains("0.40"),
            "{w}"
        );
        assert_eq!(overscan_warning("c", &t, 1000.0, 0.4), None);
        assert_eq!(overscan_warning("c", &t, 1000.0, 2.0), None);
        assert_eq!(overscan_warning("c", &[], 1000.0, 0.0), None);
        // El signo no importa.
        assert!(overscan_warning("c", &[so(1000.0, -0.4)], 1000.0, 0.0).is_some());
    }
}
