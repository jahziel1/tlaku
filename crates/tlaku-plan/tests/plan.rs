//! Pruebas de comportamiento del planificador sobre la API pública.

use tlaku_core::{Point, Polyline};
use tlaku_plan::{Blocked, Level, Plan, PlanItem, PlanOptions, Reason, Warning, plan};

struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> f64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() * n as f64) as usize % n
    }
}

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn square(x: f64, y: f64, s: f64) -> Polyline {
    Polyline {
        points: vec![pt(x, y), pt(x + s, y), pt(x + s, y + s), pt(x, y + s)],
        closed: true,
    }
}

fn circle(cx: f64, cy: f64, r: f64, n: usize) -> Polyline {
    Polyline {
        points: (0..n)
            .map(|i| {
                let a = std::f64::consts::TAU * i as f64 / n as f64;
                pt(cx + r * a.cos(), cy + r * a.sin())
            })
            .collect(),
        closed: true,
    }
}

fn line(a: (f64, f64), b: (f64, f64)) -> Polyline {
    Polyline {
        points: vec![pt(a.0, a.1), pt(b.0, b.1)],
        closed: false,
    }
}

fn item(path: Polyline) -> PlanItem {
    PlanItem {
        path,
        layer_order: 0,
        group: None,
        priority: 0,
    }
}

fn layered(path: Polyline, layer: usize) -> PlanItem {
    PlanItem {
        layer_order: layer,
        ..item(path)
    }
}

fn order(p: &Plan) -> Vec<usize> {
    p.steps.iter().map(|s| s.item).collect()
}

fn opts() -> PlanOptions {
    PlanOptions::default()
}

fn pos_of(p: &Plan, item: usize) -> usize {
    p.steps.iter().position(|s| s.item == item).unwrap()
}

// ---- comprobaciones generales ----

fn same_points(a: &Polyline, b: &Polyline) -> bool {
    let key = |p: &Point| (p.x.to_bits(), p.y.to_bits());
    let mut x: Vec<_> = a.points.iter().map(key).collect();
    let mut y: Vec<_> = b.points.iter().map(key).collect();
    x.sort_unstable();
    y.sort_unstable();
    a.closed == b.closed && x == y
}

/// El plan es una permutación, las trayectorias son rotaciones/inversiones de las originales
/// y las cifras de recorrido coinciden con un recálculo independiente.
fn check_consistency(items: &[PlanItem], o: &PlanOptions, p: &Plan) {
    let mut seen = vec![false; items.len()];
    let mut pos = o.start;
    let mut travel = 0.0;
    for s in &p.steps {
        assert!(!seen[s.item], "figura repetida");
        seen[s.item] = true;
        assert!(same_points(&s.path, &items[s.item].path));
        if items[s.item].path.closed && !o.choose_start_point {
            assert_eq!(s.path.points, items[s.item].path.points);
        }
        if !items[s.item].path.closed && !o.choose_direction {
            assert_eq!(s.path.points, items[s.item].path.points);
        }
        travel += pos.dist(s.path.start());
        pos = s.path.end();
    }
    assert!(seen.iter().all(|&b| b));
    assert!((travel - p.travel_mm).abs() < 1e-6 * (1.0 + travel));
    // La referencia es el orden de entrada hecho válido: el plan nunca es más largo.
    assert!(p.travel_mm <= p.baseline_travel_mm + 1e-9);
}

fn inside_poly(p: Point, poly: &[Point]) -> bool {
    let mut c = false;
    let mut j = poly.len() - 1;
    for i in 0..poly.len() {
        let (a, b) = (poly[j], poly[i]);
        if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
            c = !c;
        }
        j = i;
    }
    c
}

/// Contención por fuerza bruta (formas estrictamente separadas) para las pruebas.
fn brute_contains(outer: &Polyline, inner: &Polyline) -> bool {
    outer.closed && inner.points.iter().all(|&p| inside_poly(p, &outer.points))
}

fn check_inner_first(items: &[PlanItem], p: &Plan) {
    for (a, ia) in items.iter().enumerate() {
        for (b, ib) in items.iter().enumerate() {
            if a != b
                && ia.layer_order == ib.layer_order
                && brute_contains(&ia.path, &ib.path)
                && !brute_contains(&ib.path, &ia.path)
            {
                assert!(
                    pos_of(p, b) < pos_of(p, a),
                    "la figura {b} debía ir antes que su contenedor {a}"
                );
            }
        }
    }
}

// ---- casos ----

#[test]
fn empty_input_gives_empty_plan() {
    let p = plan(&[], &opts());
    assert!(p.steps.is_empty());
    assert_eq!(p.travel_mm, 0.0);
    assert_eq!(p.baseline_travel_mm, 0.0);
    assert!(p.warnings.is_empty());
    assert_eq!(p.saved_pct(), 0.0);
}

#[test]
fn single_item_is_trivial() {
    let items = vec![item(line((5.0, 0.0), (9.0, 0.0)))];
    let p = plan(&items, &opts());
    assert_eq!(order(&p), vec![0]);
    assert!((p.travel_mm - 5.0).abs() < 1e-12);
}

#[test]
fn inner_first_even_when_outer_is_closer() {
    let items = vec![
        item(square(0.0, 0.0, 100.0)),
        item(square(40.0, 40.0, 20.0)),
    ];
    let o = PlanOptions {
        start: pt(-5.0, -5.0),
        ..opts()
    };
    let p = plan(&items, &o);
    assert_eq!(order(&p), vec![1, 0]);
    // El primer paso lo explica el nivel (capa); el segundo, la cercanía.
    assert!(matches!(
        p.steps[1].reason,
        Reason::Nearest { blocked: None, .. }
    ));
    // Sin niveles activos se ve la razón de interior primero.
    let o2 = PlanOptions {
        order_by_layer: false,
        order_by_priority: false,
        ..o.clone()
    };
    let p2 = plan(&items, &o2);
    assert_eq!(order(&p2), vec![1, 0]);
    assert_eq!(p2.steps[0].reason, Reason::InnerFirst { container: 0 });
    // Sin la regla, el exterior (más cercano) iría primero.
    let free = plan(
        &items,
        &PlanOptions {
            inner_first: false,
            ..o.clone()
        },
    );
    assert_eq!(order(&free), vec![0, 1]);
    check_consistency(&items, &o, &p);
}

#[test]
fn three_nested_squares_cut_from_the_inside_out() {
    let items = vec![
        item(square(0.0, 0.0, 100.0)),
        item(square(10.0, 10.0, 80.0)),
        item(square(20.0, 20.0, 60.0)),
        item(square(30.0, 30.0, 40.0)),
    ];
    for passes in [0, 3] {
        let o = PlanOptions {
            start: pt(-1.0, -1.0),
            improve_passes: passes,
            ..opts()
        };
        let p = plan(&items, &o);
        assert_eq!(order(&p), vec![3, 2, 1, 0], "passes={passes}");
    }
}

#[test]
fn letter_o_hole_before_outer_ring() {
    // Dos "O": anillo exterior + agujero.
    let items = vec![
        item(circle(20.0, 20.0, 15.0, 48)),
        item(circle(20.0, 20.0, 8.0, 48)),
        item(circle(70.0, 20.0, 15.0, 48)),
        item(circle(70.0, 20.0, 8.0, 48)),
    ];
    let o = PlanOptions {
        start: pt(0.0, 20.0),
        ..opts()
    };
    let p = plan(&items, &o);
    assert!(pos_of(&p, 1) < pos_of(&p, 0));
    assert!(pos_of(&p, 3) < pos_of(&p, 2));
    check_inner_first(&items, &p);
    check_consistency(&items, &o, &p);
    // Sin la regla el orden sería otro (el anillo exterior está más cerca del inicio).
    let free = plan(
        &items,
        &PlanOptions {
            inner_first: false,
            ..o.clone()
        },
    );
    assert_eq!(order(&free)[0], 0);
}

#[test]
fn letter_b_two_holes_and_island_in_hole() {
    // Exterior, dos agujeros y una isla dentro del agujero 2.
    let items = vec![
        item(square(0.0, 0.0, 100.0)),
        item(square(10.0, 10.0, 30.0)),
        item(square(50.0, 10.0, 40.0)),
        item(square(60.0, 20.0, 10.0)),
    ];
    let p = plan(&items, &opts());
    check_inner_first(&items, &p);
    assert!(pos_of(&p, 3) < pos_of(&p, 2));
    assert!(pos_of(&p, 2) < pos_of(&p, 0));
    assert!(pos_of(&p, 1) < pos_of(&p, 0));
}

#[test]
fn open_path_inside_closed_goes_first() {
    let items = vec![
        item(square(0.0, 0.0, 100.0)),
        item(line((40.0, 40.0), (60.0, 60.0))),
    ];
    let o = PlanOptions {
        start: pt(-1.0, -1.0),
        order_by_layer: false,
        order_by_priority: false,
        ..opts()
    };
    let p = plan(&items, &o);
    assert_eq!(order(&p), vec![1, 0]);
    assert_eq!(p.steps[0].reason, Reason::InnerFirst { container: 0 });
}

#[test]
fn open_container_does_not_constrain() {
    // Un trazo abierto con forma de "U" no contiene nada aunque lo rodee.
    let u = Polyline {
        points: vec![
            pt(0.0, 0.0),
            pt(0.0, 100.0),
            pt(100.0, 100.0),
            pt(100.0, 0.0),
        ],
        closed: false,
    };
    let items = vec![item(u), item(square(40.0, 40.0, 10.0))];
    let o = PlanOptions {
        start: pt(0.0, 0.0),
        ..opts()
    };
    let p = plan(&items, &o);
    assert_eq!(order(&p)[0], 0);
    assert!(p.warnings.is_empty());
}

#[test]
fn layers_are_respected_over_distance() {
    let items = vec![
        layered(line((0.0, 0.0), (1.0, 0.0)), 1),
        layered(line((500.0, 0.0), (501.0, 0.0)), 0),
        layered(line((2.0, 0.0), (3.0, 0.0)), 1),
        layered(line((510.0, 0.0), (511.0, 0.0)), 0),
    ];
    let o = PlanOptions {
        improve_passes: 3,
        ..opts()
    };
    let p = plan(&items, &o);
    let layers: Vec<usize> = p.steps.iter().map(|s| items[s.item].layer_order).collect();
    assert_eq!(layers, vec![0, 0, 1, 1]);
    assert_eq!(
        p.steps[0].reason,
        Reason::FirstOfLayer {
            layer: 0,
            blocked: None
        }
    );
    assert_eq!(
        p.steps[2].reason,
        Reason::FirstOfLayer {
            layer: 1,
            blocked: None
        }
    );
    // Sin orden por capa gana la cercanía.
    let free = plan(
        &items,
        &PlanOptions {
            order_by_layer: false,
            order_by_priority: false,
            ..o
        },
    );
    assert_eq!(items[free.steps[0].item].layer_order, 1);
}

#[test]
fn hierarchy_is_layer_then_group_then_priority() {
    let mk = |layer, group, priority, x: f64| PlanItem {
        path: line((x, 0.0), (x + 1.0, 0.0)),
        layer_order: layer,
        group,
        priority,
    };
    let items = vec![
        mk(1, Some(0), 0, 0.0),
        mk(0, Some(1), 5, 10.0),
        mk(0, Some(1), 1, 20.0),
        mk(0, Some(0), 9, 30.0),
        mk(0, None, 9, 40.0),
    ];
    let o = PlanOptions {
        order_by_group: true,
        ..opts()
    };
    let p = plan(&items, &o);
    // capa 0: sin grupo, grupo 0, grupo 1 (prioridad 1 antes que 5); luego capa 1.
    assert_eq!(order(&p), vec![4, 3, 2, 1, 0]);
    assert_eq!(
        p.steps[0].reason,
        Reason::FirstOfLayer {
            layer: 0,
            blocked: None
        }
    );
    assert_eq!(
        p.steps[1].reason,
        Reason::FirstOfGroup {
            group: Some(0),
            blocked: None
        }
    );
    assert_eq!(
        p.steps[2].reason,
        Reason::FirstOfGroup {
            group: Some(1),
            blocked: None
        }
    );
    assert_eq!(
        p.steps[3].reason,
        Reason::FirstOfPriority {
            priority: 5,
            blocked: None
        }
    );
    assert_eq!(
        p.steps[4].reason,
        Reason::FirstOfLayer {
            layer: 1,
            blocked: None
        }
    );
}

#[test]
fn priority_only_when_enabled() {
    let mk = |priority, x: f64| PlanItem {
        priority,
        ..item(line((x, 0.0), (x + 1.0, 0.0)))
    };
    let items = vec![mk(2, 0.0), mk(1, 100.0)];
    let on = plan(&items, &opts());
    assert_eq!(order(&on), vec![1, 0]);
    assert_eq!(
        on.steps[0].reason,
        Reason::FirstOfLayer {
            layer: 0,
            blocked: None
        }
    );
    assert_eq!(
        on.steps[1].reason,
        Reason::FirstOfPriority {
            priority: 2,
            blocked: None
        }
    );
    let off = plan(
        &items,
        &PlanOptions {
            order_by_priority: false,
            ..opts()
        },
    );
    assert_eq!(order(&off), vec![0, 1]);
}

#[test]
fn level_overrides_inner_first_and_says_so() {
    // El exterior está en la capa 0 y el interior en la capa 1: gana la capa.
    let items = vec![
        layered(square(0.0, 0.0, 100.0), 0),
        layered(square(40.0, 40.0, 10.0), 1),
    ];
    let p = plan(&items, &opts());
    assert_eq!(order(&p), vec![0, 1]);
    assert_eq!(
        p.steps[0].reason,
        Reason::LevelOverridesInner {
            inner: 1,
            level: Level::Layer
        }
    );
    assert_eq!(
        p.warnings,
        vec![Warning::InnerOverriddenByLevel {
            inner: 1,
            container: 0,
            level: Level::Layer
        }]
    );
    let text = p.steps[0].reason.to_string();
    assert!(text.contains("capa") && text.contains("#1"), "{text}");
}

#[test]
fn chain_across_buckets_keeps_transitive_order() {
    // C contiene B contiene A; B está en otra capa. A y C comparten capa: A antes que C.
    let items = vec![
        layered(square(40.0, 40.0, 10.0), 0), // A
        layered(square(20.0, 20.0, 60.0), 1), // B
        layered(square(0.0, 0.0, 100.0), 0),  // C
    ];
    let p = plan(&items, &opts());
    assert_eq!(order(&p), vec![0, 2, 1]);
}

#[test]
fn reduce_travel_does_not_break_inner_first() {
    // Interior lejos del inicio y muchos exteriores cerca: nunca se corta un exterior antes.
    let mut items = vec![
        item(square(0.0, 0.0, 200.0)),
        item(square(150.0, 150.0, 10.0)),
    ];
    for i in 0..10 {
        items.push(item(square(10.0 + 3.0 * i as f64, 10.0, 2.0)));
    }
    let p = plan(&items, &opts());
    assert!(pos_of(&p, 1) < pos_of(&p, 0));
    check_inner_first(&items, &p);
}

#[test]
fn reduce_travel_off_keeps_input_order_with_reasons() {
    let items = vec![
        item(line((50.0, 0.0), (51.0, 0.0))),
        item(line((0.0, 0.0), (1.0, 0.0))),
        item(line((20.0, 0.0), (21.0, 0.0))),
    ];
    let o = PlanOptions {
        reduce_travel: false,
        order_by_layer: false,
        order_by_priority: false,
        ..opts()
    };
    let p = plan(&items, &o);
    assert_eq!(order(&p), vec![0, 1, 2]);
    assert!(
        p.steps
            .iter()
            .all(|s| s.reason == Reason::InputOrder { blocked: None })
    );
    assert!((p.travel_mm - p.baseline_travel_mm).abs() < 1e-9);
}

#[test]
fn reduce_travel_off_still_forces_inner_first() {
    let items = vec![
        item(square(0.0, 0.0, 100.0)),
        item(square(40.0, 40.0, 10.0)),
    ];
    let o = PlanOptions {
        reduce_travel: false,
        order_by_layer: false,
        order_by_priority: false,
        ..opts()
    };
    let p = plan(&items, &o);
    assert_eq!(order(&p), vec![1, 0]);
    assert_eq!(p.steps[0].reason, Reason::InnerFirst { container: 0 });
    assert_eq!(p.steps[1].reason, Reason::InputOrder { blocked: None });
}

#[test]
fn direction_and_start_point_options() {
    let items = vec![
        item(line((10.0, 0.0), (1.0, 0.0))),
        item(square(20.0, 0.0, 5.0)),
    ];
    let p = plan(&items, &opts());
    assert_eq!(p.steps[0].path.start(), pt(1.0, 0.0));
    let o = PlanOptions {
        choose_direction: false,
        choose_start_point: false,
        ..opts()
    };
    let q = plan(&items, &o);
    assert_eq!(q.steps[0].path.start(), pt(10.0, 0.0));
    assert_eq!(q.steps[1].path.points, items[1].path.points);
    check_consistency(&items, &o, &q);
    // Con elección de inicio, la cerrada entra por el vértice más cercano.
    let r = plan(
        &[item(square(20.0, 0.0, 5.0))],
        &PlanOptions {
            start: pt(30.0, 10.0),
            ..opts()
        },
    );
    assert_eq!(r.steps[0].path.start(), pt(25.0, 5.0));
    assert_eq!(r.steps[0].path.points.len(), 4);
}

#[test]
fn messy_grid_is_much_shorter_than_input_order() {
    // Rejilla 20x20 de segmentos cortos en orden aleatorio.
    let mut cells: Vec<(usize, usize)> =
        (0..20).flat_map(|x| (0..20).map(move |y| (x, y))).collect();
    let mut r = Lcg(42);
    for i in (1..cells.len()).rev() {
        cells.swap(i, r.below(i + 1));
    }
    let items: Vec<PlanItem> = cells
        .iter()
        .map(|&(x, y)| {
            let (x, y) = (x as f64 * 10.0, y as f64 * 10.0);
            item(line((x, y), (x + 3.0, y)))
        })
        .collect();
    let o = opts();
    let p = plan(&items, &o);
    check_consistency(&items, &o, &p);
    assert!(
        p.travel_mm < 0.2 * p.baseline_travel_mm,
        "{} vs {}",
        p.travel_mm,
        p.baseline_travel_mm
    );
    assert!(p.saved_pct() > 80.0);
    let nn = plan(
        &items,
        &PlanOptions {
            improve_passes: 0,
            ..o.clone()
        },
    );
    assert!(
        p.travel_mm <= nn.travel_mm + 1e-9,
        "{} > {}",
        p.travel_mm,
        nn.travel_mm
    );
}

#[test]
fn improvement_strictly_helps_on_random_clouds() {
    let mut r = Lcg(5);
    let mut gains = 0;
    for _ in 0..10 {
        let items: Vec<PlanItem> = (0..300)
            .map(|_| {
                let (x, y) = (r.next() * 500.0, r.next() * 500.0);
                if r.below(2) == 0 {
                    item(square(x, y, 2.0))
                } else {
                    item(line((x, y), (x + r.next() * 8.0, y + r.next() * 8.0)))
                }
            })
            .collect();
        let nn = plan(
            &items,
            &PlanOptions {
                improve_passes: 0,
                ..opts()
            },
        );
        let imp = plan(
            &items,
            &PlanOptions {
                improve_passes: 5,
                ..opts()
            },
        );
        check_consistency(&items, &opts(), &imp);
        assert!(imp.travel_mm <= nn.travel_mm + 1e-6);
        if imp.travel_mm < nn.travel_mm - 1e-6 {
            gains += 1;
        }
    }
    assert!(gains >= 8, "la mejora solo ayudó en {gains}/10");
}

#[test]
fn explanations_match_what_happened_without_improvement() {
    // Sin mejora ni contenciones, cada paso es el más cercano de los pendientes.
    let mut r = Lcg(77);
    let items: Vec<PlanItem> = (0..60)
        .map(|_| {
            let (x, y) = (r.next() * 200.0, r.next() * 200.0);
            item(line((x, y), (x + 5.0, y + 1.0)))
        })
        .collect();
    let o = PlanOptions {
        improve_passes: 0,
        ..opts()
    };
    let p = plan(&items, &o);
    let mut pos = o.start;
    let mut left: Vec<usize> = (0..items.len()).collect();
    for (k, s) in p.steps.iter().enumerate() {
        let near = left
            .iter()
            .map(|&i| {
                let (a, b) = (items[i].path.start(), items[i].path.end());
                pos.dist(a).min(pos.dist(b))
            })
            .fold(f64::INFINITY, f64::min);
        let jump = pos.dist(s.path.start());
        assert!((jump - near).abs() < 1e-9, "paso {k} no es el más cercano");
        match &s.reason {
            // El primer paso lo explica el nivel (capa).
            Reason::FirstOfLayer { .. } if k == 0 => {}
            Reason::Nearest {
                distance_mm,
                blocked: None,
            } => assert!((distance_mm - jump).abs() < 1e-9),
            other => panic!("razón inesperada en el paso {k}: {other}"),
        }
        left.retain(|&i| i != s.item);
        pos = s.path.end();
    }
}

#[test]
fn inner_first_reason_when_closest_item_is_blocked_container() {
    // Exterior cerca del inicio, interior lejos y un trazo suelto aún más cerca que ambos.
    let items = vec![
        item(square(0.0, 0.0, 100.0)),              // 0 exterior
        item(square(70.0, 70.0, 10.0)),             // 1 interior
        item(line((-20.0, -20.0), (-22.0, -20.0))), // 2 suelto, el más cercano
    ];
    let o = PlanOptions {
        start: pt(-19.0, -19.0),
        order_by_layer: false,
        order_by_priority: false,
        ..opts()
    };
    let p = plan(&items, &o);
    assert_eq!(order(&p), vec![2, 1, 0]);
    assert_eq!(p.steps[1].reason, Reason::InnerFirst { container: 0 });
}

#[test]
fn nearest_with_blocked_neighbor_that_is_not_its_container() {
    // El más cercano en general (0, exterior) espera; el elegido (2) no está dentro de él.
    let items = vec![
        item(square(0.0, 0.0, 100.0)),
        item(square(70.0, 70.0, 10.0)),
        item(line((-30.0, -10.0), (-32.0, -10.0))),
    ];
    let o = PlanOptions {
        start: pt(-10.0, -10.0),
        order_by_layer: false,
        order_by_priority: false,
        ..opts()
    };
    let p = plan(&items, &o);
    // El exterior (a 14 mm) debe esperar al interior (lejos); el trazo (20 mm) es el
    // más cercano de los permitidos.
    assert_eq!(order(&p)[0], 2);
    assert!(matches!(
        p.steps[0].reason,
        Reason::Nearest {
            blocked: Some(0),
            ..
        }
    ));
}

/// Rectángulo redondeado de `w` x `h` con esquina en (`x`, `y`) y radio `r`.
fn rounded_rect(x: f64, y: f64, w: f64, h: f64, r: f64) -> Polyline {
    let mut points = Vec::new();
    let corners = [
        (x + w - r, y + r, -90.0),
        (x + w - r, y + h - r, 0.0),
        (x + r, y + h - r, 90.0),
        (x + r, y + r, 180.0),
    ];
    for (cx, cy, start) in corners {
        for k in 0..=6 {
            let a = (start + 15.0 * k as f64).to_radians();
            points.push(pt(cx + r * a.cos(), cy + r * a.sin()));
        }
    }
    Polyline {
        points,
        closed: true,
    }
}

#[test]
fn first_of_layer_keeps_inner_first_when_nearest_is_blocked() {
    // Caso real: desde (0,0) el contorno (vértice en (2,8)) está más cerca que el círculo
    // interior, pero debe esperarlo. El círculo abre la capa y la razón debe decir ambas cosas.
    let items = vec![
        item(rounded_rect(2.0, 2.0, 76.0, 36.0, 6.0)),
        item(circle(10.0, 20.0, 3.0, 24)),
    ];
    let p = plan(&items, &opts());
    assert_eq!(order(&p), vec![1, 0]);
    assert_eq!(
        p.steps[0].reason,
        Reason::FirstOfLayer {
            layer: 0,
            blocked: Some(Blocked {
                item: 0,
                contains_this: true,
                by_overlap: false,
            })
        }
    );
    let text = p.steps[0].reason.to_string();
    assert!(text.contains("primera figura de capa 0"), "{text}");
    assert!(text.contains("#0"), "{text}");
    assert!(text.contains("interiores se cortan primero"), "{text}");
    // Sin figura saltada, el texto sigue siendo el de siempre.
    let solo = plan(&items[1..], &opts());
    assert!(
        solo.steps[0]
            .reason
            .to_string()
            .contains("las capas se cortan")
    );
}

#[test]
fn first_of_layer_keeps_inner_first_without_reduce_travel() {
    // Sin recorrido corto, la figura 0 iría primero en el orden original.
    let items = vec![
        item(rounded_rect(2.0, 2.0, 76.0, 36.0, 6.0)),
        item(circle(10.0, 20.0, 3.0, 24)),
    ];
    let o = PlanOptions {
        reduce_travel: false,
        ..opts()
    };
    let p = plan(&items, &o);
    assert_eq!(order(&p), vec![1, 0]);
    assert!(matches!(
        p.steps[0].reason,
        Reason::FirstOfLayer {
            blocked: Some(Blocked {
                item: 0,
                contains_this: true,
                by_overlap: false,
            }),
            ..
        }
    ));
}

#[test]
fn first_of_second_bucket_keeps_inner_first() {
    // Capa 0: un cuadrito lejos. Capa 1: el mismo caso del contorno con círculo interior;
    // el cabezal queda en la capa 0, así que la capa 1 también abre con un bloqueo.
    let items = vec![
        layered(square(100.0, 100.0, 2.0), 0),
        layered(rounded_rect(2.0, 2.0, 76.0, 36.0, 6.0), 1),
        layered(circle(60.0, 30.0, 3.0, 24), 1),
        layered(circle(10.0, 20.0, 3.0, 24), 1),
    ];
    let o = PlanOptions {
        start: pt(98.0, 98.0),
        ..opts()
    };
    let p = plan(&items, &o);
    assert_eq!(p.steps[0].item, 0);
    assert_eq!(
        p.steps[0].reason,
        Reason::FirstOfLayer {
            layer: 0,
            blocked: None
        }
    );
    // Desde (100,100) el contorno (vértice cercano a (78,32)) es lo más cerca, pero espera.
    let Reason::FirstOfLayer {
        layer: 1,
        blocked: Some(b),
    } = p.steps[1].reason
    else {
        panic!("razón inesperada: {}", p.steps[1].reason);
    };
    assert_eq!(b.item, 1);
    assert!(b.contains_this);
    assert!(p.steps[1].reason.to_string().contains("capa 1"));
    // Los pasos siguientes no repiten la frase de capa.
    assert!(!matches!(p.steps[2].reason, Reason::FirstOfLayer { .. }));
}

#[test]
fn first_of_layer_notes_unrelated_blocked_neighbor() {
    // La más cercana (0) espera a su interior (1); la elegida (2) no está dentro de ella.
    let items = vec![
        item(square(0.0, 0.0, 100.0)),
        item(square(70.0, 70.0, 10.0)),
        item(line((-30.0, -10.0), (-32.0, -10.0))),
    ];
    let o = PlanOptions {
        start: pt(-10.0, -10.0),
        order_by_priority: false,
        ..opts()
    };
    let p = plan(&items, &o);
    assert_eq!(
        p.steps[0].reason,
        Reason::FirstOfLayer {
            layer: 0,
            blocked: Some(Blocked {
                item: 0,
                contains_this: false,
                by_overlap: false,
            })
        }
    );
    assert!(
        p.steps[0]
            .reason
            .to_string()
            .contains("debe esperar a sus interiores")
    );
}

#[test]
fn inner_first_wins_over_nearer_unrelated_blocked_item() {
    // Dos contornos bloqueados: A (cerca, con su propio interior) y B (algo más lejos, con el
    // interior elegido). La explicación debe nombrar a B, que sí contiene a la elegida.
    let items = vec![
        item(Polyline {
            points: vec![pt(2.0, 0.0), pt(10.0, 0.0), pt(10.0, 200.0), pt(2.0, 200.0)],
            closed: true,
        }), // 0 A, el más cercano (2 mm)
        item(square(4.0, 150.0, 4.0)), // 1 interior de A, muy lejos
        item(square(-140.0, -10.0, 120.0)), // 2 B (vértice a 22 mm)
        item(square(-30.0, 10.0, 4.0)), // 3 interior de B, el elegido
    ];
    let o = PlanOptions {
        start: pt(0.0, 0.0),
        order_by_layer: false,
        order_by_priority: false,
        ..opts()
    };
    let p = plan(&items, &o);
    assert_eq!(p.steps[0].item, 3);
    assert_eq!(p.steps[0].reason, Reason::InnerFirst { container: 2 });
}

#[test]
fn nested_random_pieces_always_respect_precedence() {
    // 25 piezas: marco, agujero, isla y trazo; entrada barajada, varias capas.
    let mut r = Lcg(2024);
    let mut items = Vec::new();
    for gx in 0..5 {
        for gy in 0..5 {
            let (x, y) = (gx as f64 * 60.0, gy as f64 * 60.0);
            let layer = r.below(2);
            items.push(layered(square(x, y, 50.0), layer));
            items.push(layered(square(x + 10.0, y + 10.0, 30.0), layer));
            items.push(layered(square(x + 15.0, y + 15.0, 8.0), layer));
            items.push(layered(
                line((x + 30.0, y + 30.0), (x + 35.0, y + 35.0)),
                layer,
            ));
        }
    }
    for i in (1..items.len()).rev() {
        items.swap(i, r.below(i + 1));
    }
    for passes in [0, 1, 6] {
        let o = PlanOptions {
            improve_passes: passes,
            ..opts()
        };
        let p = plan(&items, &o);
        check_consistency(&items, &o, &p);
        check_inner_first(&items, &p);
        let layers: Vec<usize> = p.steps.iter().map(|s| items[s.item].layer_order).collect();
        assert!(layers.windows(2).all(|w| w[0] <= w[1]));
        assert!(p.warnings.is_empty());
    }
}

#[test]
fn invalid_paths_are_skipped_with_warning() {
    let items = vec![
        item(Polyline {
            points: vec![],
            closed: true,
        }),
        item(line((0.0, 0.0), (1.0, 0.0))),
        item(Polyline {
            points: vec![pt(f64::NAN, 0.0), pt(1.0, 1.0)],
            closed: false,
        }),
    ];
    let p = plan(&items, &opts());
    assert_eq!(order(&p), vec![1]);
    assert_eq!(
        p.warnings,
        vec![
            Warning::SkippedInvalid { item: 0 },
            Warning::SkippedInvalid { item: 2 }
        ]
    );
}

#[test]
fn degenerate_shapes_do_not_panic() {
    let items = vec![
        item(Polyline {
            points: vec![pt(3.0, 3.0)],
            closed: false,
        }),
        item(Polyline {
            points: vec![pt(3.0, 3.0)],
            closed: true,
        }),
        item(Polyline {
            points: vec![pt(1.0, 1.0), pt(1.0, 1.0)],
            closed: true,
        }),
        item(line((5.0, 5.0), (5.0, 5.0))),
        item(square(3.0, 3.0, 0.0)),
    ];
    let o = opts();
    let p = plan(&items, &o);
    assert_eq!(p.steps.len(), 5);
    check_consistency(&items, &o, &p);
}

#[test]
fn identical_duplicates_keep_stable_order() {
    let items = vec![item(square(0.0, 0.0, 10.0)); 4];
    let p = plan(&items, &opts());
    assert_eq!(order(&p), vec![0, 1, 2, 3]);
}

#[test]
fn collinear_and_coincident_points() {
    let items: Vec<PlanItem> = (0..200)
        .map(|i| item(line((i as f64, 0.0), (i as f64 + 0.5, 0.0))))
        .collect();
    let o = opts();
    let p = plan(&items, &o);
    check_consistency(&items, &o, &p);
    assert!(p.travel_mm <= 200.0);
}

#[test]
fn deterministic_across_runs() {
    let mut r = Lcg(9);
    let items: Vec<PlanItem> = (0..200)
        .map(|_| {
            let (x, y) = (r.below(30) as f64 * 5.0, r.below(30) as f64 * 5.0);
            layered(square(x, y, 3.0), r.below(3))
        })
        .collect();
    assert_eq!(plan(&items, &opts()), plan(&items, &opts()));
}

#[test]
fn reasons_have_spanish_display() {
    let items = vec![
        item(square(0.0, 0.0, 100.0)),
        item(square(40.0, 40.0, 10.0)),
    ];
    let o = PlanOptions {
        order_by_layer: false,
        order_by_priority: false,
        ..opts()
    };
    let p = plan(&items, &o);
    for s in &p.steps {
        let t = s.reason.to_string();
        assert!(!t.is_empty());
        assert!(!t.contains('{'), "{t}");
    }
    assert!(p.steps[0].reason.to_string().contains("interiores primero"));
}

/// Mide 10 000 trayectorias; ejecutar con `cargo test --release -- --ignored --nocapture`.
#[test]
#[ignore = "mide tiempo; usar --release"]
fn ten_thousand_paths_in_about_a_second() {
    let mut r = Lcg(123);
    // Una figura por celda de 10 mm (sin solapes) y orden de entrada barajado.
    let mut cells: Vec<usize> = (0..10_000).collect();
    for i in (1..cells.len()).rev() {
        cells.swap(i, r.below(i + 1));
    }
    let mut items = Vec::new();
    for (k, &c) in cells.iter().enumerate() {
        let (x, y) = ((c % 100) as f64 * 10.0, (c / 100) as f64 * 10.0);
        let (jx, jy) = (x + r.next() * 3.0, y + r.next() * 3.0);
        let layer = r.below(2);
        // Cuarto de piezas con agujero (dos entradas consecutivas ocupan la misma celda).
        match k % 4 {
            0 => items.push(layered(square(jx, jy, 6.0), layer)),
            1 => items.push(layered(circle(jx + 3.0, jy + 3.0, 1.5, 24), layer)),
            _ => items.push(layered(
                line((jx, jy), (jx + r.next() * 5.0, jy + r.next() * 5.0)),
                layer,
            )),
        }
    }
    // Agujeros dentro de algunos cuadrados.
    for k in (0..cells.len()).step_by(8) {
        let b = items[k].path.points[0];
        items[k + 1] = layered(square(b.x + 2.0, b.y + 2.0, 2.0), items[k].layer_order);
    }
    let o = opts();
    let t = std::time::Instant::now();
    let p = plan(&items, &o);
    let el = t.elapsed();
    println!(
        "10000 trayectorias: {el:.2?}; recorrido {:.0} mm frente a {:.0} mm originales ({:.1} % menos)",
        p.travel_mm,
        p.baseline_travel_mm,
        p.saved_pct()
    );
    let t = std::time::Instant::now();
    let nn = plan(
        &items,
        &PlanOptions {
            improve_passes: 0,
            ..o.clone()
        },
    );
    println!(
        "solo vecino más cercano: {:.2?}; {:.0} mm",
        t.elapsed(),
        nn.travel_mm
    );
    check_consistency(&items, &o, &p);
    check_inner_first(&items[..1000], &plan(&items[..1000], &o));
    assert!(el.as_secs_f64() < 2.0, "demasiado lento: {el:?}");
}

/// Casos patológicos de rendimiento; `cargo test --release -- --ignored --nocapture`.
#[test]
#[ignore = "mide tiempo; usar --release"]
fn pathological_inputs_stay_fast() {
    let time = |name: &str, items: Vec<PlanItem>| {
        let o = opts();
        let t = std::time::Instant::now();
        let p = plan(&items, &o);
        let el = t.elapsed();
        println!("{name}: {el:.2?}");
        check_consistency(&items, &o, &p);
        assert!(el.as_secs_f64() < 3.0, "{name} tardó {el:?}");
        p
    };
    let mut r = Lcg(1);

    // Una lámina con 9 999 agujeros: un contenedor con miles de interiores.
    let mut sheet = vec![item(square(0.0, 0.0, 1000.0))];
    for c in 0..9_999usize {
        let (x, y) = ((c % 100) as f64 * 10.0 + 2.0, (c / 100) as f64 * 10.0 + 2.0);
        sheet.push(item(circle(x + 2.0, y + 2.0, 1.5, 16)));
    }
    let n = sheet.len();
    for i in (1..n).rev() {
        sheet.swap(i, r.below(i + 1));
    }
    let outer = sheet
        .iter()
        .position(|it| it.path.points.len() == 4)
        .unwrap();
    let p = time("lámina con 9999 agujeros", sheet);
    assert_eq!(p.steps.last().unwrap().item, outer);

    // Anillos concéntricos: profundidad 2000.
    let rings: Vec<PlanItem> = (0..2000)
        .map(|i| item(square(i as f64, i as f64, 4000.0 - 2.0 * i as f64)))
        .collect();
    let p = time("2000 anillos concéntricos", rings);
    assert_eq!(order(&p)[0], 1999);

    // Una prioridad distinta por figura: 10 000 cubos.
    let buckets: Vec<PlanItem> = (0..10_000)
        .map(|i| PlanItem {
            priority: i,
            ..item(line((r.next() * 100.0, r.next() * 100.0), (0.0, 0.0)))
        })
        .collect();
    time("10000 cubos", buckets);

    // Todo coincidente y todo colineal.
    time(
        "10000 idénticas",
        vec![item(line((1.0, 1.0), (2.0, 2.0))); 10_000],
    );
    time(
        "10000 colineales",
        (0..10_000)
            .map(|i| item(line((i as f64, 0.0), (i as f64 + 0.5, 0.0))))
            .collect(),
    );
    // Una figura enorme (10 000 vértices) con interiores pequeños.
    let mut big = vec![item(circle(500.0, 500.0, 480.0, 10_000))];
    for c in 0..2000usize {
        big.push(item(circle(
            100.0 + (c % 40) as f64 * 20.0,
            100.0 + (c / 40) as f64 * 15.0,
            2.0,
            12,
        )));
    }
    time("círculo de 10000 vértices con 2000 interiores", big);
}

#[test]
#[ignore = "mide tiempo; usar --release"]
fn clustered_inputs_stay_fast() {
    let mut r = Lcg(8);
    let mut items = Vec::new();
    for i in 0..10_000 {
        let (x, y) = if i % 2 == 0 {
            (r.next() * 20.0, r.next() * 20.0)
        } else {
            (r.next() * 1000.0, r.next() * 1000.0)
        };
        items.push(item(line((x, y), (x + 0.2, y + 0.1))));
    }
    let o = opts();
    let t = std::time::Instant::now();
    let p = plan(&items, &o);
    let el = t.elapsed();
    println!("agrupadas: {el:.2?}");
    check_consistency(&items, &o, &p);
    assert!(el.as_secs_f64() < 3.0, "tardó {el:?}");
}

#[test]
fn fixed_direction_survives_improvement() {
    let mut r = Lcg(31);
    let items: Vec<PlanItem> = (0..150)
        .map(|_| {
            let (x, y) = (r.next() * 100.0, r.next() * 100.0);
            item(line((x, y), (x + 6.0, y - 4.0)))
        })
        .collect();
    let o = PlanOptions {
        choose_direction: false,
        improve_passes: 5,
        ..opts()
    };
    let p = plan(&items, &o);
    check_consistency(&items, &o, &p);
    let free = plan(&items, &opts());
    assert!(free.travel_mm <= p.travel_mm + 1e-9);
}

#[test]
fn without_levels_first_step_is_explained_by_distance() {
    let items = vec![
        item(line((10.0, 0.0), (11.0, 0.0))),
        item(line((2.0, 0.0), (3.0, 0.0))),
    ];
    let o = PlanOptions {
        order_by_layer: false,
        order_by_priority: false,
        ..opts()
    };
    let p = plan(&items, &o);
    assert_eq!(order(&p), vec![1, 0]);
    assert!(matches!(
        p.steps[0].reason,
        Reason::Nearest {
            blocked: None,
            distance_mm
        } if (distance_mm - 2.0).abs() < 1e-12
    ));
}

#[test]
fn improved_reason_appears_only_when_a_step_is_not_the_nearest() {
    // En nubes aleatorias con mejora, todo paso "Nearest" debe serlo de verdad.
    let mut r = Lcg(1234);
    let items: Vec<PlanItem> = (0..200)
        .map(|_| {
            item(line(
                (r.next() * 300.0, r.next() * 300.0),
                (r.next() * 300.0, r.next() * 300.0),
            ))
        })
        .collect();
    let o = PlanOptions {
        order_by_layer: false,
        order_by_priority: false,
        improve_passes: 4,
        ..opts()
    };
    let p = plan(&items, &o);
    let mut pos = o.start;
    let mut left: Vec<usize> = (0..items.len()).collect();
    let mut improved = 0;
    for s in &p.steps {
        let near = left
            .iter()
            .map(|&i| {
                let (a, b) = (items[i].path.start(), items[i].path.end());
                pos.dist(a).min(pos.dist(b))
            })
            .fold(f64::INFINITY, f64::min);
        let (a, b) = (items[s.item].path.start(), items[s.item].path.end());
        let own = pos.dist(a).min(pos.dist(b));
        match s.reason {
            Reason::Nearest { distance_mm, .. } => {
                assert!(own <= near + 1e-6, "no era la más cercana");
                assert!((distance_mm - own).abs() < 1e-9);
            }
            Reason::Improved => {
                improved += 1;
                assert!(own > near + 1e-9, "decía Improved pero era la más cercana");
            }
            ref other => panic!("razón inesperada: {other}"),
        }
        left.retain(|&i| i != s.item);
        pos = s.path.end();
    }
    assert!(improved > 0);
}

// ---- referencia válida (baseline) ----

#[test]
fn baseline_is_valid_order_in_real_case() {
    // Contenedor primero en la entrada, interior después; el cabezal viene de (75, 37).
    let items = vec![
        item(rounded_rect(2.0, 2.0, 76.0, 36.0, 6.0)),
        item(circle(10.0, 20.0, 3.0, 32)),
    ];
    let o = PlanOptions {
        improve_passes: 1,
        start: pt(75.0, 37.0),
        ..opts()
    };
    let p = plan(&items, &o);
    assert_eq!(order(&p), vec![1, 0]);
    assert!(p.saved_mm() >= -1e-9, "ahorro negativo: {}", p.saved_mm());
    assert!(p.saved_pct() >= -1e-9);
    // La referencia es el orden 1, 0 con los inicios originales.
    let base =
        o.start.dist(items[1].path.start()) + items[1].path.end().dist(items[0].path.start());
    assert!((p.baseline_travel_mm - base).abs() < 1e-9);
}

/// Escena pseudoaleatoria: cuadrados con agujeros anidados, círculos y líneas sueltas.
fn random_scene(r: &mut Lcg) -> Vec<PlanItem> {
    let mut items = Vec::new();
    let n = 2 + r.below(14);
    for cell in 0..n {
        let layer = r.below(3);
        // Una celda de 40 mm por elemento, para que no se traslapen entre sí.
        let (x, y) = (
            (cell % 4) as f64 * 40.0 + r.next() * 10.0,
            (cell / 4) as f64 * 40.0 + r.next() * 10.0,
        );
        match r.below(4) {
            0 => {
                let s = 6.0 + r.next() * 14.0;
                items.push(layered(square(x, y, s), layer));
                // Anidados a distintas profundidades y, a veces, en otra capa.
                let mut inner = (x, y, s);
                for _ in 0..r.below(3) {
                    inner = (
                        inner.0 + inner.2 * 0.2,
                        inner.1 + inner.2 * 0.2,
                        inner.2 * 0.6,
                    );
                    let l = if r.below(4) == 0 { r.below(3) } else { layer };
                    items.push(layered(square(inner.0, inner.1, inner.2), l));
                }
            }
            1 => items.push(layered(
                circle(x, y, 1.0 + r.next() * 6.0, 4 + r.below(20)),
                layer,
            )),
            2 => items.push(layered(
                line((x, y), (x + r.next() * 20.0, y + r.next() * 20.0)),
                layer,
            )),
            _ => items.push(PlanItem {
                priority: r.below(3) as i32,
                ..layered(square(x, y, 3.0 + r.next() * 5.0), layer)
            }),
        }
    }
    items
}

#[test]
fn travel_never_exceeds_baseline_random() {
    let mut r = Lcg(2024);
    for case in 0..400 {
        let items = random_scene(&mut r);
        let o = PlanOptions {
            order_by_layer: r.below(4) != 0,
            order_by_group: false,
            order_by_priority: r.below(2) == 0,
            inner_first: r.below(4) != 0,
            reduce_travel: r.below(4) != 0,
            choose_start_point: r.below(3) != 0,
            choose_direction: r.below(3) != 0,
            improve_passes: r.below(4),
            start: pt(r.next() * 100.0, r.next() * 100.0),
        };
        let p = plan(&items, &o);
        assert_eq!(p.steps.len(), items.len());
        assert!(
            p.travel_mm <= p.baseline_travel_mm + 1e-9,
            "caso {case}: {} > {} con {o:?}",
            p.travel_mm,
            p.baseline_travel_mm
        );
        assert!(p.saved_mm() >= -1e-9);
    }
}

#[test]
fn baseline_respects_precedence() {
    // Con optimización apagada el plan es la referencia: interiores antes que contenedores y
    // niveles en orden, estable respecto a la entrada.
    let items = vec![
        item(square(0.0, 0.0, 40.0)),        // 0: contiene 1 y 2
        layered(square(50.0, 0.0, 10.0), 1), // 1: otra capa
        item(square(5.0, 5.0, 10.0)),        // 2: dentro de 0
        item(square(60.0, 60.0, 5.0)),       // 3: suelta
    ];
    let o = PlanOptions {
        reduce_travel: false,
        choose_start_point: false,
        choose_direction: false,
        improve_passes: 0,
        ..opts()
    };
    let p = plan(&items, &o);
    // Capa 0: 2 (interior de 0) antes que 0; 3 conserva su lugar relativo. Capa 1: 1.
    assert_eq!(order(&p), vec![2, 0, 3, 1]);
    assert!((p.travel_mm - p.baseline_travel_mm).abs() < 1e-9);

    // En escenas aleatorias, el plan sin optimizar coincide con la referencia y respeta
    // interiores primero dentro de cada capa.
    let mut r = Lcg(77);
    for _ in 0..100 {
        let items = random_scene(&mut r);
        let p = plan(&items, &o);
        assert!((p.travel_mm - p.baseline_travel_mm).abs() < 1e-9);
        for (a, ia) in items.iter().enumerate() {
            for (b, ib) in items.iter().enumerate() {
                if a != b
                    && ia.layer_order == ib.layer_order
                    && ia.priority == ib.priority
                    && ib.path.points.iter().all(|q| {
                        let c = &ia.path.points;
                        let (xs, ys): (Vec<f64>, Vec<f64>) = c.iter().map(|p| (p.x, p.y)).unzip();
                        q.x > xs.iter().cloned().fold(f64::MAX, f64::min)
                            && q.x < xs.iter().cloned().fold(f64::MIN, f64::max)
                            && q.y > ys.iter().cloned().fold(f64::MAX, f64::min)
                            && q.y < ys.iter().cloned().fold(f64::MIN, f64::max)
                    })
                    && ib.path.closed
                    && ia.path.closed
                    && ia.path.points.len() == 4
                {
                    // `b` está dentro del cuadrado `a`: debe cortarse antes.
                    assert!(
                        pos_of(&p, b) < pos_of(&p, a),
                        "{a} dentro de {b} pero se corta después"
                    );
                }
            }
        }
    }
}

// ---- solapes parciales ----

/// Opciones sin niveles, para ver solo la regla de precedencia y la cercanía.
fn flat_opts(start: Point) -> PlanOptions {
    PlanOptions {
        order_by_layer: false,
        order_by_priority: false,
        start,
        ..opts()
    }
}

#[test]
fn partial_overlap_cuts_the_mostly_covered_figure_first() {
    // B (1) queda en un 75 % dentro de A (0) sin estar contenida. A está más cerca del inicio,
    // pero B se corta antes: de lo contrario A suelta la mayor parte de B.
    let items = vec![item(square(0.0, 0.0, 40.0)), item(square(25.0, 10.0, 20.0))];
    let o = flat_opts(pt(-5.0, -5.0));
    let p = plan(&items, &o);
    assert_eq!(order(&p), vec![1, 0]);
    assert!(
        matches!(
            p.steps[0].reason,
            Reason::OverlapFirst {
                other: 0,
                with_others: false,
                ..
            }
        ),
        "{:?}",
        p.steps[0].reason
    );
    let text = p.steps[0].reason.to_string();
    assert!(
        text.contains("#0") && text.contains("se solapa") && text.contains("75 %"),
        "{text}"
    );
    // Sin la regla, A (más cercana) iría primero.
    let free = plan(
        &items,
        &PlanOptions {
            inner_first: false,
            ..o.clone()
        },
    );
    assert_eq!(order(&free), vec![0, 1]);
    check_consistency(&items, &o, &p);
}

#[test]
fn slight_overlap_leaves_order_free() {
    let items = vec![item(square(0.0, 0.0, 40.0)), item(square(35.0, 35.0, 40.0))];
    let o = flat_opts(pt(-5.0, -5.0));
    assert_eq!(order(&plan(&items, &o)), vec![0, 1]);
}

#[test]
fn figure_inside_two_crossing_containers_goes_before_both() {
    // A (0) y B (1) se cruzan sin contenerse (25 % de A en B). C (2) cae en la intersección:
    // está dentro de las dos. Con el árbol de contención solo iba antes que A, la menor.
    let items = vec![
        item(square(0.0, 0.0, 40.0)),
        item(square(20.0, 20.0, 60.0)),
        item(square(25.0, 25.0, 6.0)),
    ];
    // Empezar cerca de B para que, sin la regla, B sea la más cercana.
    let o = flat_opts(pt(85.0, 85.0));
    let p = plan(&items, &o);
    assert!(pos_of(&p, 2) < pos_of(&p, 0), "{:?}", order(&p));
    assert!(pos_of(&p, 2) < pos_of(&p, 1), "{:?}", order(&p));
    assert_eq!(order(&p)[0], 2);
    // Ni la mejora ni el orden original lo rompen.
    for (reduce, passes) in [(false, 0), (true, 0), (true, 3)] {
        let p = plan(
            &items,
            &PlanOptions {
                reduce_travel: reduce,
                improve_passes: passes,
                ..o.clone()
            },
        );
        assert_eq!(order(&p)[0], 2, "reduce={reduce} passes={passes}");
    }
}

#[test]
fn figure_straddling_two_pieces_goes_before_both() {
    // C (2) cruza la costura entre A (0) y B (1): 40 % en cada una, 80 % en la unión.
    let items = vec![
        item(square(0.0, 0.0, 20.0)),
        item(square(20.0, 0.0, 20.0)),
        item(square(15.0, 5.0, 10.0)),
    ];
    let o = flat_opts(pt(-5.0, 10.0));
    let p = plan(&items, &o);
    assert_eq!(order(&p)[0], 2, "{:?}", order(&p));
    assert!(matches!(
        p.steps[0].reason,
        Reason::OverlapFirst {
            with_others: true,
            ..
        }
    ));
}

#[test]
fn mutual_overlap_breaks_ties_deterministically() {
    // Dos cuadrados iguales con 80 % de solape en cada sentido: sin ciclo, va primero el de
    // menor índice aunque el otro esté más cerca.
    let items = vec![item(square(0.0, 0.0, 10.0)), item(square(2.0, 0.0, 10.0))];
    let o = flat_opts(pt(20.0, 5.0));
    let p = plan(&items, &o);
    assert_eq!(order(&p), vec![0, 1]);
    let swapped = vec![items[1].clone(), items[0].clone()];
    let p2 = plan(&swapped, &o);
    assert_eq!(order(&p2), vec![0, 1]);
}

#[test]
fn line_mostly_inside_a_shape_goes_first() {
    // La línea sale un poco de la figura: el 75 % de su longitud está dentro.
    let items = vec![
        item(square(0.0, 0.0, 40.0)),
        item(line((10.0, 20.0), (50.0, 20.0))),
    ];
    let o = flat_opts(pt(-5.0, -5.0));
    let p = plan(&items, &o);
    assert_eq!(order(&p), vec![1, 0]);
    assert!(matches!(
        p.steps[0].reason,
        Reason::OverlapFirst { other: 0, .. }
    ));
    // Si cruza de lado a lado con la mitad dentro, no hay relación.
    let items = vec![
        item(square(0.0, 0.0, 40.0)),
        item(line((20.0, 20.0), (60.0, 20.0))),
    ];
    assert_eq!(order(&plan(&items, &o)), vec![0, 1]);
}

#[test]
fn overlap_does_not_cross_levels() {
    // En capas distintas manda el nivel; no hay aviso porque no es contención.
    let items = vec![
        layered(square(0.0, 0.0, 40.0), 0),
        layered(square(25.0, 10.0, 20.0), 1),
    ];
    let o = PlanOptions {
        start: pt(-5.0, -5.0),
        ..opts()
    };
    let p = plan(&items, &o);
    assert_eq!(order(&p), vec![0, 1]);
    assert!(p.warnings.is_empty());
}

/// Cuadrados solapados al azar: sin pánico, permutación válida, determinista y, cuando una
/// figura cae casi toda en otra mayor, se corta antes.
#[test]
fn random_overlapping_squares_never_cycle() {
    let mut r = Lcg(4242);
    for case in 0..150 {
        let n = 3 + r.below(40);
        let items: Vec<PlanItem> = (0..n)
            .map(|_| {
                let s = 4.0 + r.next() * 30.0;
                item(square(r.next() * 50.0, r.next() * 50.0, s))
            })
            .collect();
        let o = PlanOptions {
            reduce_travel: r.below(3) != 0,
            improve_passes: r.below(4),
            start: pt(r.next() * 80.0, r.next() * 80.0),
            ..opts()
        };
        let p = plan(&items, &o);
        assert_eq!(p.steps.len(), n, "caso {case}");
        assert!(p.warnings.is_empty());
        check_consistency(&items, &o, &p);
        assert_eq!(p, plan(&items, &o), "caso {case}: no determinista");
        // Cuadrados de área estrictamente distinta: si el menor está en más de 3/4 de su área
        // dentro del mayor (cálculo exacto de rectángulos), debe ir antes.
        for (a, ia) in items.iter().enumerate() {
            for (b, ib) in items.iter().enumerate() {
                let (pa, pb) = (&ia.path.points, &ib.path.points);
                let (sa, sb) = (pa[2].x - pa[0].x, pb[2].x - pb[0].x);
                if a == b || sa >= sb {
                    continue;
                }
                let w = (pa[2].x.min(pb[2].x) - pa[0].x.max(pb[0].x)).max(0.0);
                let h = (pa[2].y.min(pb[2].y) - pa[0].y.max(pb[0].y)).max(0.0);
                if w * h > 0.75 * sa * sa {
                    assert!(
                        pos_of(&p, a) < pos_of(&p, b),
                        "caso {case}: {a} (75 % dentro de {b}) se corta después"
                    );
                }
            }
        }
    }
}

#[test]
fn first_step_of_a_layer_names_the_overlapped_figure_it_skipped() {
    let items = vec![item(square(0.0, 0.0, 40.0)), item(square(25.0, 10.0, 20.0))];
    let p = plan(
        &items,
        &PlanOptions {
            start: pt(-5.0, -5.0),
            ..opts()
        },
    );
    assert_eq!(order(&p), vec![1, 0]);
    match &p.steps[0].reason {
        Reason::FirstOfLayer {
            blocked: Some(b), ..
        } => {
            assert_eq!(b.item, 0);
            assert!(b.contains_this && b.by_overlap);
        }
        r => panic!("{r:?}"),
    }
    assert!(
        p.steps[0]
            .reason
            .to_string()
            .contains("casi toda cae dentro")
    );
}
