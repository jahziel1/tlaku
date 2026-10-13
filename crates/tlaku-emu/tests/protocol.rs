//! Vectores de `docs/hardware/grbl.md` (secciones 11.1 a 11.9) contra el emulador.

mod common;

use common::Rig;
use tlaku_emu::{Fault, Profile, StateKind, Trigger};
use tlaku_grbl::streamer::{MachineState, StatusReport};

fn classic() -> Rig {
    let mut r = Rig::new(Profile::grbl_1_1h());
    r.drain();
    r
}

#[test]
fn v1_banner_classic() {
    let mut r = Rig::new(Profile::grbl_1_1h());
    assert_eq!(r.lines(), ["Grbl 1.1h ['$' for help]"]);
}

#[test]
fn v2_alarm_on_boot_with_homing() {
    let mut r = Rig::new(Profile::grbl_1_1h().with_setting(22, 1.0));
    assert_eq!(
        r.lines(),
        ["Grbl 1.1h ['$' for help]", "[MSG:'$H'|'$X' to unlock]"]
    );
    let st = r.status();
    let rep = StatusReport::parse(&st).unwrap();
    assert_eq!(rep.state, MachineState::Alarm);
    // En alarma se rechaza el G-code.
    assert_eq!(r.ask("G0X1"), ["error:9"]);
}

#[test]
fn v3_to_v5_banners() {
    let mut r = Rig::new(Profile::grbl_hal());
    assert_eq!(r.lines(), ["GrblHAL 1.1f ['$' or '$HELP' for help]"]);
    let mut r = Rig::new(Profile::fluidnc());
    let l = r.lines();
    assert!(l[0].starts_with("[MSG:INFO: FluidNC v3.9.7"));
    assert_eq!(
        l.last().unwrap(),
        "Grbl 3.9 [FluidNC v3.9.7 (wifi) '$' for help]"
    );
    let mut r = Rig::new(Profile::falcon2_pro());
    assert_eq!(
        r.lines(),
        [
            "usb connect succeed",
            "[OK]system start-up CV50-Pro-MASTER-Release V1.0.1 20240325-1042",
            "[OK] ILmp_app_init OK",
            "[OK] gas_init OK",
            "[MSG:Using cpu_map…esp32-s3]",
            "Grbl 1.1f ['$' for help]",
            "[OK] ILmp Link connect ok",
        ]
    );
}

#[test]
fn s1_idle_report_and_wco_cadence() {
    let mut r = classic();
    // El primer reporte tras el arranque trae WCO y Ov; los siguientes no.
    let first = r.status();
    assert_eq!(
        first,
        "<Idle|MPos:0.000,0.000,0.000|FS:0,0|WCO:0.000,0.000,0.000|Ov:100,100,100>"
    );
    let second = r.status();
    assert_eq!(second, "<Idle|MPos:0.000,0.000,0.000|FS:0,0>");
    let rep = StatusReport::parse(&second).unwrap();
    assert_eq!(rep.state, MachineState::Idle);
    assert_eq!(rep.feed, Some(0.0));
    // WCO reaparece cada 30 reportes en reposo.
    let mut seen = 0;
    for _ in 0..40 {
        if r.status().contains("WCO:") {
            seen += 1;
        }
    }
    assert_eq!(seen, 1);
}

#[test]
fn s2_bf_with_dollar10() {
    let mut r = classic();
    assert_eq!(r.ask("$10=3"), ["ok"]);
    let _ = r.status();
    let rep = StatusReport::parse(&r.status()).unwrap();
    let bf = rep.buffer.unwrap();
    assert_eq!((bf.planner_blocks, bf.rx_bytes), (15, 127));
    // WPos con $10=0 y WCO desplazado por G92.
    assert_eq!(r.ask("$10=0"), ["ok"]);
    assert_eq!(r.ask("G92X5Y6"), ["ok"]);
    let st = r.status();
    assert!(
        st.starts_with("<Idle|WPos:5.000,6.000,0.000|FS:0,0|WCO:"),
        "{st}"
    );
}

#[test]
fn s3_run_with_ov_a_and_fs() {
    let mut r = classic();
    r.ask("$10=3");
    r.line("M4S0");
    r.line("G1X100F600S500");
    r.ms(500);
    r.drain();
    r.settle_status_counters();
    let st = r.status();
    let rep = StatusReport::parse(&st).unwrap();
    assert_eq!(rep.state, MachineState::Run);
    assert!(rep.feed.unwrap() > 0.0);
    assert!(rep.spindle.unwrap() > 0.0);
    let x = rep.mpos.unwrap().x;
    assert!(x > 0.0 && x < 100.0, "{x}");
}

impl Rig {
    /// Consume los campos periódicos para que el siguiente reporte sea "normal".
    fn settle_status_counters(&mut self) {
        for _ in 0..2 {
            let _ = self.status();
        }
    }
}

#[test]
fn s6_hold_substates_and_resume() {
    let mut r = classic();
    r.line("G1X100F600");
    r.ms(500);
    r.rt(b'!');
    assert_eq!(r.h.state(), StateKind::Hold(1));
    r.ms(300);
    assert_eq!(r.h.state(), StateKind::Hold(0));
    let x = r.h.mpos()[0];
    r.ms(500);
    assert_eq!(r.h.mpos()[0], x, "en Hold:0 no se mueve");
    r.rt(b'~');
    assert_eq!(r.h.state(), StateKind::Run);
    r.ms(300);
    assert!(r.h.mpos()[0] > x);
    r.settle();
    assert_eq!(r.h.mpos()[0], 100.0);
}

#[test]
fn m1_messages_dollar_x_and_check_mode() {
    let mut r = Rig::new(Profile::grbl_1_1h().with_setting(22, 1.0));
    r.drain();
    assert_eq!(r.ask("$X"), ["[MSG:Caution: Unlocked]", "ok"]);
    assert_eq!(r.h.state(), StateKind::Idle);
    assert_eq!(r.ask("$C"), ["[MSG:Enabled]", "ok"]);
    assert_eq!(r.h.state(), StateKind::Check);
    // En modo de verificación no hay movimiento, pero se valida el G-code.
    assert_eq!(r.ask("G1X10F100"), ["ok"]);
    assert_eq!(r.h.mpos(), [0.0; 3]);
    assert_eq!(r.ask("G99"), ["error:20"]);
    let l = r.ask("$C");
    assert_eq!(l[0], "[MSG:Disabled]");
    assert_eq!(l[1], "ok");
    // Salir hace soft reset: banner (y alarma de homing, $22=1).
    assert!(l[2].starts_with("Grbl 1.1h"));
}

#[test]
fn e1_gcode_errors() {
    let mut r = classic();
    assert_eq!(r.ask("G99"), ["error:20"]);
    assert_eq!(r.ask("XYZZY"), ["error:2"]);
    assert_eq!(r.ask("1G"), ["error:1"]);
    assert_eq!(r.ask("G1X10"), ["error:22"], "sin F definido");
    assert_eq!(r.ask("G1X10F-5"), ["error:4"]);
    assert_eq!(r.ask("G0X1X2"), ["error:25"]);
    assert_eq!(r.ask("G0G1X1"), ["error:21"]);
    assert_eq!(r.ask("G1.5"), ["error:23"]);
    assert_eq!(r.ask("N99999999G0"), ["error:27"]);
    assert_eq!(r.ask("G4"), ["error:28"]);
    assert_eq!(r.ask("G10L2X1"), ["error:28"]);
    assert_eq!(r.ask("G53G2X1"), ["error:30"]);
    assert_eq!(r.ask("G80X1"), ["error:31"]);
    assert_eq!(r.ask("G2F100I1"), ["error:32"]);
    assert_eq!(r.ask("G2X1Y1F100"), ["error:35"]);
    assert_eq!(r.ask("G2X1Y1R0.1F100"), ["error:34"]);
    assert_eq!(r.ask("G0P5"), ["error:36"]);
    assert_eq!(r.ask("M7"), ["error:20"]);
    assert_eq!(r.ask("Q1"), ["error:20"]);
    // Una línea vacía contesta ok.
    assert_eq!(r.ask(""), ["ok"]);
    assert_eq!(
        r.h.mpos(),
        [0.0; 3],
        "ninguna línea con error debe mover nada"
    );
}

#[test]
fn e1b_line_too_long() {
    let mut r = classic();
    // 80 caracteres limpios caben; 81 dan error:11 (límite inferido, ver profile.rs).
    let ok = format!("G0X{}", "0".repeat(77));
    assert_eq!(ok.len(), 80);
    assert_eq!(r.ask(&ok), ["ok"]);
    let long = format!("G0X{}", "0".repeat(78));
    assert_eq!(r.ask(&long), ["error:11"]);
    // Los comentarios y espacios no cuentan.
    let commented = format!("G0 X0 ({})", "c".repeat(200));
    assert_eq!(r.ask(&commented), ["ok"]);
}

#[test]
fn e2_e3_dollar_during_run_and_gcode_in_alarm() {
    let mut r = classic();
    r.line("G1X100F600");
    r.ms(100);
    r.drain();
    assert_eq!(r.ask("$$"), ["error:8"]);
    assert_eq!(r.ask("$G"), ["error:8"]);
    assert_eq!(r.ask("$X"), ["error:8"]);
    assert_eq!(r.ask("$J=G91X1F100"), ["error:8"]);
    r.rt(0x18);
    let _ = r.lines();
    assert_eq!(r.ask("G0X1"), ["error:9"]);
    assert_eq!(r.ask("$J=G91X1F100"), ["error:9"]);
}

#[test]
fn e4_to_e6_jog_errors() {
    let mut r = Rig::new(Profile::falcon2_pro());
    r.drain();
    assert_eq!(r.ask("$H"), Vec::<String>::new());
    r.secs(10);
    assert_eq!(r.lines(), ["ok"]);
    assert_eq!(r.ask("$20=1"), ["ok"]);
    assert_eq!(r.ask("$J=G91 G21 X999 F1000"), ["error:15"]);
    assert_eq!(r.ask("$JG91X1F100"), ["error:16"]);
    assert_eq!(r.ask("$J=G1 X1 F100"), ["error:16"]);
    assert_eq!(r.ask("$J=G91 G21 X1"), ["error:22"]);
    assert_eq!(r.ask("$J=G91 G21 F100"), ["error:16"]);
    assert_eq!(r.ask("$J=G91 G21 M3 X1 F100"), ["error:16"]);
    assert_eq!(r.ask("$J=G91 G21 X1 F100"), ["ok"]);
    assert_eq!(r.h.state(), StateKind::Jog);
}

#[test]
fn jog_moves_cancels_and_does_not_alter_parser() {
    let mut r = classic();
    assert_eq!(r.ask("$J=G91G21X50F600"), ["ok"]);
    assert_eq!(r.h.state(), StateKind::Jog);
    r.ms(500);
    let st = r.status();
    assert!(st.starts_with("<Jog|"), "{st}");
    r.rt(0x85);
    r.ms(300);
    assert_eq!(r.h.state(), StateKind::Idle);
    let x = r.h.mpos()[0];
    assert!(x > 0.0 && x < 50.0, "{x}");
    // El parser sigue en G90 y sin F.
    assert_eq!(
        r.ask("$G"),
        ["[GC:G0 G54 G17 G21 G90 G94 M5 M9 T0 F0.0 S0]", "ok"]
    );
    // Un jog posterior es relativo al punto de parada.
    assert_eq!(r.ask("$J=G91G21X1F600"), ["ok"]);
    r.settle();
    assert!((r.h.mpos()[0] - (x + 1.0)).abs() < 1e-6);
}

#[test]
fn e7_hard_limit_alarm_silences_status() {
    let mut r = classic();
    r.line("G1X100F600");
    r.ms(200);
    r.drain();
    r.h.inject(Trigger::Now, Fault::Alarm(1));
    assert_eq!(r.lines(), ["ALARM:1", "[MSG:Reset to continue]"]);
    r.rt(b'?');
    assert!(r.lines().is_empty(), "tras ALARM:1 el ? no se contesta");
    r.rt(0x18);
    let l = r.lines();
    assert_eq!(l[0], "Grbl 1.1h ['$' for help]");
    assert!(r.status().starts_with("<Idle|"));
}

#[test]
fn e8_reset_in_hold1_raises_alarm3_then_banner() {
    let mut r = classic();
    r.line("G1X100F600");
    r.ms(500);
    r.drain();
    r.rt(b'!');
    assert_eq!(r.h.state(), StateKind::Hold(1));
    r.rt(0x18);
    assert_eq!(
        r.lines(),
        [
            "ALARM:3",
            "Grbl 1.1h ['$' for help]",
            "[MSG:'$H'|'$X' to unlock]"
        ]
    );
    assert_eq!(r.h.state(), StateKind::Alarm(3));
    assert!(r.h.position_lost());
    let st = r.status();
    assert!(st.starts_with("<Alarm|"), "{st}");
}

#[test]
fn e9_reset_in_hold0_or_idle_keeps_position() {
    let mut r = classic();
    r.line("G1X10F600");
    r.ms(200);
    r.drain();
    r.rt(b'!');
    r.ms(500);
    assert_eq!(r.h.state(), StateKind::Hold(0));
    let x = r.h.mpos()[0];
    r.rt(0x18);
    assert_eq!(r.lines(), ["Grbl 1.1h ['$' for help]"]);
    assert_eq!(r.h.state(), StateKind::Idle);
    assert!(!r.h.position_lost());
    assert_eq!(r.h.mpos()[0], x);
    // Reset en reposo tampoco alarma.
    r.rt(0x18);
    assert_eq!(r.lines(), ["Grbl 1.1h ['$' for help]"]);
}

#[test]
fn e10_homing_success_failure_and_silence() {
    let mut r = Rig::new(Profile::grbl_1_1h().with_setting(22, 1.0));
    r.drain();
    assert_eq!(r.ask("$H"), Vec::<String>::new());
    assert_eq!(r.h.state(), StateKind::Home);
    // Clásico: ? sin respuesta durante el homing.
    r.rt(b'?');
    assert!(r.lines().is_empty());
    r.secs(4);
    assert_eq!(r.lines(), ["ok"]);
    assert_eq!(r.h.state(), StateKind::Idle);
    // Fallo con ALARM:9 (sin ok).
    assert_eq!(r.ask("$H"), Vec::<String>::new());
    r.h.inject(Trigger::Now, Fault::Alarm(9));
    assert_eq!(r.lines(), ["ALARM:9"]);
    assert_eq!(r.h.state(), StateKind::Alarm(9));
    // Reset durante el homing: ALARM:6.
    r.rt(0x18);
    r.drain();
    assert_eq!(r.ask("$H"), Vec::<String>::new());
    r.rt(0x18);
    assert_eq!(r.lines()[0], "ALARM:6");
}

#[test]
fn e10b_homing_without_enable_is_error5() {
    let mut r = classic();
    assert_eq!(r.ask("$H"), ["error:5"]);
}

#[test]
fn e11_grblhal_persistent_error() {
    let mut r = Rig::new(Profile::grbl_hal());
    r.drain();
    assert_eq!(r.ask("G1X1F100X2"), ["error:25"]);
    assert_eq!(r.ask("G1X2"), ["error:25"]);
    assert_eq!(r.ask(""), ["ok"]);
    assert_eq!(r.ask("G1X2F100"), ["ok"]);
    r.settle();
    // Un comando $ también lo limpia.
    assert_eq!(r.ask("G1X1F100X2"), ["error:25"]);
    assert_eq!(r.ask("$G").last().unwrap(), "ok");
    assert_eq!(r.ask("G1X3"), ["ok"]);
}

#[test]
fn i1_to_i4_info() {
    let mut r = classic();
    assert_eq!(
        r.ask("$I"),
        ["[VER:1.1h.20190825:]", "[OPT:V,15,128]", "ok"]
    );
    assert_eq!(r.ask("$I=Ejemplo"), ["ok"]);
    assert_eq!(r.ask("$I")[0], "[VER:1.1h.20190825:Ejemplo]");
    let mut r = Rig::new(Profile::fluidnc());
    r.drain();
    assert_eq!(
        r.ask("$I"),
        ["[VER:3.9 FluidNC v3.9.7:]", "[OPT:MPHS]", "ok"]
    );
    let mut r = Rig::new(Profile::grbl_hal());
    r.drain();
    assert_eq!(
        r.ask("$I"),
        [
            "[VER:1.1f.20240101:]",
            "[OPT:VNM,35,1024,0]",
            "[NEWOPT:ENUMS,RT+]",
            "ok"
        ]
    );
    let mut r = Rig::new(Profile::falcon2_pro());
    r.drain();
    let l = r.ask("$I");
    assert_eq!(l[0], "[VER:1.1f.20220810:]");
    assert_eq!(l[1], "[OPT:VHL,127,65536]");
    assert!(l[2].starts_with("[software versions:"));
    assert!(l[3].starts_with("inquire_info->power:22W"));
    assert_eq!(l[4], "ok");
}

#[test]
fn p1_settings_dump_matches_vector() {
    let mut r = classic();
    let l = r.ask("$$");
    let want = "$0=10 $1=25 $2=0 $3=0 $4=0 $5=0 $6=0 $10=1 $11=0.010 $12=0.002 $13=0 $20=0 $21=0 \
$22=0 $23=0 $24=25.000 $25=500.000 $26=250 $27=1.000 $30=1000 $31=0 $32=1 $100=80.000 \
$101=80.000 $102=250.000 $110=6000.000 $111=6000.000 $112=500.000 $120=1000.000 \
$121=1000.000 $122=10.000 $130=400.000 $131=400.000 $132=200.000 ok";
    assert_eq!(l.join(" "), want);
}

#[test]
fn settings_write_validation() {
    let mut r = classic();
    assert_eq!(r.ask("$30=255"), ["ok"]);
    assert_eq!(
        r.ask("$$").iter().find(|l| l.starts_with("$30=")).unwrap(),
        "$30=255"
    );
    assert_eq!(r.ask("$11=0.05"), ["ok"]);
    assert_eq!(
        r.ask("$$").iter().find(|l| l.starts_with("$11=")).unwrap(),
        "$11=0.050"
    );
    assert_eq!(r.ask("$999=1"), ["error:3"]);
    assert_eq!(r.ask("$30=abc"), ["error:2"]);
    assert_eq!(r.ask("$30=-1"), ["error:4"]);
    assert_eq!(r.ask("$0=2"), ["error:6"]);
    assert_eq!(r.ask("$20=1"), ["error:10"]);
    assert_eq!(r.ask("$Z"), ["error:3"]);
}

#[test]
fn g1_g2_parser_state() {
    let mut r = classic();
    assert_eq!(
        r.ask("$G"),
        ["[GC:G0 G54 G17 G21 G90 G94 M5 M9 T0 F0.0 S0]", "ok"]
    );
    assert_eq!(r.ask("M4 S500"), ["ok"]);
    assert_eq!(r.ask("G1 X0 F1000"), ["ok"]);
    assert_eq!(
        r.ask("$G"),
        ["[GC:G1 G54 G17 G21 G90 G94 M4 M9 T0 F1000 S500]", "ok"]
    );
}

#[test]
fn h1_parameters_n1_help_x1() {
    let mut r = classic();
    let l = r.ask("$#");
    assert_eq!(l.len(), 12);
    assert_eq!(l[0], "[G54:0.000,0.000,0.000]");
    assert_eq!(l[6], "[G28:0.000,0.000,0.000]");
    assert_eq!(l[8], "[G92:0.000,0.000,0.000]");
    assert_eq!(l[9], "[TLO:0.000]");
    assert_eq!(l[10], "[PRB:0.000,0.000,0.000:0]");
    assert_eq!(r.ask("$N"), ["$N0=", "$N1=", "ok"]);
    assert_eq!(r.ask("$N0=G21G90"), ["ok"]);
    assert_eq!(r.ask("$N"), ["$N0=G21G90", "$N1=", "ok"]);
    assert_eq!(
        r.ask("$"),
        [
            "[HLP:$$ $# $G $I $N $x=val $Nx=line $J=line $C $X $H ~ ! ? ctrl-x]",
            "ok"
        ]
    );
    // G10 L2 y G92 aparecen en $#.
    assert_eq!(r.ask("G10L2P1X5Y6"), ["ok"]);
    assert_eq!(r.ask("G92X1"), ["ok"]);
    let l = r.ask("$#");
    assert_eq!(l[0], "[G54:5.000,6.000,0.000]");
    assert!(l[8].starts_with("[G92:"), "{l:?}");
    // El startup se ejecuta tras un reset.
    r.rt(0x18);
    assert_eq!(r.lines(), ["Grbl 1.1h ['$' for help]", ">G21G90:ok"]);
}

#[test]
fn rst_restores_defaults_and_resets() {
    let mut r = classic();
    r.ask("$30=255");
    let l = r.ask("$RST=$");
    assert_eq!(l[0], "[MSG:Restoring defaults]");
    assert_eq!(l[1], "ok");
    assert!(l[2].starts_with("Grbl 1.1h"));
    assert_eq!(r.h.setting(30), 1000.0);
}

#[test]
fn l1_line_endings_and_fragments() {
    let mut r = classic();
    // \r\n, \n, \r: cada terminador cierra una línea (los vacíos responden ok).
    r.send("G0X1\r\n");
    assert_eq!(r.lines(), ["ok", "ok"]);
    r.send("G0X2\n");
    assert_eq!(r.lines(), ["ok"]);
    r.send("G0X3\r");
    assert_eq!(r.lines(), ["ok"]);
    // Fragmentos: nada hasta el terminador.
    r.send("G0X");
    r.send("4");
    assert!(r.lines().is_empty());
    r.send("\n");
    assert_eq!(r.lines(), ["ok"]);
    // Fallas de terminador de salida.
    r.h.inject(Trigger::Now, Fault::LineEnding(tlaku_emu::LineEnding::Lf));
    r.send("G0X5\n");
    assert_eq!(r.raw(), b"ok\n");
    r.h.inject(Trigger::Now, Fault::LineEnding(tlaku_emu::LineEnding::CrLf));
    r.send("G0X6\n");
    assert_eq!(r.raw(), b"ok\r\n");
}

#[test]
fn realtime_bytes_never_enter_the_buffer() {
    let mut r = classic();
    r.send("G0X1");
    r.rt(b'?');
    r.rt(0x91);
    r.rt(0xFF);
    r.send("0\n");
    let l = r.lines();
    assert!(l[0].starts_with("<Idle|"));
    assert_eq!(l[1], "ok");
    assert_eq!(r.h.received_lines(), ["G0X10"]);
    assert_eq!(r.h.realtime_bytes(), [b'?', 0x91, 0xFF]);
    r.settle();
    assert_eq!(r.h.mpos()[0], 10.0);
}

#[test]
fn overrides_change_and_report() {
    let mut r = classic();
    r.rt(0x91);
    r.rt(0x93);
    r.rt(0x96);
    r.rt(0x9B);
    let st = r.status();
    assert!(st.contains("Ov:111,50,90"), "{st}");
    r.rt(0x90);
    r.rt(0x95);
    r.rt(0x99);
    let st = r.status();
    assert!(st.contains("Ov:100,100,100"), "{st}");
    // Los límites son 10 %-200 %.
    for _ in 0..30 {
        r.rt(0x92);
    }
    let st = r.status();
    assert!(st.contains("Ov:10,"), "{st}");
}

#[test]
fn feed_override_changes_duration() {
    fn duration(ovr: &[u8]) -> f64 {
        let mut r = classic();
        for &b in ovr {
            r.rt(b);
        }
        r.line("G1X60F600");
        r.settle();
        r.h.now().as_secs_f64()
    }
    let base = duration(&[]);
    let fast = duration(&[0x91, 0x91, 0x91, 0x91, 0x91, 0x91, 0x91, 0x91, 0x91, 0x91]);
    assert!(fast < base * 0.7, "{fast} vs {base}");
}

#[test]
fn door_cycle() {
    let mut r = classic();
    r.line("G1X100F600");
    r.ms(300);
    r.drain();
    r.rt(0x84);
    assert_eq!(r.h.state(), StateKind::Door(2));
    assert_eq!(r.lines(), ["[MSG:Check Door]"]);
    r.ms(300);
    assert_eq!(r.h.state(), StateKind::Door(1));
    r.rt(b'~');
    assert_eq!(
        r.h.state(),
        StateKind::Door(1),
        "con la puerta abierta no se reanuda"
    );
    r.h.set_door_closed(true);
    assert_eq!(r.h.state(), StateKind::Door(0));
    r.rt(b'~');
    assert_eq!(r.h.state(), StateKind::Door(3));
    r.ms(200);
    assert_eq!(r.h.state(), StateKind::Run);
    r.settle();
    assert_eq!(r.h.mpos()[0], 100.0);
}

#[test]
fn sleep_then_reset_alarm() {
    let mut r = classic();
    assert_eq!(r.ask("$SLP"), ["[MSG:Sleeping]", "ok"]);
    assert_eq!(r.h.state(), StateKind::Sleep);
    r.rt(0x18);
    assert_eq!(r.h.state(), StateKind::Alarm(0));
}

#[test]
fn nan_wco_in_status_does_not_poison_the_parser() {
    // Falcon2 con offsets dañados: el reporte trae `WCO:-nan,...`. El host no debe propagar NaN.
    let mut r = Rig::with(Profile::falcon2_pro(), |b| b.homed(true));
    r.drain();
    r.h.inject(Trigger::Now, Fault::NanOffsets { in_status: true });
    let st = r.status();
    assert!(st.contains("WCO:-nan,"), "{st}");
    let rep = StatusReport::parse(&st).unwrap();
    if let Some(w) = rep.wco {
        assert!(
            w.x.is_finite() && w.y.is_finite() && w.z.is_finite(),
            "{w:?}"
        );
    }
}
