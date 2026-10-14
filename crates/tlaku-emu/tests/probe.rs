//! Sonda de GRBL (`G38.2` a `G38.5`) contra superficies de contacto simuladas.

mod common;

use common::Rig;
use tlaku_emu::{ProbeSurface, Profile, StateKind};

fn rig(surface: ProbeSurface) -> Rig {
    let mut r = Rig::with(Profile::grbl_1_1h(), |b| b.probe_surface(surface));
    r.drain();
    r
}

fn plane(z: f64) -> ProbeSurface {
    ProbeSurface::Plane { z }
}

#[test]
fn g38_2_stops_at_the_contact_and_reports_prb() {
    let mut r = rig(plane(-10.0));
    r.line("G38.2Z-30F100");
    r.settle();
    assert_eq!(r.lines(), ["[PRB:0.000,0.000,-10.000:1]", "ok"]);
    assert_eq!(r.h.mpos(), [0.0, 0.0, -10.0]);
    assert!(r.status().contains("|Pn:P"), "{}", r.status());
    // `$#` recuerda la última sonda.
    let params = r.ask("$#");
    assert!(
        params.contains(&"[PRB:0.000,0.000,-10.000:1]".to_string()),
        "{params:?}"
    );
    // Subir suelta el pin.
    r.line("G0Z5");
    r.settle();
    assert!(!r.status().contains("Pn:"), "{}", r.status());
}

#[test]
fn probe_travel_takes_the_programmed_feed() {
    let mut r = rig(plane(-10.0));
    r.line("G38.2Z-30F60");
    r.secs(5);
    // A 1 mm/s lleva 5 mm en 5 s; el override de avance no le aplica.
    assert!((r.h.mpos()[2] + 5.0).abs() < 0.5, "{:?}", r.h.mpos());
    r.rt(0x91); // +10 % de avance
    r.secs(5);
    assert!((r.h.mpos()[2] + 10.0).abs() < 0.5, "{:?}", r.h.mpos());
    r.settle();
    assert_eq!(r.h.mpos()[2], -10.0);
}

#[test]
fn g38_2_without_contact_ends_at_the_target_with_alarm_5() {
    let mut r = rig(plane(-100.0));
    r.line("G38.2Z-20F600");
    r.settle_alarm();
    assert_eq!(r.lines(), ["[PRB:0.000,0.000,-20.000:0]", "ok", "ALARM:5"]);
    assert_eq!(r.h.alarm(), Some(5));
    assert_eq!(r.h.mpos()[2], -20.0);
}

#[test]
fn g38_3_without_contact_is_quiet() {
    let mut r = rig(plane(-100.0));
    r.line("G38.3Z-20F600");
    r.settle();
    assert_eq!(r.lines(), ["[PRB:0.000,0.000,-20.000:0]", "ok"]);
    assert_eq!(r.h.alarm(), None);
    // Sin ninguna superficie, tampoco hay contacto.
    let mut r = Rig::new(Profile::grbl_1_1h());
    r.drain();
    r.line("G38.3X10F600");
    r.settle();
    assert_eq!(r.lines(), ["[PRB:10.000,0.000,0.000:0]", "ok"]);
}

#[test]
fn probe_already_touching_raises_alarm_4() {
    let mut r = rig(plane(5.0));
    // La cabeza arranca en Z=0, por debajo del plano: ya toca.
    r.line("G38.2Z-10F100");
    r.ms(20);
    assert_eq!(r.lines(), ["ok", "ALARM:4"]);
    assert_eq!(r.h.alarm(), Some(4));
    assert_eq!(r.h.mpos(), [0.0, 0.0, 0.0]);
}

#[test]
fn g38_4_leaves_the_surface_and_needs_to_start_touching() {
    let mut r = rig(plane(5.0));
    r.line("G38.4Z20F300");
    r.settle();
    assert_eq!(r.lines(), ["[PRB:0.000,0.000,5.000:1]", "ok"]);
    // Ahora ya no toca: G38.4 de nuevo da ALARM:4.
    r.line("G0Z10");
    r.settle();
    r.drain();
    r.line("G38.4Z20F300");
    r.ms(20);
    assert_eq!(r.lines(), ["ok", "ALARM:4"]);
}

#[test]
fn invalid_probe_lines_give_the_documented_errors() {
    let mut r = rig(plane(-10.0));
    assert_eq!(r.ask("G38.2F100"), ["error:26"], "sin ejes");
    assert_eq!(r.ask("G38.2Z-5"), ["error:22"], "sin avance");
    assert_eq!(r.ask("G38.2Z0F100"), ["error:33"], "mismo punto");
    assert_eq!(r.ask("G93"), ["ok"]);
    assert_eq!(r.ask("G38.2Z-5F2"), ["error:22"], "tiempo inverso");
    assert_eq!(
        r.ask("G38.2G1Z-5F100"),
        ["error:21"],
        "dos de un grupo modal"
    );
    assert_eq!(r.h.state(), StateKind::Idle);
}

#[test]
fn probing_never_fires_the_laser() {
    let mut r = rig(plane(-10.0));
    assert_eq!(r.ask("M3S1000"), ["ok"]);
    r.line("G38.2Z-30F600");
    r.settle();
    r.drain();
    // M3 en reposo puede dejar un tramo parado, pero no hay ni un milímetro quemado en el recorrido.
    assert_eq!(
        r.h.trace().total_length(),
        0.0,
        "el láser no puede arder durante la sonda"
    );
    assert_eq!(r.h.mpos()[2], -10.0);
}

#[test]
fn three_touches_on_a_round_object_give_its_center() {
    // Un objeto redondo de 12 mm de radio centrado en (60, 45), a 2 mm de alto.
    let (cx, cy, rad) = (60.0, 45.0, 12.0);
    let mut r = rig(ProbeSurface::Disc {
        center: [cx, cy],
        radius: rad,
        top: 2.0,
    });
    r.line("G90");
    let mut pts = Vec::new();
    // Tres lados: desde la izquierda, desde abajo y desde arriba/derecha.
    for (start, target) in [
        ([30.0, 45.0], [60.0, 45.0]),
        ([60.0, 10.0], [60.0, 45.0]),
        ([90.0, 70.0], [60.0, 45.0]),
    ] {
        r.line("G0Z1");
        r.line(&format!("G0X{}Y{}", start[0], start[1]));
        r.line(&format!("G38.2X{}Y{}F200", target[0], target[1]));
        r.settle();
        let p = r.h.mpos();
        pts.push([p[0], p[1]]);
        r.drain();
    }
    // El punto de cada toque queda sobre el borde.
    for p in &pts {
        assert!(
            ((p[0] - cx).hypot(p[1] - cy) - rad).abs() < 1e-6,
            "{p:?} no está en el borde"
        );
    }
    // Centro de la circunferencia por tres puntos.
    let [a, b, c] = [pts[0], pts[1], pts[2]];
    let d = 2.0 * (a[0] * (b[1] - c[1]) + b[0] * (c[1] - a[1]) + c[0] * (a[1] - b[1]));
    let sq = |p: [f64; 2]| p[0] * p[0] + p[1] * p[1];
    let ux = (sq(a) * (b[1] - c[1]) + sq(b) * (c[1] - a[1]) + sq(c) * (a[1] - b[1])) / d;
    let uy = (sq(a) * (c[0] - b[0]) + sq(b) * (a[0] - c[0]) + sq(c) * (b[0] - a[0])) / d;
    assert!(
        (ux - cx).abs() < 1e-6 && (uy - cy).abs() < 1e-6,
        "{ux},{uy}"
    );
}
