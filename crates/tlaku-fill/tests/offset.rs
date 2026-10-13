//! Relleno por contornos concéntricos: forma de los anillos, orden, huecos y robustez.

mod common;

use common::*;
use std::time::Instant;
use tlaku_core::toolpath::{Move, ToolpathBlock};
use tlaku_core::{Point, Polyline};
use tlaku_fill::{FillPattern, FillRule, FillSettings, fill};

fn base(interval: f64) -> FillSettings {
    FillSettings {
        interval_mm: interval,
        pattern: FillPattern::Offset,
        ..Default::default()
    }
}

/// Contornos del bloque: cada `Travel` abre uno nuevo y los `Cut` que siguen lo recorren.
fn rings(block: &ToolpathBlock) -> Vec<Vec<Point>> {
    let mut out: Vec<Vec<Point>> = Vec::new();
    for m in &block.moves {
        match *m {
            Move::Travel { to } => out.push(vec![to]),
            Move::Cut { to, .. } => out.last_mut().expect("Cut sin Travel previo").push(to),
            Move::Dwell { .. } => {}
        }
    }
    out
}

fn bbox(pts: &[Point]) -> (f64, f64, f64, f64) {
    pts.iter().fold(
        (f64::MAX, f64::MAX, f64::MIN, f64::MIN),
        |(x0, y0, x1, y1), p| (x0.min(p.x), y0.min(p.y), x1.max(p.x), y1.max(p.y)),
    )
}

fn assert_closed(r: &[Point]) {
    assert!(r.len() >= 4, "contorno con {} puntos", r.len());
    assert!(r[0].dist(*r.last().unwrap()) < 1e-9, "contorno sin cerrar");
}

#[test]
fn square_gives_nested_rings_from_outside_in() {
    let r = fill(&[rect(0.0, 0.0, 10.0, 10.0)], &base(1.0)).unwrap();
    assert!(r.ignored_open.is_empty());
    let rs = rings(&r.block);
    assert_eq!(rs.len(), 5, "offsets 0,5 / 1,5 / 2,5 / 3,5 / 4,5");
    for (k, ring) in rs.iter().enumerate() {
        assert_closed(ring);
        let (x0, y0, x1, y1) = bbox(ring);
        let inset = 0.5 + k as f64;
        for v in [x0, y0] {
            assert!((v - inset).abs() < 1e-6, "anillo {k}: {v} vs {inset}");
        }
        for v in [x1, y1] {
            assert!((v - (10.0 - inset)).abs() < 1e-6);
        }
    }
    // Todo es corte a la potencia pedida; sin overscan ni potencia 0.
    assert!(burns(&r.block).iter().all(|b| b.2 == 20.0));
    assert_eq!(idle_length(&r.block), 0.0);
    // El viaje entre anillos es de unos pocos intervalos.
    let mut pen = None::<Point>;
    for ring in &rs {
        if let Some(p) = pen {
            assert!(p.dist(ring[0]) < 1.5, "salto de {}", p.dist(ring[0]));
        }
        pen = Some(ring[0]);
    }
}

#[test]
fn ring_shape_does_not_fill_the_hole() {
    let polys = [rect(0.0, 0.0, 20.0, 20.0), rect(5.0, 5.0, 15.0, 15.0)];
    let r = fill(&polys, &base(1.0)).unwrap();
    let rs = rings(&r.block);
    assert!(rs.len() > 5);
    for ring in &rs {
        assert_closed(ring);
        for p in ring {
            assert!(
                is_inside(*p, &polys, false) || on_edge(*p),
                "punto en el hueco o fuera: {p:?}"
            );
        }
    }
    // Hay un contorno a medio intervalo del hueco y otro a medio intervalo del borde exterior.
    let has = |f: &dyn Fn(&[Point]) -> bool| rs.iter().any(|r| f(r));
    assert!(has(&|r| {
        let b = bbox(r);
        (b.0 - 4.5).abs() < 1e-6 && (b.2 - 15.5).abs() < 1e-6
    }));
    assert!(has(&|r| {
        let b = bbox(r);
        (b.0 - 0.5).abs() < 1e-6 && (b.2 - 19.5).abs() < 1e-6
    }));
    // Ningún tramo cruza el hueco: el punto medio de cada lado tampoco cae dentro.
    for ring in &rs {
        for w in ring.windows(2) {
            let mid = Point::new((w[0].x + w[1].x) / 2.0, (w[0].y + w[1].y) / 2.0);
            let strictly_in_hole = mid.x > 5.0 + 1e-6
                && mid.x < 15.0 - 1e-6
                && mid.y > 5.0 + 1e-6
                && mid.y < 15.0 - 1e-6;
            assert!(!strictly_in_hole);
        }
    }
}

fn on_edge(_p: Point) -> bool {
    // Los anillos nunca tocan el borde (están a medio intervalo), así que no se admite nada.
    false
}

#[test]
fn hole_is_a_single_chain_not_interleaved() {
    // Placa con un hueco pequeño: el recorrido no debe saltar de un anillo exterior al del
    // hueco en cada nivel.
    let polys = [rect(0.0, 0.0, 60.0, 60.0), rect(28.0, 28.0, 32.0, 32.0)];
    let r = fill(&polys, &base(1.0)).unwrap();
    let rs = rings(&r.block);
    let mut long_jumps = 0;
    for pair in rs.windows(2) {
        if pair[0][0].dist(pair[1][0]) > 5.0 {
            long_jumps += 1;
        }
    }
    assert!(long_jumps <= 6, "{long_jumps} saltos largos");
}

#[test]
fn two_separate_shapes_are_traced_one_after_another() {
    let polys = [rect(0.0, 0.0, 10.0, 10.0), rect(30.0, 0.0, 40.0, 10.0)];
    let r = fill(&polys, &base(1.0)).unwrap();
    let rs = rings(&r.block);
    assert_eq!(rs.len(), 10);
    let side: Vec<bool> = rs.iter().map(|r| r[0].x > 20.0).collect();
    assert_eq!(side.iter().filter(|&&s| s).count(), 5);
    let changes = side.windows(2).filter(|w| w[0] != w[1]).count();
    assert_eq!(changes, 1, "las formas no deben intercalarse");
}

#[test]
fn thin_shape_gets_a_center_line() {
    // 10 x 0,4 con intervalo 1: más angosta que el intervalo.
    let r = fill(&[rect(0.0, 0.0, 10.0, 0.4)], &base(1.0)).unwrap();
    let rs = rings(&r.block);
    assert_eq!(rs.len(), 1);
    assert_closed(&rs[0]);
    for p in &rs[0] {
        assert!(
            p.y >= -1e-9 && p.y <= 0.4 + 1e-9,
            "fuera de la forma: {p:?}"
        );
        assert!(
            (p.y - 0.2).abs() < 0.05,
            "no está en la línea central: {p:?}"
        );
    }
    let (x0, _, x1, _) = bbox(&rs[0]);
    assert!(x1 - x0 > 9.0, "la línea central debe recorrer la forma");
}

#[test]
fn leftover_core_gets_a_center_line() {
    // 3 x 3 con intervalo 1: un anillo a 0,5 (lado 2) y un núcleo de 1 x 1 sin cubrir.
    let r = fill(&[rect(0.0, 0.0, 3.0, 3.0)], &base(1.0)).unwrap();
    let rs = rings(&r.block);
    assert_eq!(rs.len(), 2);
    let (x0, y0, x1, y1) = bbox(&rs[0]);
    for (v, w) in [(x0, 0.5), (y0, 0.5), (x1, 2.5), (y1, 2.5)] {
        assert!((v - w).abs() < 1e-6);
    }
    // El segundo es el núcleo: un cuadrado diminuto o una línea alrededor del centro.
    for p in &rs[1] {
        assert!((p.x - 1.5).abs() < 0.6 && (p.y - 1.5).abs() < 0.6);
    }
}

#[test]
fn tiny_shapes_and_degenerates_do_not_fail() {
    let polys = [
        rect(0.0, 0.0, 0.01, 0.01),
        poly(&[(0.0, 0.0), (5.0, 0.0), (10.0, 0.0)]), // colineal
        poly(&[(1.0, 1.0), (1.0, 1.0), (1.0, 1.0)]),  // un punto
        poly(&[(0.0, 0.0), (f64::NAN, 1.0), (1.0, 1.0)]),
        Polyline {
            points: vec![Point::new(0.0, 0.0), Point::new(1.0, 1.0)],
            closed: false,
        },
    ];
    let r = fill(&polys, &base(0.1)).unwrap();
    assert_eq!(r.ignored_open, vec![4]);
    assert_eq!(rings(&r.block).len(), 1, "solo la mota de 0,01");
    assert!(fill(&[], &base(0.1)).unwrap().block.moves.is_empty());
}

#[test]
fn angle_bidirectional_crosshatch_and_overscan_are_ignored() {
    let shape = [circle(0.0, 0.0, 6.0, 64)];
    let a = fill(&shape, &base(0.5)).unwrap();
    let b = fill(
        &shape,
        &FillSettings {
            angle_deg: 37.0,
            bidirectional: false,
            crosshatch: true,
            overscan_mm: 2.0,
            ..base(0.5)
        },
    )
    .unwrap();
    assert_eq!(a.block.moves, b.block.moves);
}

#[test]
fn fill_rule_decides_overlaps() {
    // Dos cuadrados que se solapan, ambos antihorarios.
    let polys = [rect(0.0, 0.0, 10.0, 10.0), rect(5.0, 5.0, 15.0, 15.0)];
    let nz = fill(
        &polys,
        &FillSettings {
            fill_rule: FillRule::NonZero,
            ..base(1.0)
        },
    )
    .unwrap();
    let eo = fill(&polys, &base(1.0)).unwrap();
    let inside_overlap = |b: &ToolpathBlock| {
        burns(b)
            .iter()
            .any(|(_, p, _)| p.x > 5.2 && p.x < 9.8 && p.y > 5.2 && p.y < 9.8)
    };
    // Par-impar: el solape es un hueco y no se quema; no cero: la unión se rellena.
    assert!(!inside_overlap(&eo.block));
    assert!(inside_overlap(&nz.block));
    // Y todo quemado cae dentro de la regla elegida.
    for (_, p, _) in burns(&eo.block) {
        assert!(is_inside(p, &polys, false) || p.x.min(p.y) > 4.0);
    }
}

#[test]
fn settings_are_copied_to_block() {
    let s = FillSettings {
        speed_mm_min: 4321.0,
        air_assist: true,
        power_pct: 55.0,
        ..base(1.0)
    };
    let r = fill(&[rect(0.0, 0.0, 5.0, 5.0)], &s).unwrap();
    assert_eq!(r.block.speed_mm_min, 4321.0);
    assert!(r.block.air_assist);
    assert!(burns(&r.block).iter().all(|b| b.2 == 55.0));
}

#[test]
fn circle_coverage_is_close_to_area() {
    // Las pasadas de ancho `interval` deben cubrir casi toda el área del disco.
    let (r0, iv) = (10.0, 0.25);
    let r = fill(&[circle(0.0, 0.0, r0, 360)], &base(iv)).unwrap();
    let covered = burned_length(&r.block) * iv;
    let area = std::f64::consts::PI * r0 * r0;
    assert!(
        (covered / area - 1.0).abs() < 0.03,
        "cubre {covered:.2} de {area:.2}"
    );
}

#[test]
fn chained_levels_do_not_drift() {
    // 200 niveles sobre un círculo (el caso más sesgado para una simplificación entre niveles):
    // el radio de cada contorno debe seguir la progresión ideal.
    let (r0, iv) = (20.0, 0.1);
    let r = fill(&[circle(0.0, 0.0, r0, 720)], &base(iv)).unwrap();
    let rs = rings(&r.block);
    assert!(rs.len() >= 195, "{} contornos", rs.len());
    let mut worst: f64 = 0.0;
    for (k, ring) in rs.iter().enumerate().take(195) {
        let mean = ring.iter().map(|p| p.x.hypot(p.y)).sum::<f64>() / ring.len() as f64;
        let ideal = r0 - iv / 2.0 - iv * k as f64;
        worst = worst.max((mean - ideal).abs());
    }
    eprintln!("deriva máxima: {worst:.5} mm");
    assert!(worst < 0.01, "deriva de {worst} mm");
}

// --- robustez ---

struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> f64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }
}

/// Aproximación a una letra "O": dos círculos concéntricos (anillo).
fn letter(cx: f64, cy: f64, size: f64) -> [Polyline; 2] {
    [
        circle(cx, cy, size / 2.0, 24),
        circle(cx, cy, size / 4.0, 24),
    ]
}

fn text_grid(count: usize) -> Vec<Polyline> {
    let cols = 50;
    (0..count)
        .flat_map(|i| {
            let (c, r) = ((i % cols) as f64, (i / cols) as f64);
            letter(c * 8.0, r * 8.0, 6.0)
        })
        .collect()
}

fn run(polys: &[Polyline], interval: f64) -> (std::time::Duration, ToolpathBlock) {
    let t = Instant::now();
    let r = fill(polys, &base(interval)).unwrap();
    (t.elapsed(), r.block)
}

#[test]
fn medium_text_finishes_quickly() {
    let polys = text_grid(150);
    let (dt, block) = run(&polys, 0.25);
    eprintln!("150 letras: {dt:?}, {} movimientos", block.moves.len());
    assert!(dt.as_secs_f64() < 20.0);
    assert!(rings(&block).len() > 150 * 4);
}

#[test]
fn threaded_result_is_deterministic() {
    // 150 formas: se reparten entre hilos y el resultado no cambia de una corrida a otra.
    let polys = text_grid(150);
    let a = fill(&polys, &base(0.5)).unwrap();
    let b = fill(&polys, &base(0.5)).unwrap();
    assert_eq!(a.block.moves, b.block.moves);
}

#[test]
#[ignore = "rendimiento: tarda varios segundos en debug"]
fn two_thousand_letters_finish() {
    let polys = text_grid(2000);
    let (dt, block) = run(&polys, 0.1);
    eprintln!("2000 letras: {dt:?}, {} movimientos", block.moves.len());
    assert!(dt.as_secs_f64() < 60.0);
}

/// Contorno irregular de `n` puntos (radio con ruido de alta frecuencia).
fn noisy_blob(n: usize, radius: f64) -> Polyline {
    let mut rng = Lcg(7);
    Polyline {
        points: (0..n)
            .map(|i| {
                let a = std::f64::consts::TAU * i as f64 / n as f64;
                let r = radius * (1.0 + 0.1 * (5.0 * a).sin()) + 0.02 * (rng.next() - 0.5);
                Point::new(r * a.cos(), r * a.sin())
            })
            .collect(),
        closed: true,
    }
}

#[test]
fn contour_with_many_points_is_simplified_and_finishes() {
    let polys = [noisy_blob(20_000, 12.0)];
    let (dt, block) = run(&polys, 0.2);
    eprintln!("20 000 puntos: {dt:?}, {} movimientos", block.moves.len());
    assert!(dt.as_secs_f64() < 30.0);
    assert!(rings(&block).len() > 30);
}

#[test]
#[ignore = "rendimiento: tarda varios segundos en debug"]
fn contour_with_fifty_thousand_points_finishes() {
    let polys = [noisy_blob(50_000, 20.0)];
    let (dt, block) = run(&polys, 0.1);
    eprintln!("50 000 puntos: {dt:?}, {} movimientos", block.moves.len());
    assert!(dt.as_secs_f64() < 60.0);
}

#[test]
fn absurd_interval_fails_fast_instead_of_hanging() {
    let t = Instant::now();
    let e = fill(&[rect(0.0, 0.0, 1000.0, 1000.0)], &base(0.0005)).unwrap_err();
    assert!(e.contains("demasiad"), "{e}");
    assert!(t.elapsed().as_secs_f64() < 30.0);
}

#[test]
fn huge_interval_gives_one_small_contour_or_nothing() {
    let r = fill(&[rect(0.0, 0.0, 10.0, 10.0)], &base(500.0)).unwrap();
    assert!(rings(&r.block).len() <= 1);
}

#[test]
fn coordinates_out_of_range_are_skipped() {
    let polys = [rect(0.0, 0.0, 1e12, 1e12), rect(0.0, 0.0, 4.0, 4.0)];
    let r = fill(&polys, &base(1.0)).unwrap();
    assert_eq!(rings(&r.block).len(), 2);
}
