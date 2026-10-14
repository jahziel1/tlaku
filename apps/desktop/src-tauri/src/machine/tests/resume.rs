//! Continuar un trabajo interrumpido, con el emulador (`tlaku-emu`) y las máquinas virtuales: un
//! corte de luz y un cierre de la app a mitad del trabajo, y la reanudación completa (homing o
//! confirmación, encuadre, preámbulo con el láser apagado).

use super::*;
use crate::machine::resume::{
    self, CheckpointPlan, plan::PlanOptions, start, store::CheckpointStore, walker::Parsed,
    walker::StepKind,
};
use std::fs;
use std::path::PathBuf;
use tlaku_doc::{DeviceKind, DeviceProfile, JobOptions, ResumeKind, ResumeStart};
use tlaku_emu::trace::dist_point_segment;
use tlaku_emu::{Fault, Trigger};

/// Banco de pruebas: una máquina virtual que sobrevive a las conexiones (como la placa de verdad
/// sobrevive a que la app se cierre o se corte la luz) y una carpeta de punto de control propia.
struct Bench {
    dir: PathBuf,
    store: Arc<CheckpointStore>,
    emu: tlaku_emu::Handle,
    port: String,
    device: DeviceProfile,
    sink: Arc<Collect>,
    conn: Option<Connection>,
}

impl Bench {
    fn new(name: &str, port: &str, kind: DeviceKind) -> Self {
        let dir =
            std::env::temp_dir().join(format!("tlaku-resume-it-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let (_vm, emu) = virtual_port::build(port).expect("máquina virtual");
        Self {
            store: Arc::new(CheckpointStore::new(&dir)),
            dir,
            emu,
            port: port.to_string(),
            device: DeviceProfile::for_kind(kind),
            sink: Arc::new(Collect::default()),
            conn: None,
        }
    }

    /// «Abre otra vez el puerto» de la misma placa.
    fn connect(&mut self) -> Handle {
        self.sink = Arc::new(Collect::default());
        let vm = self.emu.reconnect();
        let (conn, _) = Connection::start_with(
            Box::new(vm),
            &self.port,
            self.sink.clone(),
            // El homing de la Falcon2 simulada dura 8 s de reloj de la máquina.
            Timing {
                home_timeout: Duration::from_secs(10),
                ..fast()
            },
            Some(self.emu.clone()),
        )
        .expect("conectar");
        let h = conn.handle();
        self.conn = Some(conn);
        self.sink.wait("estado", |s| !s.statuses().is_empty());
        h
    }

    fn plan(&self, lines: &[String]) -> CheckpointPlan {
        resume::plan_for_job(
            self.store.clone(),
            &self.device,
            &JobOptions::default(),
            Some("pieza.tlaku".into()),
            None,
            lines,
        )
        .expect("el perfil se puede reanudar")
    }

    fn start(&self, h: &Handle, lines: &[String]) {
        h.start_job_checkpointed(
            lines.to_vec(),
            LinkProfile::for_device(&self.device),
            Some(self.device.s_max),
            None,
            Some(self.plan(lines)),
        )
        .expect("iniciar");
    }

    /// Espera a que el láser lleve `mm` grabados.
    fn wait_burned(&self, mm: f64) {
        let end = Instant::now() + Duration::from_secs(30);
        while self.emu.trace().total_length() < mm {
            assert!(Instant::now() < end, "el trabajo no avanzó");
            thread::sleep(Duration::from_millis(5));
        }
    }

    /// Las líneas recibidas por la placa desde `from` que no son de consulta (`$...`).
    fn gcode_since(&self, from: usize) -> Vec<String> {
        self.emu.received_lines()[from..]
            .iter()
            .filter(|l| !l.starts_with('$'))
            .cloned()
            .collect()
    }
}

impl Drop for Bench {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

/// Un rayado de 20 filas de 40 mm (zig-zag, 800 mm de corte): unos 4 s a ×4.
fn hatch_job() -> Vec<String> {
    let mut v: Vec<String> = ["G90", "G21", "M4 S0", "G0 X10 Y10"]
        .map(String::from)
        .to_vec();
    for i in 0..20 {
        let y = 10.0 + f64::from(i) * 0.5;
        let (from, to) = if i % 2 == 0 { (10, 50) } else { (50, 10) };
        v.push(format!("G0 X{from} Y{y}"));
        v.push(if i == 0 {
            format!("G1 X{to} S500 F3000")
        } else {
            format!("G1 X{to}")
        });
    }
    v.extend(["M5", "G0 X0 Y0"].map(String::from));
    v
}

fn cut_length(lines: &[String]) -> f64 {
    Parsed::parse(lines)
        .steps
        .iter()
        .filter(|s| s.kind == StepKind::Cut)
        .map(|s| s.length())
        .sum()
}

fn args(
    l: &resume::store::Loaded,
    view: &tlaku_doc::ResumeView,
) -> impl Fn(bool, bool, bool) -> ResumeStart + use<> {
    let (id, line, at) = (l.meta.id.clone(), view.suggested.line, view.suggested.at);
    move |confirm_unmoved, framed_ok, checklist_ok| ResumeStart {
        id: id.clone(),
        line,
        at,
        confirm_unmoved,
        framed_ok,
        checklist_ok,
    }
}

/// Todo punto de un corte del trabajo tiene el láser encendido a menos de `tol` mm en el rastro.
fn uncovered(lines: &[String], emu: &tlaku_emu::Handle, tol: f64) -> usize {
    let trace = emu.trace();
    let mut missing = 0;
    for s in Parsed::parse(lines)
        .steps
        .iter()
        .filter(|s| s.kind == StepKind::Cut)
    {
        let n = (s.length() / 0.5).ceil().max(1.0) as usize;
        for k in 0..=n {
            let t = k as f64 / n as f64;
            let p = [
                s.from[0] + (s.to[0] - s.from[0]) * t,
                s.from[1] + (s.to[1] - s.from[1]) * t,
            ];
            let near = trace.segments().iter().any(|g| {
                dist_point_segment(p, [g.start[0], g.start[1]], [g.end[0], g.end[1]]) <= tol
            });
            if !near {
                missing += 1;
            }
        }
    }
    missing
}

/// De la reconexión al final: sin home no se puede, con home y sin encuadre tampoco, y al
/// reanudar el láser nunca se enciende de camino al punto de inicio. Devuelve el punto sugerido.
fn continue_after_interruption(b: &mut Bench, lines: &[String]) -> tlaku_doc::ResumeView {
    let loaded = b.store.load().expect("quedó el punto de control");
    let info = resume::summarize(&loaded);
    assert!(info.can_resume, "{info:?}");
    assert!(
        info.percent > 0.0 && info.percent < 100.0,
        "{}",
        info.percent
    );
    let view = resume::view(&b.store, &loaded, &PlanOptions::default()).unwrap();
    assert!(!view.suggested.mismatch, "{:?}", view.suggested);
    let make = args(&loaded, &view);

    let h = b.connect();
    // La placa volvió sin posición: con homing activado hay que referenciarla.
    let before = b.emu.received_lines().len();
    let ready = start::readiness(&h);
    assert_eq!(ready.homing, Some(true));
    assert!(!ready.homed, "{ready:?}");
    let e = start::resume_now(&h, b.store.clone(), &loaded, &make(true, true, true)).unwrap_err();
    assert!(e.contains("homing"), "{e}");
    // Ni con la confirmación de «no se movió»: con homing, el homing es obligatorio.
    assert!(
        b.gcode_since(before).is_empty(),
        "no debió enviarse nada: {:?}",
        b.gcode_since(before)
    );

    h.command("$H").unwrap();
    assert!(h.is_homed());
    // Sin encuadre o sin lista de comprobación, tampoco.
    let e = start::resume_now(&h, b.store.clone(), &loaded, &make(false, false, true)).unwrap_err();
    assert!(e.contains("Encuadra"), "{e}");
    let e = start::resume_now(&h, b.store.clone(), &loaded, &make(false, true, false)).unwrap_err();
    assert!(e.contains("lista de comprobación"), "{e}");
    assert!(b.gcode_since(before).is_empty());

    // Encuadre del trabajo completo con el láser apagado.
    let burned_before_frame = b.emu.trace().segments().len();
    start::frame(&h, &b.store, &loaded, 6000.0).unwrap();
    b.sink.wait("encuadre", |s| {
        s.logs().iter().any(|(_, t)| t.contains("frame_done"))
    });
    assert_eq!(
        b.emu.trace().segments().len(),
        burned_before_frame,
        "el encuadre no debe quemar"
    );

    // Reanudar.
    let mark = b.emu.received_lines().len();
    let trace_mark = b.emu.trace().segments().len();
    start::resume_now(&h, b.store.clone(), &loaded, &make(false, true, true)).unwrap();
    b.sink.wait("trabajo terminado", |s| {
        s.jobs().contains(&JobState::Finished)
    });

    // El láser no se enciende hasta llegar al punto: M5 primero, el G0 antes que M4.
    let sent = b.gcode_since(mark);
    assert_eq!(sent[0], "M5", "{sent:?}");
    let g0 = sent
        .iter()
        .position(|l| l.starts_with("G0X"))
        .expect("G0 al punto");
    let on = sent
        .iter()
        .position(|l| l.starts_with("M4") || l.starts_with("M3"))
        .expect("enciende el láser");
    assert!(g0 < on, "el láser se enciende antes de llegar: {sent:?}");
    assert!(b.emu.trace().burned_during_rapids().is_empty());
    // Lo primero que quema la reanudación empieza en el punto de inicio.
    let trace = b.emu.trace();
    let first = &trace.segments()[trace_mark];
    let start_pt = view.suggested.at.unwrap_or_else(|| {
        resume::walker::modal_at(lines, view.suggested.line as usize, false)
            .1
            .unwrap()
    });
    let d =
        ((first.start[0] - start_pt[0]).powi(2) + (first.start[1] - start_pt[1]).powi(2)).sqrt();
    assert!(
        d < 0.5,
        "empezó a {d} mm del punto: {first:?} vs {start_pt:?}"
    );

    // Lo grabado antes más lo reanudado cubre todo el trabajo, con solape mínimo.
    assert_eq!(uncovered(lines, &b.emu, 0.3), 0, "quedaron huecos");
    // El solape es lo que avanzó la cabeza desde el último reporte guardado: el sondeo `?` del
    // trabajo (200 ms) más el intervalo del punto de control (30 ms aquí, 1 s en la app), a
    // 50 mm/s y ×4 de velocidad simulada: ~70 mm sobre 800 mm en un equipo normal. En los equipos
    // lentos de la CI el retraso del hilo lo agranda; el tope de 160 mm (20 % del trabajo) sigue
    // detectando un reinicio desde muy atrás.
    let overlap = b.emu.trace().total_length() - cut_length(lines);
    assert!((-1.0..160.0).contains(&overlap), "solape de {overlap} mm");
    // Y al terminar bien el punto de control se borra.
    assert!(b.store.load().is_none());
    view
}

#[test]
fn a_power_cut_in_the_middle_can_be_continued_after_homing_and_framing() {
    let mut b = Bench::new("power", "virtual:falcon2@4", DeviceKind::Grbl);
    let lines = hatch_job();
    let h = b.connect();
    b.start(&h, &lines);
    b.wait_burned(cut_length(&lines) * 0.4);
    // Se va la luz: la placa pierde la posición y se reinicia, la conexión se cae.
    b.emu.inject(Trigger::Now, Fault::PowerLoss);
    b.sink.wait("conexión perdida", |s| {
        s.connections().iter().any(|c| !c.connected)
    });
    assert!(b.emu.position_lost());
    let burned = b.emu.trace().total_length();
    assert!(burned < cut_length(&lines) * 0.9, "el corte llegó tarde");
    b.conn = None;
    // El punto de control sobrevivió (y dice cuánto se hizo).
    let info = resume::summarize(&b.store.load().unwrap());
    assert_eq!(info.kind, ResumeKind::Gcode);
    assert_eq!(info.document.as_deref(), Some("pieza.tlaku"));
    assert!(info.mpos.is_some());
    let view = continue_after_interruption(&mut b, &lines);
    // Retrocedió algo, pero no volvió a empezar.
    assert!(view.suggested.line > 6);
}

#[test]
fn closing_the_app_in_the_middle_can_be_continued_too() {
    let mut b = Bench::new("close", "virtual:falcon2@4", DeviceKind::Grbl);
    let lines = hatch_job();
    let h = b.connect();
    b.start(&h, &lines);
    b.wait_burned(cut_length(&lines) * 0.5);
    // La app se cierra con el trabajo en marcha: se suelta la conexión sin cancelar a propósito.
    b.conn.take().unwrap().close();
    assert!(b.sink.jobs().contains(&JobState::Cancelled));
    let l = b
        .store
        .load()
        .expect("el cierre no es cancelar a propósito");
    assert!(l.progress.is_some());
    let view = continue_after_interruption(&mut b, &lines);
    assert!(view.suggested.line > 6);
}

#[test]
fn without_homing_the_user_must_confirm_that_nothing_moved() {
    let mut b = Bench::new("nohome", "virtual:grbl11@4", DeviceKind::Grbl);
    let lines = hatch_job();
    let h = b.connect();
    b.start(&h, &lines);
    b.wait_burned(cut_length(&lines) * 0.4);
    b.conn.take().unwrap().close();
    let loaded = b.store.load().unwrap();
    let view = resume::view(&b.store, &loaded, &PlanOptions::default()).unwrap();
    let make = args(&loaded, &view);
    let h = b.connect();
    let ready = start::readiness(&h);
    assert_eq!(ready.homing, Some(false));
    let before = b.emu.received_lines().len();
    // Sin la confirmación de que la cabeza y el material no se movieron, no se reanuda.
    let e = start::resume_now(&h, b.store.clone(), &loaded, &make(false, true, true)).unwrap_err();
    assert!(e.contains("no tiene homing"), "{e}");
    assert!(b.gcode_since(before).is_empty());
    // El encuadre y la lista también hacen falta.
    assert!(start::resume_now(&h, b.store.clone(), &loaded, &make(true, false, true)).is_err());
    assert!(start::resume_now(&h, b.store.clone(), &loaded, &make(true, true, false)).is_err());
    assert!(b.gcode_since(before).is_empty());
    // Con todo, sigue y termina.
    start::resume_now(&h, b.store.clone(), &loaded, &make(true, true, true)).unwrap();
    b.sink
        .wait("terminado", |s| s.jobs().contains(&JobState::Finished));
    assert_eq!(uncovered(&lines, &b.emu, 0.3), 0);
    assert!(b.emu.trace().burned_during_rapids().is_empty());
    assert!(b.store.load().is_none());
}

#[test]
fn marlin_resumes_with_the_same_flow() {
    let mut b = Bench::new("marlin", "virtual:marlin@10", DeviceKind::Marlin);
    // Marlin escribe `S` entera y `F` también en los G0.
    let mut lines: Vec<String> = ["G90", "G21", "M3 S0 I", "G0 X10 Y10 F6000"]
        .map(String::from)
        .to_vec();
    for i in 0..40 {
        let y = 10.0 + f64::from(i) * 0.5;
        let (from, to) = if i % 2 == 0 { (10, 50) } else { (50, 10) };
        lines.push(format!("G0 X{from} Y{y}"));
        lines.push(if i == 0 {
            format!("G1 X{to} S128 F3000")
        } else {
            format!("G1 X{to}")
        });
    }
    lines.extend(["M5", "G0 X0 Y0"].map(String::from));
    let h = b.connect();
    b.start(&h, &lines);
    b.wait_burned(cut_length(&lines) * 0.4);
    b.conn.take().unwrap().close();
    let loaded = b.store.load().expect("punto de control");
    assert!(resume::summarize(&loaded).can_resume);
    let view = resume::view(&b.store, &loaded, &PlanOptions::default()).unwrap();
    let make = args(&loaded, &view);
    let h = b.connect();
    // Marlin no contesta `$$`: no se supone homing, así que pide la confirmación.
    assert_eq!(start::readiness(&h).homing, None);
    assert!(start::resume_now(&h, b.store.clone(), &loaded, &make(false, true, true)).is_err());
    let mark = b.emu.received_lines().len();
    start::resume_now(&h, b.store.clone(), &loaded, &make(true, true, true)).unwrap();
    b.sink
        .wait("terminado", |s| s.jobs().contains(&JobState::Finished));
    let sent = b.gcode_since(mark);
    // (La comprobación previa pregunta `M115` antes de nada.)
    let m5 = sent.iter().position(|l| l == "M5").unwrap();
    let g0 = sent.iter().position(|l| l.starts_with("G0 X")).unwrap();
    assert!(m5 < g0, "{sent:?}");
    let on = sent.iter().position(|l| l.starts_with("M3")).unwrap();
    assert!(g0 < on, "{sent:?}");
    assert!(
        sent[on].ends_with(" I"),
        "la bandera inline se conserva: {}",
        sent[on]
    );
    assert_eq!(uncovered(&lines, &b.emu, 0.3), 0);
}

#[test]
fn cancelling_on_purpose_or_finishing_clears_the_checkpoint_but_an_alarm_keeps_it() {
    // Terminar bien.
    let mut b = Bench::new("clear-ok", "virtual:grbl11@50", DeviceKind::Grbl);
    let h = b.connect();
    let short = [
        "G90",
        "G21",
        "M4 S0",
        "G0 X1 Y1",
        "G1 X5 Y5 S100 F6000",
        "M5",
    ]
    .map(String::from)
    .to_vec();
    b.start(&h, &short);
    b.sink
        .wait("terminado", |s| s.jobs().contains(&JobState::Finished));
    assert!(b.store.load().is_none());

    // Cancelar a propósito.
    let mut b = Bench::new("clear-cancel", "virtual:grbl11@4", DeviceKind::Grbl);
    let lines = hatch_job();
    let h = b.connect();
    b.start(&h, &lines);
    b.wait_burned(30.0);
    // El hilo del punto de control lo crea sin bloquear el envío: en discos lentos (Windows en la
    // CI) puede tardar más que lo que se tarda en quemar los primeros milímetros.
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    while b.store.load().is_none() && std::time::Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(
        b.store.load().is_some(),
        "durante el trabajo hay punto de control"
    );
    h.cancel().unwrap();
    b.sink
        .wait("cancelado", |s| s.jobs().contains(&JobState::Cancelled));
    assert!(b.store.load().is_none());

    // Una alarma en pleno trabajo: se conserva.
    let mut b = Bench::new("keep-alarm", "virtual:grbl11@4", DeviceKind::Grbl);
    let h = b.connect();
    b.start(&h, &lines);
    b.wait_burned(30.0);
    b.emu.inject(Trigger::Now, Fault::Alarm(2));
    b.sink
        .wait("error", |s| s.jobs().contains(&JobState::Error));
    let l = b
        .store
        .load()
        .expect("la alarma no borra el punto de control");
    assert!(l.progress.is_some());
}

#[test]
fn a_new_connection_to_another_machine_forgets_the_old_checkpoint() {
    let mut b = Bench::new("other", "virtual:grbl11@4", DeviceKind::Grbl);
    let lines = hatch_job();
    let h = b.connect();
    b.start(&h, &lines);
    b.wait_burned(30.0);
    b.conn.take().unwrap().close();
    assert!(b.store.load().is_some());
    assert!(!resume::forget_if_other_machine(&b.store, &b.device.id));
    assert!(
        b.store.load().is_some(),
        "la misma máquina conserva el punto"
    );
    assert!(resume::forget_if_other_machine(&b.store, "otra-maquina"));
    assert!(b.store.load().is_none());
}

#[test]
fn a_checkpoint_half_written_by_the_power_cut_does_not_break_anything() {
    let mut b = Bench::new("torn", "virtual:grbl11@4", DeviceKind::Grbl);
    let lines = hatch_job();
    let h = b.connect();
    b.start(&h, &lines);
    b.wait_burned(cut_length(&lines) * 0.3);
    b.conn.take().unwrap().close();
    // La luz se fue mientras se escribía el avance: temporal truncado y archivo dañado.
    fs::write(
        b.dir.join("progress.json.tmp"),
        br#"{"version":1,"seq":99,"ack"#,
    )
    .unwrap();
    let ok = b.store.load().unwrap();
    assert!(ok.progress.is_some(), "el temporal suelto se ignora");
    fs::write(b.dir.join("progress.json"), br#"{"version":1,"seq":9"#).unwrap();
    let torn = b.store.load().expect("el trabajo sigue ahí");
    assert!(torn.progress.is_none());
    let info = resume::summarize(&torn);
    assert!(info.can_resume && !info.progress_known);
    // Sin avance legible se propone volver a empezar y se avisa; se puede elegir el punto a mano.
    let view = resume::view(&b.store, &torn, &PlanOptions::default()).unwrap();
    assert_eq!(view.suggested.line, 0);
    assert!(!view.warnings.is_empty());
    // Un nuevo trabajo reemplaza lo anterior sin arrastrar el avance dañado.
    let h = b.connect();
    b.start(&h, &lines);
    b.wait_burned(b.emu.trace().total_length() + 20.0);
    thread::sleep(Duration::from_millis(100));
    let fresh = b.store.load().unwrap();
    assert_ne!(fresh.meta.id, torn.meta.id);
}
