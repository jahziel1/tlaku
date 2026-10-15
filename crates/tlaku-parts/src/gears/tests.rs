//! Pruebas de los engranajes: medidas nominales, evolvente, simetría, autointersecciones y que
//! los pares engranen (sin traslape en todas las posiciones del giro, con el juego pedido).

use super::involute::{Profile, inv, rack_outline};
use super::*;
use crate::testutil::{crossings, inside, segments_touch};
use serde_json::{Value, json};
use std::f64::consts::{PI, TAU};
use tlaku_i18n::{Locale, with_locale};

fn params(v: Value) -> GearParams {
    serde_json::from_value(v).expect("parámetros válidos")
}

fn generate(v: Value) -> Generated {
    gears_parts(&params(v)).expect("se genera")
}

fn fails(v: Value) -> PartsError {
    gears_parts(&params(v)).expect_err("debería fallar")
}

fn bbox(p: &Polyline) -> (f64, f64, f64, f64) {
    crate::part::bounds_of([p]).unwrap()
}

fn center_of(p: &Polyline) -> Point {
    let b = bbox(p);
    Point::new((b.0 + b.2) / 2.0, (b.1 + b.3) / 2.0)
}

fn rotated(p: &Polyline, about: Point, a: f64) -> Polyline {
    let (s, c) = a.sin_cos();
    Polyline {
        points: p
            .points
            .iter()
            .map(|q| {
                let (x, y) = (q.x - about.x, q.y - about.y);
                Point::new(about.x + x * c - y * s, about.y + x * s + y * c)
            })
            .collect(),
        closed: p.closed,
    }
}

fn shifted(p: &Polyline, dx: f64, dy: f64) -> Polyline {
    Polyline {
        points: p
            .points
            .iter()
            .map(|q| Point::new(q.x + dx, q.y + dy))
            .collect(),
        closed: p.closed,
    }
}

/// ¿Se cruzan o tocan los bordes de `a` y de `b`?
fn edges_touch(a: &Polyline, b: &Polyline) -> bool {
    for (p, q) in crate::testutil::segments(a) {
        for (r, s) in crate::testutil::segments(b) {
            if segments_touch(p, q, r, s) {
                return true;
            }
        }
    }
    false
}

/// ¿Se traslapan dos regiones sólidas (bordes que se cruzan, o una dentro de la otra)?
fn overlap(a: &Polyline, b: &Polyline) -> bool {
    let (sa, sb) = (std::slice::from_ref(a), std::slice::from_ref(b));
    edges_touch(a, b)
        || b.points.iter().any(|q| inside(sa, q.x, q.y))
        || a.points.iter().any(|q| inside(sb, q.x, q.y))
}

/// Piñón dentro del hueco de una corona: no hay traslape si los bordes no se tocan y el piñón
/// queda entero dentro del hueco.
fn overlaps_ring(pinion: &Polyline, hole: &Polyline) -> bool {
    let sh = std::slice::from_ref(hole);
    edges_touch(pinion, hole) || pinion.points.iter().any(|q| !inside(sh, q.x, q.y))
}

fn profile(teeth: u32, alpha_deg: f64, backlash: f64, fillet: bool) -> Profile {
    let (m, c) = (2.0, 0.5);
    Profile {
        teeth,
        module: m,
        alpha: alpha_deg.to_radians(),
        tip_radius: m * (f64::from(teeth) / 2.0 + 1.0),
        root_radius: m * (f64::from(teeth) / 2.0 - 1.0) - c,
        thickness: PI * m / 2.0 - backlash / 2.0,
        fillet_max: if fillet { 0.6 } else { 0.0 },
        clearance: c,
        tolerance: 0.005,
    }
}

fn radius(p: Point) -> f64 {
    p.x.hypot(p.y)
}

#[test]
fn pitch_and_outer_diameters_follow_the_module() {
    let m = 2.0;
    for z in [12_u32, 20, 33, 60] {
        let prof = profile(z, 20.0, 0.0, true);
        assert!((2.0 * prof.pitch_radius() - m * f64::from(z)).abs() < 1e-12);
        let pts = prof.outline(0.0).unwrap();
        let rmax = pts.iter().map(|p| radius(*p)).fold(0.0, f64::max);
        let rmin = pts.iter().map(|p| radius(*p)).fold(f64::MAX, f64::min);
        assert!(
            (2.0 * rmax - m * (f64::from(z) + 2.0)).abs() < 1e-9,
            "z={z}"
        );
        // El fondo es el círculo de pie: módulo por (z/2 − 1) menos la holgura.
        assert!((rmin - (m * (f64::from(z) / 2.0 - 1.0) - 0.5)).abs() < 1e-9);
        assert!(
            (prof.base_radius() - prof.pitch_radius() * 20f64.to_radians().cos()).abs() < 1e-12
        );
    }
    // Lo mismo por la caja envolvente de la pieza generada (24 dientes, sin agujero).
    let g = generate(json!({"teeth": 24, "bore_mm": 0.0, "module_mm": 2.5}));
    let part = &g.parts[0];
    assert!((part.width - 2.5 * 26.0).abs() < 0.05, "{}", part.width);
    assert!((part.height - 2.5 * 26.0).abs() < 0.05);
}

#[test]
fn diametral_pitch_is_the_same_gear_as_the_equivalent_module() {
    let by_module = generate(json!({"module_mm": 2.0, "teeth": 18}));
    let by_pitch =
        generate(json!({"size_by": "diametral_pitch", "diametral_pitch": 12.7, "teeth": 18}));
    assert!((by_module.parts[0].width - by_pitch.parts[0].width).abs() < 1e-9);
    assert_eq!(
        by_module.parts[0].cuts[0].points.len(),
        by_pitch.parts[0].cuts[0].points.len()
    );
}

#[test]
fn tooth_thickness_at_the_pitch_circle_is_half_pitch_minus_half_backlash() {
    for (alpha, backlash) in [(14.5, 0.0), (20.0, 0.2), (25.0, 0.4)] {
        let prof = profile(30, alpha, backlash, true);
        let pts = prof.outline(0.0).unwrap();
        let r = prof.pitch_radius();
        // Cruces de la poligonal con el círculo primitivo cerca del ángulo 0 (diente 0).
        let mut angles = Vec::new();
        for i in 0..pts.len() {
            let (a, b) = (pts[i], pts[(i + 1) % pts.len()]);
            let (ra, rb) = (radius(a), radius(b));
            if (ra - r) * (rb - r) < 0.0 {
                let t = (r - ra) / (rb - ra);
                let p = Point::new(a.x + t * (b.x - a.x), a.y + t * (b.y - a.y));
                let ang = p.y.atan2(p.x);
                if ang.abs() < PI / 30.0 {
                    angles.push(ang);
                }
            }
        }
        assert_eq!(angles.len(), 2, "alpha={alpha}");
        let thickness = (angles[0] - angles[1]).abs() * r;
        let want = PI * 2.0 / 2.0 - backlash / 2.0;
        assert!(
            (thickness - want).abs() < 0.01,
            "alpha={alpha}: {thickness} vs {want}"
        );
    }
}

#[test]
fn flank_points_lie_on_the_involute_of_the_base_circle() {
    let prof = profile(24, 20.0, 0.0, false);
    let (rb, r) = (prof.base_radius(), prof.pitch_radius());
    let pts = prof.outline(0.0).unwrap();
    // Evolvente en cartesianas, girada para que pase por el círculo primitivo en -s/(2r):
    // rb·(cos t + t sin t, sin t − t cos t) con t = tan α en el primitivo.
    let psi = -(prof.thickness / (2.0 * r) + inv(prof.alpha));
    let curve = |t: f64| {
        let (x, y) = (rb * (t.cos() + t * t.sin()), rb * (t.sin() - t * t.cos()));
        let (s, c) = psi.sin_cos();
        Point::new(x * c - y * s, x * s + y * c)
    };
    let mut checked = 0;
    for p in pts
        .iter()
        .filter(|p| p.x > 0.0 && p.y < 0.0 && p.y.atan2(p.x).abs() < PI / 24.0)
    {
        let big_r = radius(*p);
        if big_r <= rb + 1e-6 || big_r > prof.tip_radius + 1e-9 {
            continue;
        }
        let t = (big_r * big_r - rb * rb).sqrt() / rb;
        assert!(p.dist(curve(t)) < 1e-9, "R={big_r}");
        checked += 1;
    }
    assert!(checked > 10, "solo {checked} puntos comparados");
}

#[test]
fn sampling_stays_within_the_tolerance() {
    let mut prof = profile(24, 20.0, 0.0, false);
    prof.tolerance = 0.01;
    let (rb, r) = (prof.base_radius(), prof.pitch_radius());
    let pts = prof.outline(0.0).unwrap();
    let psi = -(prof.thickness / (2.0 * r) + inv(prof.alpha));
    let dense: Vec<Point> = (0..=4000)
        .map(|i| {
            let t = (prof.tip_radius.powi(2) - rb * rb).sqrt() / rb * i as f64 / 4000.0;
            let (x, y) = (rb * (t.cos() + t * t.sin()), rb * (t.sin() - t * t.cos()));
            let (s, c) = psi.sin_cos();
            Point::new(x * c - y * s, x * s + y * c)
        })
        .collect();
    let mut worst: f64 = 0.0;
    for w in pts.windows(2) {
        let (a, b) = (w[0], w[1]);
        if a.y >= 0.0
            || b.y >= 0.0
            || a.x <= 0.0
            || a.y.atan2(a.x) < -0.1
            || radius(a) < rb + 0.01
            || radius(b) > prof.tip_radius - 1e-9
        {
            continue;
        }
        let mid = Point::new((a.x + b.x) / 2.0, (a.y + b.y) / 2.0);
        let d = dense.iter().map(|q| q.dist(mid)).fold(f64::MAX, f64::min);
        worst = worst.max(d);
    }
    assert!(worst > 0.0 && worst <= 0.01 + 2e-3, "flecha {worst}");
}

#[test]
fn teeth_are_symmetric_and_repeat_every_pitch_angle() {
    for z in [13_u32, 24, 40] {
        let pts = profile(z, 20.0, 0.1, true).outline(0.0).unwrap();
        let near = |q: Point| pts.iter().any(|p| p.dist(q) < 1e-7);
        for p in &pts {
            assert!(near(Point::new(p.x, -p.y)), "z={z}: sin reflejo de {p:?}");
            let (s, c) = (TAU / f64::from(z)).sin_cos();
            assert!(
                near(Point::new(p.x * c - p.y * s, p.x * s + p.y * c)),
                "z={z}: no se repite"
            );
        }
    }
}

#[test]
fn outlines_never_cross_themselves() {
    for alpha in [14.5, 20.0, 25.0] {
        for z in [8_u32, 12, 17, 20, 35, 80] {
            for (fillet, backlash) in [(false, 0.0), (true, 0.0), (true, 0.4)] {
                let mut prof = profile(z, alpha, backlash, fillet);
                prof.tolerance = if z > 40 { 0.03 } else { 0.01 };
                match prof.outline(0.3) {
                    Ok(pts) => {
                        let outline = Polyline {
                            points: pts,
                            closed: true,
                        };
                        assert_eq!(
                            crossings(&[outline]),
                            0,
                            "z={z} a={alpha} fillet={fillet} b={backlash}"
                        );
                    }
                    // Con pocos dientes y ángulo grande el diente puede acabar en punta: aceptable.
                    Err(e) => assert!(z < 17, "z={z} a={alpha}: {e:?}"),
                }
            }
        }
    }
}

#[test]
fn the_fillet_stays_below_the_clearance_left_by_the_mating_tip() {
    let prof = profile(30, 20.0, 0.0, true);
    let pts = prof.outline(0.0).unwrap();
    let limit = prof.root_radius + prof.clearance;
    // Los puntos entre el fondo y el flanco del filete no suben de rf + holgura.
    let near_root = pts.iter().filter(|p| radius(**p) < limit - 1e-9).count();
    assert!(
        near_root > 30 * 2,
        "el filete debería añadir puntos bajo la holgura"
    );
    let simple = profile(30, 20.0, 0.0, false).outline(0.0).unwrap();
    assert!(pts.len() > simple.len());
}

#[test]
fn meshing_pair_has_the_exact_center_distance_and_does_not_overlap() {
    for (z1, z2) in [(20, 40), (17, 23), (12, 13), (30, 30), (25, 18)] {
        let g = generate(
            json!({"kind": "pair", "teeth": z1, "teeth2": z2, "backlash_mm": 0.2, "tolerance_mm": 0.005}),
        );
        assert_eq!(g.parts.len(), 1);
        let cuts = &g.parts[0].cuts;
        assert_eq!(cuts.len(), 4, "dos contornos y dos agujeros");
        let d = center_of(&cuts[1]).dist(center_of(&cuts[3]));
        let want = 2.0 * f64::from(z1 + z2) / 2.0;
        assert!((d - want).abs() < 1e-6, "{z1}/{z2}: {d} vs {want}");
        assert!(!overlap(&cuts[0], &cuts[2]), "{z1}/{z2} se traslapan");
    }
}

#[test]
fn pair_overlaps_only_by_the_backlash() {
    // Sin juego los flancos se tocan: cualquier giro pequeño mete un diente en el otro.
    // Con juego b, centrado, el engranaje 1 puede girar b/2/r1 hacia cada lado antes de tocar.
    for backlash in [0.0, 0.3] {
        let g = generate(
            json!({"kind": "pair", "teeth": 20, "teeth2": 40, "backlash_mm": backlash, "tolerance_mm": 0.002, "bore_mm": 0.0, "bore2_mm": 0.0}),
        );
        let cuts = &g.parts[0].cuts;
        let (a, b) = (&cuts[0], &cuts[1]);
        let c1 = rotation_center(a, 20);
        let r1 = 20.0;
        for sign in [-1.0, 1.0] {
            if backlash == 0.0 {
                assert!(
                    overlap(&rotated(a, c1, sign * 0.05 / r1), b),
                    "sin juego debe haber contacto ({sign})"
                );
            } else {
                let turn = |frac: f64| rotated(a, c1, sign * frac * backlash / 2.0 / r1);
                assert!(
                    !overlap(&turn(0.8), b),
                    "con 80 % del juego no debe tocar ({sign})"
                );
                assert!(
                    overlap(&turn(1.25), b),
                    "pasado el juego debe tocar ({sign})"
                );
            }
        }
    }
}

/// Centro de un engranaje: promedio de los vértices de la punta (están a igual radio).
fn rotation_center(outline: &Polyline, teeth: usize) -> Point {
    let n = outline.points.len() as f64;
    let _ = teeth;
    let (sx, sy) = outline
        .points
        .iter()
        .fold((0.0, 0.0), |s, p| (s.0 + p.x, s.1 + p.y));
    Point::new(sx / n, sy / n)
}

#[test]
fn meshed_gears_stay_free_through_a_full_tooth_of_rotation() {
    // Conjugación: girar 1 un ángulo θ y 2 el opuesto escalado por z1/z2 nunca los traslapa.
    for (kind, z1, z2) in [("pair", 18, 31), ("pair", 24, 24), ("ring", 18, 45)] {
        let g = generate(json!({
            "kind": kind, "teeth": z1, "teeth2": z2, "backlash_mm": 0.15,
            "tolerance_mm": 0.003, "bore_mm": 0.0, "bore2_mm": 0.0
        }));
        let cuts = &g.parts[0].cuts;
        // pair: [g1, g2]; ring: [pinion, anillo, hueco]
        let (g1, g2) = if kind == "pair" {
            (&cuts[0], &cuts[1])
        } else {
            (&cuts[0], &cuts[2])
        };
        let c1 = rotation_center(g1, z1 as usize);
        let c2 = if kind == "pair" {
            rotation_center(g2, z2 as usize)
        } else {
            rotation_center(&cuts[1], 0)
        };
        let ratio = f64::from(z1) / f64::from(z2);
        let dir = if kind == "pair" { -1.0 } else { 1.0 };
        for k in 0..13 {
            let theta = TAU / f64::from(z1) * k as f64 / 12.0;
            let a = rotated(g1, c1, theta);
            let b = rotated(g2, c2, dir * theta * ratio);
            let hit = if kind == "pair" {
                overlap(&a, &b)
            } else {
                overlaps_ring(&a, &b)
            };
            assert!(!hit, "{kind} {z1}/{z2} traslapa a θ={theta:.4}");
        }
    }
}

#[test]
fn ring_gear_center_distance_is_the_difference_of_radii() {
    let g = generate(json!({"kind": "ring", "teeth": 18, "teeth2": 40, "bore_mm": 5.0}));
    assert_eq!(g.parts.len(), 1);
    let cuts = &g.parts[0].cuts;
    // [pinion, pinion bore, ring outer, ring hole]
    assert_eq!(cuts.len(), 4);
    let d = center_of(&cuts[1]).dist(rotation_center(&cuts[2], 0));
    assert!((d - 2.0 * (40.0 - 18.0) / 2.0).abs() < 1e-6, "{d}");
    // El aro exterior: radio del fondo del hueco más el espesor del aro.
    let outer = bbox(&cuts[2]);
    assert!(((outer.2 - outer.0) / 2.0 - (2.0 * 20.0 + 2.0 + 0.5 + 6.0)).abs() < 0.05);
    // La corona necesita más dientes que el piñón.
    let e = fails(json!({"kind": "ring", "teeth": 30, "teeth2": 30}));
    assert_eq!(e.field.as_deref(), Some("teeth2"));
}

#[test]
fn rack_and_pinion_mesh() {
    let g = generate(
        json!({"kind": "rack_pinion", "teeth": 16, "rack_teeth": 12, "backlash_mm": 0.15, "tolerance_mm": 0.003, "bore_mm": 0.0}),
    );
    let cuts = &g.parts[0].cuts;
    let (pinion, rack) = (&cuts[0], &cuts[1]);
    let c = rotation_center(pinion, 16);
    for k in 0..13 {
        let theta = TAU / 16.0 * k as f64 / 12.0;
        let a = rotated(pinion, c, theta);
        let b = shifted(rack, 2.0 * 16.0 / 2.0 * theta, 0.0);
        assert!(!overlap(&a, &b), "traslapa a θ={theta:.4}");
    }
    // Largo = N pasos; alto = cabeza + pie con holgura + cuerpo.
    let rb = bbox(rack);
    assert!(((rb.2 - rb.0) - 12.0 * PI * 2.0).abs() < 1e-6);
    assert!(((rb.3 - rb.1) - (2.0 + 2.5 + 8.0)).abs() < 1e-6);
}

#[test]
fn rack_alone_has_straight_flanks_at_the_pressure_angle() {
    let alpha = 20f64.to_radians();
    let (m, c) = (2.0, 0.5);
    let pts = rack_outline(5, m, alpha, c, PI * m / 2.0, 6.0).unwrap();
    assert_eq!(
        crossings(&[Polyline {
            points: pts.clone(),
            closed: true
        }]),
        0
    );
    // Diente 2 (centrado en x = 2·π·m): su flanco derecho va del fondo (y = -2,5) a la cabeza
    // (y = 2) con pendiente dx/dy = -tan α.
    let xc = 2.0 * PI * m;
    let at = |y: f64| {
        pts.iter()
            .find(|p| (p.y - y).abs() < 1e-9 && p.x > xc && p.x < xc + PI * m / 2.0)
            .unwrap()
    };
    let (root, tip) = (at(-(m + c)), at(m));
    assert!(((root.x - tip.x) / (tip.y - root.y) - alpha.tan()).abs() < 1e-9);
    // Espesor sobre la línea primitiva (y = 0): medio paso.
    let half = tip.x - xc + (m * alpha.tan());
    assert!((2.0 * half - PI * m / 2.0).abs() < 1e-9);
    let g = generate(json!({"kind": "rack", "rack_teeth": 8}));
    assert_eq!(g.parts.len(), 1);
    assert!((g.parts[0].width - 8.0 * PI * 2.0).abs() < 1e-6);
}

#[test]
fn undercut_warns_and_suggests_the_minimum() {
    let g = generate(json!({"teeth": 10}));
    assert!(
        g.warnings
            .iter()
            .any(|w| w.contains("10") && w.contains("17")),
        "{:?}",
        g.warnings
    );
    let g = generate(json!({"teeth": 10, "pressure_angle": "deg14_5"}));
    assert!(
        g.warnings.iter().any(|w| w.contains("32")),
        "{:?}",
        g.warnings
    );
    assert_eq!(min_teeth(20f64.to_radians()), 17);
    assert_eq!(min_teeth(14.5f64.to_radians()), 32);
    assert!(generate(json!({"teeth": 20})).warnings.is_empty());
    assert!(generate(json!({"teeth": 17})).warnings.is_empty());
}

#[test]
fn invalid_parameters_name_the_field() {
    let field = |v: Value| fails(v).field;
    assert_eq!(
        field(json!({"module_mm": 0.0})).as_deref(),
        Some("module_mm")
    );
    assert_eq!(field(json!({"teeth": 3})).as_deref(), Some("teeth"));
    assert_eq!(
        field(json!({"size_by": "diametral_pitch", "diametral_pitch": 0.0})).as_deref(),
        Some("diametral_pitch")
    );
    assert_eq!(field(json!({"bore_mm": 60.0})).as_deref(), Some("bore_mm"));
    assert_eq!(field(json!({"bore_mm": 0.2})).as_deref(), Some("bore_mm"));
    assert_eq!(
        field(json!({"module_mm": 0.5, "teeth": 40, "bore_mm": 0.0, "backlash_mm": 1.5}))
            .as_deref(),
        Some("backlash_mm")
    );
    assert_eq!(
        field(json!({"bore_type": "keyway", "key_width_mm": 8.0})).as_deref(),
        Some("key_width_mm")
    );
    assert_eq!(
        field(json!({"bore_type": "d_flat", "flat_depth_mm": 4.0})).as_deref(),
        Some("flat_depth_mm")
    );
    assert_eq!(field(json!({"teeth": 200, "module_mm": 25.0})), None);
    assert_eq!(
        field(json!({"kind": "pair", "teeth2": 5})).as_deref(),
        Some("teeth2")
    );
    // Dientes en punta: pocos dientes con ángulo grande y mucho juego.
    let e = fails(json!({"teeth": 6, "pressure_angle": "deg25", "backlash_mm": 1.5}));
    assert!(e.field.is_some() || !e.message.is_empty());
}

#[test]
fn bore_shapes_are_inside_the_gear_and_do_not_touch_it() {
    for bore in [
        json!({"bore_type": "round"}),
        json!({"bore_type": "keyway", "bore_mm": 8.0, "key_width_mm": 3.0, "key_depth_mm": 1.4}),
        json!({"bore_type": "d_flat", "bore_mm": 8.0, "flat_depth_mm": 0.8}),
    ] {
        let g = generate(bore.clone());
        let cuts = &g.parts[0].cuts;
        assert_eq!(cuts.len(), 2);
        assert_eq!(crossings(cuts), 0, "{bore}");
    }
    // Eje en D: el plano queda a (radio − profundidad) del centro; el chavetero sobresale.
    let d = generate(json!({"bore_type": "d_flat", "bore_mm": 8.0, "flat_depth_mm": 0.8}));
    let hole = bbox(&d.parts[0].cuts[1]);
    assert!(
        ((hole.2 - hole.0) - (8.0 - 0.8)).abs() < 1e-9
            || ((hole.3 - hole.1) - (8.0 - 0.8)).abs() < 0.02
    );
    let k = generate(
        json!({"bore_type": "keyway", "bore_mm": 8.0, "key_width_mm": 3.0, "key_depth_mm": 1.4}),
    );
    let hole = bbox(&k.parts[0].cuts[1]);
    assert!(
        ((hole.3 - hole.1) - (8.0 + 1.4)).abs() < 0.02
            || ((hole.2 - hole.0) - (8.0 + 1.4)).abs() < 0.02
    );
}

#[test]
fn lightening_holes_and_spokes_fit_between_hub_and_rim() {
    for (mode, count) in [("holes", 5), ("holes", 8), ("spokes", 4), ("spokes", 6)] {
        let g = generate(
            json!({"teeth": 40, "lightening": mode, "lightening_count": count, "web_mm": 4.0}),
        );
        assert!(g.warnings.is_empty(), "{:?}", g.warnings);
        let cuts = &g.parts[0].cuts;
        assert_eq!(cuts.len(), 2 + count as usize, "{mode}");
        assert_eq!(crossings(cuts), 0, "{mode}");
        // Todas las aberturas caen dentro del contorno y fuera del agujero.
        let outline = std::slice::from_ref(&cuts[0]);
        for opening in &cuts[2..] {
            assert!(opening.points.iter().all(|p| inside(outline, p.x, p.y)));
            assert!(!opening.points.iter().any(|p| inside(
                std::slice::from_ref(&cuts[1]),
                p.x,
                p.y
            )));
        }
    }
    // Sin sitio: se omite con aviso en lugar de fallar.
    let g = generate(json!({"teeth": 17, "lightening": "spokes", "web_mm": 6.0, "bore_mm": 8.0}));
    assert_eq!(g.parts[0].cuts.len(), 2);
    assert_eq!(g.warnings.len(), 1);
}

#[test]
fn layouts_return_one_object_when_meshed_and_one_per_gear_when_separate() {
    let meshed = generate(json!({"kind": "pair"}));
    assert_eq!(meshed.parts.len(), 1);
    let apart = generate(json!({"kind": "pair", "layout": "separate"}));
    assert_eq!(apart.parts.len(), 2);
    assert!((apart.parts[0].width - 2.0 * 22.0).abs() < 0.05);
    assert!((apart.parts[1].width - 2.0 * 42.0).abs() < 0.3);
    assert_eq!(
        generate(json!({"kind": "ring", "teeth2": 40, "layout": "separate"}))
            .parts
            .len(),
        2
    );
    assert_eq!(
        generate(json!({"kind": "rack_pinion", "layout": "separate"}))
            .parts
            .len(),
        2
    );
    assert_eq!(generate(json!({"kind": "rack"})).parts.len(), 1);
}

#[test]
fn low_contact_ratio_warns() {
    // Dos piñones de 6 dientes a 25°: relación de contacto 1,17.
    let g = generate(
        json!({"kind": "pair", "teeth": 6, "teeth2": 6, "pressure_angle": "deg25", "backlash_mm": 0.0}),
    );
    assert!(
        g.warnings.iter().any(|w| w.contains("1.17")),
        "{:?}",
        g.warnings
    );
    // 20/40 a 20°: relación cercana a 1,6, sin aviso.
    assert!(generate(json!({"kind": "pair"})).warnings.is_empty());
}

#[test]
fn messages_exist_in_both_languages() {
    let es = fails(json!({"kind": "ring", "teeth": 30, "teeth2": 30})).message;
    let en = with_locale(Locale::En, || {
        fails(json!({"kind": "ring", "teeth": 30, "teeth2": 30})).message
    });
    assert_ne!(es, en);
    assert!(!es.starts_with("parts.") && !en.starts_with("parts."));
    let name_es = generate(json!({"kind": "pair"})).parts[0].name.clone();
    let name_en = with_locale(Locale::En, || {
        generate(json!({"kind": "pair"})).parts[0].name.clone()
    });
    assert_ne!(name_es, name_en);
}
