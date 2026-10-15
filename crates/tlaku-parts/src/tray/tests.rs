use super::*;
use crate::boxes::build_panels;
use crate::part::bounds_of;
use crate::testutil::{crossings, inside};

fn part<'a>(g: &'a Generated, key: &str) -> &'a Part {
    g.parts
        .iter()
        .find(|p| p.key == key)
        .unwrap_or_else(|| panic!("falta la pieza {key}"))
}

fn keys(g: &Generated) -> Vec<&str> {
    g.parts.iter().map(|p| p.key.as_str()).collect()
}

fn area(p: &Part) -> f64 {
    p.cuts.iter().map(signed_area).sum()
}

#[test]
fn default_tray_is_an_open_box() {
    let g = tray_parts(&TrayParams::default()).unwrap();
    assert_eq!(keys(&g), ["front", "back", "left", "right", "bottom"]);
    let (front, left, bottom) = (part(&g, "front"), part(&g, "left"), part(&g, "bottom"));
    assert!((front.width - 120.0).abs() < 1e-9 && (front.height - 30.0).abs() < 1e-9);
    assert!((left.width - 90.0).abs() < 1e-9 && (left.height - 30.0).abs() < 1e-9);
    assert!((bottom.width - 120.0).abs() < 1e-9 && (bottom.height - 90.0).abs() < 1e-9);
    // Los mismos contornos que la caja abierta con esas medidas.
    let boxed = crate::boxes::box_parts(&BoxParams {
        kind: crate::boxes::BoxKind::Open,
        width_mm: 120.0,
        depth_mm: 90.0,
        height_mm: 30.0,
        ..BoxParams::default()
    })
    .unwrap();
    for (a, b) in g.parts.iter().zip(&boxed.parts) {
        assert_eq!(a.cuts, b.cuts, "{}", a.key);
    }
}

#[test]
fn notch_handles_cut_two_sides_with_rounded_bottom() {
    let plain = tray_parts(&TrayParams::default()).unwrap();
    let p = TrayParams {
        handle: Handle::Notch,
        handle_width_mm: 30.0,
        handle_depth_mm: 12.0,
        ..TrayParams::default()
    };
    let g = tray_parts(&p).unwrap();
    for key in ["left", "right"] {
        let (a, b) = (part(&g, key), part(&plain, key));
        // Misma caja envolvente: la muesca no cambia las medidas.
        assert!((a.width - b.width).abs() < 1e-9 && (a.height - b.height).abs() < 1e-9);
        assert_eq!(crossings(&a.cuts), 0, "{key} se cruza");
        let cx = a.width / 2.0;
        // Dentro de la muesca no hay material; justo debajo y a los lados sí.
        assert!(!inside(&a.cuts, cx, a.height - 1.0));
        assert!(!inside(&a.cuts, cx, a.height - 11.5));
        assert!(!inside(&a.cuts, cx - 14.5, a.height - 1.0));
        assert!(inside(&a.cuts, cx, a.height - 12.5));
        assert!(inside(&a.cuts, cx - 15.5, a.height - 1.0));
        // Fondo redondeado: radio 12 < 15, así que las esquinas bajas quedan con material.
        assert!(inside(&a.cuts, cx - 14.9, a.height - 11.9));
        // Área quitada: algo menos que 30 x 12 por las esquinas redondas.
        let removed = area(b) - area(a);
        let r = 12.0_f64;
        let rect = 30.0 * 12.0 - (4.0 - std::f64::consts::PI) * r * r * 0.5;
        assert!(
            (removed - rect).abs() < 6.0,
            "{key}: quitó {removed}, esperado {rect}"
        );
    }
    // Frente, trasero y base no cambian.
    for key in ["front", "back", "bottom"] {
        assert_eq!(part(&g, key).cuts, part(&plain, key).cuts, "{key}");
    }
}

#[test]
fn hole_handles_are_closed_pills_inside_the_walls() {
    let p = TrayParams {
        handle: Handle::Hole,
        handle_width_mm: 30.0,
        handle_depth_mm: 10.0,
        ..TrayParams::default()
    };
    let g = tray_parts(&p).unwrap();
    for key in ["left", "right"] {
        let a = part(&g, key);
        assert_eq!(a.cuts.len(), 2, "contorno y ojal");
        assert_eq!(crossings(&a.cuts), 0);
        let cx = a.width / 2.0;
        // Ojal: su borde alto a 6 mm del borde superior (el mayor de 2T y 5) y 10 mm de alto.
        assert!(!inside(&a.cuts, cx, a.height - 11.0));
        assert!(inside(&a.cuts, cx, a.height - 2.0));
        assert!(inside(&a.cuts, cx, a.height - 17.0));
        assert!(!inside(&a.cuts, cx + 14.0, a.height - 11.0));
        assert!(inside(&a.cuts, cx + 15.5, a.height - 11.0));
    }
}

#[test]
fn handle_validation_names_the_field() {
    let f = |p: TrayParams| tray_parts(&p).err().and_then(|e| e.field);
    let base = TrayParams {
        handle: Handle::Notch,
        ..TrayParams::default()
    };
    assert_eq!(
        f(TrayParams {
            handle_width_mm: 90.0,
            ..base.clone()
        })
        .as_deref(),
        Some("handle_width_mm")
    );
    assert_eq!(
        f(TrayParams {
            handle_depth_mm: 40.0,
            ..base.clone()
        })
        .as_deref(),
        Some("handle_depth_mm")
    );
    assert_eq!(
        f(TrayParams {
            handle_depth_mm: 1.0,
            ..base.clone()
        })
        .as_deref(),
        Some("handle_depth_mm")
    );
    assert_eq!(
        f(TrayParams {
            thickness_mm: -1.0,
            ..base
        })
        .as_deref(),
        Some("thickness_mm")
    );
}

#[test]
fn drawer_front_covers_the_opening_and_carries_the_handle() {
    let p = TrayParams {
        kind: TrayKind::Drawer,
        front_overlap_mm: 8.0,
        handle: Handle::Hole,
        handle_width_mm: 30.0,
        handle_depth_mm: 10.0,
        ..TrayParams::default()
    };
    let g = tray_parts(&p).unwrap();
    assert_eq!(
        keys(&g),
        ["front", "back", "left", "right", "bottom", "drawer_front"]
    );
    let plate = part(&g, "drawer_front");
    assert!((plate.width - 136.0).abs() < 1e-9 && (plate.height - 46.0).abs() < 1e-9);
    // Ojal en el frente y el mismo ojal, desplazado, en la pared que queda detrás.
    // El ojal queda entero dentro de la pared: 6 mm bajo su borde (46 - 8 - 6 = 32 en el frente).
    let (cx, cy) = (68.0, 32.0 - 5.0);
    assert!(!inside(&plate.cuts, cx, cy));
    assert!(inside(&plate.cuts, cx, cy + 8.0));
    let wall = part(&g, "front");
    assert!(!inside(&wall.cuts, cx - 8.0, cy - 8.0));
    assert!(inside(&wall.cuts, cx - 8.0, cy - 8.0 + 8.0));
    assert!(g.warnings.iter().any(|w| w.contains("136.0")));
}

#[test]
fn drawer_notch_goes_through_the_front_wall_when_deeper_than_the_overhang() {
    let p = TrayParams {
        kind: TrayKind::Drawer,
        front_overlap_mm: 4.0,
        handle: Handle::Notch,
        handle_width_mm: 30.0,
        handle_depth_mm: 14.0,
        ..TrayParams::default()
    };
    let g = tray_parts(&p).unwrap();
    let plate = part(&g, "drawer_front");
    let cx = plate.width / 2.0;
    assert!(!inside(&plate.cuts, cx, plate.height - 12.0));
    assert!(inside(&plate.cuts, cx, plate.height - 15.0));
    // En la pared: baja 14 - 4 = 10 mm desde su borde.
    let wall = part(&g, "front");
    let wx = wall.width / 2.0;
    assert!(!inside(&wall.cuts, wx, wall.height - 8.0));
    assert!(inside(&wall.cuts, wx, wall.height - 11.0));
    assert_eq!(crossings(&wall.cuts), 0);
    assert_eq!(crossings(&plate.cuts), 0);
}

#[test]
fn shallow_notch_does_not_touch_the_front_wall() {
    let p = TrayParams {
        kind: TrayKind::Drawer,
        front_overlap_mm: 10.0,
        handle: Handle::Notch,
        handle_depth_mm: 8.0,
        ..TrayParams::default()
    };
    let g = tray_parts(&p).unwrap();
    let plain = tray_parts(&TrayParams::default()).unwrap();
    assert_eq!(part(&g, "front").cuts, part(&plain, "front").cuts);
}

#[test]
fn outer_case_holds_the_drawer_with_the_requested_clearance() {
    let p = TrayParams {
        kind: TrayKind::DrawerCase,
        drawer_clearance_mm: 0.6,
        front_overlap_mm: 8.0,
        ..TrayParams::default()
    };
    let g = tray_parts(&p).unwrap();
    let ks = keys(&g);
    for k in [
        "drawer_front",
        "case_bottom",
        "case_top",
        "case_left",
        "case_right",
        "case_back",
    ] {
        assert!(ks.contains(&k), "{k}");
    }
    let t = 3.0;
    let (w, d, h, c) = (120.0, 90.0, 30.0, 0.6);
    // Interior de la caja = cajón + 2 c; exterior = interior + dos grosores (y uno de fondo).
    let (cw, ch, cd) = (w + 2.0 * c + 2.0 * t, h + 2.0 * c + 2.0 * t, d + c + t);
    let back = part(&g, "case_back");
    assert!((back.width - cw).abs() < 1e-9 && (back.height - ch).abs() < 1e-9);
    for k in ["case_bottom", "case_top"] {
        let s = part(&g, k);
        assert!(
            (s.width - cw).abs() < 1e-9 && (s.height - cd).abs() < 1e-9,
            "{k}"
        );
    }
    for k in ["case_left", "case_right"] {
        let s = part(&g, k);
        assert!(
            (s.width - ch).abs() < 1e-9 && (s.height - cd).abs() < 1e-9,
            "{k}"
        );
    }
    // La cavidad de la caja es la que da la caja abierta de dientes: el hueco del cajón
    // `w x h x d` cabe con `c` por lado. Se comprueba con la geometría 3D de esa caja.
    let case = BoxParams {
        kind: crate::boxes::BoxKind::Open,
        dims: Dims::Inner,
        width_mm: w + 2.0 * c,
        depth_mm: h + 2.0 * c,
        height_mm: d + c,
        ..BoxParams::default()
    };
    let (s, _, _) = build_panels(&case).unwrap();
    assert!((s.w - 2.0 * t - (w + 2.0 * c)).abs() < 1e-9);
    assert!((s.d - 2.0 * t - (h + 2.0 * c)).abs() < 1e-9);
    assert!((s.h - t - (d + c)).abs() < 1e-9);
    assert!(g.warnings.iter().any(|m| m.contains("0.6")));
}

#[test]
fn front_that_does_not_cover_the_gap_warns() {
    let p = TrayParams {
        kind: TrayKind::DrawerCase,
        drawer_clearance_mm: 2.0,
        front_overlap_mm: 3.0,
        ..TrayParams::default()
    };
    let g = tray_parts(&p).unwrap();
    assert!(g.warnings.iter().any(|w| w.contains("2 mm")));
}

#[test]
fn inner_dimensions_give_the_requested_cavity() {
    let p = TrayParams {
        dims: Dims::Inner,
        width_mm: 100.0,
        depth_mm: 80.0,
        height_mm: 20.0,
        ..TrayParams::default()
    };
    let g = tray_parts(&p).unwrap();
    let front = part(&g, "front");
    assert!((front.width - 106.0).abs() < 1e-9 && (front.height - 23.0).abs() < 1e-9);
    let all: Vec<_> = g.parts.iter().flat_map(|p| &p.cuts).collect();
    assert!(bounds_of(all.into_iter()).is_some());
}

#[test]
fn old_saves_and_partial_parameters_still_load() {
    let r = crate::generate("tray", &serde_json::json!({"kind": "drawer"}), None, 5.0).unwrap();
    assert!(r.error.is_none());
    assert_eq!(r.parts.len(), 6);
    let r = crate::generate("tray", &serde_json::json!({"handle": "notch"}), None, 5.0).unwrap();
    assert!(r.error.is_none());
}
