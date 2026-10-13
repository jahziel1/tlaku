//! Relleno por regiones (flood) y agrupación del relleno: cobertura, viajes y orden.

mod common;

use common::*;
use tlaku_core::toolpath::{Move, ToolpathBlock};
use tlaku_core::{Point, Polyline};
use tlaku_fill::{FillPattern, FillSettings, Grouping, fill, fill_with_groups};

fn flood_settings(interval: f64) -> FillSettings {
    FillSettings {
        interval_mm: interval,
        pattern: FillPattern::Flood,
        ..Default::default()
    }
}

fn scan_settings(interval: f64) -> FillSettings {
    FillSettings {
        pattern: FillPattern::Scan,
        ..flood_settings(interval)
    }
}

/// Longitud sin quemar: viajes y `Cut` a potencia 0. Se ignora el primer viaje desde el origen.
fn idle_travel(block: &ToolpathBlock) -> f64 {
    let mut pos: Option<Point> = None;
    let mut total = 0.0;
    for m in &block.moves {
        let idle = match *m {
            Move::Travel { .. } => true,
            Move::Cut { power_pct, .. } => power_pct == 0.0,
            _ => false,
        };
        if idle && let Some(p) = pos {
            total += p.dist(m.to());
        }
        pos = Some(m.to());
    }
    total
}

/// Tramos quemados como (y, x_min, x_max) redondeados a 1e-6, ordenados.
fn burn_set(block: &ToolpathBlock) -> Vec<(i64, i64, i64)> {
    let q = |v: f64| (v * 1e6).round() as i64;
    let mut v: Vec<_> = burns(block)
        .iter()
        .map(|(a, b, _)| (q(a.y), q(a.x.min(b.x)), q(a.x.max(b.x))))
        .collect();
    v.sort_unstable();
    v
}

fn at(poly: &Polyline, dx: f64, dy: f64) -> Polyline {
    Polyline {
        points: poly
            .points
            .iter()
            .map(|p| Point::new(p.x + dx, p.y + dy))
            .collect(),
        closed: true,
    }
}

/// Letras de 4 x 6 mm: O (con hueco), H, U e I.
fn glyph(kind: usize) -> Vec<Polyline> {
    match kind % 4 {
        0 => vec![rect(0.0, 0.0, 4.0, 6.0), rect(1.0, 1.0, 3.0, 5.0)],
        1 => vec![poly(&[
            (0.0, 0.0),
            (1.0, 0.0),
            (1.0, 2.5),
            (3.0, 2.5),
            (3.0, 0.0),
            (4.0, 0.0),
            (4.0, 6.0),
            (3.0, 6.0),
            (3.0, 3.5),
            (1.0, 3.5),
            (1.0, 6.0),
            (0.0, 6.0),
        ])],
        2 => vec![poly(&[
            (0.0, 0.0),
            (4.0, 0.0),
            (4.0, 6.0),
            (3.0, 6.0),
            (3.0, 1.0),
            (1.0, 1.0),
            (1.0, 6.0),
            (0.0, 6.0),
        ])],
        _ => vec![rect(1.5, 0.0, 2.5, 6.0)],
    }
}

/// `rows` líneas de `cols` letras, separadas 6 mm en x y 9 mm en y.
fn text(rows: usize, cols: usize) -> Vec<Polyline> {
    let mut out = Vec::new();
    for r in 0..rows {
        for c in 0..cols {
            for g in glyph(r * 3 + c) {
                out.push(at(&g, c as f64 * 6.0, r as f64 * 9.0));
            }
        }
    }
    out
}

#[test]
fn flood_covers_exactly_what_scan_covers() {
    let paths = text(3, 8);
    let a = fill(&paths, &scan_settings(0.2)).unwrap();
    let b = fill(&paths, &flood_settings(0.2)).unwrap();
    assert_eq!(burn_set(&a.block), burn_set(&b.block));
}

#[test]
fn flood_covers_the_same_with_angle_crosshatch_and_nonzero() {
    let paths = text(2, 5);
    for (angle, crosshatch, bidirectional) in
        [(30.0, false, true), (90.0, true, true), (0.0, true, false)]
    {
        let s = FillSettings {
            angle_deg: angle,
            crosshatch,
            bidirectional,
            ..scan_settings(0.25)
        };
        let f = FillSettings {
            pattern: FillPattern::Flood,
            ..s.clone()
        };
        let (a, b) = (fill(&paths, &s).unwrap(), fill(&paths, &f).unwrap());
        let (la, lb) = (burned_length(&a.block), burned_length(&b.block));
        assert!((la - lb).abs() < 1e-6 * la, "{angle}: {la} frente a {lb}");
        assert_eq!(burns(&a.block).len(), burns(&b.block).len());
    }
}

#[test]
fn flood_never_crosses_a_hole() {
    // Con overscan 0 todo `Cut` a potencia 0 sería un cruce de hueco o un salto entre islas.
    let paths = text(2, 6);
    let r = fill(&paths, &flood_settings(0.2)).unwrap();
    assert_eq!(idle_length(&r.block), 0.0);
    let s = fill(&paths, &scan_settings(0.2)).unwrap();
    assert!(idle_length(&s.block) > 100.0);
}

#[test]
fn flood_text_travels_far_less_than_scan() {
    let paths = text(4, 20);
    let scan = fill(&paths, &scan_settings(0.2)).unwrap();
    let flood = fill(&paths, &flood_settings(0.2)).unwrap();
    let (ts, tf) = (idle_travel(&scan.block), idle_travel(&flood.block));
    eprintln!(
        "texto de {} formas: viaje Scan = {ts:.1} mm, Flood = {tf:.1} mm (Flood/Scan = {:.3})",
        paths.len(),
        tf / ts
    );
    assert!(tf < ts * 0.25, "Flood {tf} no mejora a Scan {ts}");
    // Mismo grabado.
    assert_eq!(burn_set(&scan.block), burn_set(&flood.block));
}

#[test]
fn rectangle_is_one_continuous_serpentine() {
    let r = fill(&[rect(0.0, 0.0, 10.0, 10.0)], &flood_settings(1.0)).unwrap();
    let b = burns(&r.block);
    assert_eq!(b.len(), 10);
    // Líneas consecutivas, sentidos alternos y solo el salto de una línea entre ellas.
    for (i, w) in b.windows(2).enumerate() {
        assert!((w[1].0.y - w[0].0.y - 1.0).abs() < 1e-9, "línea {i}");
        assert!((w[0].1.x - w[0].0.x) * (w[1].1.x - w[1].0.x) < 0.0);
    }
    assert!((idle_travel(&r.block) - 9.0).abs() < 1e-9);
}

#[test]
fn ring_and_u_are_traversed_without_long_jumps() {
    // Un anillo de 40 x 40 con borde de 2: el barrido global cruza el hueco en cada línea.
    let ring = vec![rect(0.0, 0.0, 40.0, 40.0), rect(2.0, 2.0, 38.0, 38.0)];
    let u = vec![poly(&[
        (0.0, 0.0),
        (40.0, 0.0),
        (40.0, 40.0),
        (38.0, 40.0),
        (38.0, 2.0),
        (2.0, 2.0),
        (2.0, 40.0),
        (0.0, 40.0),
    ])];
    for shape in [ring, u] {
        let s = fill(&shape, &scan_settings(0.5)).unwrap();
        let f = fill(&shape, &flood_settings(0.5)).unwrap();
        assert_eq!(burn_set(&s.block), burn_set(&f.block));
        assert_eq!(idle_length(&f.block), 0.0);
        assert!(idle_travel(&f.block) < idle_travel(&s.block));
        // Ningún viaje individual es más largo que el lado de la pieza.
        let mut pos = f.block.moves[0].to();
        for m in &f.block.moves[1..] {
            if matches!(m, Move::Travel { .. }) {
                assert!(pos.dist(m.to()) < 40.0, "salto largo {}", pos.dist(m.to()));
            }
            pos = m.to();
        }
    }
}

#[test]
fn flood_overscan_wraps_every_span() {
    let s = FillSettings {
        overscan_mm: 2.0,
        ..flood_settings(1.0)
    };
    let r = fill(&[rect(0.0, 0.0, 10.0, 4.0)], &s).unwrap();
    let m = &r.block.moves;
    // Travel, Cut0, Cut(quema), Cut0 por línea.
    assert_eq!(m.len(), 16);
    assert_eq!(m[0].to(), Point::new(-2.0, 0.5));
    assert_eq!(m[1].to(), Point::new(0.0, 0.5));
    assert_eq!(m[2].to(), Point::new(10.0, 0.5));
    assert_eq!(m[3].to(), Point::new(12.0, 0.5));
    // La segunda línea entra por el lado donde terminó la primera.
    assert_eq!(m[4].to(), Point::new(12.0, 1.5));
}

#[test]
fn flood_unidirectional_burns_left_to_right() {
    let s = FillSettings {
        bidirectional: false,
        ..flood_settings(0.5)
    };
    let r = fill(&text(1, 4), &s).unwrap();
    assert!(burns(&r.block).iter().all(|(a, b, _)| b.x > a.x));
}

#[test]
fn flood_handles_empty_open_and_degenerate_input() {
    let s = flood_settings(0.5);
    assert!(fill(&[], &s).unwrap().block.moves.is_empty());
    let open = Polyline {
        points: vec![
            Point::new(0.0, 0.0),
            Point::new(5.0, 0.0),
            Point::new(5.0, 5.0),
        ],
        closed: false,
    };
    let r = fill(&[open], &s).unwrap();
    assert_eq!(r.ignored_open, vec![0]);
    assert!(r.block.moves.is_empty());
    // Una línea horizontal degenerada no aporta área.
    let flat = poly(&[(0.0, 1.0), (5.0, 1.0), (2.0, 1.0)]);
    assert!(fill(&[flat], &s).unwrap().block.moves.is_empty());
}

#[test]
fn flood_with_many_islands_is_fast() {
    // 90 x 90 puntos de 0,5 mm (8100 islas) a 0,1 mm.
    let mut paths = Vec::new();
    for i in 0..90 {
        for j in 0..90 {
            paths.push(rect(
                i as f64 * 1.0,
                j as f64 * 1.0,
                i as f64 + 0.5,
                j as f64 + 0.5,
            ));
        }
    }
    let t = std::time::Instant::now();
    let r = fill(&paths, &flood_settings(0.1)).unwrap();
    let elapsed = t.elapsed();
    eprintln!("{} islas: {elapsed:?}", paths.len());
    assert_eq!(burns(&r.block).len(), 8100 * 5);
    assert!(elapsed.as_secs_f64() < 5.0, "tardó {elapsed:?}");
}

// --- Agrupación del relleno ---

/// Índice de la primera forma de `shapes` que contiene el punto.
fn owner(p: Point, shapes: &[Vec<Polyline>]) -> usize {
    shapes
        .iter()
        .position(|s| winding(p, s) != 0)
        .expect("tramo fuera de todas las formas")
}

fn owners_in_order(block: &ToolpathBlock, shapes: &[Vec<Polyline>]) -> Vec<usize> {
    let mut seq: Vec<usize> = burns(block)
        .iter()
        .map(|(a, b, _)| owner(Point::new((a.x + b.x) / 2.0, a.y), shapes))
        .collect();
    seq.dedup();
    seq
}

#[test]
fn per_shape_fills_each_shape_completely_before_the_next() {
    // Dos triángulos encajados: sus cajas se montan, pero no se tocan.
    let a = poly(&[(0.0, 0.0), (10.0, 0.0), (0.0, 10.0)]);
    let b = poly(&[(11.0, 1.0), (11.0, 11.0), (1.0, 11.0)]);
    let shapes = vec![vec![a.clone()], vec![b.clone()]];
    let paths = vec![a, b];

    for pattern in [FillPattern::Scan, FillPattern::Flood] {
        let all = FillSettings {
            pattern,
            grouping: Grouping::All,
            ..scan_settings(0.5)
        };
        let per = FillSettings {
            grouping: Grouping::PerShape,
            ..all.clone()
        };
        let r_all = fill(&paths, &all).unwrap();
        let r_per = fill(&paths, &per).unwrap();
        if pattern == FillPattern::Scan {
            // El barrido global alterna entre las dos formas en cada línea.
            assert!(owners_in_order(&r_all.block, &shapes).len() > 2);
        }
        assert_eq!(
            owners_in_order(&r_per.block, &shapes),
            vec![0, 1],
            "{pattern:?}"
        );
        assert_eq!(burn_set(&r_all.block), burn_set(&r_per.block));
    }
}

#[test]
fn per_shape_keeps_holes_and_nested_islands_with_their_shape() {
    // Marco con hueco, una isla dentro del hueco y, aparte, otro cuadrado dentro de la caja.
    let frame = vec![
        rect(0.0, 0.0, 20.0, 20.0),
        rect(2.0, 2.0, 18.0, 18.0),
        rect(8.0, 8.0, 12.0, 12.0),
    ];
    let other = vec![rect(30.0, 5.0, 35.0, 10.0)];
    let paths: Vec<Polyline> = frame.iter().chain(&other).cloned().collect();
    let shapes = vec![frame, other];
    let all = fill(&paths, &scan_settings(0.5)).unwrap();
    let per = fill(
        &paths,
        &FillSettings {
            grouping: Grouping::PerShape,
            ..scan_settings(0.5)
        },
    )
    .unwrap();
    assert_eq!(owners_in_order(&per.block, &shapes), vec![0, 1]);
    // El hueco sigue vacío y la isla se rellena: igual que sin agrupar.
    assert_eq!(burn_set(&all.block), burn_set(&per.block));
}

#[test]
fn by_group_sweeps_each_group_together_in_first_appearance_order() {
    // Tres cuadrados en fila; los grupos son {0, 2} y {1}.
    let sq: Vec<Polyline> = (0..3)
        .map(|i| rect(i as f64 * 10.0, 0.0, i as f64 * 10.0 + 4.0, 4.0))
        .collect();
    let shapes: Vec<Vec<Polyline>> = sq.iter().map(|p| vec![p.clone()]).collect();
    let s = FillSettings {
        grouping: Grouping::ByGroup,
        ..scan_settings(1.0)
    };
    let r = fill_with_groups(&sq, Some(&[7, 3, 7]), &s).unwrap();
    // Grupo 7 (cuadrados 0 y 2, juntos en cada línea) y luego el grupo 3 (cuadrado 1).
    let order = owners_in_order(&r.block, &shapes);
    assert_eq!(order.last(), Some(&1));
    assert!(!order[..order.len() - 1].contains(&1));
    // Con ids [3, 7, 7] el cuadrado 0 va solo y primero; los otros dos comparten líneas.
    let r = fill_with_groups(&sq, Some(&[3, 7, 7]), &s).unwrap();
    let order = owners_in_order(&r.block, &shapes);
    assert_eq!(order[0], 0);
    assert!(!order[1..].contains(&0));
    assert_eq!(
        burn_set(&r.block),
        burn_set(&fill(&sq, &scan_settings(1.0)).unwrap().block)
    );
}

#[test]
fn by_group_without_ids_falls_back_to_per_shape_and_checks_length() {
    let sq = vec![rect(0.0, 0.0, 4.0, 4.0), rect(10.0, 0.0, 14.0, 4.0)];
    let s = FillSettings {
        grouping: Grouping::ByGroup,
        ..scan_settings(1.0)
    };
    let per = FillSettings {
        grouping: Grouping::PerShape,
        ..s.clone()
    };
    assert_eq!(fill(&sq, &s).unwrap(), fill(&sq, &per).unwrap());
    assert!(
        fill_with_groups(&sq, Some(&[1]), &s)
            .unwrap_err()
            .contains("id de grupo")
    );
}

#[test]
fn group_ids_index_the_original_paths_even_with_open_ones() {
    let open = Polyline {
        points: vec![Point::new(0.0, 0.0), Point::new(1.0, 1.0)],
        closed: false,
    };
    let paths = vec![rect(0.0, 0.0, 4.0, 4.0), open, rect(10.0, 0.0, 14.0, 4.0)];
    let s = FillSettings {
        grouping: Grouping::ByGroup,
        ..scan_settings(1.0)
    };
    let r = fill_with_groups(&paths, Some(&[1, 9, 1]), &s).unwrap();
    assert_eq!(r.ignored_open, vec![1]);
    // Los dos cuadrados comparten grupo: cada línea los cruza a ambos.
    assert_eq!(burns(&r.block).len(), 8);
    assert_eq!(
        r.block
            .moves
            .iter()
            .filter(|m| matches!(m, Move::Travel { .. }))
            .count(),
        4
    );
}
