//! Resistencia con el trabajador real (`Connection`/`Worker`): un trabajo largo con forma de
//! trama de imagen (muchas líneas cortas con `S` variable) contra la Falcon2 Pro emulada, con las
//! fallas medidas de la placa real (`docs/hardware/falcon2.md` §10.4–10.6): estado cortado
//! pegado al `ok`, la placa que deja de leer el USB (el `write` se cuelga y la placa se para),
//! silencios, `?` perdidos, lecturas partidas y el planificador lleno (`Bf:0`).
//!
//! La prueba grande con el pipeline real y 50 000–120 000 líneas está en
//! `crates/tlaku-emu/tests/soak.rs`; aquí se prueba lo que añade el trabajador: el avance que ve
//! la interfaz, el punto de control y qué pasa con una pausa de «40 s» (a escala: 8 veces el
//! plazo de silencio de `fast()`, igual que 40 s frente a 5 s).

use super::resume::{Bench, long_job as slow_rows, uncovered};
use super::*;
use crate::machine::resume::{self, store::CheckpointStore};
use tlaku_doc::{DeviceKind, DeviceProfile, JobOptions};
use tlaku_emu::flaky::{
    FlakyConfig, FlakyCounters, FlakyLink, FreezeClock, Rng, truncated_schedule,
};
use tlaku_emu::{Fault, Profile, StateKind, Trigger, VirtualMachine};

/// Trama de imagen sintética: filas en zigzag cada 0,1 mm con tramos de 0,1 a 0,5 mm y `S`
/// alternando entre 0 y un valor al azar (50–350), como el dithering de la Falcon real.
fn raster_job(rows: usize, width_mm: f64, seed: u64) -> Vec<String> {
    let mut rng = Rng::new(seed);
    let mut lines: Vec<String> = ["G90", "G21", "M4 S0", "G0 X10 Y10"]
        .map(String::from)
        .to_vec();
    for row in 0..rows {
        let y = 10.0 + row as f64 * 0.1;
        let (mut x, end, dir) = if row % 2 == 0 {
            (10.0, 10.0 + width_mm, 1.0)
        } else {
            (10.0 + width_mm, 10.0, -1.0)
        };
        lines.push(format!("G0 X{x:.1} Y{y:.1}"));
        let mut first = row == 0;
        let mut on = false;
        while (end - x) * dir > 1e-9 {
            let step = (0.1 * rng.range(1, 5) as f64).min((end - x) * dir);
            x += step * dir;
            let s = if on { rng.range(50, 350) } else { 0 };
            on = !on;
            let word = if first { " F3000" } else { "" };
            first = false;
            lines.push(format!("G1 X{x:.1} S{s}{word}"));
        }
    }
    lines.extend(["M5", "G0 X0 Y0"].map(String::from));
    lines
}

fn sent_form(lines: &[String]) -> Vec<String> {
    lines
        .iter()
        .map(|l| tlaku_grbl::streamer::prepare_line(l).unwrap())
        .filter(|l| !l.is_empty())
        .collect()
}

/// Falcon2 emulada con reloj congelable detrás de un enlace con fallas, conectada con el
/// trabajador real. Las fallas se arman al empezar el trabajo.
struct FlakyRig {
    conn: Option<Connection>,
    handle: Handle,
    sink: Arc<Collect>,
    emu: tlaku_emu::Handle,
    counters: Arc<FlakyCounters>,
    store: Arc<CheckpointStore>,
    device: DeviceProfile,
    dir: std::path::PathBuf,
}

impl FlakyRig {
    fn new(name: &str, speed: f64, flaky: FlakyConfig, timing: Timing) -> Self {
        let dir = std::env::temp_dir().join(format!("tlaku-soak-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let clock = FreezeClock::new(speed);
        let (mut vm, emu) = VirtualMachine::builder(Profile::falcon2_pro())
            .clock(clock.clone())
            .homed(true)
            .build();
        tlaku_grbl::serial::Transport::clear_input(&mut vm).unwrap();
        let (link, counters) = FlakyLink::new(vm, Some(clock), flaky);
        counters.arm(false);
        let sink = Arc::new(Collect::default());
        let (conn, _) = Connection::start_with(
            Box::new(link),
            "virtual:falcon2",
            sink.clone(),
            timing,
            Some(emu.clone()),
        )
        .expect("conectar");
        let handle = conn.handle();
        sink.wait("estado", |s| !s.statuses().is_empty());
        // Para llenar el planificador de la Falcon hay que tener más bytes en vuelo que 127.
        let device = DeviceProfile {
            rx_buffer: 4096,
            ..DeviceProfile::for_kind(DeviceKind::Grbl)
        };
        Self {
            conn: Some(conn),
            handle,
            sink,
            emu,
            counters,
            store: Arc::new(CheckpointStore::new(&dir)),
            device,
            dir,
        }
    }

    /// Arranca el trabajo con punto de control y las fallas armadas.
    fn start(&self, lines: &[String], truncated_every: usize, seed: u64) -> usize {
        let base = self.emu.lines_received();
        let schedule = truncated_schedule(seed, lines.len(), truncated_every);
        for &n in &schedule {
            self.emu
                .inject(Trigger::Line(base + n), Fault::TruncatedStatusBeforeReply);
        }
        let plan = resume::plan_for_job(
            self.store.clone(),
            &self.device,
            &JobOptions::default(),
            Some("foto.tlaku".into()),
            None,
            lines,
        )
        .expect("el perfil se puede reanudar");
        self.counters.arm(true);
        self.handle
            .start_job_checkpointed(
                lines.to_vec(),
                LinkProfile::for_device(&self.device),
                Some(self.device.s_max),
                None,
                Some(plan),
            )
            .expect("iniciar");
        schedule.len()
    }
}

impl Drop for FlakyRig {
    fn drop(&mut self) {
        drop(self.conn.take());
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// Plazos de la app a escala: el aviso de silencio de 2 s se queda en 1 s y las pausas duran 0,3–1,2 s.
fn soak_timing() -> Timing {
    Timing {
        status_interval: Duration::from_millis(20),
        silence_notice: Duration::from_secs(1),
        ..fast()
    }
}

/// Un trabajo denso entero a través del trabajador, con las fallas de `flaky`.
fn dense_job_through_the_worker(name: &str, rows: usize, flaky: FlakyConfig, timing: Timing) {
    let seed = flaky.seed;
    let lines = raster_job(rows, 12.0, 7);
    let rig = FlakyRig::new(name, 25.0, flaky, timing);
    let truncations = rig.start(&lines, 200, seed);
    assert!(truncations >= lines.len() / 600, "{truncations}");

    // Mientras corre, el punto de control de disco nunca se contradice: lo confirmado sube, nunca
    // pasa de lo entregado ni del total.
    let (mut last_acked, mut samples) = (0u32, 0usize);
    let end = Instant::now() + Duration::from_secs(1200);
    while !rig.sink.jobs().contains(&JobState::Finished) {
        assert!(Instant::now() < end, "el trabajo no terminó");
        assert!(
            !rig.sink.jobs().contains(&JobState::Error),
            "{:?}",
            rig.sink.job_messages()
        );
        if let Some(p) = rig.store.load().and_then(|l| l.progress) {
            assert!(p.acked <= p.sent, "{p:?}");
            assert!(p.sent as usize <= lines.len(), "{p:?}");
            assert!(p.acked >= last_acked, "{p:?} tras {last_acked}");
            last_acked = p.acked;
            samples += 1;
        }
        thread::sleep(Duration::from_millis(40));
    }
    assert!(
        samples > 5,
        "el punto de control casi no se escribió: {samples}"
    );

    // Terminó bien, sin desconexión falsa ni errores.
    assert_eq!(rig.sink.jobs(), [JobState::Started, JobState::Finished]);
    assert!(rig.sink.connections().iter().all(|c| c.connected));
    assert!(!rig.handle.is_closed());
    let progress = rig.sink.progress();
    let last = progress.last().unwrap();
    let total = sent_form(&lines).len() as u32;
    assert_eq!((last.acked, last.total), (total, total));
    assert!(progress.windows(2).all(|w| w[0].acked <= w[1].acked));
    // Al terminar bien el punto de control se borra.
    assert!(rig.store.load().is_none());
    // La placa recibió exactamente el trabajo, en orden (sin contar las consultas `$`).
    let got: Vec<String> = rig
        .emu
        .received_lines()
        .into_iter()
        .filter(|l| !l.starts_with('$'))
        .collect();
    assert_eq!(got.len(), sent_form(&lines).len());
    assert!(
        got == sent_form(&lines),
        "las líneas llegaron cambiadas de orden o de texto"
    );
    assert!(rig.emu.wait_idle(Duration::from_secs(30)));
    assert_eq!(rig.emu.state(), StateKind::Idle);
    assert_eq!(rig.emu.mpos(), [0.0, 0.0, 0.0]);
    assert!(!rig.emu.laser_on());
    assert!(rig.emu.trace().burned_during_rapids().is_empty());
    // Los reportes cortados no se tomaron por overrides del 10 % ni mandaron mensajes a la consola.
    for s in rig.sink.statuses() {
        if let Some(ov) = s.overrides {
            assert_eq!((ov.feed, ov.rapid, ov.spindle), (100, 100, 100));
        }
    }
    // La consola solo recibe lo que no es del trabajo: unas pocas líneas fijas y un fragmento por
    // cada estado cortado inyectado; nada por cada línea del G-code.
    assert!(
        rig.sink.logs().len() <= truncations + 100,
        "la consola se llenó: {} líneas para {truncations} estados cortados",
        rig.sink.logs().len()
    );
    // Hubo fallas de verdad.
    let c = &rig.counters;
    assert!(c.write_stalls() >= 2, "{}", c.write_stalls());
    assert!(c.read_mutes() >= 2, "{}", c.read_mutes());
    assert!(c.skipped_status() >= 1, "{}", c.skipped_status());
    eprintln!(
        "[trabajador] {} líneas; {} estados cortados, {} bloqueos ({:.1} s), {} silencios, {} `?` saltados, \
         {} eventos de avance",
        lines.len(),
        truncations,
        c.write_stalls(),
        c.write_stall_total().as_secs_f64(),
        c.read_mutes(),
        c.skipped_status(),
        progress.len()
    );
}

#[test]
fn a_dense_raster_job_finishes_through_the_worker_despite_the_falcon_faults() {
    dense_job_through_the_worker(
        "dense",
        250,
        FlakyConfig {
            seed: 0x5EED_0001,
            write_stall_every: 30,
            write_stall: (Duration::from_millis(300), Duration::from_millis(1200)),
            read_mute_every: 80,
            read_mute: (Duration::from_millis(100), Duration::from_millis(500)),
            skip_status: 0.15,
            skip_burst: 0.3,
            fragment: 0.2,
            ..FlakyConfig::default()
        },
        soak_timing(),
    );
}

/// A escala real: ~120 000 líneas, bloqueos de 2–20 s y los plazos por omisión de la app (silencio
/// de 5 s). Tarda varios minutos.
#[test]
#[ignore = "tarda ~6-7 min: cargo test -p tlaku-desktop --lib machine::tests::soak -- --ignored --nocapture"]
fn a_120k_line_raster_job_finishes_through_the_worker_with_real_stalls() {
    dense_job_through_the_worker(
        "dense-full",
        2900,
        FlakyConfig {
            seed: 0x5EED_2020,
            write_stall_every: 1000,
            write_stall: (Duration::from_secs(2), Duration::from_secs(20)),
            read_mute_every: 1000,
            read_mute: (Duration::from_secs(1), Duration::from_millis(4200)),
            skip_status: 0.15,
            skip_burst: 0.3,
            fragment: 0.2,
            ..FlakyConfig::default()
        },
        Timing {
            status_interval: Duration::from_millis(20),
            ..Timing::default()
        },
    );
}

/// La placa deja de leer 8 veces el plazo de silencio (como 40 s frente a 5 s) con la escritura
/// colgada: el trabajador no lo toma por una conexión perdida.
#[test]
fn a_write_stall_eight_times_the_silence_window_does_not_drop_the_connection() {
    let lines = raster_job(40, 12.0, 11);
    let rig = FlakyRig::new(
        "stall8",
        25.0,
        FlakyConfig {
            scripted_write_stall: Some((15, Duration::from_secs(8))),
            ..FlakyConfig::default()
        },
        soak_timing(),
    );
    rig.start(&lines, 1_000_000_000, 1);
    rig.sink.wait("trabajo terminado", |s| {
        s.jobs().contains(&JobState::Finished) || s.jobs().contains(&JobState::Error)
    });
    assert_eq!(
        rig.sink.jobs(),
        [JobState::Started, JobState::Finished],
        "{:?}",
        rig.sink.job_messages()
    );
    assert!(rig.sink.connections().iter().all(|c| c.connected));
    assert_eq!(rig.counters.write_stalls(), 1);
    assert!(rig.store.load().is_none());
}

/// La placa deja de leer 20 s con la escritura colgada y el usuario cierra la conexión: el cierre
/// no espera la escritura (el `SerialTransport` real reintenta 30 s), porque el hilo del envío
/// recibe el indicador de «abandonar» (§2.8, propuesta 2).
#[test]
fn closing_the_connection_does_not_wait_for_a_stalled_write() {
    let lines = raster_job(40, 12.0, 11);
    let mut rig = FlakyRig::new(
        "stall-close",
        25.0,
        FlakyConfig {
            scripted_write_stall: Some((15, Duration::from_secs(20))),
            ..FlakyConfig::default()
        },
        soak_timing(),
    );
    rig.start(&lines, 1_000_000_000, 1);
    // Espera a que el envío esté dentro de la escritura colgada.
    let end = Instant::now() + Duration::from_secs(30);
    while rig.counters.write_stalls() == 0 {
        assert!(Instant::now() < end, "la escritura no se colgó");
        thread::sleep(Duration::from_millis(5));
    }
    thread::sleep(Duration::from_millis(300));
    let t = Instant::now();
    rig.conn.take().unwrap().close();
    let took = t.elapsed();
    assert!(took < Duration::from_secs(8), "el cierre tardó {took:?}");
    // El trabajo se da por detenido sin quedar marcado como terminado.
    assert!(!rig.sink.jobs().contains(&JobState::Finished));
}

/// La placa calla 8 veces el antiguo plazo (8 s frente a 1 s a escala; 40 s frente a 5 s reales)
/// sin bloquear al anfitrión: **no se da por perdida** (como LightBurn, se espera). El trabajo
/// sigue y termina entero, el aviso de silencio sale y se retira, no se manda ningún `!` ni reset,
/// y el punto de control se borra al terminar. Si de verdad se cae la conexión (cable fuera), el
/// punto de control permite continuar: eso lo cubren las pruebas de `resume.rs` (`kill_app`).
#[test]
fn a_long_silence_does_not_lose_the_job() {
    let mut b = Bench::new("soak-pause", "virtual:falcon2@4", DeviceKind::Grbl);
    let lines = slow_rows();
    let mut counters = None;
    let h = b.connect_via(|vm| {
        let (link, c) = FlakyLink::new(
            vm,
            None,
            FlakyConfig {
                scripted_read_mute: Some((30, Duration::from_secs(8))),
                ..FlakyConfig::default()
            },
        );
        c.arm(false);
        counters = Some(c);
        Box::new(link)
    });
    let counters = counters.unwrap();
    counters.arm(true);
    b.start(&h, &lines);
    b.sink.wait("trabajo terminado", |s| {
        s.jobs().contains(&JobState::Finished) || s.jobs().contains(&JobState::Error)
    });
    assert_eq!(counters.read_mutes(), 1);
    let jobs = b.sink.jobs();
    assert_eq!(jobs, [JobState::Started, JobState::Finished], "{jobs:?}");
    assert!(b.sink.connections().iter().all(|c| c.connected));
    // El aviso salió (el silencio supera los 2 s de `Timing::silence_notice`) y se retiró.
    let silences = b.sink.silences();
    assert!(silences.iter().any(|&m| m >= 2000), "{silences:?}");
    assert_eq!(silences.last(), Some(&0), "{silences:?}");
    // Nadie mandó una parada: solo `?`.
    assert!(
        b.emu.realtime_bytes().iter().all(|&x| x == b'?'),
        "{:?}",
        b.emu.realtime_bytes()
    );
    assert!(b.store.load().is_none());
    assert!(uncovered(&lines, &b.emu, 0.3) == 0, "quedaron huecos");
}
