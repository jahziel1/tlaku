use super::panels::{Id, Panel};
use super::*;
use crate::fingers::Cells;
use crate::testutil::{crossings, inside};
use tlaku_core::Polyline;
use tlaku_i18n::{Locale, with_locale};

fn built(p: &BoxParams) -> (Sizes, Vec<Panel>) {
    let (s, panels, _) = build_panels(p).expect("parámetros válidos");
    (s, panels)
}

fn panel(panels: &[Panel], id: Id) -> &Panel {
    panels.iter().find(|p| p.id == id).expect("panel")
}

/// Paneles cuya losa (rectángulo x espesor) contiene el punto 3D.
fn slabs(panels: &[Panel], p: [f64; 3]) -> Vec<Id> {
    panels
        .iter()
        .filter(|pn| {
            let d = pn.depth(p);
            let (u, v) = pn.local(p);
            d > 0.0 && d < pn.thickness && u > 0.0 && u < pn.w && v > 0.0 && v < pn.h
        })
        .map(|pn| pn.id)
        .collect()
}

/// Paneles que tienen material en el punto 3D.
fn holders(panels: &[Panel], p: [f64; 3]) -> Vec<Id> {
    slabs(panels, p)
        .into_iter()
        .filter(|&id| {
            let pn = panel(panels, id);
            let (u, v) = pn.local(p);
            inside(&pn.cuts, u, v)
        })
        .collect()
}

fn in_void(panels: &[Panel], p: [f64; 3]) -> bool {
    slabs(panels, p).into_iter().any(|id| {
        let pn = panel(panels, id);
        let (u, v) = pn.local(p);
        pn.voids
            .iter()
            .any(|r| u > r.x0 && u < r.x1 && v > r.y0 && v < r.y1)
    })
}

fn all_cuts(panels: &[Panel]) -> Vec<Vec<Polyline>> {
    panels.iter().map(|p| p.cuts.clone()).collect()
}

fn bbox(cuts: &[Polyline]) -> (f64, f64, f64, f64) {
    crate::part::bounds_of(cuts.iter()).expect("hay cortes")
}

fn param_sets() -> Vec<BoxParams> {
    let base = BoxParams::default();
    vec![
        base.clone(),
        BoxParams {
            kind: BoxKind::Open,
            ..base.clone()
        },
        BoxParams {
            kind: BoxKind::SlidingLid,
            ..base.clone()
        },
        BoxParams {
            dims: Dims::Inner,
            ..base.clone()
        },
        BoxParams {
            dims: Dims::Inner,
            kind: BoxKind::SlidingLid,
            ..base.clone()
        },
        BoxParams {
            width_mm: 57.3,
            depth_mm: 41.9,
            height_mm: 33.3,
            thickness_mm: 4.0,
            finger_mm: 7.0,
            ..base.clone()
        },
        BoxParams {
            width_mm: 250.0,
            depth_mm: 120.0,
            height_mm: 90.0,
            thickness_mm: 6.0,
            finger_mm: 18.0,
            kind: BoxKind::Open,
            ..base.clone()
        },
        BoxParams {
            thickness_mm: 1.5,
            finger_mm: 4.0,
            width_mm: 40.0,
            depth_mm: 30.0,
            height_mm: 25.0,
            ..base.clone()
        },
        BoxParams {
            fit_mm: 0.15,
            ..base.clone()
        },
        BoxParams {
            fit_mm: -0.1,
            kind: BoxKind::Open,
            ..base
        },
    ]
}

#[test]
fn panels_have_the_requested_outer_sizes() {
    let (s, panels) = built(&BoxParams::default());
    assert_eq!((s.w, s.d, s.h), (100.0, 80.0, 60.0));
    let want = [
        (Id::Front, 100.0, 60.0),
        (Id::Back, 100.0, 60.0),
        (Id::Left, 80.0, 60.0),
        (Id::Right, 80.0, 60.0),
        (Id::Bottom, 100.0, 80.0),
        (Id::Top, 100.0, 80.0),
    ];
    for (id, w, h) in want {
        let (x0, y0, x1, y1) = bbox(&panel(&panels, id).cuts);
        assert!(
            (x0.abs() + y0.abs()) < 1e-9,
            "{id:?} no empieza en el origen"
        );
        assert!(
            (x1 - w).abs() < 1e-9 && (y1 - h).abs() < 1e-9,
            "{id:?}: {x1}x{y1}"
        );
    }
    // `Part` lleva lo mismo.
    let parts = box_parts(&BoxParams::default()).unwrap().parts;
    assert_eq!(parts.len(), 6);
    let front = parts.iter().find(|p| p.key == "front").unwrap();
    assert!((front.width - 100.0).abs() < 1e-9 && (front.height - 60.0).abs() < 1e-9);
}

#[test]
fn inner_dimensions_leave_the_requested_cavity() {
    let p = BoxParams {
        width_mm: 90.0,
        depth_mm: 70.0,
        height_mm: 40.0,
        dims: Dims::Inner,
        ..BoxParams::default()
    };
    let (s, panels) = built(&p);
    assert_eq!((s.w, s.d, s.h), (96.0, 76.0, 46.0));
    // La cavidad [T, W-T] x [T, D-T] x [T, H-T] no tiene material.
    let t = 3.0;
    let mut n = 0;
    let mut x = t + 0.37;
    while x < s.w - t {
        let mut y = t + 0.41;
        while y < s.d - t {
            let mut z = t + 0.43;
            while z < s.h - t {
                assert!(
                    holders(&panels, [x, y, z]).is_empty(),
                    "material en la cavidad {x},{y},{z}"
                );
                n += 1;
                z += 2.3;
            }
            y += 2.3;
        }
        x += 2.3;
    }
    assert!(n > 1000);
}

#[test]
fn open_box_inner_height_is_the_requested_one() {
    let p = BoxParams {
        height_mm: 40.0,
        dims: Dims::Inner,
        kind: BoxKind::Open,
        ..BoxParams::default()
    };
    let (s, panels) = built(&p);
    // Un solo fondo: alto exterior = interior + T.
    assert!((s.h - 43.0).abs() < 1e-12);
    assert!(panels.iter().all(|p| p.id != Id::Top));
}

#[test]
fn sliding_lid_inner_height_is_under_the_lid() {
    let p = BoxParams {
        height_mm: 40.0,
        dims: Dims::Inner,
        kind: BoxKind::SlidingLid,
        ..BoxParams::default()
    };
    let (s, _) = built(&p);
    // El trasero llega a T + interior; la tapa corre justo encima.
    assert!((s.back_h - 43.0).abs() < 1e-12, "{}", s.back_h);
}

/// Cada punto del volumen que cae en la losa de algún panel tiene material de exactamente un
/// panel (sin huecos ni solapes), salvo las ranuras de la tapa.
#[test]
fn every_slab_point_is_owned_by_exactly_one_panel() {
    for p in param_sets().into_iter().filter(|p| p.fit_mm == 0.0) {
        let (s, panels) = built(&p);
        let step = 1.37;
        let (mut checked, mut shared) = (0usize, 0usize);
        let mut x = 0.21;
        while x < s.w {
            let mut y = 0.27;
            while y < s.d {
                let mut z = 0.33;
                while z < s.h {
                    let pt = [x, y, z];
                    let slab = slabs(&panels, pt);
                    if !slab.is_empty() {
                        let own = holders(&panels, pt);
                        if in_void(&panels, pt) {
                            // En la ranura solo puede estar la tapa.
                            assert!(
                                own.iter().all(|&i| i == Id::Lid),
                                "{p:?}: material en la ranura {pt:?}"
                            );
                        } else {
                            assert_eq!(
                                own.len(),
                                1,
                                "{p:?}: punto {pt:?} losas {slab:?} dueños {own:?}"
                            );
                        }
                        checked += 1;
                        shared += usize::from(slab.len() > 1);
                    }
                    z += step;
                }
                y += step;
            }
            x += step;
        }
        assert!(checked > 500, "{p:?}: pocos puntos");
        assert!(shared > 100, "{p:?}: no se probaron las aristas ({shared})");
    }
}

/// Con holgura distinta de cero nunca hay dos paneles en el mismo punto.
#[test]
fn no_two_panels_share_material_with_any_fit() {
    // Con holgura negativa (ajuste apretado) las piezas se solapan a propósito.
    for p in param_sets().into_iter().filter(|p| p.fit_mm >= 0.0) {
        let (s, panels) = built(&p);
        let mut x = 0.19;
        while x < s.w {
            let mut y = 0.23;
            while y < s.d {
                let mut z = 0.29;
                while z < s.h {
                    let own = holders(&panels, [x, y, z]);
                    assert!(own.len() <= 1, "{p:?}: solape en {x},{y},{z}: {own:?}");
                    z += 1.13;
                }
                y += 1.13;
            }
            x += 1.13;
        }
    }
}

#[test]
fn outlines_are_simple_polygons() {
    for p in param_sets() {
        let (_, panels) = built(&p);
        for pn in &panels {
            assert!(!pn.cuts.is_empty());
            assert_eq!(crossings(&pn.cuts), 0, "{p:?} {:?} se cruza", pn.id);
            for c in &pn.cuts {
                assert!(c.closed && c.points.len() >= 4);
            }
        }
    }
}

/// Recorre una recta del espacio y devuelve los tramos `(inicio, fin, dueños)` donde cambia
/// quién tiene material.
fn runs(
    panels: &[Panel],
    axis: usize,
    base: [f64; 3],
    from: f64,
    to: f64,
) -> Vec<(f64, f64, Vec<Id>)> {
    let step = 0.005;
    let n = ((to - from) / step) as usize;
    let mut out: Vec<(f64, f64, Vec<Id>)> = Vec::new();
    for i in 0..n {
        let c = from + (i as f64 + 0.5) * step;
        let mut pt = base;
        pt[axis] = c;
        let own = holders(panels, pt);
        match out.last_mut() {
            Some(last) if last.2 == own => last.1 = c + step / 2.0,
            _ => out.push((c - step / 2.0, c + step / 2.0, own)),
        }
    }
    out
}

#[test]
fn teeth_of_a_shared_edge_alternate_and_complement() {
    let p = BoxParams::default();
    let (_, panels) = built(&p);
    // Arista frontal-inferior (eje X, y = T/2, z = T/2): dueño por celda.
    let cells = Cells::plan(0.0, 100.0, p.finger_mm);
    let t = p.thickness_mm;
    let runs = runs(&panels, 0, [0.0, t / 2.0, t / 2.0], 0.0, 100.0);
    assert_eq!(runs.len(), cells.n, "una racha por celda: {runs:?}");
    for (k, (a, b, own)) in runs.iter().enumerate() {
        let (ca, cb) = cells.cell(k);
        assert!(
            (a - ca).abs() < 0.01 && (b - cb).abs() < 0.01,
            "celda {k}: {a}..{b} vs {ca}..{cb}"
        );
        let want = if k % 2 == 0 { Id::Front } else { Id::Bottom };
        assert_eq!(own, &vec![want], "celda {k}");
    }
    // Mismo número de dientes en cada pieza: (n+1)/2 y (n-1)/2, y simétricos.
    let front = runs.iter().filter(|r| r.2 == [Id::Front]).count();
    let bottom = runs.iter().filter(|r| r.2 == [Id::Bottom]).count();
    assert_eq!((front, bottom), (cells.even_count(), cells.odd_count()));
    for k in 0..cells.n {
        assert_eq!(
            runs[k].2,
            runs[cells.n - 1 - k].2,
            "simetría de la celda {k}"
        );
    }
}

#[test]
fn lateral_and_bottom_edges_alternate_between_the_corner_cubes() {
    let p = BoxParams::default();
    let (s, panels) = built(&p);
    let t = p.thickness_mm;
    // Arista izquierda-inferior (eje Y, x = T/2, z = T/2): de T a D - T alternan izquierdo e inferior.
    let cells = Cells::plan(t, s.d - 2.0 * t, p.finger_mm);
    let r = runs(&panels, 1, [t / 2.0, 0.0, t / 2.0], t, s.d - t);
    assert_eq!(r.len(), cells.n);
    for (k, (_, _, own)) in r.iter().enumerate() {
        let want = if k % 2 == 0 { Id::Left } else { Id::Bottom };
        assert_eq!(own, &vec![want], "celda {k}");
    }
    // Los cubos de las esquinas son del frontal y del trasero.
    let corner = holders(&panels, [t / 2.0, t / 2.0, t / 2.0]);
    assert_eq!(corner, vec![Id::Front]);
    let corner = holders(&panels, [t / 2.0, s.d - t / 2.0, t / 2.0]);
    assert_eq!(corner, vec![Id::Back]);
}

#[test]
fn notch_depth_equals_thickness() {
    for t in [1.5, 3.0, 4.7, 6.0] {
        let p = BoxParams {
            thickness_mm: t,
            finger_mm: 3.0 * t,
            width_mm: 40.0 * t / 3.0,
            depth_mm: 30.0 * t,
            height_mm: 25.0 * t,
            ..BoxParams::default()
        };
        let (_, panels) = built(&p);
        let front = panel(&panels, Id::Front);
        let cells = Cells::plan(0.0, front.w, p.finger_mm);
        // Centro de una celda impar: el frontal tiene muesca y el material empieza a z = T.
        let x = (cells.at(1) + cells.at(2)) / 2.0;
        let mut z = 0.0;
        while !inside(&front.cuts, x, z + 1e-4) && z < front.h {
            z += 0.001;
        }
        assert!((z - t).abs() < 0.002, "T={t}: la muesca mide {z}");
    }
}

#[test]
fn finger_width_adjusts_so_cells_are_equal() {
    let p = BoxParams {
        width_mm: 94.0,
        finger_mm: 10.0,
        ..BoxParams::default()
    };
    let (_, panels) = built(&p);
    let front = panel(&panels, Id::Front);
    let cells = Cells::plan(0.0, 94.0, 10.0);
    assert_eq!(cells.n % 2, 1);
    // Todos los vértices del contorno inferior caen en límites de celda.
    for pt in front
        .cuts
        .iter()
        .flat_map(|c| &c.points)
        .filter(|q| q.y.abs() < 1e-9 || (q.y - 3.0).abs() < 1e-9)
    {
        let on = (0..=cells.n).any(|k| (cells.at(k) - pt.x).abs() < 1e-9)
            || pt.x.abs() < 1e-9
            || (pt.x - 94.0).abs() < 1e-9
            || (pt.x - 3.0).abs() < 1e-9;
        assert!(on, "vértice {pt:?} fuera de la rejilla de dientes");
    }
}

#[test]
fn fit_makes_slot_wider_than_finger_by_exactly_fit() {
    let fit = 0.2;
    let p = BoxParams {
        fit_mm: fit,
        ..BoxParams::default()
    };
    let (_, panels) = built(&p);
    let t = p.thickness_mm;
    let cells = Cells::plan(0.0, 100.0, p.finger_mm);
    let c = cells.width();
    // A mitad de profundidad: los tramos con material del frontal (dientes), del inferior y los huecos.
    let r = runs(&panels, 0, [0.0, t / 2.0, t / 2.0], 0.0, 100.0);
    let width = |id: Id, k: usize| {
        let mut rs = r.iter().filter(|x| x.2 == [id]);
        let item = rs.nth(k).unwrap();
        item.1 - item.0
    };
    let gaps: Vec<f64> = r
        .iter()
        .filter(|x| x.2.is_empty())
        .map(|x| x.1 - x.0)
        .collect();
    // Dientes interiores (no de los extremos): ancho c - fit/2; huecos entre dientes: fit/2.
    assert!(
        (width(Id::Front, 1) - (c - fit / 2.0)).abs() < 0.011,
        "{}",
        width(Id::Front, 1)
    );
    assert!((width(Id::Bottom, 1) - (c - fit / 2.0)).abs() < 0.011);
    assert!(
        gaps.iter().all(|g| (g - fit / 2.0).abs() < 0.011),
        "{gaps:?}"
    );
    // Ranura del frontal = hueco + diente del inferior + hueco (tres tramos seguidos, empezando en
    // el primer hueco tras el diente 0 del frontal); menos el diente que entra = fit.
    let i = r.iter().position(|x| x.2 == [Id::Bottom]).unwrap();
    assert!(r[i - 1].2.is_empty() && r[i + 1].2.is_empty());
    let slot = r[i + 1].1 - r[i - 1].0;
    let finger = r[i].1 - r[i].0;
    assert!(
        (slot - finger - fit).abs() < 0.011,
        "ranura {slot} diente {finger}"
    );
}

#[test]
fn kerf_compensation_restores_nominal_widths() {
    // La compensación de kerf de la capa (offset positivo) ensancha los dientes y estrecha las
    // muescas por igual: con el kerf real del haz, el diente y la muesca vuelven a medir lo
    // dibujado. Aquí se comprueba el efecto sobre el contorno del frontal.
    let p = BoxParams::default();
    let (_, panels) = built(&p);
    let front = panel(&panels, Id::Front);
    let k = 0.1;
    let comp = tlaku_geom::kerf_compensate(&front.cuts, k);
    let cells = Cells::plan(0.0, front.w, p.finger_mm);
    let x = (cells.at(1) + cells.at(2)) / 2.0; // centro de una muesca
    let t = p.thickness_mm;
    // Ancho de la muesca a z = T/2 antes y después.
    let width_at = |polys: &[Polyline]| {
        let mut xl = x;
        while !inside(polys, xl - 0.001, t / 2.0) {
            xl -= 0.001;
            if xl < 0.0 {
                break;
            }
        }
        let mut xr = x;
        while !inside(polys, xr + 0.001, t / 2.0) {
            xr += 0.001;
            if xr > front.w {
                break;
            }
        }
        xr - xl
    };
    let before = width_at(&front.cuts);
    let after = width_at(&comp);
    assert!((before - cells.width()).abs() < 0.003);
    assert!(
        (after - (before - 2.0 * k)).abs() < 0.004,
        "{before} -> {after}"
    );
}

#[test]
fn sliding_lid_geometry() {
    let p = BoxParams {
        kind: BoxKind::SlidingLid,
        ..BoxParams::default()
    };
    let (s, panels) = built(&p);
    let t = p.thickness_mm;
    assert!(s.back_h < s.h);
    let back = panel(&panels, Id::Back);
    let (_, _, _, by1) = bbox(&back.cuts);
    assert!((by1 - s.back_h).abs() < 1e-9);
    // La tapa cabe por la ranura (que es un hueco pasante abierto hacia atrás) y no choca con nada.
    let lid = panel(&panels, Id::Lid);
    let mid = [lid.w / 2.0, lid.h / 2.0];
    let pt = lid.to3(mid[0], mid[1]);
    assert_eq!(
        holders(&panels, [pt[0], pt[1], pt[2] - t / 2.0]),
        vec![Id::Lid]
    );
    let left = panel(&panels, Id::Left);
    assert_eq!(left.voids.len(), 1);
    let slot = left.voids[0];
    assert!((slot.y1 - slot.y0 - (t + p.lid_clearance_mm)).abs() < 1e-12);
    assert!((slot.x1 - (s.d - 2.0 * t)).abs() < 1e-12 && slot.x0 == 0.0);
    // El contorno del lateral tiene un único tramo de borde trasero abierto: la ranura toca el borde.
    assert_eq!(left.cuts.len(), 1);
}

#[test]
fn validation_reports_the_failing_field() {
    let bad = |p: BoxParams| box_parts(&p).unwrap_err();
    assert_eq!(
        bad(BoxParams {
            thickness_mm: 0.0,
            ..BoxParams::default()
        })
        .field
        .as_deref(),
        Some("thickness_mm")
    );
    assert_eq!(
        bad(BoxParams {
            thickness_mm: f64::NAN,
            ..BoxParams::default()
        })
        .field
        .as_deref(),
        Some("thickness_mm")
    );
    assert_eq!(
        bad(BoxParams {
            fit_mm: 2.0,
            ..BoxParams::default()
        })
        .field
        .as_deref(),
        Some("fit_mm")
    );
    assert_eq!(
        bad(BoxParams {
            finger_mm: 0.5,
            ..BoxParams::default()
        })
        .field
        .as_deref(),
        Some("finger_mm")
    );
    assert_eq!(
        bad(BoxParams {
            width_mm: 8.0,
            ..BoxParams::default()
        })
        .field
        .as_deref(),
        Some("width_mm")
    );
    assert_eq!(
        bad(BoxParams {
            height_mm: 5.0,
            ..BoxParams::default()
        })
        .field
        .as_deref(),
        Some("height_mm")
    );
    // Con tapa deslizante el alto útil es menor.
    assert!(
        box_parts(&BoxParams {
            height_mm: 15.0,
            kind: BoxKind::SlidingLid,
            ..BoxParams::default()
        })
        .is_err()
    );
}

#[test]
fn messages_follow_the_active_language() {
    let p = BoxParams {
        thickness_mm: 0.0,
        ..BoxParams::default()
    };
    let es = box_parts(&p).unwrap_err().message;
    let en = with_locale(Locale::En, || box_parts(&p).unwrap_err().message);
    assert_ne!(es, en);
    assert!(es.contains("entre"), "{es}");
    assert!(en.contains("between"), "{en}");
}

#[test]
fn labels_are_engraved_inside_each_panel() {
    let p = BoxParams {
        labels: true,
        ..BoxParams::default()
    };
    let (_, panels) = built(&p);
    for pn in &panels {
        assert!(!pn.engraves.is_empty(), "{:?} sin rótulo", pn.id);
        let (x0, y0, x1, y1) = bbox(&pn.engraves);
        let t = p.thickness_mm;
        assert!(
            x0 >= t && y0 >= t && x1 <= pn.w - t && y1 <= pn.h - t,
            "{:?} rótulo fuera de la zona libre",
            pn.id
        );
        // El rótulo cae sobre material.
        assert!(inside(&pn.cuts, (x0 + x1) / 2.0, y0 - 0.5) || inside(&pn.cuts, x0, y0));
    }
    let off = built(&BoxParams::default()).1;
    assert!(off.iter().all(|p| p.engraves.is_empty()));
    let en = with_locale(Locale::En, || box_parts(&p).unwrap());
    assert!(en.warnings.iter().all(|w| !w.is_empty()));
}

#[test]
fn small_panel_skips_its_label_with_a_warning() {
    let p = BoxParams {
        labels: true,
        width_mm: 16.0,
        depth_mm: 16.0,
        height_mm: 16.0,
        thickness_mm: 3.0,
        finger_mm: 3.0,
        ..BoxParams::default()
    };
    let g = box_parts(&p).unwrap();
    assert!(
        g.warnings.iter().any(|w| w.contains("rótulo")),
        "{:?}",
        g.warnings
    );
}

#[test]
fn fit_and_thin_fingers_raise_warnings() {
    let g = box_parts(&BoxParams {
        fit_mm: 0.1,
        ..BoxParams::default()
    })
    .unwrap();
    assert!(g.warnings.iter().any(|w| w.contains("kerf")));
    let g = box_parts(&BoxParams {
        finger_mm: 2.0,
        ..BoxParams::default()
    })
    .unwrap();
    assert!(
        g.warnings.iter().any(|w| w.contains("frágiles")),
        "{:?}",
        g.warnings
    );
}

#[test]
fn old_saves_without_new_fields_still_load() {
    let p: BoxParams = serde_json::from_str(r#"{"width_mm": 50, "kind": "open"}"#).unwrap();
    assert_eq!(p.width_mm, 50.0);
    assert_eq!(p.kind, BoxKind::Open);
    assert_eq!(p.thickness_mm, 3.0);
    assert!(!p.labels);
    let _ = all_cuts(&[]);
}
