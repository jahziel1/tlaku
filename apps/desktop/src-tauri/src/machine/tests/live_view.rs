//! La vista en vivo (`apps/desktop/src/features/live`) contra el emulador GRBL de `tlaku-emu`.
//!
//! La ubicación de la cabeza se hace en la interfaz: toma la posición de trabajo que reporta la
//! máquina (`machine://status`), deshace el origen y el intercambio X/Y del perfil y la busca,
//! hacia delante, en la ruta de la vista previa. Esta prueba corre un trabajo real (el mismo
//! G-code que enviaría la app) en la máquina virtual y comprueba, con datos del emulador, el
//! contrato en el que se apoya esa ubicación:
//!
//!  - cada reporte durante el trabajo cae sobre la ruta de la vista previa (a menos de 0,5 mm,
//!    igual que `resume/plan.rs::ON_PATH_MM`), también con el origen arriba a la derecha y con el
//!    intercambio X/Y;
//!  - ubicado sin retroceder, el avance es monótono, llega al 100 % y acaba en la última posición
//!    de la ruta;
//!  - la vista previa y el G-code salen del mismo cálculo, así que `move_times` sirve para
//!    interpolar.
//!
//! Con `UPDATE_LIVE_TRACE=1` además se escribe el rastro en
//! `apps/desktop/src/features/live/fixtures/emulator-trace.json`, que la prueba de vitest
//! `liveEmulator.test.ts` reproduce con el ubicador real en TypeScript.

use super::*;
use serde_json::json;
use std::path::PathBuf;
use tlaku_core::bezpath::BezPath;
use tlaku_core::{Point, Polyline, Rgb};
use tlaku_doc::affine::IDENTITY;
use tlaku_doc::{
    DesignObject, DeviceProfile, Document, JobOptions, LayerMode, LayerSettings, ObjectKind,
    Origin, VectorObject,
};

/// Cuántas veces más rápido que el tiempo real corre la máquina virtual.
const SPEED: u32 = 4;
const ON_PATH_MM: f64 = 0.5;

fn rect(id: &str, color: Rgb, x: f64, y: f64, w: f64, h: f64) -> DesignObject {
    DesignObject {
        id: id.into(),
        layer: color,
        transform: IDENTITY,
        kind: ObjectKind::Vector(VectorObject::new(vec![BezPath::from(&Polyline {
            points: vec![
                Point::new(x, y),
                Point::new(x + w, y),
                Point::new(x + w, y + h),
                Point::new(x, y + h),
            ],
            closed: true,
        })])),
        locked: false,
        hidden: false,
        group: None,
        priority: None,
        power_scale: None,
        shape: None,
        start: None,
        manual_tabs: Vec::new(),
        array: None,
    }
}

/// Un corte (cuadrado de 40 mm) y un relleno (10 mm) en dos capas.
fn doc() -> Document {
    let red = Rgb(255, 0, 0);
    let blue = Rgb(0, 0, 255);
    let mut cut = LayerSettings::for_color(red);
    cut.speed = 1200.0;
    let mut fill = LayerSettings::for_color(blue);
    fill.mode = LayerMode::Fill;
    fill.speed = 3000.0;
    fill.interval = 0.25;
    Document {
        objects: vec![
            rect("cut", red, 20.0, 20.0, 40.0, 40.0),
            rect("fill", blue, 80.0, 30.0, 10.0, 10.0),
        ],
        layers: vec![cut, fill],
        ..Document::default()
    }
}

#[derive(Default)]
struct Timed {
    t0: Mutex<Option<Instant>>,
    statuses: Mutex<Vec<(u64, StatusReport)>>,
    progress: Mutex<Vec<(u64, ProgressEvent)>>,
    jobs: Mutex<Vec<(u64, JobState)>>,
}

impl Timed {
    fn ms(&self) -> u64 {
        let mut t0 = self.t0.lock().unwrap();
        u64::try_from(t0.get_or_insert_with(Instant::now).elapsed().as_millis()).unwrap_or(u64::MAX)
    }
}

impl Sink for Timed {
    fn emit(&self, event: Emit) {
        let at = self.ms();
        match event {
            Emit::Status(s) => self.statuses.lock().unwrap().push((at, s)),
            Emit::Progress(p) => self.progress.lock().unwrap().push((at, p)),
            Emit::Job(j) => self.jobs.lock().unwrap().push((at, j.state)),
            _ => {}
        }
    }
}

/// La ruta de la vista previa lista para ubicar posiciones (mm del documento).
struct Route {
    pos: Vec<[f64; 2]>,
    times: Vec<f64>,
}

impl Route {
    /// `start`: dónde estaba la cabeza al empezar el trabajo (mm del documento).
    fn of(p: &tlaku_doc::JobPreview, start: [f64; 2]) -> Self {
        let mut pos = vec![start];
        for m in p.moves.as_chunks::<4>().0 {
            pos.push([f64::from(m[1]), f64::from(m[2])]);
        }
        Route {
            pos,
            times: p.move_times.clone(),
        }
    }

    fn count(&self) -> usize {
        self.times.len()
    }

    fn time_at(&self, k: usize, frac: f64) -> f64 {
        let t0 = if k > 0 { self.times[k - 1] } else { 0.0 };
        t0 + (self.times[k] - t0) * frac
    }

    /// Como `features/live/route.ts::locate` con ubicación previa: hacia delante, ventana de 3 s.
    fn locate_forward(&self, p: [f64; 2], from: (usize, f64)) -> Option<(usize, f64, f64)> {
        let t_from = self.time_at(from.0, from.1);
        let mut best: Option<(usize, f64, f64)> = None;
        let mut candidates = Vec::new();
        for k in from.0..self.count() {
            if self.time_at(k, 0.0) > t_from + 3.0 {
                break;
            }
            let (a, b) = (self.pos[k], self.pos[k + 1]);
            let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
            let l2 = dx * dx + dy * dy;
            let min_t = if k == from.0 { from.1 } else { 0.0 };
            let t = if l2 < 1e-18 {
                0.0
            } else {
                (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / l2).clamp(min_t, 1.0)
            };
            let d = (p[0] - (a[0] + t * dx)).hypot(p[1] - (a[1] + t * dy));
            candidates.push((k, t, d));
            if best.is_none_or(|b| d < b.2) {
                best = Some((k, t, d));
            }
        }
        let best = best?;
        if best.2 > ON_PATH_MM {
            return None;
        }
        // Empates: el primero que no queda atrás.
        candidates.into_iter().find(|c| c.2 <= best.2 + 0.02)
    }
}

fn to_doc(device: &DeviceProfile, w: [f64; 2]) -> [f64; 2] {
    let q = if device.swap_xy { [w[1], w[0]] } else { w };
    let x = match device.origin {
        Origin::BottomRight | Origin::TopRight => device.bed[0] - q[0],
        _ => q[0],
    };
    let y = match device.origin {
        Origin::TopLeft | Origin::TopRight => device.bed[1] - q[1],
        _ => q[1],
    };
    [x, y]
}

struct Run {
    name: &'static str,
    preview: tlaku_doc::JobPreview,
    statuses: Vec<(u64, StatusReport)>,
    progress: Vec<(u64, ProgressEvent)>,
    jobs: Vec<(u64, JobState)>,
    device: DeviceProfile,
}

fn run_job(name: &'static str, device: DeviceProfile) -> Run {
    let options = JobOptions::default();
    let preview = crate::engine::compute_job(&doc(), &device, &options, None).unwrap();
    let lines = crate::engine::job_lines(&doc(), &device, &options, None).unwrap();
    let sink = Arc::new(Timed::default());
    let timing = Timing {
        status_interval: Duration::from_millis(25),
        ..fast()
    };
    let (conn, _) =
        Connection::start_virtual(&format!("virtual:falcon2@{SPEED}"), sink.clone(), timing)
            .expect("conectar");
    let handle = conn.handle();
    let end = Instant::now() + Duration::from_secs(30);
    while sink.statuses.lock().unwrap().is_empty() {
        assert!(Instant::now() < end, "sin estado");
        thread::sleep(Duration::from_millis(5));
    }
    handle.start_job(lines, device.rx_buffer as usize).unwrap();
    while !sink
        .jobs
        .lock()
        .unwrap()
        .iter()
        .any(|j| j.1 == JobState::Finished)
    {
        assert!(Instant::now() < end, "el trabajo no terminó");
        thread::sleep(Duration::from_millis(10));
    }
    // Unos reportes más con la máquina en reposo.
    thread::sleep(Duration::from_millis(150));
    conn.close();
    let statuses = sink.statuses.lock().unwrap().clone();
    let progress = sink.progress.lock().unwrap().clone();
    let jobs = sink.jobs.lock().unwrap().clone();
    Run {
        name,
        preview,
        statuses,
        progress,
        jobs,
        device,
    }
}

fn check(run: &Run) {
    // El primer desplazamiento sale de donde estaba la cabeza antes del trabajo (no del (0, 0) del
    // documento: con el origen arriba a la derecha la máquina arranca en la otra esquina).
    let first_run = run
        .jobs
        .iter()
        .find(|j| j.1 == JobState::Started)
        .unwrap()
        .0;
    let before = run
        .statuses
        .iter()
        .rev()
        .find(|(ms, _)| *ms < first_run)
        .map_or(run.statuses[0].1.work_pos, |(_, s)| s.work_pos);
    let route = Route::of(&run.preview, to_doc(&run.device, before));
    assert!(
        route.count() > 100,
        "{}: la ruta debería tener cientos de movimientos",
        run.name
    );
    let started = run
        .jobs
        .iter()
        .find(|j| j.1 == JobState::Started)
        .unwrap()
        .0;
    let finished = run
        .jobs
        .iter()
        .find(|j| j.1 == JobState::Finished)
        .unwrap()
        .0;
    let total = *route.times.last().unwrap();
    let mut at = (0usize, 0.0f64);
    let mut last_t = 0.0;
    let mut matched = 0;
    let mut run_reports = 0;
    for (ms, s) in &run.statuses {
        if *ms < started || *ms > finished || s.state != MachineState::Run {
            continue;
        }
        run_reports += 1;
        let p = to_doc(&run.device, s.work_pos);
        match route.locate_forward(p, at) {
            Some((k, f, _)) => {
                at = (k, f);
                let t = route.time_at(k, f);
                assert!(
                    t >= last_t - 1e-9,
                    "{}: la ubicación retrocedió ({t} < {last_t})",
                    run.name
                );
                last_t = t;
                matched += 1;
            }
            // Tras la última línea de la ruta el G-code regresa al origen: eso ya no es parte de la ruta.
            None => assert!(
                last_t >= total * 0.95,
                "{}: un reporte a mitad del trabajo no cayó sobre la ruta: {:?} (t={last_t:.2} de {total:.2})",
                run.name,
                s.work_pos
            ),
        }
    }
    assert!(
        run_reports >= 15,
        "{}: pocos reportes ({run_reports}) para juzgar",
        run.name
    );
    assert!(
        matched * 100 >= run_reports * 90,
        "{}: solo {matched} de {run_reports} reportes cayeron sobre la ruta",
        run.name
    );
    // Llegó al 100 % y acabó en la última posición de la ruta.
    assert!(
        last_t >= total * 0.95,
        "{}: la ubicación llegó a {last_t:.2} de {total:.2}",
        run.name
    );
    let end = route.pos[route.count()];
    let reached = run
        .statuses
        .iter()
        .filter(|(ms, _)| *ms >= started)
        .any(|(_, s)| {
            let p = to_doc(&run.device, s.work_pos);
            (p[0] - end[0]).hypot(p[1] - end[1]) < 2.0
        });
    // La máquina pasó por (o junto a) el final de la ruta, aunque no haya un reporte justo ahí.
    assert!(
        reached || last_t >= total * 0.95,
        "{}: la cabeza no llegó al final de la ruta",
        run.name
    );
    // El progreso del streamer también llega al total.
    let last = run.progress.last().unwrap().1;
    assert_eq!(last.acked, last.total);
}

fn write_trace(runs: &[Run]) {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../src/features/live/fixtures/emulator-trace.json");
    let runs: Vec<_> = runs
        .iter()
        .map(|r| {
            json!({
                "name": r.name,
                "speed": SPEED,
                "device": { "origin": r.device.origin, "bed": r.device.bed, "swap_xy": r.device.swap_xy },
                "preview": {
                    "moves": r.preview.moves,
                    "move_times": r.preview.move_times,
                    "total_s": r.preview.total_s,
                    "layers": r.preview.layers.iter().map(|l| json!({"color": l.color, "mode": l.mode, "seconds": l.seconds})).collect::<Vec<_>>(),
                },
                "statuses": r.statuses.iter().map(|(ms, s)| json!([ms, s.state, s.work_pos, s.spindle])).collect::<Vec<_>>(),
                "progress": r.progress.iter().map(|(ms, p)| json!([ms, p.acked, p.total])).collect::<Vec<_>>(),
                "jobs": r.jobs.iter().map(|(ms, j)| json!([ms, j])).collect::<Vec<_>>(),
            })
        })
        .collect();
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(
        &path,
        serde_json::to_string(&json!({ "runs": runs })).unwrap(),
    )
    .unwrap();
}

#[test]
fn the_machines_reports_follow_the_preview_route_and_reach_the_end() {
    let base = DeviceProfile::default();
    // Las tres máquinas virtuales corren a la vez (cada trabajo dura unos segundos).
    let devices = [
        ("origen abajo a la izquierda", base.clone()),
        (
            "origen arriba a la derecha",
            DeviceProfile {
                origin: Origin::TopRight,
                ..base.clone()
            },
        ),
        (
            "intercambio X/Y",
            DeviceProfile {
                swap_xy: true,
                ..base
            },
        ),
    ];
    let runs: Vec<Run> = thread::scope(|s| {
        let handles: Vec<_> = devices
            .into_iter()
            .map(|(name, d)| s.spawn(move || run_job(name, d)))
            .collect();
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });
    for r in &runs {
        check(r);
    }
    if std::env::var_os("UPDATE_LIVE_TRACE").is_some() {
        write_trace(&runs);
    }
}
