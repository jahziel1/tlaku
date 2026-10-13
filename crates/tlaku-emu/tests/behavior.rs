//! Comportamiento del emulador: planificador, sincronización, desborde del RX, láser y fallas.

mod common;

use std::io::ErrorKind;
use std::time::Duration;

use common::Rig;
use tlaku_emu::{Fault, MoveKind, Profile, SpindleMode, StateKind, Trigger};
use tlaku_grbl::serial::Transport;

fn classic() -> Rig {
    let mut r = Rig::new(Profile::grbl_1_1h());
    r.drain();
    r
}

fn oks(lines: &[String]) -> usize {
    lines.iter().filter(|l| *l == "ok").count()
}

// ---------------------------------------------------------------------------
// Planificador y sincronización
// ---------------------------------------------------------------------------

#[test]
fn planner_full_withholds_ok() {
    let mut r = Rig::with(Profile::grbl_1_1h(), |b| b.planner_blocks(4));
    r.drain();
    assert_eq!(r.ask("$10=3"), ["ok"]);
    r.line("G91");
    for _ in 0..9 {
        r.line("G1X10F60");
    }
    // G91 y cuatro bloques entran; el quinto espera sitio.
    assert_eq!(r.lines(), ["ok"; 5]);
    let st = r.status();
    assert!(st.contains("|Bf:0,"), "planificador lleno: {st}");
    r.secs(5);
    assert!(r.lines().is_empty(), "aún no terminó el primer bloque");
    r.secs(6);
    assert_eq!(
        r.lines(),
        ["ok"],
        "al terminar un bloque entra el siguiente"
    );
    r.settle();
    assert_eq!(r.h.mpos()[0], 90.0);
    // Cada línea con ok recibió el suyo, ni uno más.
    assert_eq!(r.h.lines_received(), 11);
}

#[test]
fn spindle_change_and_dwell_sync_the_ok() {
    let mut r = classic();
    for l in ["G91", "M4S0", "G1X10F600S500", "M5", "G1X10"] {
        r.line(l);
    }
    // G91, M4 (planificador vacío) y el G1 contestan al instante; M5 espera a que termine.
    assert_eq!(r.lines(), ["ok"; 3]);
    r.ms(500);
    assert!(r.lines().is_empty());
    r.ms(800);
    assert_eq!(r.lines(), ["ok"; 2]);
    r.settle();
    // G4 contesta al terminar la espera.
    r.line("G4P0.5");
    r.ms(300);
    assert!(r.lines().is_empty());
    r.ms(300);
    assert_eq!(r.lines(), ["ok"]);
}

#[test]
fn sync_rules_in_laser_mode() {
    let mut r = classic();
    // M4 con cambios de S entre movimientos no frena: todos los ok llegan al instante.
    for l in ["M4S500", "G1X10F600S0", "G1X20S800", "G1X30S100"] {
        r.line(l);
    }
    assert_eq!(oks(&r.lines()), 4);
    r.settle();
    // M4 con cambio de S sin movimiento tampoco.
    r.line("G1X40S200F600");
    r.line("S800");
    assert_eq!(oks(&r.lines()), 2);
    r.settle();
    // M3 con cambio de S sin movimiento sí fuerza una parada.
    r.line("M3S500");
    r.line("G1X50F600");
    r.line("S800");
    let l = r.lines();
    assert_eq!(oks(&l), 2, "M3 y el G1 contestan; S800 espera: {l:?}");
    r.settle();
    assert_eq!(oks(&r.lines()), 1);
    // M3 -> M4 es un cambio de estado del husillo: parada.
    r.line("G1X60F600");
    r.line("M4");
    assert_eq!(oks(&r.lines()), 1);
    r.settle();
    assert_eq!(oks(&r.lines()), 1);
}

#[test]
fn m2_ends_program_and_resets_modal_state() {
    let mut r = classic();
    r.line("M4S500");
    r.line("G91");
    r.line("G1X1F600");
    r.drain();
    r.line("M2");
    r.ms(5);
    assert!(r.lines().is_empty(), "espera a que termine el movimiento");
    r.settle();
    assert_eq!(r.lines(), ["[MSG:Pgm End]", "ok"]);
    assert_eq!(
        r.ask("$G"),
        ["[GC:G1 G54 G17 G21 G90 G94 M5 M9 T0 F600 S500]", "ok"]
    );
}

#[test]
fn m0_pause_waits_for_cycle_start() {
    let mut r = classic();
    r.line("G91");
    r.line("G1X1F600");
    r.line("M0");
    r.line("G1X1");
    r.settle_motion();
    assert_eq!(r.h.state(), StateKind::Hold(0));
    assert_eq!(oks(&r.lines()), 2, "M0 y lo que le sigue esperan el ~");
    r.rt(b'~');
    r.ms(5);
    assert_eq!(oks(&r.lines()), 2);
    r.settle();
    assert_eq!(r.h.mpos()[0], 2.0);
}

impl Rig {
    /// Deja pasar un par de segundos de movimiento sin esperar al reposo.
    fn settle_motion(&mut self) {
        self.secs(3);
    }
}

#[test]
fn arcs_follow_the_circle() {
    let mut r = classic();
    r.line("G0X10Y0");
    r.line("M4S300");
    r.line("G3X0Y10I-10J0F1200");
    r.settle();
    let t = r.h.trace();
    assert!(!t.is_empty());
    let (lo, hi) = t.bounds().unwrap();
    assert!(
        lo[0] >= -1e-6 && lo[1] >= -1e-6 && hi[0] <= 10.0 + 1e-6,
        "{lo:?} {hi:?}"
    );
    // Todos los puntos del arco están a 10 mm del centro.
    for s in t.segments() {
        for p in [s.start, s.end] {
            assert!((p[0].hypot(p[1]) - 10.0).abs() < 0.05, "{p:?}");
        }
    }
    let len = t.total_length();
    assert!((len - 15.708).abs() < 0.2, "{len}");
    assert!((r.h.mpos()[0]).abs() < 1e-6 && (r.h.mpos()[1] - 10.0).abs() < 1e-6);
    // Con R el centro sale del mismo lado.
    r.line("G0X10Y0");
    r.settle();
    r.h.clear_trace();
    r.line("G3X0Y10R10F1200");
    r.settle();
    assert!((r.h.trace().total_length() - 15.708).abs() < 0.2);
}

// ---------------------------------------------------------------------------
// Conteo de caracteres y desborde
// ---------------------------------------------------------------------------

/// Línea de exactamente `total` bytes con su `\n`, rellenada con espacios.
fn padded(body: &str, total: usize) -> String {
    let mut s = body.to_string();
    while s.len() + 1 < total {
        s.push(' ');
    }
    s.push('\n');
    assert_eq!(s.len(), total);
    s
}

#[test]
fn official_char_counting_scenario_never_overflows() {
    // Líneas de 25, 40, 31, 58 y 20 bytes (grbl.md 11.6) con el planificador de un solo
    // bloque, para que el RX se acumule de verdad.
    let mut r = Rig::with(Profile::grbl_1_1h(), |b| b.planner_blocks(1));
    r.drain();
    let sizes = [25usize, 40, 31, 58, 20];
    let mut bodies = vec!["G91".to_string()];
    bodies.extend((0..sizes.len() - 1).map(|_| "G1X1F600".to_string()));
    let lines: Vec<String> = bodies
        .iter()
        .zip(sizes)
        .map(|(b, n)| padded(b, n))
        .collect();
    let mut pending: std::collections::VecDeque<usize> = Default::default();
    let mut used = 0usize;
    let mut next = 0usize;
    let mut done = 0usize;
    let mut max_used = 0usize;
    for _ in 0..20_000 {
        while next < lines.len() && used + lines[next].len() <= 127 {
            r.send(&lines[next]);
            pending.push_back(lines[next].len());
            used += lines[next].len();
            max_used = max_used.max(used);
            next += 1;
        }
        r.ms(5);
        for l in r.lines() {
            if l == "ok" {
                used -= pending.pop_front().unwrap();
                done += 1;
            }
        }
        if done == lines.len() {
            break;
        }
    }
    assert_eq!(done, lines.len());
    assert!(max_used <= 127);
    assert_eq!(r.h.rx_overflow_bytes(), 0);
    assert!(r.h.max_in_flight() <= 127, "{}", r.h.max_in_flight());
    assert_eq!(r.h.max_in_flight(), max_used);
}

#[test]
fn overflow_is_lost_counted_and_corrupts_lines() {
    let mut r = Rig::with(Profile::grbl_1_1h(), |b| b.planner_blocks(1));
    r.drain();
    r.line("G91");
    // Un host que ignora el conteo: 12 líneas de 30 bytes de golpe con el planificador bloqueado.
    let mut blob = String::new();
    for _ in 0..12 {
        blob.push_str(&padded("G1X1F600", 30));
    }
    r.send(&blob);
    assert!(r.h.rx_overflow_bytes() > 0, "el RX debió desbordarse");
    assert!(r.h.max_in_flight() > 127);
    r.secs(60);
    let l = r.lines();
    // Hubo bytes perdidos: faltan respuestas (el anfitrión nunca vería los 13 ok) o hay errores.
    let replies = l
        .iter()
        .filter(|x| x.starts_with("ok") || x.starts_with("error"))
        .count();
    assert!(replies < 13, "{replies} respuestas de 13 líneas");
}

#[test]
fn realtime_bytes_survive_a_full_rx() {
    let mut r = Rig::with(Profile::grbl_1_1h(), |b| b.planner_blocks(1));
    r.drain();
    r.line("G91");
    for _ in 0..6 {
        r.send(&padded("G1X10F60", 30));
    }
    r.ms(100);
    r.drain();
    assert!(r.h.rx_overflow_bytes() == 0);
    r.rt(b'?');
    let l = r.lines();
    assert!(l.iter().any(|x| x.starts_with("<Run|")), "{l:?}");
    r.rt(b'!');
    assert!(matches!(r.h.state(), StateKind::Hold(_)));
}

// ---------------------------------------------------------------------------
// Láser (grbl.md 11.7)
// ---------------------------------------------------------------------------

#[test]
fn laser_m4_without_motion_is_off_and_m3_is_on() {
    let mut r = classic();
    r.ask("M4S500");
    r.ms(10);
    assert!(!r.h.laser_on());
    assert!(r.h.trace().is_empty());
    r.ask("M3S500");
    r.ms(10);
    assert!(r.h.laser_on());
    assert!(!r.h.trace().stationary_burns().is_empty());
    r.ask("M5");
    assert!(!r.h.laser_on());
    assert_eq!(r.h.laser_power(), 0.0);
}

#[test]
fn laser_m4_moves_burn_only_on_g1() {
    let mut r = classic();
    for l in ["M4S500", "G0X10", "G1X20F1200", "G0X30", "G1X40", "M5"] {
        r.line(l);
    }
    r.settle();
    let t = r.h.trace();
    assert!(t.burned_during_rapids().is_empty());
    assert!(t.stationary_burns().is_empty());
    // Solo se quemó de 10 a 20 y de 30 a 40.
    let total = t.total_length();
    assert!((total - 20.0).abs() < 0.05, "{total}");
    for s in t.segments() {
        assert_eq!(s.kind, MoveKind::Feed);
        assert_eq!(s.mode, SpindleMode::Ccw);
        let (a, b) = (s.start[0].min(s.end[0]), s.start[0].max(s.end[0]));
        assert!((a >= 10.0 - 1e-6 && b <= 20.0 + 1e-6) || (a >= 30.0 - 1e-6 && b <= 40.0 + 1e-6));
        assert!(s.s_peak <= 500.0 + 1e-9 && s.s_programmed == 500.0);
    }
    // M4 es dinámico: en un trazo corto que no alcanza el crucero la potencia media es menor.
    r.h.clear_trace();
    for l in ["M4S500", "G0X0", "G1X1F6000", "M5"] {
        r.line(l);
    }
    r.settle();
    let t = r.h.trace();
    let avg: f64 = t.segments().iter().map(|s| s.s_avg()).fold(0.0, f64::max);
    assert!(avg > 0.0 && avg < 450.0, "{avg}");
}

#[test]
fn laser_m3_off_during_g0_and_g0_m3_sets_modal_state() {
    let mut r = classic();
    for l in ["M3S500", "G0X10"] {
        r.line(l);
    }
    r.ms(100);
    assert!(!r.h.laser_on(), "G0 apaga el láser aunque M3 siga activo");
    r.settle();
    assert!(r.h.trace().burned_during_rapids().is_empty());
    assert!(r.h.laser_on(), "parado con M3 vuelve a quemar");
    r.line("M5");
    r.settle();
    r.h.clear_trace();
    // G0 M3 S1000 no enciende; el siguiente G1 sí, con S1000.
    r.line("G0M3S1000X5");
    r.settle();
    assert!(r.h.trace().burned_during_rapids().is_empty());
    r.h.clear_trace();
    r.line("G1X8F600");
    r.settle();
    let t = r.h.trace();
    assert!(
        t.segments()
            .iter()
            .any(|s| s.s_programmed == 1000.0 && s.kind == MoveKind::Feed)
    );
}

#[test]
fn laser_off_in_hold_depends_on_mode() {
    // M4: parado no quema.
    let mut r = classic();
    r.line("M4S500");
    r.line("G1X100F600");
    r.ms(500);
    assert!(r.h.laser_on());
    r.rt(b'!');
    r.ms(500);
    assert_eq!(r.h.state(), StateKind::Hold(0));
    assert!(!r.h.laser_on());
    // M3: GRBL clásico deja el láser encendido en el hold (riesgo documentado, punto 20).
    let mut r = classic();
    r.line("M3S500");
    r.line("G1X100F600");
    r.ms(500);
    r.rt(b'!');
    r.ms(500);
    assert_eq!(r.h.state(), StateKind::Hold(0));
    assert!(r.h.laser_on());
    // 0x9E (paro de husillo en hold) lo apaga; al reanudar vuelve.
    r.rt(0x9E);
    assert!(!r.h.laser_on());
    r.rt(b'~');
    r.ms(50);
    assert!(r.h.laser_on());
    // grblHAL con $63: se apaga en el hold.
    let mut p = Profile::grbl_hal();
    p.laser_off_in_hold = true;
    let mut r = Rig::new(p);
    r.drain();
    r.line("M3S500");
    r.line("G1X100F600");
    r.ms(500);
    r.rt(b'!');
    r.ms(500);
    assert!(!r.h.laser_on());
}

#[test]
fn spindle_mode_without_laser_burns_in_rapids() {
    let mut r = classic();
    assert_eq!(r.ask("$32=0"), ["ok"]);
    for l in ["M3S500", "G0X10"] {
        r.line(l);
    }
    r.ms(100);
    assert!(r.h.laser_on(), "con $32=0 el husillo gira también en G0");
    r.settle();
    assert!(!r.h.trace().burned_during_rapids().is_empty());
    r.drain();
    // Y cada cambio de S frena (sync): el segundo G1 no contesta hasta que termine el primero.
    r.line("G1X20F600S100");
    r.line("G1X30S200");
    assert_eq!(
        oks(&r.lines()),
        1,
        "el segundo G1 espera a que el primero termine"
    );
    r.settle();
    assert_eq!(oks(&r.lines()), 1);
}

#[test]
fn lpc_default_s_max_saturates() {
    let mut r = Rig::new(Profile::grbl_lpc());
    r.drain();
    assert!(r.ask("$$").contains(&"$30=1.000".to_string()));
    for l in ["M4S500", "G1X10F600"] {
        r.line(l);
    }
    r.settle();
    let peak =
        r.h.trace()
            .segments()
            .iter()
            .map(|s| s.s_peak)
            .fold(0.0, f64::max);
    assert!(
        (peak - 1.0).abs() < 1e-9,
        "S500 con $30=1 satura al máximo: {peak}"
    );
    r.ask("$30=1000");
    r.h.clear_trace();
    r.line("G1X20");
    r.settle();
    let peak =
        r.h.trace()
            .segments()
            .iter()
            .map(|s| s.s_peak)
            .fold(0.0, f64::max);
    assert!((peak - 500.0).abs() < 1e-6, "{peak}");
}

#[test]
fn spindle_override_scales_power() {
    let mut r = classic();
    r.line("M4S400");
    r.line("G1X100F600");
    r.ms(2000);
    let p0 = r.h.laser_power();
    assert!((p0 - 400.0).abs() < 1.0, "{p0}");
    r.rt(0x9A);
    r.rt(0x9A);
    r.ms(5);
    assert!((r.h.laser_power() - 480.0).abs() < 1.0);
    r.rt(0x9B);
    r.rt(0x9B);
    r.rt(0x9B);
    r.ms(5);
    assert!((r.h.laser_power() - 360.0).abs() < 1.0);
}

// ---------------------------------------------------------------------------
// Fallas
// ---------------------------------------------------------------------------

#[test]
fn fault_noise_error_reply_and_drop_reply() {
    let mut r = classic();
    r.h.inject(Trigger::Now, Fault::Noise(b"ab\xff\xfe\r\n".to_vec()));
    assert_eq!(r.raw(), b"ab\xff\xfe\r\n");
    r.h.inject(Trigger::Line(2), Fault::ErrorReply(20));
    r.h.inject(Trigger::Line(3), Fault::DropReply);
    r.line("G91");
    r.line("G0X5");
    r.line("G0X5");
    r.line("G0X5");
    // 1: ok; 2: error:20 (no se ejecuta); 3: ejecutada pero sin respuesta; 4: ok.
    assert_eq!(r.lines(), ["ok", "error:20", "ok"]);
    r.settle();
    assert_eq!(r.h.mpos()[0], 10.0);
}

#[test]
fn fault_creality_error_halts_the_job() {
    let mut r = classic();
    r.h.inject(
        Trigger::Line(4),
        Fault::CrealityError {
            code: 5,
            halt: true,
        },
    );
    for l in ["M4S500", "G91", "G1X50F600", "G1X10", "G1X10"] {
        r.line(l);
    }
    let l = r.lines();
    assert_eq!(
        l,
        ["ok", "ok", "ok", "ERROR:05"],
        "la línea 4 y las siguientes no reciben ok"
    );
    r.ms(100);
    assert!(!r.h.laser_on());
    assert_eq!(r.h.planner_used(), 0);
    // Sigue contestando a ?, en Alarm.
    assert!(r.status().starts_with("<Alarm|"));
    r.line("G1X1");
    r.ms(10);
    assert!(r.lines().is_empty());
    // El reset lo recupera.
    r.rt(0x18);
    assert!(r.lines()[0].starts_with("Grbl"));
    assert_eq!(r.ask("G0X1"), ["ok"]);
}

#[test]
fn fault_creality_error_without_halt_keeps_running() {
    let mut r = classic();
    r.h.inject(
        Trigger::Now,
        Fault::CrealityError {
            code: 24,
            halt: false,
        },
    );
    assert_eq!(r.lines(), ["ERROR:24"]);
    assert_eq!(r.ask("G0X1"), ["ok"]);
}

#[test]
fn fault_alarm3_with_banner_mid_job() {
    let mut r = Rig::new(Profile::falcon2_pro());
    r.drain();
    r.line("$X");
    r.line("M4S500");
    r.line("G1X100F600");
    r.drain();
    r.ms(500);
    assert_eq!(r.h.state(), StateKind::Run);
    r.h.inject(Trigger::Now, Fault::Alarm3WithBanner);
    let l = r.lines();
    assert_eq!(l[0], "ALARM:3");
    assert_eq!(l[1], "usb connect succeed");
    assert!(l.contains(&"Grbl 1.1f ['$' for help]".to_string()));
    assert_eq!(l.last().unwrap(), "[OK] ILmp Link connect ok");
    assert_eq!(r.h.state(), StateKind::Alarm(3));
    assert!(r.h.position_lost());
    assert!(!r.h.laser_on());
}

#[test]
fn fault_at_time_fires_without_traffic() {
    let mut r = classic();
    r.line("G1X1000F600");
    r.drain();
    r.h.inject(Trigger::At(Duration::from_secs(3)), Fault::Alarm(2));
    r.ms(2900);
    assert_eq!(r.h.state(), StateKind::Run);
    r.ms(200);
    assert_eq!(r.lines(), ["ALARM:2", "[MSG:Reset to continue]"]);
    assert_eq!(r.h.alarm(), Some(2));
}

#[test]
fn fault_silence_drops_or_delays_output() {
    for drop_output in [true, false] {
        let mut r = classic();
        r.h.inject(
            Trigger::Now,
            Fault::Silence {
                duration: Duration::from_secs(2),
                drop_output,
            },
        );
        r.line("G0X1");
        r.rt(b'?');
        r.ms(1000);
        assert!(r.lines().is_empty());
        r.ms(1500);
        let l = r.lines();
        if drop_output {
            assert!(l.is_empty(), "{l:?}");
        } else {
            assert_eq!(l.len(), 2, "{l:?}");
        }
        // Pasada la falla vuelve a contestar.
        assert_eq!(r.ask("G0X2"), ["ok"]);
        // Lo que el anfitrión envió en silencio sí se procesó.
        assert_eq!(r.h.received_lines()[0], "G0X1");
    }
}

#[test]
fn fault_disconnect_drains_then_breaks_the_pipe() {
    let mut r = classic();
    r.line("G0X1");
    r.h.inject(Trigger::Now, Fault::Disconnect);
    let mut buf = [0u8; 64];
    let n =
        r.vm.read_timeout(&mut buf, Duration::from_millis(1))
            .unwrap();
    assert_eq!(&buf[..n], b"ok\r\n");
    let e =
        r.vm.read_timeout(&mut buf, Duration::from_millis(1))
            .unwrap_err();
    assert_eq!(e.kind(), ErrorKind::BrokenPipe);
    assert_eq!(
        r.vm.write_all(b"G0X2\n").unwrap_err().kind(),
        ErrorKind::BrokenPipe
    );
    assert!(!r.h.is_connected());
}

#[test]
fn fault_nan_offsets() {
    let mut r = Rig::new(Profile::falcon2_pro());
    r.drain();
    assert_eq!(r.ask("$X").last().unwrap(), "ok");
    r.h.inject(Trigger::Now, Fault::NanOffsets { in_status: false });
    let l = r.ask("$#");
    assert_eq!(l[0], "[G54:-nan,1.579,1.619]");
    assert!(!r.status().contains("nan"));
    r.h.inject(Trigger::Now, Fault::NanOffsets { in_status: true });
    r.send("G92X1\n");
    r.drain();
    let st = r.status();
    assert!(st.contains("WCO:-nan,"), "{st}");
}

#[test]
fn fault_reboot_in_idle_only_prints_banner() {
    let mut r = classic();
    r.h.inject(Trigger::Now, Fault::Reboot);
    assert_eq!(r.lines(), ["Grbl 1.1h ['$' for help]"]);
    assert_eq!(r.h.state(), StateKind::Idle);
}

// ---------------------------------------------------------------------------
// Perfiles
// ---------------------------------------------------------------------------

#[test]
fn falcon_status_lines_match_the_captures() {
    let mut r = Rig::with(Profile::falcon2_pro(), |b| b.position([50.0, 50.0, 0.0]));
    r.drain();
    // Con $22=1 arranca bloqueada: se desbloquea con homing.
    assert_eq!(r.h.state(), StateKind::Alarm(0));
    assert_eq!(r.ask("$H"), Vec::<String>::new());
    r.ms(500);
    let st = r.status();
    assert!(st.starts_with("<Home|MPos:"), "{st}");
    assert!(st.contains("|Bf:127,65535|FS:4000,0|"), "{st}");
    r.secs(10);
    assert_eq!(r.lines(), ["ok"]);
    assert_eq!(
        r.status(),
        "<Idle|MPos:0.000,0.000,0.000|Bf:127,65535|FS:0,0|Pn:XY>"
    );
    // El primer reporte de una placa ya referenciada es el de la captura del documento.
    let mut f = Rig::with(Profile::falcon2_pro(), |b| b.homed(true));
    f.drain();
    assert_eq!(
        f.status(),
        "<Idle|MPos:0.000,0.000,0.000|Bf:127,65535|FS:0,0|Pn:XY|WCO:0.000,0.000,0.000>"
    );
    // En movimiento: sin Pn y con Bf decreciente, MPos positivo hacia dentro.
    for l in ["M4S0", "G1X100Y100F3000S300"] {
        r.line(l);
    }
    r.ms(1000);
    r.drain();
    let st = r.status();
    assert!(st.starts_with("<Run|MPos:"), "{st}");
    assert!(st.contains("|Bf:126,65"), "{st}");
    assert!(!st.contains("Pn:"));
    assert!(!st.contains("Ov:"), "la Falcon no informa Ov");
    assert!(r.h.mpos()[0] > 0.0);
}

#[test]
fn falcon_settings_dump_has_creality_entries() {
    let mut r = Rig::new(Profile::falcon2_pro());
    r.drain();
    let l = r.ask("$$");
    for want in [
        "$10=3",
        "$21=1",
        "$22=1",
        "$23=3",
        "$30=1000",
        "$32=1",
        "$100=80.000",
        "$150=36",
        "$151=100",
        "$152=30",
        "$154=1",
        "$155=1",
        "$161=13.00",
        "$130=400.000",
    ] {
        assert!(l.contains(&want.to_string()), "falta {want}");
    }
    assert!(!l.iter().any(|x| x.starts_with("$102=")));
    assert_eq!(l.last().unwrap(), "ok");
}

#[test]
fn falcon_reset_locks_with_homing_and_accepts_air_assist() {
    let mut r = Rig::new(Profile::falcon2_pro());
    r.drain();
    r.send("$H\n");
    r.secs(10);
    r.drain();
    assert_eq!(r.ask("M8"), ["ok"]);
    assert_eq!(r.ask("M9"), ["ok"]);
    r.rt(0x18);
    let l = r.lines();
    assert!(l.contains(&"Grbl 1.1f ['$' for help]".to_string()));
    assert!(
        !l.iter().any(|x| x.contains("unlock")),
        "la Falcon no imprime el aviso"
    );
    assert_eq!(r.h.state(), StateKind::Alarm(0));
    assert_eq!(r.ask("G0X1"), ["error:9"]);
}

#[test]
fn grblhal_extras() {
    let mut r = Rig::new(Profile::grbl_hal());
    r.drain();
    let st = r.status();
    assert!(
        st.starts_with("<Idle|MPos:0.000,0.000,0.000|Bf:35,1023|FS:0,0,0|"),
        "{st}"
    );
    assert!(st.contains("|WCS:G54|H:0>"), "{st}");
    // 0x80 hace de ?, 0x87 da el reporte completo.
    r.rt(0x80);
    assert!(r.lines()[0].starts_with("<Idle|"));
    r.rt(0x87);
    let full = r.lines().remove(0);
    assert!(
        full.contains("|WCO:") && full.contains("|Ov:100,100,100"),
        "{full}"
    );
    // Estado Alarm:N.
    r.h.inject(Trigger::Now, Fault::Alarm(11));
    r.drain();
    assert!(r.status().starts_with("<Alarm:11|"));
    r.ask("$X");
    // Cambio de herramienta: estado Tool hasta ~.
    r.line("M6T2");
    r.ms(10);
    assert_eq!(r.h.state(), StateKind::Tool);
    assert!(r.status().starts_with("<Tool|"));
    assert!(r.lines().is_empty());
    r.rt(b'~');
    r.ms(5);
    assert_eq!(r.lines(), ["ok"]);
    assert_eq!(r.h.state(), StateKind::Idle);
    // Línea numerada e informe Ln con $10 bit 2.
    r.ask("$10=127");
    r.line("N77G1X50F600");
    r.ms(100);
    r.drain();
    assert!(r.status().contains("|Ln:77|"));
    // M6 no existe en GRBL clásico.
    let mut c = classic();
    assert_eq!(c.ask("M6"), ["error:20"]);
}

#[test]
fn fluidnc_and_lpc_basic_session() {
    for p in [Profile::fluidnc(), Profile::grbl_lpc()] {
        let mut r = Rig::new(p);
        r.drain();
        assert_eq!(r.ask("G90"), ["ok"]);
        assert!(r.status().starts_with("<Idle|"));
        r.rt(0x18);
        assert!(r.lines().iter().any(|l| l.starts_with("Grbl")));
    }
}

#[test]
fn marlin_v7_inline_job() {
    let mut r = Rig::new(Profile::marlin());
    assert_eq!(r.lines(), ["start", "echo:Marlin 2.1.2.5"]);
    let job = [
        "G90",
        "G21",
        "M3 S0 I",
        "M8",
        "G0 X0 Y0 F6000",
        "G1 X10 S128 F1000",
        "G1 Y10",
        "G1 X0",
        "G1 Y0",
        "M9",
        "M5",
        "G0 X0 Y0 F6000",
    ];
    for l in job {
        // Ping-pong: una línea, esperar su ok.
        r.line(l);
        let mut got = Vec::new();
        for _ in 0..600_000 {
            r.ms(1);
            got = r.lines();
            if !got.is_empty() {
                break;
            }
        }
        assert_eq!(got, ["ok"], "tras {l}");
    }
    let t = r.h.trace();
    assert!(t.burned_during_rapids().is_empty());
    assert!(
        (t.total_length() - 40.0).abs() < 0.05,
        "{}",
        t.total_length()
    );
    assert!(t.segments().iter().all(|s| s.s_programmed == 128.0));
    let l = r.ask("M115");
    assert!(l[0].starts_with("FIRMWARE_NAME:Marlin"));
    assert_eq!(l[1], "ok");
    assert_eq!(
        r.ask("M114"),
        ["X:0.00 Y:0.00 Z:0.00 E:0.00 Count X: 0 Y:0 Z:0", "ok"]
    );
    // Sin $ ni ?: se tratan como comandos desconocidos.
    assert_eq!(r.ask("$$"), ["echo:Unknown command: \"$$\"", "ok"]);
    r.rt(b'?');
    assert!(r.lines().is_empty());
}

#[test]
fn smoothie_basic_session() {
    let mut r = Rig::new(Profile::smoothie());
    assert_eq!(r.lines(), ["Smoothie"]);
    for l in ["G90", "G21", "M3 S0", "G0 X0 Y0 F3000"] {
        assert_eq!(r.ask(l), ["ok"], "{l}");
    }
    r.line("G1 X10 S0.5 F1000");
    r.ms(5);
    assert_eq!(r.lines(), ["ok"]);
    assert!(
        r.ask("M400").is_empty(),
        "M400 espera al fin del movimiento"
    );
    r.secs(2);
    assert_eq!(r.lines(), ["ok"]);
    let peak =
        r.h.trace()
            .segments()
            .iter()
            .map(|s| s.s_peak)
            .fold(0.0, f64::max);
    assert!((peak - 0.5).abs() < 1e-9, "{peak}");
    assert!(r.ask("M115")[0].starts_with("FIRMWARE_NAME:Smoothieware"));
}

#[test]
fn every_profile_answers_a_trivial_session() {
    for p in Profile::all() {
        let name = p.name.clone();
        let mut r = Rig::new(p);
        assert!(!r.lines().is_empty(), "{name}: sin banner");
        r.h.inject(Trigger::Now, Fault::Reboot);
        assert!(!r.lines().is_empty(), "{name}: sin banner tras reboot");
    }
}

// ---------------------------------------------------------------------------
// Transporte y relojes
// ---------------------------------------------------------------------------

#[test]
fn transport_reads_with_timeout_and_clear_input() {
    use tlaku_emu::{AutoClock, VirtualMachine};
    let clock = AutoClock::new();
    let (mut vm, h) = VirtualMachine::builder(Profile::grbl_1_1h())
        .clock(clock.clone())
        .build();
    let mut buf = [0u8; 8];
    // Primero el banner, troceado por el tamaño del búfer.
    let n = vm.read_timeout(&mut buf, Duration::from_millis(5)).unwrap();
    assert_eq!(&buf[..n], b"Grbl 1.1");
    vm.clear_input().unwrap();
    // Sin datos: espera el tiempo pedido (simulado) y devuelve 0.
    let t0 = h.now();
    assert_eq!(
        vm.read_timeout(&mut buf, Duration::from_millis(20))
            .unwrap(),
        0
    );
    assert!(h.now() - t0 >= Duration::from_millis(20));
    // Un movimiento lento avanza solo mientras el anfitrión espera.
    vm.write_all(b"G1X5F60\n").unwrap();
    assert!(h.wait_idle(Duration::from_secs(30)));
    assert_eq!(h.mpos()[0], 5.0);
}

#[test]
fn works_as_grbl_command_transport() {
    use tlaku_emu::VirtualMachine;
    use tlaku_grbl::serial::Grbl;
    let (vm, _h) = VirtualMachine::builder(Profile::grbl_1_1h())
        .banner_on_connect(false)
        .build();
    let mut grbl = Grbl::with_transport(Box::new(vm));
    let lines = grbl.command("$I", Duration::from_secs(1)).unwrap();
    assert_eq!(lines, ["[VER:1.1h.20190825:]", "[OPT:V,15,128]"]);
    let e = grbl.command("G99", Duration::from_secs(1)).unwrap_err();
    assert!(e.to_string().contains("error:20"));
}

#[test]
fn scaled_clock_runs_in_wall_time() {
    use tlaku_emu::{ScaledClock, VirtualMachine};
    let (mut vm, h) = VirtualMachine::builder(Profile::grbl_1_1h())
        .clock(ScaledClock::new(100.0))
        .build();
    vm.write_all(b"G1X50F600\n").unwrap();
    // 5 s simulados son ~50 ms reales.
    let start = std::time::Instant::now();
    let mut buf = [0u8; 64];
    while h.state() != StateKind::Idle || start.elapsed() < Duration::from_millis(5) {
        let _ = vm.read_timeout(&mut buf, Duration::from_millis(2)).unwrap();
        assert!(start.elapsed() < Duration::from_secs(5));
    }
    assert!(start.elapsed() < Duration::from_secs(2));
    assert_eq!(h.mpos()[0], 50.0);
}
