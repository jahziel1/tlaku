//! Prueba de resistencia («soak»): un trabajo de imagen LARGO termina bien con las fallas medidas
//! en una Creality Falcon2 Pro real (`docs/hardware/falcon2.md` §10.4–10.6):
//!
//! - estado cortado pegado al `ok` (`<Run|…|Ov:100,100,10ok`), ~1 de cada 600 líneas en la placa
//!   real (aquí 1 de cada 250, más a propósito);
//! - la placa deja de leer el USB (el `write` del anfitrión se queda colgado) y luego sigue;
//! - silencios de lectura, reportes `?` saltados (aislados y en ráfagas), lecturas partidas;
//! - el planificador lleno (`Bf:0`): el búfer del anfitrión es de 4096 bytes, así que las ~300
//!   líneas cortas en vuelo llenan los 127 bloques de la Falcon.
//!
//! El G-code sale del pipeline real: una imagen sintética con dithering Floyd–Steinberg a 0,1 mm.
//! El trabajo corre con el `Streamer` real contra la Falcon2 emulada (`tlaku-emu`) envuelta en un
//! [`FlakyLink`]; el azar usa semilla fija.
//!
//! - `soak_*` (siempre): bloqueos de 0,3–1,4 s con aviso de silencio a 1 s. Es la versión a escala
//!   de la real (bloqueos de 2–20 s con aviso a 2 s): lo que importa es que haya bloqueos MÁS
//!   LARGOS que el aviso y MÁS LARGOS que `WRITE_STALL` (250 ms). La conexión **nunca** se da por
//!   perdida por silencio (§2.8): todos los trabajos terminan enteros.
//! - `soak_full_scale_*` (`#[ignore]`, ~3 min): bloqueos de 2–20 s reales y los plazos por
//!   omisión. A mano: `cargo test -p tlaku-emu --test soak -- --ignored --nocapture`.

mod common;

use std::time::{Duration, Instant};

use tlaku_doc::{
    DesignObject, DeviceProfile, Document, ImageMode, ImageObject, JobOptions, LayerMode,
    LayerSettings, ObjectKind,
};
use tlaku_emu::flaky::{FlakyConfig, FlakyCounters, FlakyLink, FreezeClock, truncated_schedule};
use tlaku_emu::{Fault, Handle, Profile, StateKind, Trigger, VirtualMachine};
use tlaku_grbl::streamer::{Event, Outcome, Streamer, StreamerConfig, prepare_line};
use tlaku_pipeline::{compute, gcode};

use std::sync::Arc;

// ---------------------------------------------------------------------------
// El trabajo: imagen sintética -> pipeline real -> G-code
// ---------------------------------------------------------------------------

/// BMP de 24 bits; la fila 0 de `pixel` queda arriba.
fn bmp(width: u32, height: u32, pixel: impl Fn(u32, u32) -> u8) -> Vec<u8> {
    let row_bytes = (width * 3).div_ceil(4) * 4;
    let size = 54 + row_bytes * height;
    let mut b = b"BM".to_vec();
    b.extend(size.to_le_bytes());
    b.extend([0u8; 4]);
    b.extend(54u32.to_le_bytes());
    b.extend(40u32.to_le_bytes());
    b.extend(width.to_le_bytes());
    b.extend(height.to_le_bytes());
    b.extend(1u16.to_le_bytes());
    b.extend(24u16.to_le_bytes());
    b.extend([0u8; 24]);
    for y in (0..height).rev() {
        let mut row: Vec<u8> = (0..width).flat_map(|x| [pixel(x, y); 3]).collect();
        row.resize(row_bytes as usize, 0);
        b.extend(row);
    }
    b
}

/// G-code de una «foto» de `w_mm` x `h_mm` a 0,1 mm con dithering Floyd–Steinberg a 3000 mm/min y
/// S hasta 350 (como la imagen de la Falcon real). Anillos concéntricos más un degradado: tonos
/// medios por todas partes, que es lo que más segmentos cortos produce.
fn image_gcode(w_mm: f64, h_mm: f64) -> Vec<String> {
    let (w, h) = ((w_mm * 10.0) as u32, (h_mm * 10.0) as u32);
    // Una carpeta por llamada: las pruebas de este archivo corren en paralelo.
    static CALLS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let dir = std::env::temp_dir().join(format!(
        "tlaku-soak-{}-{}",
        std::process::id(),
        CALLS.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("foto.bmp");
    std::fs::write(
        &path,
        bmp(w, h, |x, y| {
            let (fx, fy) = (f64::from(x) / f64::from(w), f64::from(y) / f64::from(h));
            let r = ((fx - 0.5).powi(2) + (fy - 0.5).powi(2)).sqrt();
            let v = 0.5 + 0.35 * (r * 14.0).sin() * (1.0 - r) + 0.15 * (fx * 3.0).cos();
            (v.clamp(0.0, 1.0) * 255.0) as u8
        }),
    )
    .unwrap();
    let object = DesignObject {
        id: "foto".into(),
        layer: LayerSettings::IMAGE_COLOR,
        transform: [1.0, 0.0, 0.0, 1.0, 10.0, 10.0],
        kind: ObjectKind::Image(ImageObject {
            source_path: path.to_str().unwrap().into(),
            width_px: w,
            height_px: h,
            size_mm: [w_mm, h_mm],
            mask: Vec::new(),
            variable: None,
        }),
        locked: false,
        hidden: false,
        group: None,
        priority: None,
        power_scale: None,
        shape: None,
        start: None,
        manual_tabs: Vec::new(),
        array: None,
    };
    let layer = LayerSettings {
        mode: LayerMode::Image,
        interval: 0.1,
        speed: 3000.0,
        power: 35.0,
        image: tlaku_doc::ImageSettings {
            mode: ImageMode::Floyd,
            ..tlaku_doc::ImageSettings::default()
        },
        ..LayerSettings::for_images()
    };
    let doc = Document {
        objects: vec![object],
        layers: vec![layer],
        ..Document::default()
    };
    let device = DeviceProfile {
        bed: [410.0, 400.0],
        ..DeviceProfile::default()
    };
    let job = compute(&doc, &device, &JobOptions::default()).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    gcode(&job, &device).lines().map(str::to_string).collect()
}

/// Longitud (mm) que debe quemar el láser: tramos `G1` con `S > 0` (modal) en modo dinámico.
fn expected_burn_mm(lines: &[String]) -> f64 {
    let (mut x, mut y, mut s, mut total) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
    for l in lines {
        let l = l.split(';').next().unwrap_or("");
        let mut words = l.split_whitespace();
        let Some(first) = words.next() else { continue };
        let rapid = first == "G0";
        if !(rapid || first == "G1" || first == "M4") {
            continue;
        }
        let (mut nx, mut ny) = (x, y);
        for w in words.chain(std::iter::empty()) {
            let (k, v) = w.split_at(1);
            let Ok(v) = v.parse::<f64>() else { continue };
            match k {
                "X" => nx = v,
                "Y" => ny = v,
                "S" => s = v,
                _ => {}
            }
        }
        if !rapid && first == "G1" && s > 0.0 {
            total += ((nx - x).powi(2) + (ny - y).powi(2)).sqrt();
        }
        x = nx;
        y = ny;
    }
    total
}

// ---------------------------------------------------------------------------
// Medida de memoria del proceso
// ---------------------------------------------------------------------------

/// RSS del proceso en KiB (`ps`; solo Unix).
fn rss_kib() -> Option<u64> {
    #[cfg(unix)]
    {
        let out = std::process::Command::new("ps")
            .args(["-o", "rss=", "-p", &std::process::id().to_string()])
            .output()
            .ok()?;
        String::from_utf8_lossy(&out.stdout).trim().parse().ok()
    }
    #[cfg(not(unix))]
    {
        None
    }
}

// ---------------------------------------------------------------------------
// El banco
// ---------------------------------------------------------------------------

struct Soak {
    lines: Vec<String>,
    speed: f64,
    /// Una línea de cada tantas tiene el estado cortado pegado al `ok`.
    truncated_every: usize,
    flaky: FlakyConfig,
    config: StreamerConfig,
    deadline: Duration,
}

#[derive(Default)]
struct Report {
    outcome: Option<Outcome>,
    timed_out: bool,
    panicked: bool,
    elapsed: Duration,
    /// Eventos `Progress` y último valor `(acked, sent, total)`.
    progress_events: usize,
    last_progress: Option<(usize, usize, usize)>,
    /// `acked` no bajó nunca, `sent >= acked` y `acked <= total`.
    progress_sane: bool,
    max_buffer_used: usize,
    statuses: usize,
    planner_full: usize,
    bad_overrides: usize,
    /// Mensajes que no son fragmentos de estado cortado.
    other_messages: Vec<String>,
    truncated_messages: usize,
    errors: usize,
    alarms: usize,
    resets: usize,
    /// Avisos tempranos de silencio (`Event::Silence`) con más de 0 ms y el último valor visto.
    silence_notices: usize,
    last_silence_ms: Option<u64>,
    /// (líneas confirmadas, RSS en KiB) a cada ~10 % del trabajo.
    rss: Vec<(usize, u64)>,
    scheduled_truncations: usize,
    counters: Option<Arc<FlakyCounters>>,
    handle: Option<Handle>,
}

fn run_soak(spec: &Soak) -> Report {
    let clock = FreezeClock::new(spec.speed);
    let (mut vm, h) = VirtualMachine::builder(Profile::falcon2_pro())
        .clock(clock.clone())
        .homed(true)
        .build();
    use tlaku_grbl::serial::Transport;
    vm.clear_input().unwrap();
    let schedule = truncated_schedule(spec.flaky.seed, spec.lines.len(), spec.truncated_every);
    for &n in &schedule {
        h.inject(Trigger::Line(n), Fault::TruncatedStatusBeforeReply);
    }
    let (link, counters) = FlakyLink::new(vm, Some(clock), spec.flaky.clone());
    let total_hint = spec
        .lines
        .iter()
        .filter(|l| !prepare_line(l).unwrap().is_empty())
        .count();

    let mut r = Report {
        progress_sane: true,
        scheduled_truncations: schedule.len(),
        counters: Some(counters),
        handle: Some(h),
        ..Report::default()
    };
    let baseline = rss_kib();
    let started = Instant::now();
    let job = Streamer::new(link, spec.config.clone())
        .spawn(&spec.lines)
        .expect("líneas válidas");
    let control = job.control.clone();
    let mut cancelled_at: Option<Instant> = None;
    let mut next_sample = 0usize;
    loop {
        match job.events.recv_timeout(Duration::from_millis(20)) {
            Ok(Event::Finished(o)) => {
                r.outcome = Some(o);
                break;
            }
            Ok(Event::Progress {
                acked,
                sent,
                total,
                buffer_used,
            }) => {
                r.progress_events += 1;
                if let Some((a0, _, _)) = r.last_progress
                    && acked < a0
                {
                    r.progress_sane = false;
                }
                if sent < acked || acked > total || total != total_hint {
                    r.progress_sane = false;
                }
                r.max_buffer_used = r.max_buffer_used.max(buffer_used);
                r.last_progress = Some((acked, sent, total));
                if acked >= next_sample {
                    if let Some(kb) = rss_kib() {
                        r.rss.push((acked, kb));
                    }
                    next_sample += total / 10;
                }
            }
            Ok(Event::Status(s)) => {
                r.statuses += 1;
                if s.buffer.is_some_and(|b| b.planner_blocks == 0) {
                    r.planner_full += 1;
                }
                if let Some(ov) = s.overrides
                    && (ov.feed, ov.rapid, ov.spindle) != (100, 100, 100)
                {
                    r.bad_overrides += 1;
                }
            }
            Ok(Event::Message(m)) => {
                if m.starts_with('<') && !m.ends_with('>') {
                    r.truncated_messages += 1;
                } else if r.other_messages.len() < 20 {
                    r.other_messages.push(m);
                }
            }
            Ok(Event::Error { .. }) => r.errors += 1,
            Ok(Event::Alarm(_)) => r.alarms += 1,
            Ok(Event::Reset(_)) => r.resets += 1,
            // Aviso temprano de silencio (no cambia el resultado).
            Ok(Event::Silence { ms }) => {
                if ms > 0 {
                    r.silence_notices += 1;
                }
                r.last_silence_ms = Some(ms);
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                if started.elapsed() > spec.deadline && !r.timed_out {
                    r.timed_out = true;
                    control.cancel();
                    cancelled_at = Some(Instant::now());
                }
                if cancelled_at.is_some_and(|t| t.elapsed() > Duration::from_secs(5)) {
                    break;
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
    r.panicked = job.join().is_err();
    r.elapsed = started.elapsed();
    if let (Some(b), Some(&(_, last))) = (baseline, r.rss.last()) {
        eprintln!("RSS: base {b} KiB, muestras {:?}, último {last} KiB", r.rss);
    }
    r
}

/// Comprobaciones comunes a todos los soak que deben terminar bien.
fn assert_clean_finish(spec: &Soak, r: &Report) {
    let h = r.handle.as_ref().unwrap();
    // Las líneas que se envían de verdad: sin comentarios ni vacías.
    let n = spec
        .lines
        .iter()
        .filter(|l| !prepare_line(l).unwrap().is_empty())
        .count();
    assert!(!r.panicked, "el hilo del streamer murió");
    assert!(
        !r.timed_out,
        "el trabajo no terminó en {:?} ({:?})",
        spec.deadline, r.last_progress
    );
    // Terminó bien, sin desconexión falsa ni errores.
    assert_eq!(
        r.outcome,
        Some(Outcome::Completed { errors: 0 }),
        "{:?}",
        r.outcome
    );
    assert_eq!((r.errors, r.alarms, r.resets), (0, 0, 0));
    // El avance llega al total y nunca retrocede.
    assert_eq!(r.last_progress, Some((n, n, n)));
    assert!(r.progress_sane, "el avance retrocedió o se pasó del total");
    assert!(
        r.max_buffer_used <= spec.config.rx_buffer,
        "{}",
        r.max_buffer_used
    );
    // La placa recibió exactamente todas las líneas y en orden.
    let sent: Vec<String> = spec
        .lines
        .iter()
        .map(|l| prepare_line(l).unwrap())
        .filter(|l| !l.is_empty())
        .collect();
    let got = h.received_lines();
    assert_eq!(got.len(), sent.len(), "líneas recibidas");
    if let Some(i) = got.iter().zip(&sent).position(|(a, b)| a != b) {
        panic!("línea {i} distinta: {:?} vs {:?}", got[i], sent[i]);
    }
    assert_eq!(h.rx_overflow_bytes(), 0);
    assert!(h.max_in_flight() <= spec.config.rx_buffer);
    // Cada estado cortado inyectado se entregó aparte y no se tomó por un reporte.
    assert_eq!(r.truncated_messages, r.scheduled_truncations);
    assert!(r.other_messages.is_empty(), "{:?}", r.other_messages);
    assert_eq!(
        r.bad_overrides, 0,
        "un `Ov:100,100,10` se leyó como override"
    );
    // La máquina quedó en reposo, en el origen y con el láser apagado.
    assert!(h.wait_idle(Duration::from_secs(30)));
    assert_eq!(h.state(), StateKind::Idle);
    assert_eq!(h.mpos(), [0.0, 0.0, 0.0]);
    assert!(!h.laser_on());
    // El láser quemó exactamente lo que dice el G-code: ni una línea perdida ni repetida.
    let trace = h.trace();
    assert!(trace.burned_during_rapids().is_empty());
    let want = expected_burn_mm(&spec.lines);
    let got = trace.total_length();
    assert!(
        (got - want).abs() <= want * 1e-3,
        "quemó {got} mm y el G-code dice {want} mm"
    );
}

fn report(name: &str, spec: &Soak, r: &Report) {
    let c = r.counters.as_ref().unwrap();
    eprintln!(
        "[{name}] tiempo simulado {:.0} s; {} líneas ({} bytes) en {:.1} s reales; {} avances, {} reportes de estado \
         ({} con Bf:0); inyectado: {} estados cortados, {} bloqueos de escritura ({:.1} s), \
         {} silencios ({:.1} s), {} `?` saltados, {} lecturas partidas ({} escrituras y {} lecturas con datos)",
        r.handle.as_ref().unwrap().now().as_secs_f64(),
        spec.lines.len(),
        spec.lines.iter().map(|l| l.len() + 1).sum::<usize>(),
        r.elapsed.as_secs_f64(),
        r.progress_events,
        r.statuses,
        r.planner_full,
        r.scheduled_truncations,
        c.write_stalls(),
        c.write_stall_total().as_secs_f64(),
        c.read_mutes(),
        c.read_mute_total().as_secs_f64(),
        c.skipped_status(),
        c.fragmented_reads(),
        c.data_writes(),
        c.data_reads(),
    );
}

/// Memoria: el RSS no puede crecer más que lo que guardan el emulador y el propio G-code (que
/// son O(n) por diseño); se compara el tramo 10 %–100 % con una cota generosa por línea.
fn assert_memory_is_flat(r: &Report, lines: usize) {
    let Some(&(_, first)) = r.rss.get(1) else {
        return; // sin `ps` (Windows): no se mide
    };
    let last = r.rss.last().unwrap().1;
    let grown_kib = last.saturating_sub(first);
    // El registro y el rastro del emulador guardan ~0,2 KiB por línea; el anfitrión no suma.
    let allowed = (lines as u64) * 3 / 10 + 20_000;
    assert!(
        grown_kib <= allowed,
        "RSS creció {grown_kib} KiB entre el 10 % y el 100 % de {lines} líneas (tope {allowed})"
    );
}

fn falcon_config(notice: Duration) -> StreamerConfig {
    StreamerConfig {
        // Con 4096 bytes en vuelo (~300 líneas cortas) el planificador de 127 bloques se llena.
        rx_buffer: 4096,
        status_interval: Some(Duration::from_millis(20)),
        silence_notice: Some(notice),
        ..StreamerConfig::default()
    }
}

// ---------------------------------------------------------------------------
// Las pruebas
// ---------------------------------------------------------------------------

#[test]
fn soak_long_image_job_survives_the_falcon_faults() {
    let lines = image_gcode(38.0, 20.0);
    assert!(lines.len() > 50_000, "{} líneas", lines.len());
    let spec = Soak {
        speed: 25.0,
        truncated_every: 250,
        flaky: FlakyConfig {
            seed: 0xFA1C_0042,
            write_stall_every: 200,
            write_stall: (Duration::from_millis(300), Duration::from_millis(1200)),
            read_mute_every: 250,
            read_mute: (Duration::from_millis(100), Duration::from_millis(500)),
            skip_status: 0.05,
            skip_burst: 0.2,
            fragment: 0.2,
            ..FlakyConfig::default()
        },
        config: falcon_config(Duration::from_secs(1)),
        deadline: Duration::from_secs(240),
        lines,
    };
    let r = run_soak(&spec);
    report("corto", &spec, &r);
    assert_clean_finish(&spec, &r);
    let c = r.counters.as_ref().unwrap();
    // La prueba de verdad probó algo: hubo de todo.
    assert!(
        r.scheduled_truncations >= 100,
        "{}",
        r.scheduled_truncations
    );
    assert!(c.write_stalls() >= 3, "{}", c.write_stalls());
    assert!(c.read_mutes() >= 3, "{}", c.read_mutes());
    assert!(c.skipped_status() >= 20, "{}", c.skipped_status());
    assert!(
        r.planner_full > 10,
        "el planificador no llegó a llenarse: {}",
        r.planner_full
    );
    // Algún bloqueo fue más largo que el plazo de silencio: el anfitrión no lo contó como silencio.
    assert!(c.write_stall_total() > Duration::from_secs(1));
    assert_memory_is_flat(&r, spec.lines.len());
}

/// A escala real: bloqueos de 2–20 s y los plazos por omisión (silencio de 5 s). Tarda minutos.
#[test]
#[ignore = "tarda ~3 min (bloqueos reales de 2-20 s): cargo test -p tlaku-emu --test soak -- --ignored --nocapture"]
fn soak_full_scale_with_real_2_to_20_second_stalls() {
    let lines = image_gcode(50.0, 35.0);
    let spec = Soak {
        speed: 25.0,
        truncated_every: 250,
        flaky: FlakyConfig {
            seed: 0xFA1C_2020,
            write_stall_every: 400,
            write_stall: (Duration::from_secs(2), Duration::from_secs(20)),
            read_mute_every: 400,
            read_mute: (Duration::from_secs(1), Duration::from_millis(4200)),
            skip_status: 0.05,
            skip_burst: 0.2,
            fragment: 0.2,
            ..FlakyConfig::default()
        },
        config: falcon_config(Duration::from_secs(5)),
        deadline: Duration::from_secs(1200),
        lines,
    };
    let r = run_soak(&spec);
    report("completo", &spec, &r);
    assert_clean_finish(&spec, &r);
    let c = r.counters.as_ref().unwrap();
    assert!(c.write_stalls() >= 2, "{}", c.write_stalls());
    assert!(c.write_stall_total() > Duration::from_secs(5));
    assert_memory_is_flat(&r, spec.lines.len());
}

// ---------------------------------------------------------------------------
// Una pausa de 40 s DURANTE el trabajo: ¿qué hace Tlaku?
// ---------------------------------------------------------------------------

/// Trabajo corto (~15 000 líneas) para las pausas puntuales: dura más que ellas.
fn short_spec(silence: Duration, flaky: FlakyConfig) -> Soak {
    Soak {
        lines: image_gcode(12.0, 10.0),
        speed: 25.0,
        truncated_every: 1_000_000_000,
        flaky,
        config: falcon_config(silence),
        deadline: Duration::from_secs(120),
    }
}

/// La placa deja de leer el USB y el `write` del anfitrión queda colgado MUCHO más que el plazo de
/// silencio (8 veces, como 40 s frente a 5 s): no es silencio de la placa, y al volver el trabajo
/// sigue y termina entero.
#[test]
fn a_write_stall_eight_times_the_silence_window_is_forgiven() {
    let spec = short_spec(
        Duration::from_millis(500),
        FlakyConfig {
            scripted_write_stall: Some((20, Duration::from_secs(4))),
            ..FlakyConfig::default()
        },
    );
    let r = run_soak(&spec);
    report("bloqueo 8x", &spec, &r);
    assert_clean_finish(&spec, &r);
    assert_eq!(r.counters.as_ref().unwrap().write_stalls(), 1);
}

/// La placa calla 8 veces el antiguo plazo de silencio sin bloquear al anfitrión (lo normal con
/// `?` sin respuesta): ya no se da por perdida; se avisa (`Event::Silence`) y, al volver la
/// respuesta, el trabajo termina entero y el aviso se retira.
#[test]
fn a_silence_eight_times_the_old_window_is_waited_out_and_the_job_completes() {
    let mute = Duration::from_secs(4);
    let spec = short_spec(
        Duration::from_millis(500),
        FlakyConfig {
            scripted_read_mute: Some((20, mute)),
            ..FlakyConfig::default()
        },
    );
    let r = run_soak(&spec);
    report("silencio 8x", &spec, &r);
    assert_clean_finish(&spec, &r);
    assert_eq!(r.counters.as_ref().unwrap().read_mutes(), 1);
    assert!(r.silence_notices >= 1, "no hubo aviso de silencio");
    assert_eq!(r.last_silence_ms, Some(0), "el aviso no se retiró");
    // El streamer no mandó ninguna parada (`!` ni reset): solo `?`.
    let h = r.handle.as_ref().unwrap();
    assert!(
        h.realtime_bytes().iter().all(|&b| b == b'?'),
        "{:?}",
        h.realtime_bytes()
    );
}

/// El caso real a escala: 40 s de pausa y el aviso a los 2 s. Tarda ~1 min.
#[test]
#[ignore = "tarda ~1 min (pausas reales de 40 s): cargo test -p tlaku-emu --test soak -- --ignored --nocapture"]
fn a_real_forty_second_pause_write_stall_and_read_silence() {
    // (a) Escritura colgada 40 s: el puerto espera (como el `SerialTransport` real, sin límite) y
    //     el trabajo termina entero.
    let spec = short_spec(
        Duration::from_secs(2),
        FlakyConfig {
            scripted_write_stall: Some((20, Duration::from_secs(40))),
            ..FlakyConfig::default()
        },
    );
    let r = run_soak(&spec);
    report("40 s escritura", &spec, &r);
    assert_clean_finish(&spec, &r);

    // (b) La placa calla 40 s sin bloquear al anfitrión: también termina entero, con el aviso
    //     (y, pasado el minuto, la interfaz lo vuelve más serio; el streamer sigue sin cortar).
    let spec = short_spec(
        Duration::from_secs(2),
        FlakyConfig {
            scripted_read_mute: Some((20, Duration::from_secs(40))),
            ..FlakyConfig::default()
        },
    );
    let r = run_soak(&spec);
    report("40 s silencio", &spec, &r);
    assert_clean_finish(&spec, &r);
    assert!(r.silence_notices >= 30, "{}", r.silence_notices);
    assert_eq!(r.last_silence_ms, Some(0));
}
