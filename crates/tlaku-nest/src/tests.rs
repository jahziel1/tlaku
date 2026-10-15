use super::*;
use crate::geom::{Pt, dist_point_segment};
use std::sync::atomic::AtomicBool;
use tlaku_core::Point;

fn poly(pts: &[Pt]) -> Polyline {
    Polyline {
        points: pts.iter().map(|p| Point::new(p[0], p[1])).collect(),
        closed: true,
    }
}

fn rect(w: f64, h: f64) -> Vec<Polyline> {
    vec![poly(&[[0.0, 0.0], [w, 0.0], [w, h], [0.0, h]])]
}

/// Ele de tres celdas de `c` mm.
fn ele(c: f64) -> Vec<Polyline> {
    vec![poly(&[
        [0.0, 0.0],
        [2.0 * c, 0.0],
        [2.0 * c, c],
        [c, c],
        [c, 2.0 * c],
        [0.0, 2.0 * c],
    ])]
}

fn circle(r: f64, n: usize) -> Polyline {
    poly(
        &(0..n)
            .map(|i| {
                let a = i as f64 * std::f64::consts::TAU / n as f64;
                [r * a.cos(), r * a.sin()]
            })
            .collect::<Vec<_>>(),
    )
}

fn input(
    pieces: Vec<(Vec<Polyline>, u32)>,
    container: NestContainer,
    options: NestOptions,
) -> NestInput {
    NestInput {
        pieces: pieces
            .into_iter()
            .map(|(region, copies)| NestPiece {
                region,
                copies,
                rotation: PieceRotation::Inherit,
            })
            .collect(),
        container,
        options,
    }
}

fn quick() -> NestOptions {
    NestOptions {
        time_limit: Duration::from_secs(20),
        max_evaluations: Some(8),
        ..NestOptions::default()
    }
}

fn run(inp: &NestInput) -> Outcome {
    nest(inp, &AtomicBool::new(false), &mut |_| {}).expect("la petición es válida")
}

/// Contornos de una copia colocada, en coordenadas finales.
fn placed_paths(inp: &NestInput, pl: &Placement) -> Vec<Vec<Pt>> {
    let t = pl.transform;
    inp.pieces[pl.piece]
        .region
        .iter()
        .map(|p| {
            p.points
                .iter()
                .map(|q| {
                    [
                        t[0] * q.x + t[2] * q.y + t[4],
                        t[1] * q.x + t[3] * q.y + t[5],
                    ]
                })
                .collect()
        })
        .collect()
}

fn seg_intersect(a: Pt, b: Pt, c: Pt, d: Pt) -> bool {
    let o = |p: Pt, q: Pt, r: Pt| (q[0] - p[0]) * (r[1] - p[1]) - (q[1] - p[1]) * (r[0] - p[0]);
    o(a, b, c) * o(a, b, d) < 0.0 && o(c, d, a) * o(c, d, b) < 0.0
}

fn inside(p: Pt, ring: &[Pt]) -> bool {
    let n = ring.len();
    let mut ins = false;
    for i in 0..n {
        let (a, b) = (ring[i], ring[(i + 1) % n]);
        if (a[1] > p[1]) != (b[1] > p[1])
            && p[0] < a[0] + (p[1] - a[1]) / (b[1] - a[1]) * (b[0] - a[0])
        {
            ins = !ins;
        }
    }
    ins
}

/// Distancia entre dos regiones (contornos exteriores): 0 si se solapan.
fn distance(a: &[Vec<Pt>], b: &[Vec<Pt>]) -> f64 {
    let mut best = f64::INFINITY;
    for ra in a {
        for rb in b {
            let (na, nb) = (ra.len(), rb.len());
            for i in 0..na {
                let (p, q) = (ra[i], ra[(i + 1) % na]);
                for j in 0..nb {
                    let (r, s) = (rb[j], rb[(j + 1) % nb]);
                    if seg_intersect(p, q, r, s) {
                        return 0.0;
                    }
                    best = best
                        .min(dist_point_segment(p, r, s))
                        .min(dist_point_segment(q, r, s))
                        .min(dist_point_segment(r, p, q))
                        .min(dist_point_segment(s, p, q));
                }
            }
            if inside(ra[0], rb) || inside(rb[0], ra) {
                return 0.0;
            }
        }
    }
    best
}

/// Distancia mínima entre cualquier par de piezas colocadas.
fn min_pair_distance(inp: &NestInput, out: &Outcome) -> f64 {
    let paths: Vec<_> = out
        .placements
        .iter()
        .map(|p| placed_paths(inp, p))
        .collect();
    let mut best = f64::INFINITY;
    for i in 0..paths.len() {
        for j in i + 1..paths.len() {
            best = best.min(distance(&paths[i], &paths[j]));
        }
    }
    best
}

fn bbox_of(paths: &[Vec<Pt>]) -> (Pt, Pt) {
    crate::geom::bounds(paths).unwrap()
}

#[test]
fn cuatro_rectangulos_caben_exactos() {
    let inp = input(
        vec![(rect(50.0, 50.0), 4)],
        NestContainer::Rect {
            width: 100.0,
            height: 100.0,
        },
        quick(),
    );
    let out = run(&inp);
    assert!(out.unplaced.is_empty(), "no cupieron {:?}", out.unplaced);
    assert_eq!(out.placements.len(), 4);
    assert!(
        (out.utilization - 100.0).abs() < 1e-6,
        "{}",
        out.utilization
    );
    // Dentro de la cama (con la tolerancia numérica) y sin solaparse.
    for pl in &out.placements {
        let (lo, hi) = bbox_of(&placed_paths(&inp, pl));
        assert!(
            lo[0] > -0.01 && lo[1] > -0.01 && hi[0] < 100.01 && hi[1] < 100.01,
            "{lo:?} {hi:?}"
        );
    }
    assert!(min_pair_distance(&inp, &out) < 0.01); // se tocan, pero ...
    // ... sin solaparse: el área común es cero (las cajas no se cruzan más que la tolerancia).
    for i in 0..4 {
        for j in i + 1..4 {
            let (a0, a1) = bbox_of(&placed_paths(&inp, &out.placements[i]));
            let (b0, b1) = bbox_of(&placed_paths(&inp, &out.placements[j]));
            let ox = (a1[0].min(b1[0]) - a0[0].max(b0[0])).max(0.0);
            let oy = (a1[1].min(b1[1]) - a0[1].max(b0[1])).max(0.0);
            assert!(ox * oy < 0.5, "solape {ox} x {oy}");
        }
    }
}

#[test]
fn piezas_en_ele_se_acoplan() {
    // Dos eles de tres celdas llenan exactamente 2 x 3 celdas, pero solo si una se gira 180°.
    let inp = input(
        vec![(ele(10.0), 2)],
        NestContainer::Rect {
            width: 30.0,
            height: 20.0,
        },
        NestOptions {
            max_evaluations: Some(60),
            ..quick()
        },
    );
    let out = run(&inp);
    assert!(out.unplaced.is_empty(), "no cupieron {:?}", out.unplaced);
    assert!(
        (out.utilization - 100.0).abs() < 1e-6,
        "{}",
        out.utilization
    );
}

#[test]
fn una_pieza_que_no_cabe_se_reporta() {
    let inp = input(
        vec![(rect(30.0, 30.0), 1), (rect(500.0, 10.0), 1)],
        NestContainer::Rect {
            width: 100.0,
            height: 100.0,
        },
        quick(),
    );
    let out = run(&inp);
    assert_eq!(out.placements.len(), 1);
    assert_eq!(out.unplaced, vec![(1, 0)]);
    assert!((out.utilization - 9.0).abs() < 1e-6);
}

#[test]
fn la_separacion_se_respeta_con_distancias() {
    let mut pieces = vec![(ele(10.0), 3), (rect(25.0, 15.0), 3)];
    pieces.push((vec![circle(8.0, 60)], 3));
    let inp = input(
        pieces,
        NestContainer::Rect {
            width: 120.0,
            height: 100.0,
        },
        NestOptions {
            spacing: 3.0,
            margin: 5.0,
            max_evaluations: Some(20),
            rotations: Rotations::Step(30.0),
            ..quick()
        },
    );
    let out = run(&inp);
    assert!(out.unplaced.is_empty(), "no cupieron {:?}", out.unplaced);
    let d = min_pair_distance(&inp, &out);
    assert!(d >= 3.0 - 0.01, "distancia mínima {d}");
    for pl in &out.placements {
        let (lo, hi) = bbox_of(&placed_paths(&inp, pl));
        assert!(lo[0] >= 5.0 - 0.01 && lo[1] >= 5.0 - 0.01, "{lo:?}");
        assert!(hi[0] <= 115.0 + 0.01 && hi[1] <= 95.0 + 0.01, "{hi:?}");
    }
}

#[test]
fn las_rotaciones_permiten_lo_que_sin_ellas_no_cabe() {
    let pieces = vec![(rect(80.0, 10.0), 1)];
    let container = NestContainer::Rect {
        width: 12.0,
        height: 90.0,
    };
    let sin = run(&input(
        pieces.clone(),
        container.clone(),
        NestOptions {
            rotations: Rotations::Fixed,
            ..quick()
        },
    ));
    assert_eq!(sin.unplaced.len(), 1);
    let con_inp = input(pieces, container, quick());
    let con = run(&con_inp);
    assert!(con.unplaced.is_empty());
    let pl = &con.placements[0];
    assert!((pl.rotation_deg - 90.0).abs() < 1e-9 || (pl.rotation_deg - 270.0).abs() < 1e-9);
    let (lo, hi) = bbox_of(&placed_paths(&con_inp, pl));
    assert!((hi[0] - lo[0] - 10.0).abs() < 1e-6 && (hi[1] - lo[1] - 80.0).abs() < 1e-6);
}

#[test]
fn el_espejo_solo_se_usa_si_se_permite() {
    // Triángulo con el ángulo recto abajo a la derecha en un material triangular con el ángulo
    // recto abajo a la izquierda: sin giros, solo cabe reflejado.
    let piece = vec![poly(&[[0.0, 0.0], [30.0, 0.0], [30.0, 30.0]])];
    let material = vec![poly(&[[0.0, 0.0], [30.0, 0.0], [0.0, 30.0]])];
    let make = |allow_mirror| {
        input(
            vec![(piece.clone(), 1)],
            NestContainer::Region(material.clone()),
            NestOptions {
                rotations: Rotations::Fixed,
                allow_mirror,
                ..quick()
            },
        )
    };
    assert_eq!(run(&make(false)).unplaced, vec![(0, 0)]);
    let out = run(&make(true));
    assert!(out.unplaced.is_empty());
    assert!(out.placements[0].mirror);
    // El espejo conserva el área: el transform tiene determinante negativo.
    let t = out.placements[0].transform;
    assert!(t[0] * t[3] - t[1] * t[2] < 0.0);
}

#[test]
fn misma_semilla_mismo_resultado() {
    let make = |seed| {
        input(
            vec![
                (ele(10.0), 3),
                (rect(35.0, 12.0), 3),
                (vec![circle(9.0, 40)], 2),
            ],
            NestContainer::Rect {
                width: 90.0,
                height: 60.0,
            },
            NestOptions {
                seed,
                max_evaluations: Some(25),
                rotations: Rotations::Step(45.0),
                ..quick()
            },
        )
    };
    let (a, b) = (run(&make(7)), run(&make(7)));
    assert_eq!(a.placements, b.placements);
    assert_eq!(a.unplaced, b.unplaced);
    assert_eq!(a.evaluations, b.evaluations);
}

/// Área común de la caja de una pieza con un rectángulo `(x0, y0, x1, y1)`.
fn box_overlap(paths: &[Vec<Pt>], r: (f64, f64, f64, f64)) -> f64 {
    let (lo, hi) = bbox_of(paths);
    let ox = (hi[0].min(r.2) - lo[0].max(r.0)).max(0.0);
    let oy = (hi[1].min(r.3) - lo[1].max(r.1)).max(0.0);
    ox * oy
}

#[test]
fn contenedor_irregular_y_con_hueco() {
    // Material en L con un agujero cuadrado de 20 mm: nada puede caer en el agujero ni en la
    // esquina que le falta a la L.
    let l = poly(&[
        [0.0, 0.0],
        [100.0, 0.0],
        [100.0, 40.0],
        [40.0, 40.0],
        [40.0, 100.0],
        [0.0, 100.0],
    ]);
    let hole = poly(&[[10.0, 10.0], [30.0, 10.0], [30.0, 30.0], [10.0, 30.0]]);
    let inp = input(
        vec![(rect(18.0, 18.0), 8)],
        NestContainer::Region(vec![l, hole]),
        NestOptions {
            max_evaluations: Some(10),
            ..quick()
        },
    );
    let out = run(&inp);
    assert!(
        out.placements.len() >= 7,
        "solo caben {}",
        out.placements.len()
    );
    for pl in &out.placements {
        let paths = placed_paths(&inp, pl);
        assert!(
            box_overlap(&paths, (10.0, 10.0, 30.0, 30.0)) < 0.5,
            "pieza sobre el agujero"
        );
        assert!(
            box_overlap(&paths, (40.0, 40.0, 100.0, 100.0)) < 0.5,
            "pieza fuera de la L"
        );
        let (lo, hi) = bbox_of(&paths);
        assert!(lo[0] > -0.01 && lo[1] > -0.01 && hi[0] < 100.01 && hi[1] < 100.01);
    }
    assert!(min_pair_distance(&inp, &out) > -1e-9);
}

#[test]
fn una_pieza_cabe_en_el_hueco_de_otra() {
    // Un marco de 60x60 con un hueco de 30x30 y un cuadrado de 20x20: en una cama de 62x62
    // el cuadrado solo cabe dentro del hueco.
    let frame = vec![
        poly(&[[0.0, 0.0], [60.0, 0.0], [60.0, 60.0], [0.0, 60.0]]),
        poly(&[[15.0, 15.0], [45.0, 15.0], [45.0, 45.0], [15.0, 45.0]]),
    ];
    let make = |use_holes| {
        input(
            vec![(frame.clone(), 1), (rect(20.0, 20.0), 1)],
            NestContainer::Rect {
                width: 62.0,
                height: 62.0,
            },
            NestOptions {
                use_holes,
                rotations: Rotations::Fixed,
                ..quick()
            },
        )
    };
    let sin = run(&make(false));
    assert_eq!(sin.unplaced, vec![(1, 0)]);
    let inp = make(true);
    let con = run(&inp);
    assert!(con.unplaced.is_empty(), "{:?}", con.unplaced);
    let sq = con.placements.iter().find(|p| p.piece == 1).unwrap();
    let (lo, hi) = bbox_of(&placed_paths(&inp, sq));
    assert!(
        lo[0] >= 15.0 - 0.01
            && lo[1] >= 15.0 - 0.01
            && hi[0] <= 45.0 + 0.01
            && hi[1] <= 45.0 + 0.01,
        "{lo:?} {hi:?}"
    );
}

#[test]
fn parar_devuelve_lo_mejor_hasta_ahora() {
    let inp = input(
        vec![(rect(10.0, 10.0), 30)],
        NestContainer::Rect {
            width: 200.0,
            height: 200.0,
        },
        NestOptions {
            max_evaluations: None,
            ..quick()
        },
    );
    let stop = AtomicBool::new(false);
    let mut seen = 0;
    let out = nest(&inp, &stop, &mut |o| {
        seen += 1;
        if o.evaluations >= 3 {
            stop.store(true, std::sync::atomic::Ordering::Relaxed);
        }
    })
    .unwrap();
    assert!(seen >= 3 && out.interrupted);
    assert_eq!(out.placements.len(), 30);
}

#[test]
fn entradas_invalidas_dan_errores_en_espanol() {
    let base = input(
        vec![(rect(10.0, 10.0), 1)],
        NestContainer::Rect {
            width: 50.0,
            height: 50.0,
        },
        quick(),
    );
    let mut none = base.clone();
    none.pieces[0].copies = 0;
    assert_eq!(
        nest(&none, &AtomicBool::new(false), &mut |_| {}).unwrap_err(),
        NestError::NoPieces
    );
    let mut flat = base.clone();
    flat.pieces[0].region = vec![poly(&[[0.0, 0.0], [5.0, 0.0], [10.0, 0.0]])];
    let e = nest(&flat, &AtomicBool::new(false), &mut |_| {}).unwrap_err();
    assert_eq!(e, NestError::EmptyPiece(0));
    assert!(e.to_string().contains("pieza 1"));
    let mut bad = base.clone();
    bad.container = NestContainer::Rect {
        width: 0.0,
        height: 50.0,
    };
    assert_eq!(
        nest(&bad, &AtomicBool::new(false), &mut |_| {}).unwrap_err(),
        NestError::InvalidContainer
    );
    let mut neg = base;
    neg.options.spacing = -1.0;
    assert!(matches!(
        nest(&neg, &AtomicBool::new(false), &mut |_| {}).unwrap_err(),
        NestError::InvalidOption(_)
    ));
}

#[test]
fn doscientas_piezas_respetan_el_tope_de_tiempo() {
    let mut pieces = vec![(ele(6.0), 70), (rect(14.0, 9.0), 70)];
    pieces.push((vec![circle(5.0, 40)], 60));
    let limit = Duration::from_secs(4);
    let inp = input(
        pieces,
        NestContainer::Rect {
            width: 600.0,
            height: 400.0,
        },
        NestOptions {
            spacing: 1.0,
            time_limit: limit,
            max_evaluations: None,
            ..NestOptions::default()
        },
    );
    let t0 = Instant::now();
    let out = run(&inp);
    let took = t0.elapsed();
    eprintln!(
        "200 piezas: {:.2} s, {} evaluaciones, {:.1} % aprovechado, {} sin colocar",
        took.as_secs_f64(),
        out.evaluations,
        out.utilization,
        out.unplaced.len()
    );
    assert!(took < limit + Duration::from_secs(6), "tardó {took:?}");
    // Siempre se informa de todas las piezas; en compilación optimizada además caben todas
    // (en depuración la primera colocación puede no terminar dentro del tope).
    assert_eq!(out.placements.len() + out.unplaced.len(), 200);
    if !cfg!(debug_assertions) {
        assert!(out.unplaced.is_empty());
    }
    assert!(min_pair_distance(&inp, &out) >= 1.0 - 0.01);
}
