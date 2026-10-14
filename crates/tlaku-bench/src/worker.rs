//! Trabajador: mide un archivo de principio a fin en este proceso. El padre lo lanza aparte para
//! contener pánicos, desbordes de pila y falta de memoria, y para poder cortarlo por tiempo.

use crate::report::Report;
use std::io::Write;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::Path;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tlaku_core::bezpath::PathSeg;
use tlaku_core::toolpath::Move;
use tlaku_doc::affine::{self, Transform};
use tlaku_doc::{
    DeviceProfile, Document, ImportResult, JobOptions, LayerMode, ObjectKind, Overscan,
};
use tlaku_emu::{Profile, ScaledClock, VirtualMachine};
use tlaku_grbl::serial::Transport;
use tlaku_grbl::streamer::{Event, Outcome, Streamer, StreamerConfig};
use tlaku_pipeline::{ComputedJob, compute, export_rd_for_device, gcode, import_file};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Capas de línea tal como las crea el importador.
    Line,
    /// Las capas vectoriales pasan a relleno (0,2 mm): la ruta más pesada del cálculo.
    Fill,
}

impl Mode {
    pub fn name(self) -> &'static str {
        match self {
            Mode::Line => "line",
            Mode::Fill => "fill",
        }
    }
}

pub struct WorkerArgs {
    pub mode: Mode,
    pub threshold_s: f64,
    pub emu: bool,
    pub emu_max_lines: usize,
    pub emu_max_speed: f64,
    pub emu_deadline_s: f64,
}

/// Líneas por segundo real que el anfitrión (streamer + máquina virtual) sostiene, medido.
const HOST_LINES_PER_S: f64 = 15_000.0;

static PANIC_MSG: Mutex<String> = Mutex::new(String::new());

/// Memoria residente máxima del proceso hasta ahora, en MB. En Windows no se mide (devuelve 0).
#[cfg(not(unix))]
fn peak_rss_mb() -> f64 {
    0.0
}

/// Memoria residente máxima del proceso hasta ahora, en MB.
#[cfg(unix)]
fn peak_rss_mb() -> f64 {
    // SAFETY: `getrusage` solo escribe en la estructura que le pasamos.
    let mut ru: libc::rusage = unsafe { std::mem::zeroed() };
    let rc = unsafe { libc::getrusage(libc::RUSAGE_SELF, &mut ru) };
    if rc != 0 {
        return 0.0;
    }
    let raw = ru.ru_maxrss as f64;
    // macOS da bytes; Linux, KiB.
    if cfg!(target_os = "macos") {
        raw / 1_048_576.0
    } else {
        raw / 1024.0
    }
}

fn emit(rep: &Report) {
    let mut out = std::io::stdout().lock();
    let _ = writeln!(out, "{}", serde_json::to_string(rep).unwrap_or_default());
    let _ = out.flush();
}

/// Ejecuta una fase con medición de tiempo y contención de pánicos. `None` si falló (el error
/// ya quedó en el informe).
fn phase<T>(
    rep: &mut Report,
    name: &str,
    f: impl FnOnce(&mut Report) -> Result<T, String>,
) -> Option<(T, f64)> {
    rep.running = name.to_string();
    emit(rep);
    let t0 = Instant::now();
    let res = catch_unwind(AssertUnwindSafe(|| f(rep)));
    let secs = t0.elapsed().as_secs_f64();
    rep.running.clear();
    match res {
        Ok(Ok(v)) => Some((v, secs)),
        Ok(Err(e)) => {
            // Relleno de un diseño sin formas cerradas: no es un fallo, no hay nada que rellenar.
            let nothing = name == "compute" && tlaku_pipeline::is_nothing_to_send(&e);
            rep.status = if nothing && rep.mode == "fill" {
                "na"
            } else {
                "error"
            }
            .into();
            rep.error_phase = name.into();
            rep.error = e;
            None
        }
        Err(_) => {
            rep.status = "panic".into();
            rep.error_phase = name.into();
            rep.error = PANIC_MSG.lock().map(|m| m.clone()).unwrap_or_default();
            None
        }
    }
}

fn count_paths(res: &ImportResult, rep: &mut Report) {
    rep.objects = res.objects.len();
    rep.layers = res.new_layers.len();
    rep.import_warnings = res.warnings.len();
    for o in &res.objects {
        match &o.kind {
            ObjectKind::Vector(v) => {
                rep.vector_objects += 1;
                rep.paths += v.paths.len();
                for p in &v.paths {
                    for s in &p.0 {
                        match s {
                            PathSeg::MoveTo { .. } => {
                                rep.subpaths += 1;
                                rep.nodes += 1;
                            }
                            PathSeg::Close => {}
                            _ => rep.nodes += 1,
                        }
                    }
                }
            }
            ObjectKind::Image(i) => {
                rep.image_objects += 1;
                rep.image_px += u64::from(i.width_px) * u64::from(i.height_px);
            }
            ObjectKind::Text(t) => {
                rep.text_objects += 1;
                rep.paths += t.outline.len();
            }
        }
    }
}

/// Escala y traslada el diseño para que quepa en la cama. Devuelve el factor aplicado.
fn fit_to_bed(doc: &mut Document, bed: [f64; 2]) -> Result<f64, String> {
    let b = doc.bounds().ok_or("el diseño no tiene geometría")?;
    let (w, h) = (b.max.x - b.min.x, b.max.y - b.min.y);
    if !(w.is_finite() && h.is_finite()) {
        return Err("la caja del diseño no es finita".into());
    }
    let margin = 2.0;
    let avail = [bed[0] - 2.0 * margin, bed[1] - 2.0 * margin];
    let s = if w > avail[0] || h > avail[1] {
        (avail[0] / w.max(1e-9)).min(avail[1] / h.max(1e-9))
    } else {
        1.0
    };
    let t: Transform = affine::compose(
        &affine::translate(margin, margin),
        &affine::compose(&affine::scale(s, s), &affine::translate(-b.min.x, -b.min.y)),
    );
    for o in &mut doc.objects {
        o.transform = affine::compose(&t, &o.transform);
    }
    Ok(s)
}

fn build_document(res: ImportResult, mode: Mode) -> Document {
    let mut layers = res.new_layers;
    if mode == Mode::Fill {
        for l in &mut layers {
            if l.mode == LayerMode::Line {
                l.mode = LayerMode::Fill;
                l.interval = 0.2;
                l.overscan = Overscan::Auto;
            }
        }
    }
    Document {
        objects: res.objects,
        layers,
        ..Document::default()
    }
}

/// Opciones del trabajo. La variable `TLAKU_BENCH_DISABLE` (lista separada por comas con
/// `inner_first`, `reduce_travel`, `best_start`, `choose_direction`, `by_priority`) apaga esas
/// optimizaciones para ver cuál domina el tiempo de cálculo.
fn job_options() -> JobOptions {
    let mut o = JobOptions::default();
    if let Ok(list) = std::env::var("TLAKU_BENCH_DISABLE") {
        for name in list.split(',').map(str::trim) {
            match name {
                "inner_first" => o.optimization.inner_first = false,
                "reduce_travel" => o.optimization.reduce_travel = false,
                "best_start" => o.optimization.best_start = false,
                "choose_direction" => o.optimization.choose_direction = false,
                "by_priority" => o.optimization.by_priority = false,
                _ => {}
            }
        }
    }
    o
}

/// Longitud (mm) de los movimientos `Cut` con potencia, que es lo que debe quemar el láser.
fn cut_stats(job: &ComputedJob) -> (usize, usize, f64) {
    let (mut moves, mut cuts, mut len) = (0usize, 0usize, 0.0f64);
    let (mut px, mut py) = (0.0f64, 0.0f64);
    for b in &job.toolpath.blocks {
        for m in &b.moves {
            moves += 1;
            let p = m.to();
            if let Move::Cut { power_pct, .. } = m
                && *power_pct > 0.0
            {
                cuts += 1;
                len += (p.x - px).hypot(p.y - py);
            }
            px = p.x;
            py = p.y;
        }
    }
    (moves, cuts, len)
}

pub fn run(path: &Path, args: &WorkerArgs) {
    std::panic::set_hook(Box::new(|info| {
        let msg = format!("{info}").replace('\n', " | ");
        if let Ok(mut m) = PANIC_MSG.lock() {
            *m = msg;
        }
    }));
    let started = Instant::now();
    let mut rep = Report {
        file: path.display().to_string(),
        category: path
            .parent()
            .and_then(Path::file_name)
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default(),
        mode: args.mode.name().into(),
        file_bytes: std::fs::metadata(path).map_or(0, |m| m.len()),
        fit_scale: 1.0,
        status: "ok".into(),
        ..Report::default()
    };
    let mut slow: Vec<&str> = Vec::new();
    let mark = |slow: &mut Vec<&str>, name: &'static str, secs: f64| {
        if secs > args.threshold_s {
            slow.push(name);
        }
    };
    let device = DeviceProfile::default();

    // 1. importar
    let Some((res, secs)) = phase(&mut rep, "import", |_| import_file(path)) else {
        finish(rep, started, &slow);
        return;
    };
    rep.import_s = secs;
    count_paths(&res, &mut rep);
    rep.import_rss_mb = peak_rss_mb();
    mark(&mut slow, "import", secs);

    // 2. calcular el trabajo con el perfil GRBL típico
    let mut doc = build_document(res, args.mode);
    let Some((job, secs)) = phase(&mut rep, "compute", |rep| {
        rep.fit_scale = fit_to_bed(&mut doc, device.bed)?;
        compute(&doc, &device, &job_options())
    }) else {
        finish(rep, started, &slow);
        return;
    };
    rep.compute_s = secs;
    rep.est_s = job.preview.total_s;
    rep.blocks = job.toolpath.blocks.len();
    let (moves, cuts, len) = cut_stats(&job);
    rep.moves = moves;
    rep.cut_moves = cuts;
    rep.cut_len_mm = len;
    rep.compute_warnings = job.preview.warnings.len();
    rep.compute_rss_mb = peak_rss_mb();
    mark(&mut slow, "compute", secs);
    drop(doc);

    // 3. G-code
    let mut lines: Vec<String> = Vec::new();
    if let Some((text, secs)) = phase(&mut rep, "gcode", |_| Ok(gcode(&job, &device))) {
        rep.gcode_s = secs;
        rep.gcode_bytes = text.len();
        lines = text.lines().map(str::to_string).collect();
        rep.gcode_lines = lines.len();
        rep.gcode_rss_mb = peak_rss_mb();
        mark(&mut slow, "gcode", secs);
    }

    // 4. exportar .rd
    if let Some((rd, secs)) = phase(&mut rep, "rd", |_| export_rd_for_device(&job, &device)) {
        rep.rd_s = secs;
        rep.rd_bytes = rd.len();
        rep.rd_rss_mb = peak_rss_mb();
        mark(&mut slow, "rd", secs);
    }

    // 5. emulador GRBL con reloj acelerado
    if !lines.is_empty() {
        if !args.emu {
            rep.emu_status = "omitido: --no-emu".into();
        } else if lines.len() > args.emu_max_lines {
            rep.emu_status = format!("omitido: {} líneas (> {})", lines.len(), args.emu_max_lines);
        } else if let Some((_, secs)) = phase(&mut rep, "emu", |rep| {
            emulate(rep, &lines, &device, args);
            Ok(())
        }) {
            rep.emu_real_s = secs;
            rep.emu_rss_mb = peak_rss_mb();
        } else if rep.emu_status.is_empty() {
            rep.emu_status = "fallo: pánico".into();
        }
    }
    finish(rep, started, &slow);
}

fn finish(mut rep: Report, started: Instant, slow: &[&str]) {
    rep.total_s = started.elapsed().as_secs_f64();
    rep.peak_rss_mb = peak_rss_mb();
    rep.slow = slow.join("+");
    rep.running.clear();
    emit(&rep);
}

fn emulate(rep: &mut Report, lines: &[String], device: &DeviceProfile, args: &WorkerArgs) {
    // La máquina virtual vive en este proceso: si el anfitrión no alcanza a darle líneas, se
    // queda sin planificador y el tiempo simulado se infla. Se limita la aceleración del reloj
    // para que la máquina gaste las líneas bastante más despacio de lo que el anfitrión las da.
    let lines_per_sim_s = lines.len() as f64 / rep.est_s.max(1.0);
    let host_cap = HOST_LINES_PER_S * 0.2 / lines_per_sim_s.max(1e-9);
    let speed = (rep.est_s / 8.0)
        .min(host_cap)
        .clamp(1.0, args.emu_max_speed);
    rep.emu_speed = speed;
    let profile = Profile::grbl_1_1h()
        .with_setting(30, device.s_max)
        .with_setting(110, device.max_rate[0])
        .with_setting(111, device.max_rate[1])
        .with_setting(120, device.accel[0])
        .with_setting(121, device.accel[1])
        .with_setting(130, device.bed[0])
        .with_setting(131, device.bed[1]);
    let (mut vm, h) = VirtualMachine::builder(profile)
        .clock(ScaledClock::new(speed))
        .homed(false)
        .build();
    if vm.clear_input().is_err() {
        rep.emu_status = "fallo: no se pudo leer el banner".into();
        return;
    }
    let cfg = StreamerConfig {
        status_interval: Some(Duration::from_millis(1)),
        ..StreamerConfig::default()
    };
    let streamer = Streamer::new(vm, cfg);
    let sim0 = h.now();
    let job = match streamer.spawn(lines) {
        Ok(j) => j,
        Err(e) => {
            rep.emu_status = format!("fallo: líneas inválidas: {e:?}");
            return;
        }
    };
    let control = job.control.clone();
    let start = Instant::now();
    let deadline = Duration::from_secs_f64(args.emu_deadline_s);
    let mut outcome: Option<Outcome> = None;
    let mut timed_out = false;
    let mut errors = 0usize;
    let mut cancelled_at: Option<Instant> = None;
    let (mut samples, mut starved) = (0u64, 0u64);
    let mut next_sample = Instant::now();
    let mut pending = true;
    loop {
        if Instant::now() >= next_sample {
            next_sample = Instant::now() + Duration::from_millis(2);
            if pending {
                samples += 1;
                if h.planner_used() == 0 {
                    starved += 1;
                }
            }
        }
        match job.events.recv_timeout(Duration::from_millis(5)) {
            Ok(Event::Finished(o)) => {
                outcome = Some(o);
                break;
            }
            Ok(Event::Error { .. }) => errors += 1,
            Ok(Event::Progress { sent, total, .. }) => pending = sent < total,
            Ok(_) => {}
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                if start.elapsed() > deadline && !timed_out {
                    timed_out = true;
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
    let sim_s = (h.now() - sim0).as_secs_f64();
    if job.join().is_err() {
        rep.emu_status = "fallo: pánico del hilo del streamer".into();
        return;
    }
    rep.emu_errors = errors;
    rep.emu_starved_pct = if samples > 0 {
        starved as f64 / samples as f64 * 100.0
    } else {
        0.0
    };
    rep.emu_sim_s = sim_s;
    if timed_out {
        rep.emu_status = format!("fallo: plazo de {:.0} s real agotado", args.emu_deadline_s);
        return;
    }
    match outcome {
        Some(Outcome::Completed { .. }) => {}
        other => {
            rep.emu_status = format!("fallo: {other:?}");
            return;
        }
    }
    let trace = h.trace();
    rep.emu_rapid_burns = trace.burned_during_rapids().len();
    rep.emu_stationary_burns = trace.stationary_burns().len();
    rep.emu_burn_mm = trace.total_length();
    rep.emu_diff_pct = if rep.est_s > 0.0 {
        (sim_s - rep.est_s) / rep.est_s * 100.0
    } else {
        0.0
    };
    rep.emu_len_diff_pct = if rep.cut_len_mm > 0.0 {
        (rep.emu_burn_mm - rep.cut_len_mm) / rep.cut_len_mm * 100.0
    } else {
        0.0
    };
    rep.emu_status = "ok".into();
}
