//! Pruebas de varias hojas, giros por pieza (veta, sin giro) y sobrantes.

use crate::geom::Pt;
use crate::tests::{bbox_of, distance, ele, inside, placed_paths, poly, quick, rect};
use crate::*;
use std::sync::atomic::AtomicBool;

fn pieces_of(list: Vec<(Vec<Polyline>, u32, PieceRotation)>) -> Vec<NestPiece> {
    list.into_iter()
        .map(|(region, copies, rotation)| NestPiece {
            region,
            copies,
            rotation,
        })
        .collect()
}

fn sheets_input(
    pieces: Vec<NestPiece>,
    sheets: Vec<NestContainer>,
    refill: Option<NestContainer>,
    max_sheets: usize,
    options: NestOptions,
) -> SheetsInput {
    SheetsInput {
        pieces,
        sheets,
        refill,
        max_sheets,
        options,
    }
}

fn run_sheets(inp: &SheetsInput) -> SheetsOutcome {
    nest_sheets(inp, &AtomicBool::new(false), &mut |_| {}).expect("la petición es válida")
}

fn as_nest_input(inp: &SheetsInput) -> NestInput {
    NestInput {
        pieces: inp.pieces.clone(),
        container: NestContainer::Rect {
            width: 1.0,
            height: 1.0,
        },
        options: inp.options.clone(),
    }
}

/// Comprueba lo que debe cumplir cualquier reparto: cada copia aparece exactamente una vez
/// (colocada o sin colocar), las piezas de cada hoja están dentro de ella y separadas.
fn check_partition(inp: &SheetsInput, out: &SheetsOutcome, sheet_box: impl Fn(usize) -> (Pt, Pt)) {
    let ni = as_nest_input(inp);
    let mut seen = std::collections::HashSet::new();
    for s in &out.sheets {
        let (lo, hi) = sheet_box(s.index);
        let mut paths = Vec::new();
        for pl in &s.placements {
            assert!(
                seen.insert((pl.piece, pl.copy)),
                "copia repetida {:?}",
                (pl.piece, pl.copy)
            );
            let p = placed_paths(&ni, pl);
            let (a, b) = bbox_of(&p);
            assert!(
                a[0] >= lo[0] - 0.01
                    && a[1] >= lo[1] - 0.01
                    && b[0] <= hi[0] + 0.01
                    && b[1] <= hi[1] + 0.01,
                "fuera de la hoja {}: {a:?} {b:?}",
                s.index
            );
            paths.push(p);
        }
        for i in 0..paths.len() {
            for j in i + 1..paths.len() {
                let d = distance(&paths[i], &paths[j]);
                assert!(
                    d >= inp.options.spacing - 0.01,
                    "hoja {}: piezas a {d} mm (separación {})",
                    s.index,
                    inp.options.spacing
                );
            }
        }
    }
    for &u in &out.unplaced {
        assert!(seen.insert(u), "copia repetida {u:?}");
    }
    let total: usize = inp.pieces.iter().map(|p| p.copies as usize).sum();
    assert_eq!(seen.len(), total, "falta alguna copia");
}

#[test]
fn si_no_caben_en_una_hoja_siguen_en_otras_sin_solapes() {
    let pieces = pieces_of(vec![
        (ele(10.0), 5, PieceRotation::Inherit),
        (rect(18.0, 12.0), 7, PieceRotation::Inherit),
        (
            vec![crate::tests::circle(6.0, 40)],
            6,
            PieceRotation::Inherit,
        ),
    ]);
    let inp = sheets_input(
        pieces,
        Vec::new(),
        Some(NestContainer::Rect {
            width: 60.0,
            height: 50.0,
        }),
        20,
        NestOptions {
            spacing: 2.0,
            margin: 1.0,
            max_evaluations: Some(15),
            ..quick()
        },
    );
    let out = run_sheets(&inp);
    assert!(out.unplaced.is_empty(), "sin colocar {:?}", out.unplaced);
    assert!(out.sheets.len() >= 2, "hojas: {}", out.sheets.len());
    assert_eq!(out.placed_count(), 18);
    // Las hojas nuevas se numeran desde `sheets.len()` (aquí 0).
    for (k, s) in out.sheets.iter().enumerate() {
        assert_eq!(s.index, k);
        assert!(s.utilization > 0.0 && s.utilization <= 100.0);
    }
    check_partition(&inp, &out, |_| ([1.0, 1.0], [59.0, 49.0]));
}

#[test]
fn reparto_exacto_en_tres_hojas() {
    // Nueve cuadrados de 50 mm en hojas de 100 x 100: 4 + 4 + 1.
    let inp = sheets_input(
        pieces_of(vec![(rect(50.0, 50.0), 9, PieceRotation::Inherit)]),
        Vec::new(),
        Some(NestContainer::Rect {
            width: 100.0,
            height: 100.0,
        }),
        10,
        quick(),
    );
    let out = run_sheets(&inp);
    assert!(out.unplaced.is_empty());
    let counts: Vec<usize> = out.sheets.iter().map(|s| s.placements.len()).collect();
    assert_eq!(counts, vec![4, 4, 1]);
    assert!((out.sheets[0].utilization - 100.0).abs() < 1e-6);
    assert!((out.sheets[1].utilization - 100.0).abs() < 1e-6);
    assert!((out.sheets[2].utilization - 25.0).abs() < 1e-6);
    // Aprovechamiento global: 9 * 2500 / 30000.
    assert!((out.utilization - 75.0).abs() < 1e-6, "{}", out.utilization);
    check_partition(&inp, &out, |_| ([0.0, 0.0], [100.0, 100.0]));
}

#[test]
fn las_hojas_de_la_lista_van_primero_y_la_que_no_sirve_se_salta() {
    let tiny = NestContainer::Rect {
        width: 10.0,
        height: 10.0,
    };
    let scrap = NestContainer::Region(vec![poly(&[
        [0.0, 0.0],
        [70.0, 0.0],
        [70.0, 40.0],
        [0.0, 40.0],
    ])]);
    let inp = sheets_input(
        pieces_of(vec![(rect(30.0, 30.0), 6, PieceRotation::Inherit)]),
        vec![tiny, scrap],
        Some(NestContainer::Rect {
            width: 100.0,
            height: 100.0,
        }),
        10,
        quick(),
    );
    let out = run_sheets(&inp);
    assert!(out.unplaced.is_empty());
    // El retazo diminuto no admite nada y no cuenta; el retazo de 70 x 40 recibe 2 piezas;
    // el resto va a una hoja nueva (índice = hojas de la lista + 0).
    let idx: Vec<usize> = out.sheets.iter().map(|s| s.index).collect();
    assert_eq!(idx, vec![1, 2]);
    assert_eq!(out.sheets[0].placements.len(), 2);
    assert_eq!(out.sheets[1].placements.len(), 4);
}

#[test]
fn un_retazo_inutil_no_gasta_el_tiempo_del_trabajo() {
    // Sin tope de evaluaciones y con 600 s de margen: el retazo de 10 x 10 no admite nada y se
    // abandona al instante; la hoja nueva termina por estancamiento (2 piezas).
    let inp = sheets_input(
        pieces_of(vec![(rect(30.0, 30.0), 2, PieceRotation::Inherit)]),
        vec![NestContainer::Rect {
            width: 10.0,
            height: 10.0,
        }],
        Some(NestContainer::Rect {
            width: 100.0,
            height: 100.0,
        }),
        5,
        NestOptions {
            time_limit: Duration::from_secs(600),
            max_evaluations: None,
            ..NestOptions::default()
        },
    );
    let out = run_sheets(&inp);
    assert!(out.unplaced.is_empty());
    assert_eq!(out.sheets.len(), 1);
    assert_eq!(out.sheets[0].index, 1);
    assert!(
        out.elapsed < Duration::from_secs(20),
        "tardó {:?}: el retazo inútil consumió su tiempo",
        out.elapsed
    );
}

#[test]
fn sin_hoja_nueva_lo_que_no_cabe_queda_sin_colocar() {
    let inp = sheets_input(
        pieces_of(vec![(rect(50.0, 50.0), 6, PieceRotation::Inherit)]),
        vec![NestContainer::Rect {
            width: 100.0,
            height: 100.0,
        }],
        None,
        10,
        quick(),
    );
    let out = run_sheets(&inp);
    assert_eq!(out.placed_count(), 4);
    assert_eq!(out.unplaced.len(), 2);
}

#[test]
fn el_tope_de_hojas_se_respeta() {
    let inp = sheets_input(
        pieces_of(vec![(rect(50.0, 50.0), 9, PieceRotation::Inherit)]),
        Vec::new(),
        Some(NestContainer::Rect {
            width: 100.0,
            height: 100.0,
        }),
        2,
        quick(),
    );
    let out = run_sheets(&inp);
    assert_eq!(out.sheets.len(), 2);
    assert_eq!(out.placed_count(), 8);
    assert_eq!(out.unplaced.len(), 1);
}

#[test]
fn una_pieza_mas_grande_que_cualquier_hoja_no_provoca_hojas_vacias() {
    let inp = sheets_input(
        pieces_of(vec![
            (rect(500.0, 10.0), 1, PieceRotation::Inherit),
            (rect(20.0, 20.0), 3, PieceRotation::Inherit),
        ]),
        Vec::new(),
        Some(NestContainer::Rect {
            width: 100.0,
            height: 100.0,
        }),
        10,
        quick(),
    );
    let out = run_sheets(&inp);
    assert_eq!(out.sheets.len(), 1);
    assert_eq!(out.placed_count(), 3);
    assert_eq!(out.unplaced, vec![(0, 0)]);
}

#[test]
fn varias_hojas_con_la_misma_semilla_dan_lo_mismo() {
    let make = || {
        sheets_input(
            pieces_of(vec![
                (ele(10.0), 6, PieceRotation::Inherit),
                (rect(18.0, 12.0), 6, PieceRotation::Inherit),
            ]),
            Vec::new(),
            Some(NestContainer::Rect {
                width: 60.0,
                height: 50.0,
            }),
            10,
            NestOptions {
                spacing: 1.0,
                max_evaluations: Some(12),
                ..quick()
            },
        )
    };
    let (a, b) = (run_sheets(&make()), run_sheets(&make()));
    assert_eq!(a.sheets.len(), b.sheets.len());
    for (x, y) in a.sheets.iter().zip(&b.sheets) {
        assert_eq!(x.placements, y.placements);
    }
}

#[test]
fn las_opciones_de_hojas_invalidas_dan_error() {
    let mut inp = sheets_input(
        pieces_of(vec![(rect(10.0, 10.0), 1, PieceRotation::Inherit)]),
        Vec::new(),
        Some(NestContainer::Rect {
            width: 50.0,
            height: 50.0,
        }),
        0,
        quick(),
    );
    let e = nest_sheets(&inp, &AtomicBool::new(false), &mut |_| {}).unwrap_err();
    assert_eq!(e, NestError::InvalidOption(BadOption::MaxSheets));
    assert!(e.to_string().contains("máximo de hojas"));
    inp.max_sheets = 3;
    inp.refill = None;
    assert_eq!(
        nest_sheets(&inp, &AtomicBool::new(false), &mut |_| {}).unwrap_err(),
        NestError::InvalidContainer
    );
    inp.refill = Some(NestContainer::Rect {
        width: -1.0,
        height: 5.0,
    });
    assert_eq!(
        nest_sheets(&inp, &AtomicBool::new(false), &mut |_| {}).unwrap_err(),
        NestError::InvalidContainer
    );
}

#[test]
fn el_progreso_de_hojas_avanza_y_termina_igual_que_el_resultado() {
    let inp = sheets_input(
        pieces_of(vec![(rect(50.0, 50.0), 6, PieceRotation::Inherit)]),
        Vec::new(),
        Some(NestContainer::Rect {
            width: 100.0,
            height: 100.0,
        }),
        10,
        quick(),
    );
    let mut calls = 0;
    let mut max_sheets_seen = 0;
    let out = nest_sheets(&inp, &AtomicBool::new(false), &mut |s| {
        calls += 1;
        max_sheets_seen = max_sheets_seen.max(s.sheets.len());
    })
    .unwrap();
    assert!(calls > 2);
    assert_eq!(max_sheets_seen, out.sheets.len());
}

// ---- Giros por pieza ----

fn one_rect_input(rotation: PieceRotation, w: f64, h: f64, rots: Rotations) -> NestInput {
    NestInput {
        pieces: pieces_of(vec![(rect(60.0, 20.0), 1, rotation)]),
        container: NestContainer::Rect {
            width: w,
            height: h,
        },
        options: NestOptions {
            rotations: rots,
            ..quick()
        },
    }
}

fn run_one(inp: &NestInput) -> Outcome {
    nest(inp, &AtomicBool::new(false), &mut |_| {}).expect("válida")
}

#[test]
fn la_veta_impide_girar_de_lado() {
    // Cabe de pie (20 de ancho) pero no tumbada: con giros libres entra girada 90°; con veta no.
    let free = run_one(&one_rect_input(
        PieceRotation::Inherit,
        25.0,
        100.0,
        Rotations::Quarter,
    ));
    assert_eq!(free.placements.len(), 1);
    assert!(
        (free.placements[0].rotation_deg - 90.0).abs() < 1e-9
            || (free.placements[0].rotation_deg - 270.0).abs() < 1e-9
    );
    let grain = run_one(&one_rect_input(
        PieceRotation::grain(),
        25.0,
        100.0,
        Rotations::Quarter,
    ));
    assert!(grain.placements.is_empty());
    assert_eq!(grain.unplaced, vec![(0, 0)]);
}

#[test]
fn la_veta_solo_usa_0_y_180_aunque_el_giro_global_sea_libre() {
    // Piezas en ele (asimétricas): con giro libre usarían muchos ángulos.
    let inp = NestInput {
        pieces: pieces_of(vec![
            (ele(10.0), 8, PieceRotation::grain()),
            (rect(25.0, 15.0), 4, PieceRotation::Inherit),
        ]),
        container: NestContainer::Rect {
            width: 120.0,
            height: 80.0,
        },
        options: NestOptions {
            rotations: Rotations::Step(30.0),
            max_evaluations: Some(40),
            spacing: 1.0,
            ..quick()
        },
    };
    let out = run_one(&inp);
    assert!(out.unplaced.is_empty());
    let mut used_180 = false;
    for p in &out.placements {
        if p.piece == 0 {
            let a = p.rotation_deg.rem_euclid(360.0);
            assert!(a < 1e-6 || (a - 180.0).abs() < 1e-6, "ángulo {a} con veta");
            used_180 |= (a - 180.0).abs() < 1e-6;
        }
    }
    // El motor sí aprovecha el volteo de 180° cuando ayuda (las eles se acoplan de dos en dos).
    assert!(used_180, "ninguna ele se volteó 180°");
}

#[test]
fn sin_giro_deja_la_pieza_como_esta() {
    let inp = NestInput {
        pieces: pieces_of(vec![
            (ele(10.0), 6, PieceRotation::Fixed),
            (rect(25.0, 15.0), 3, PieceRotation::Inherit),
        ]),
        container: NestContainer::Rect {
            width: 100.0,
            height: 60.0,
        },
        options: NestOptions {
            max_evaluations: Some(30),
            ..quick()
        },
    };
    let out = run_one(&inp);
    assert!(out.unplaced.is_empty());
    for p in out.placements.iter().filter(|p| p.piece == 0) {
        assert!(p.rotation_deg.abs() < 1e-9, "giró {}", p.rotation_deg);
        // Sin giro ni espejo, la afín es solo una traslación.
        assert!((p.transform[0] - 1.0).abs() < 1e-9 && p.transform[1].abs() < 1e-9);
        assert!(p.transform[2].abs() < 1e-9 && (p.transform[3] - 1.0).abs() < 1e-9);
    }
}

#[test]
fn angulos_propios_se_normalizan_y_un_angulo_no_finito_es_error() {
    // -90° y 270° son lo mismo: la pieza solo puede estar a 270°.
    let out = run_one(&one_rect_input(
        PieceRotation::Only(vec![-90.0, 270.0]),
        25.0,
        100.0,
        Rotations::Fixed,
    ));
    assert_eq!(out.placements.len(), 1);
    assert!((out.placements[0].rotation_deg - 270.0).abs() < 1e-6);
    let bad = one_rect_input(
        PieceRotation::Only(vec![f64::NAN]),
        25.0,
        100.0,
        Rotations::Fixed,
    );
    let e = nest(&bad, &AtomicBool::new(false), &mut |_| {}).unwrap_err();
    assert_eq!(e, NestError::InvalidOption(BadOption::PieceAngle));
}

// ---- Sobrante ----

fn area_of(region: &[Polyline]) -> f64 {
    let mut a: Vec<f64> = region
        .iter()
        .map(|p| {
            let n = p.points.len();
            (0..n)
                .map(|i| {
                    let (u, v) = (p.points[i], p.points[(i + 1) % n]);
                    u.x * v.y - v.x * u.y
                })
                .sum::<f64>()
                / 2.0
        })
        .collect();
    // Exterior (positivo) y huecos (negativos): el área neta es la suma con signo.
    a.retain(|x| x.abs() > 1e-9);
    a.iter().sum()
}

#[test]
fn el_sobrante_es_la_hoja_menos_las_piezas_con_margen() {
    let sheet = NestContainer::Rect {
        width: 100.0,
        height: 100.0,
    };
    let placed = vec![rect(50.0, 100.0)];
    let opts = LeftoverOptions {
        clearance: 2.0,
        min_width: 10.0,
        min_area: 100.0,
    };
    let rem = leftover(&sheet, &placed, &opts);
    assert_eq!(rem.len(), 1);
    // 100 x 100 menos la pieza de 50 x 100 con 2 mm de margen: 48 x 100.
    let a = area_of(&rem[0]);
    assert!((a - 4800.0).abs() < 1.0, "área {a}");
    // Ningún punto del retazo invade la pieza ni su margen.
    let piece = vec![vec![[0.0, 0.0], [50.0, 0.0], [50.0, 100.0], [0.0, 100.0]]];
    let rem_paths: Vec<Vec<Pt>> = rem[0]
        .iter()
        .map(|p| p.points.iter().map(|q| [q.x, q.y]).collect())
        .collect();
    assert!(distance(&piece, &rem_paths) >= 2.0 - 0.01);
}

#[test]
fn el_sobrante_deja_huecos_y_descarta_astillas() {
    let sheet = NestContainer::Rect {
        width: 100.0,
        height: 100.0,
    };
    // Una pieza al centro: el retazo es un marco con un hueco.
    let placed = vec![vec![poly(&[
        [30.0, 30.0],
        [70.0, 30.0],
        [70.0, 70.0],
        [30.0, 70.0],
    ])]];
    let opts = LeftoverOptions {
        clearance: 0.0,
        min_width: 10.0,
        min_area: 100.0,
    };
    let rem = leftover(&sheet, &placed, &opts);
    assert_eq!(rem.len(), 1);
    assert_eq!(rem[0].len(), 2, "exterior más un hueco");
    assert!((area_of(&rem[0]) - (10000.0 - 1600.0)).abs() < 1.0);
    let inner: Vec<Pt> = rem[0][1].points.iter().map(|q| [q.x, q.y]).collect();
    assert!(inside([50.0, 50.0], &inner));
    // Una pieza que deja solo una tira de 4 mm: no es aprovechable (ancho mínimo 10).
    let strip = vec![rect(96.0, 100.0)];
    assert!(leftover(&sheet, &strip, &opts).is_empty());
    // Hoja llena: no sobra nada.
    assert!(leftover(&sheet, &[rect(100.0, 100.0)], &opts).is_empty());
}

#[test]
fn el_sobrante_de_un_retazo_irregular_queda_dentro_del_retazo() {
    // Material en L; una pieza en el brazo horizontal.
    let l = vec![poly(&[
        [0.0, 0.0],
        [100.0, 0.0],
        [100.0, 40.0],
        [40.0, 40.0],
        [40.0, 100.0],
        [0.0, 100.0],
    ])];
    let sheet = NestContainer::Region(l.clone());
    let placed = vec![vec![poly(&[
        [50.0, 0.0],
        [100.0, 0.0],
        [100.0, 40.0],
        [50.0, 40.0],
    ])]];
    let opts = LeftoverOptions {
        clearance: 1.0,
        min_width: 10.0,
        min_area: 100.0,
    };
    let rem = leftover(&sheet, &placed, &opts);
    assert_eq!(rem.len(), 1);
    let net = area_of(&rem[0]);
    assert!(net > 4000.0 && net < 7600.0, "área {net}");
    // Todos los vértices están dentro de la L (o en su borde).
    let l_ring: Vec<Pt> = l[0].points.iter().map(|q| [q.x, q.y]).collect();
    for q in &rem[0][0].points {
        let on_edge =
            q.x < 1e-6 || q.y < 1e-6 || (q.x - 100.0).abs() < 1e-6 || (q.y - 100.0).abs() < 1e-6;
        assert!(
            inside([q.x, q.y], &l_ring)
                || on_edge
                || (q.x - 40.0).abs() < 1e-6
                || (q.y - 40.0).abs() < 1e-6,
            "{q:?}"
        );
    }
}
