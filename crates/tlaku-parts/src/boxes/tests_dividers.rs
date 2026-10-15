//! Pruebas de los divisores, el zócalo y las patas de la caja.

use super::dividers::{Axis, Divider};
use super::panels::{Id, Panel};
use super::tests::{bbox, holders, in_void, panel, slabs};
use super::*;
use crate::rectilinear::Rect;
use crate::testutil::{crossings, inside};

fn with_dividers(p: BoxParams) -> BoxParams {
    BoxParams {
        dividers_x: 2,
        dividers_y: 1,
        ..p
    }
}

fn base() -> BoxParams {
    BoxParams {
        width_mm: 150.0,
        depth_mm: 100.0,
        height_mm: 50.0,
        ..BoxParams::default()
    }
}

fn divider_sets() -> Vec<BoxParams> {
    vec![
        with_dividers(base()),
        with_dividers(BoxParams {
            kind: BoxKind::Open,
            ..base()
        }),
        with_dividers(BoxParams {
            kind: BoxKind::SlidingLid,
            ..base()
        }),
        BoxParams {
            dividers_x: 3,
            dividers_y: 2,
            thickness_mm: 4.0,
            finger_mm: 8.0,
            dims: Dims::Inner,
            width_mm: 140.0,
            depth_mm: 90.0,
            height_mm: 45.0,
            ..base()
        },
        BoxParams {
            dividers_x: 1,
            divider_widths_x: "30".into(),
            ..base()
        },
    ]
}

fn built_all(p: &BoxParams) -> (Sizes, Vec<Panel>, Vec<Divider>) {
    let b = build_all(p).expect("parámetros válidos");
    (b.sizes, b.panels, b.dividers)
}

/// Dentro de la losa (grosor) de un divisor.
fn in_slab(d: &Divider, t: f64, p: [f64; 3]) -> bool {
    let depth = p[d.axis.thick()] - d.pos;
    depth > 0.0 && depth < t
}

/// ¿El punto cae en la losa del divisor y dentro de su caja envolvente (cuerpo y pestañas)?
fn in_extent(d: &Divider, t: f64, p: [f64; 3]) -> bool {
    let (x0, y0, x1, y1) = bbox(&d.cuts);
    let (s, z) = (p[d.axis.run()] - d.o[d.axis.run()], p[2] - d.o[2]);
    in_slab(d, t, p) && s > x0 && s < x1 && z > y0 && z < y1
}

/// Divisores con material en el punto 3D.
fn div_holders(divs: &[Divider], t: f64, p: [f64; 3]) -> usize {
    divs.iter()
        .filter(|d| in_slab(d, t, p) && inside(&d.cuts, p[d.axis.run()] - d.o[d.axis.run()], p[2] - d.o[2]))
        .count()
}

/// Con holgura 0 cada punto de una losa de panel o de divisor tiene material de exactamente una
/// pieza: las pestañas llenan sus ranuras y las dos mitades de un cruce se complementan. Con
/// holgura positiva nunca hay dos piezas en el mismo punto.
#[test]
fn dividers_fill_slots_exactly_and_never_overlap_in_3d() {
    for b in divider_sets() {
        for fit in [0.0, 0.2] {
            let p = BoxParams { fit_mm: fit, ..b.clone() };
            let (s, panels, divs) = built_all(&p);
            let t = p.thickness_mm;
            let step = if fit == 0.0 { 1.07 } else { 1.9 };
            let (mut checked, mut tab_points, mut cross_points) = (0usize, 0usize, 0usize);
            let mut x = 0.1729;
            while x < s.w {
                let mut y = 0.2371;
                while y < s.d {
                    let mut z = 0.2117;
                    while z < s.h {
                        let pt = [x, y, z];
                        let slab = slabs(&panels, pt);
                        let in_div = divs.iter().filter(|d| in_extent(d, t, pt)).count();
                        let own = holders(&panels, pt).len() + div_holders(&divs, t, pt);
                        assert!(own <= 1, "{p:?}: solape en {pt:?}");
                        if fit == 0.0 && (!slab.is_empty() || in_div > 0) && !in_void(&panels, pt) {
                            assert_eq!(own, 1, "{p:?}: hueco en {pt:?} (losas {slab:?})");
                            checked += 1;
                            tab_points += usize::from(!slab.is_empty() && in_div > 0);
                        }
                        cross_points += usize::from(in_div == 2);
                        z += step;
                    }
                    y += step;
                }
                x += step;
            }
            if fit == 0.0 {
                assert!(checked > 1000, "{p:?}: pocos puntos ({checked})");
                assert!(tab_points > 50, "{p:?}: las pestañas no se probaron");
                assert!(p.dividers_x * p.dividers_y == 0 || cross_points > 20, "{p:?}: los cruces no se probaron");
            }
        }
    }
}

#[test]
fn dividers_split_the_cavity_into_equal_compartments() {
    let (_, _, divs) = built_all(&with_dividers(base()));
    let xs: Vec<f64> = divs.iter().filter(|d| d.axis == Axis::X).map(|d| d.pos).collect();
    // Cavidad 144 = 3 compartimentos + 2 divisores de 3: compartimentos de 46.
    assert_eq!(xs.len(), 2);
    assert!((xs[0] - 49.0).abs() < 1e-9, "{xs:?}");
    assert!((xs[1] - 98.0).abs() < 1e-9);
    let ys: Vec<f64> = divs.iter().filter(|d| d.axis == Axis::Y).map(|d| d.pos).collect();
    // Cavidad 94 = 2 compartimentos + 1 divisor: 45,5 cada uno.
    assert!((ys[0] - (3.0 + 45.5)).abs() < 1e-9);
}

#[test]
fn custom_widths_are_exact_and_rest_is_shared() {
    let p = BoxParams {
        dividers_x: 2,
        divider_widths_x: "30, 50".into(),
        ..BoxParams::default() // cavidad 94 = 30 + 50 + resto + 2 divisores de 3
    };
    let (_, _, divs) = built_all(&p);
    assert!((divs[0].pos - 33.0).abs() < 1e-9);
    assert!((divs[1].pos - 86.0).abs() < 1e-9);
    // Último compartimento: 94 - 6 - 80 = 8, de 89 a 97 - 3 = 94... termina en la pared.
    assert!((divs[1].pos + 3.0 + 8.0 - 97.0).abs() < 1e-9);

    let sum = |text: &str| {
        build_all(&BoxParams {
            dividers_x: 1,
            divider_widths_x: text.into(),
            ..BoxParams::default()
        })
        .err()
        .map(|e| e.field)
    };
    // Todos los anchos dados deben sumar la cavidad menos los divisores (94 - 3 = 91).
    assert_eq!(sum("40 51"), None);
    assert_eq!(sum("40 50"), Some(Some("divider_widths_x".to_string())));
    for bad in ["abc", "10 20 30", "2 89"] {
        assert_eq!(sum(bad), Some(Some("divider_widths_x".to_string())), "{bad}");
    }
}

#[test]
fn crossing_slots_are_complementary_half_laps() {
    for fit in [0.0, 0.15] {
        let p = with_dividers(BoxParams { fit_mm: fit, ..base() });
        let t = p.thickness_mm;
        let (_, _, divs) = built_all(&p);
        let height = 50.0 - 2.0 * t;
        let y_div = divs.iter().find(|d| d.axis == Axis::Y).unwrap();
        let cross_s = y_div.pos - t; // y del cruce, desde el inicio de la cavidad
        for d in divs.iter().filter(|d| d.axis == Axis::X) {
            // El de X: sin material arriba y con material abajo; el hueco mide T + holgura.
            let (lo, hi) = (cross_s - fit / 2.0, cross_s + t + fit / 2.0);
            for z in [height / 2.0 + 0.01, height - 0.01] {
                assert!(!inside(&d.cuts, (lo + hi) / 2.0, z));
                assert!(!inside(&d.cuts, lo + 0.005, z) && !inside(&d.cuts, hi - 0.005, z));
                assert!(inside(&d.cuts, lo - 0.005, z) && inside(&d.cuts, hi + 0.005, z));
            }
            for z in [0.01, height / 2.0 - 0.01] {
                assert!(inside(&d.cuts, (lo + hi) / 2.0, z), "el de X conserva su mitad inferior");
            }
            assert!((hi - lo - (t + fit)).abs() < 1e-9);
        }
        // El de Y: al revés.
        let x_div = divs.iter().find(|d| d.axis == Axis::X).unwrap();
        let cross_x = x_div.pos - t + t / 2.0;
        for z in [0.01, height / 2.0 - 0.01] {
            assert!(!inside(&y_div.cuts, cross_x, z));
        }
        for z in [height / 2.0 + 0.01, height - 0.01] {
            assert!(inside(&y_div.cuts, cross_x, z));
        }
    }
}

#[test]
fn tabs_fit_their_slots_with_exactly_the_requested_clearance() {
    let fit = 0.2;
    let p = with_dividers(BoxParams { fit_mm: fit, ..base() });
    let t = p.thickness_mm;
    let (_, panels, divs) = built_all(&p);
    // Toda ranura mide T + holgura en el grosor del divisor.
    for pn in &panels {
        for r in &pn.slots {
            let (w, h) = (r.x1 - r.x0, r.y1 - r.y0);
            assert!(
                (w - (t + fit)).abs() < 1e-9 || (h - (t + fit)).abs() < 1e-9,
                "{:?}: ranura {w} x {h}",
                pn.id
            );
        }
    }
    // Cada ranura del frente rodea una pestaña del divisor de X y le sobra `fit` en ancho y largo.
    let d = divs.iter().find(|d| d.axis == Axis::X).unwrap();
    let front = panel(&panels, Id::Front);
    let mut seen = 0;
    for r in &front.slots {
        if (r.x0 + fit / 2.0 - d.pos).abs() > 1e-9 {
            continue; // ranura de otro divisor
        }
        seen += 1;
        // La pestaña ocupa z desde la ranura menos la holgura; z del frente = T + z del divisor.
        let (z0, z1) = (r.y0 + fit / 2.0 - t, r.y1 - fit / 2.0 - t);
        let mid = (z0 + z1) / 2.0;
        assert!(inside(&d.cuts, -t / 2.0, mid), "pestaña ausente");
        assert!(!inside(&d.cuts, -t / 2.0, z0 - 0.01) && !inside(&d.cuts, -t / 2.0, z1 + 0.01));
        assert!(inside(&d.cuts, -t / 2.0, z0 + 0.01) && inside(&d.cuts, -t / 2.0, z1 - 0.01));
        // Y la ranura es un agujero del panel.
        assert!(!inside(&front.cuts, (r.x0 + r.x1) / 2.0, (r.y0 + r.y1) / 2.0));
    }
    assert!(seen >= 1);
}

#[test]
fn slots_stay_clear_of_notches_and_each_other() {
    for p in divider_sets() {
        let (_, panels, _) = built_all(&p);
        for pn in &panels {
            assert_eq!(crossings(&pn.cuts), 0, "{:?} se cruza", pn.id);
            let grow = |r: &Rect, g: f64| Rect::new(r.x0 - g, r.y0 - g, r.x1 + g, r.y1 + g);
            for (i, s) in pn.slots.iter().enumerate() {
                // Al menos 1 mm de material entre una ranura y cualquier muesca o ranura.
                let g = grow(s, 1.0);
                assert!(!pn.cut_rects.iter().any(|r| r.overlaps(&g)), "{:?}", pn.id);
                assert!(!pn.voids.iter().any(|r| r.overlaps(&g)), "{:?}", pn.id);
                for o in &pn.slots[i + 1..] {
                    assert!(!o.overlaps(&g), "{:?}: ranuras pegadas", pn.id);
                }
                assert!(s.x0 > 0.0 && s.y0 > 0.0 && s.x1 < pn.w && s.y1 < pn.h);
            }
        }
    }
}

#[test]
fn divider_outlines_are_simple() {
    for p in divider_sets() {
        let (_, _, divs) = built_all(&p);
        for d in &divs {
            assert_eq!(crossings(&d.cuts), 0, "{} se cruza", d.key());
            assert!(d.cuts.iter().all(|c| c.closed));
        }
    }
}

#[test]
fn loose_dividers_have_no_tabs_and_keep_the_clearance() {
    let p = BoxParams {
        dividers_x: 1,
        dividers_y: 1,
        divider_joint: DividerJoint::Loose,
        divider_clearance_mm: 0.3,
        ..base()
    };
    let (s, panels, divs) = built_all(&p);
    assert!(panels.iter().all(|pn| pn.slots.is_empty()));
    let d = divs.iter().find(|d| d.axis == Axis::X).unwrap();
    let (x0, y0, x1, y1) = bbox(&d.cuts);
    // Largo y alto de la cavidad menos el juego.
    assert!((x1 - x0 - (s.d - 6.0 - 0.3)).abs() < 1e-9, "{x0} {x1}");
    assert!((y1 - y0 - (s.h - 6.0 - 0.3)).abs() < 1e-9, "{y0} {y1}");
    assert!(x0 > 0.0 && y0 >= 0.0);
}

#[test]
fn divider_validation_reports_the_failing_field() {
    let f = |p: BoxParams| build_all(&p).err().and_then(|e| e.field);
    assert_eq!(
        f(BoxParams { dividers_x: 11, ..BoxParams::default() }).as_deref(),
        Some("dividers_x")
    );
    assert_eq!(
        f(BoxParams { dividers_y: 10, depth_mm: 40.0, ..BoxParams::default() }).as_deref(),
        Some("divider_widths_y")
    );
    let e = box_parts(&BoxParams {
        feet: Feet::Plinth,
        foot_height_mm: 60.0,
        ..BoxParams::default()
    })
    .err();
    assert_eq!(e.and_then(|e| e.field).as_deref(), Some("foot_height_mm"));
}

#[test]
fn dividers_add_one_part_each() {
    assert_eq!(box_parts(&BoxParams::default()).unwrap().parts.len(), 6);
    let parts = box_parts(&with_dividers(BoxParams::default())).unwrap().parts;
    assert_eq!(parts.len(), 6 + 3);
    let keys: Vec<&str> = parts.iter().map(|p| p.key.as_str()).collect();
    for k in ["divider_x1", "divider_x2", "divider_y1"] {
        assert!(keys.contains(&k), "{k}");
    }
}

#[test]
fn plinth_and_legs_stack_layers_of_one_thickness() {
    let base = BoxParams {
        foot_height_mm: 9.0,
        ..BoxParams::default()
    };
    let plinth = box_parts(&BoxParams { feet: Feet::Plinth, ..base.clone() }).unwrap();
    let rings: Vec<_> = plinth.parts.iter().filter(|p| p.key.starts_with("plinth")).collect();
    assert_eq!(rings.len(), 3, "9 mm de zócalo con material de 3 mm = 3 capas");
    // Marco retranqueado un grosor por lado, con un agujero.
    assert!((rings[0].width - 94.0).abs() < 1e-9 && (rings[0].height - 74.0).abs() < 1e-9);
    assert_eq!(rings[0].cuts.len(), 2);
    let legs = box_parts(&BoxParams { feet: Feet::Legs, ..base }).unwrap();
    let sets: Vec<_> = legs.parts.iter().filter(|p| p.key.starts_with("legs")).collect();
    assert_eq!(sets.len(), 3);
    assert!(sets.iter().all(|s| s.cuts.len() == 4));
    assert!(legs.warnings.iter().any(|w| w.contains("9.0")));
}
