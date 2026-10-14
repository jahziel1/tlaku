//! Ola 3: puentes manuales y punto de inicio / sentido manuales.

use super::*;
use tlaku_core::Point;
use tlaku_core::toolpath::Move;
use tlaku_doc::{DesignObject, JobOptions, StartFrom};

fn with_tabs(mut o: DesignObject, tabs: &[[f64; 2]]) -> DesignObject {
    o.manual_tabs = tabs.to_vec();
    o
}

fn with_start(mut o: DesignObject, point: Option<[f64; 2]>, reverse: bool) -> DesignObject {
    o.start = Some(tlaku_doc::StartSpec { point, reverse });
    o
}

/// Destinos de los cortes con el láser apagado (el cruce de cada hueco) del primer bloque.
fn gap_ends(job: &ComputedJob) -> Vec<Point> {
    job.toolpath.blocks[0]
        .moves
        .iter()
        .filter_map(|m| match *m {
            Move::Cut { to, power_pct: 0.0 } => Some(to),
            _ => None,
        })
        .collect()
}

fn first_target(job: &ComputedJob) -> Point {
    job.toolpath.blocks[0].moves[0].to()
}

#[test]
fn manual_tabs_replace_the_automatic_ones_of_their_object() {
    let d = doc(
        vec![
            with_tabs(
                rect_object("a", RED, 10.0, 10.0, 40.0, 40.0),
                &[[30.0, 10.0], [50.0, 30.0]],
            ),
            // Sin puentes manuales, este usa los 4 automáticos de la capa.
            rect_object("b", RED, 100.0, 10.0, 40.0, 40.0),
        ],
        vec![tabbed(line_layer(RED))],
    );
    let job = compute(&d, &device(), &JobOptions::default()).unwrap();
    assert_eq!(job.layers[0].tabs, 2 + 4);
    // Los dos huecos del primero están centrados donde se pidió, con el ancho de la capa (2 mm).
    let ends = gap_ends(&job);
    let has = |x: f64, y: f64| ends.iter().any(|p| p.dist(Point::new(x, y)) < 1e-6);
    assert!(has(31.0, 10.0), "{ends:?}");
    assert!(has(50.0, 31.0), "{ends:?}");
}

#[test]
fn manual_tabs_ignore_skip_inner_and_snap_to_the_contour() {
    let d = doc(
        vec![
            rect_object("outer", RED, 10.0, 10.0, 40.0, 40.0),
            // Un hueco: la regla automática lo omite, pero uno manual sí se respeta. El punto
            // está 3 mm fuera del borde y se lleva al más cercano.
            with_tabs(
                rect_object("hole", RED, 20.0, 20.0, 10.0, 10.0),
                &[[25.0, 17.0]],
            ),
        ],
        vec![tabbed(line_layer(RED))],
    );
    let job = compute(&d, &device(), &JobOptions::default()).unwrap();
    assert_eq!(job.layers[0].tabs, 4 + 1);
    let ends = gap_ends(&job);
    assert!(
        ends.iter().any(|p| p.dist(Point::new(26.0, 20.0)) < 1e-6),
        "{ends:?}"
    );
}

#[test]
fn manual_tabs_follow_the_job_placement() {
    let opts = JobOptions {
        start_from: StartFrom::UserOrigin,
        user_origin: Some([100.0, 100.0]),
        job_origin: 6,
        ..JobOptions::default()
    };
    let d = doc(
        vec![with_tabs(
            rect_object("a", RED, 10.0, 10.0, 40.0, 40.0),
            &[[30.0, 10.0]],
        )],
        vec![tabbed(line_layer(RED))],
    );
    let job = compute(&d, &device(), &opts).unwrap();
    assert_eq!(job.layers[0].tabs, 1);
    let ends = gap_ends(&job);
    assert!(
        ends.iter().any(|p| p.dist(Point::new(121.0, 100.0)) < 1e-6),
        "{ends:?}"
    );
}

#[test]
fn manual_tabs_need_the_layer_tabs_on_and_warn_otherwise() {
    let d = doc(
        vec![with_tabs(
            rect_object("a", RED, 10.0, 10.0, 40.0, 40.0),
            &[[30.0, 10.0]],
        )],
        vec![line_layer(RED)],
    );
    let job = compute(&d, &device(), &JobOptions::default()).unwrap();
    assert_eq!(job.layers[0].tabs, 0);
    assert!(gap_ends(&job).is_empty());
    assert!(
        job.preview
            .warnings
            .iter()
            .any(|w| w.contains("puentes manuales") && w.contains("desactivados")),
        "{:?}",
        job.preview.warnings
    );
}

#[test]
fn manual_tabs_that_overlap_are_one_tab_and_too_many_warn() {
    let d = doc(
        vec![with_tabs(
            rect_object("a", RED, 10.0, 10.0, 40.0, 40.0),
            &[[30.0, 10.0], [30.5, 10.0]],
        )],
        vec![tabbed(line_layer(RED))],
    );
    let job = compute(&d, &device(), &JobOptions::default()).unwrap();
    assert_eq!(job.layers[0].tabs, 1);
    assert!(
        job.preview.warnings.is_empty(),
        "{:?}",
        job.preview.warnings
    );

    // Un cuadrado de 3 mm (perímetro 12) admite 3 puentes de 2 mm; se piden las cuatro esquinas.
    let small = doc(
        vec![with_tabs(
            rect_object("s", RED, 0.0, 0.0, 3.0, 3.0),
            &[[1.5, 0.0], [3.0, 1.5], [1.5, 3.0], [0.0, 1.5]],
        )],
        vec![tabbed(line_layer(RED))],
    );
    let job = compute(&small, &device(), &JobOptions::default()).unwrap();
    assert_eq!(job.layers[0].tabs, 3);
    assert!(
        job.preview.warnings.iter().any(|w| w.contains("no caben")),
        "{:?}",
        job.preview.warnings
    );
}

#[test]
fn a_manual_start_point_overrides_the_planner() {
    let plain = doc(
        vec![rect_object("a", RED, 10.0, 10.0, 40.0, 40.0)],
        vec![line_layer(RED)],
    );
    let job = compute(&plain, &device(), &JobOptions::default()).unwrap();
    // El planificador entra por la esquina más cercana a la cabeza.
    assert!(first_target(&job).dist(Point::new(10.0, 10.0)) < 1e-6);

    let fixed = doc(
        vec![with_start(
            rect_object("a", RED, 10.0, 10.0, 40.0, 40.0),
            Some([50.0, 50.0]),
            false,
        )],
        vec![line_layer(RED)],
    );
    let job = compute(&fixed, &device(), &JobOptions::default()).unwrap();
    assert!(first_target(&job).dist(Point::new(50.0, 50.0)) < 1e-6);
    // Y el punto puede estar a mitad de un lado.
    let mid = doc(
        vec![with_start(
            rect_object("a", RED, 10.0, 10.0, 40.0, 40.0),
            Some([30.0, 52.0]),
            false,
        )],
        vec![line_layer(RED)],
    );
    let job = compute(&mid, &device(), &JobOptions::default()).unwrap();
    assert!(first_target(&job).dist(Point::new(30.0, 50.0)) < 1e-6);
}

#[test]
fn reverse_flips_the_direction_and_the_planner_keeps_it() {
    let forward = doc(
        vec![rect_object("a", RED, 10.0, 10.0, 40.0, 40.0)],
        vec![line_layer(RED)],
    );
    let job = compute(&forward, &device(), &JobOptions::default()).unwrap();
    let second = |j: &ComputedJob| j.toolpath.blocks[0].moves[1].to();
    // Sin pedirlo: del (10,10) hacia (50,10).
    assert!(second(&job).dist(Point::new(50.0, 10.0)) < 1e-6);
    let backward = doc(
        vec![with_start(
            rect_object("a", RED, 10.0, 10.0, 40.0, 40.0),
            None,
            true,
        )],
        vec![line_layer(RED)],
    );
    let job = compute(&backward, &device(), &JobOptions::default()).unwrap();
    assert!(first_target(&job).dist(Point::new(10.0, 10.0)) < 1e-6);
    assert!(second(&job).dist(Point::new(10.0, 50.0)) < 1e-6);
    // También con «unificar el sentido»: la forma fijada no se gira.
    let mut opts = JobOptions::default();
    opts.optimization.hide_backlash = true;
    let job = compute(&backward, &device(), &opts).unwrap();
    assert!(second(&job).dist(Point::new(10.0, 50.0)) < 1e-6);
}

#[test]
fn only_the_object_with_a_manual_start_is_fixed() {
    let d = doc(
        vec![
            with_start(
                rect_object("a", RED, 10.0, 10.0, 10.0, 10.0),
                Some([20.0, 20.0]),
                false,
            ),
            rect_object("b", RED, 40.0, 10.0, 10.0, 10.0),
        ],
        vec![line_layer(RED)],
    );
    let job = compute(&d, &device(), &JobOptions::default()).unwrap();
    // "a" entra por (20,20) aunque (10,10) esté más cerca; "b" lo elige el planificador, que
    // sale desde donde terminó "a" (su mismo (20,20)) y entra por el vértice más próximo: (40,20).
    let moves = &job.toolpath.blocks[0].moves;
    assert!(moves[0].to().dist(Point::new(20.0, 20.0)) < 1e-6);
    let second_travel = moves
        .iter()
        .skip(1)
        .find_map(|m| match m {
            Move::Travel { to } => Some(*to),
            _ => None,
        })
        .unwrap();
    assert!(
        second_travel.dist(Point::new(40.0, 20.0)) < 1e-6,
        "{second_travel:?}"
    );
}

#[test]
fn the_manual_start_survives_the_kerf() {
    let mut l = line_layer(RED);
    l.kerf = 1.0;
    let d = doc(
        vec![with_start(
            rect_object("a", RED, 10.0, 10.0, 40.0, 40.0),
            Some([50.0, 50.0]),
            true,
        )],
        vec![l],
    );
    let job = compute(&d, &device(), &JobOptions::default()).unwrap();
    // El contorno compensado empieza junto a la esquina elegida (a un kerf de ella) ...
    let first = first_target(&job);
    assert!(first.dist(Point::new(50.0, 50.0)) < 2.0, "{first:?}");
    // ... y va al revés: del (50,50) hacia (50,10), es decir, hacia -y.
    let next = job.toolpath.blocks[0].moves[1..]
        .iter()
        .map(|m| m.to())
        .find(|p| p.dist(first) > 0.5)
        .unwrap();
    assert!(next.y < first.y, "{first:?} {next:?}");
}
