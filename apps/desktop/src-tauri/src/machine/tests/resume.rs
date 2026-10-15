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
pub(super) struct Bench {
    pub(super) dir: PathBuf,
    pub(super) store: Arc<CheckpointStore>,
    pub(super) emu: tlaku_emu::Handle,
    pub(super) port: String,
    pub(super) device: DeviceProfile,
    pub(super) sink: Arc<Collect>,
    pub(super) conn: Option<Connection>,
}

impl Bench {
    pub(super) fn new(name: &str, port: &str, kind: DeviceKind) -> Self {
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
    pub(super) fn connect(&mut self) -> Handle {
        self.connect_via(|vm| Box::new(vm))
    }

    /// Como [`connect`](Self::connect), pasando el puerto por `wrap` (p. ej. un enlace con fallas).
    pub(super) fn connect_via(
        &mut self,
        wrap: impl FnOnce(tlaku_emu::VirtualMachine) -> Box<dyn Transport>,
    ) -> Handle {
        self.sink = Arc::new(Collect::default());
        let vm = self.emu.reconnect();
        let (conn, _) = Connection::start_with(
            wrap(vm),
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

    pub(super) fn plan(&self, lines: &[String]) -> CheckpointPlan {
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

    pub(super) fn start(&self, h: &Handle, lines: &[String]) {
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
    pub(super) fn wait_burned(&self, mm: f64) {
        let end = Instant::now() + Duration::from_secs(30);
        while self.emu.trace().total_length() < mm {
            assert!(Instant::now() < end, "el trabajo no avanzó");
            thread::sleep(Duration::from_millis(5));
        }
    }

    /// Espera a que el punto de control del trabajo en curso esté en disco. Lo crea el hilo del
    /// escritor sin bloquear el envío, así que en discos lentos (Windows en la CI, con muchas
    /// pruebas haciendo `sync_all` a la vez) puede tardar más que en quemar los primeros
    /// milímetros.
    pub(super) fn wait_checkpoint(&self) {
        let end = Instant::now() + Duration::from_secs(30);
        while self.store.load().is_none() && Instant::now() < end {
            thread::sleep(Duration::from_millis(10));
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

/// Dónde empieza a quemar de verdad una reanudación que arranca en la línea `line` (desde 0):
/// `at` si cae a mitad de un corte; si no, el inicio del primer corte en o después de esa línea.
/// Cuando el punto cae en un `G0` (o al final de un corte), el preámbulo va hasta la posición
/// previa a ese desplazamiento, pero el láser solo se enciende en el corte que viene después, que
/// puede estar a un paso del rayado (0,5 mm) de aquel punto.
fn first_burn_point(lines: &[String], line: u32, at: Option<[f64; 2]>) -> [f64; 2] {
    at.unwrap_or_else(|| {
        let parsed = Parsed::parse(lines);
        parsed
            .steps
            .iter()
            .find(|s| s.line >= line && s.kind == StepKind::Cut)
            .expect("queda algún corte desde el punto")
            .from
    })
}

pub(super) fn cut_length(lines: &[String]) -> f64 {
    Parsed::parse(lines)
        .steps
        .iter()
        .filter(|s| s.kind == StepKind::Cut)
        .map(|s| s.length())
        .sum()
}

pub(super) fn args(
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
pub(super) fn uncovered(lines: &[String], emu: &tlaku_emu::Handle, tol: f64) -> usize {
    uncovered_where(lines, emu, tol, |_| true)
}

/// Como [`uncovered`], mirando solo los puntos que cumplen `wanted` (lo que se grabó antes de la
/// interrupción no está en el rastro de una máquina virtual nueva).
fn uncovered_where(
    lines: &[String],
    emu: &tlaku_emu::Handle,
    tol: f64,
    wanted: impl Fn([f64; 2]) -> bool,
) -> usize {
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
            if !near && wanted(p) {
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
    let info = resume::summarize(&b.store, &loaded, None);
    assert!(info.can_resume, "{info:?}");
    assert!(
        info.percent > 0.0 && info.percent < 100.0,
        "{}",
        info.percent
    );
    let view = resume::view(&b.store, &loaded, &PlanOptions::default(), None).unwrap();
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
    // Lo primero que quema la reanudación empieza donde empieza el primer corte desde el punto.
    let trace = b.emu.trace();
    let first = &trace.segments()[trace_mark];
    let start_pt = first_burn_point(lines, view.suggested.line, view.suggested.at);
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
    let info = resume::summarize(&b.store, &b.store.load().unwrap(), None);
    assert_eq!(info.kind, ResumeKind::Gcode);
    assert_eq!(info.document.as_deref(), Some("pieza.tlaku"));
    assert!(info.mpos.is_some());
    let view = continue_after_interruption(&mut b, &lines);
    // Retrocedió algo, pero no volvió a empezar.
    assert!(view.suggested.line > 6);
}

/// Reproduce sin azar el caso que hacía intermitente la prueba del reinicio: el punto sugerido cae
/// en el `G0` de una fila y lo primero que se quema es el inicio de la fila siguiente, a 0,5 mm de
/// la posición que da `modal_at` (el final de la fila anterior).
#[test]
fn the_first_burn_after_a_rapid_is_the_next_cut_not_the_position_before_it() {
    let lines = hatch_job();
    // Fila 8 (y = 14): su G0 está en la línea 4 + 2·8 = 20 y la fila anterior acabó en (10, 13.5).
    let g0_line = 20;
    assert_eq!(lines[g0_line as usize], "G0 X10 Y14");
    let before = resume::walker::modal_at(&lines, g0_line as usize, false).1;
    assert_eq!(before, Some([10.0, 13.5]));
    assert_eq!(first_burn_point(&lines, g0_line, None), [10.0, 14.0]);
    // En el corte mismo, y con un punto a mitad de corte, nada cambia.
    assert_eq!(first_burn_point(&lines, g0_line + 1, None), [10.0, 14.0]);
    assert_eq!(
        first_burn_point(&lines, g0_line + 1, Some([20.0, 14.0])),
        [20.0, 14.0]
    );
}

#[test]
fn a_board_reboot_in_the_middle_stops_the_job_and_keeps_the_checkpoint() {
    let mut b = Bench::new("reboot", "virtual:falcon2@4", DeviceKind::Grbl);
    let lines = hatch_job();
    let h = b.connect();
    b.start(&h, &lines);
    b.wait_burned(cut_length(&lines) * 0.4);
    // La placa se reinicia sola (final de carrera, tapa, brownout) con la conexión en pie.
    b.emu.inject(Trigger::Now, Fault::Reboot);
    b.sink
        .wait("trabajo con error", |s| s.jobs().contains(&JobState::Error));
    assert!(
        b.sink
            .job_messages()
            .iter()
            .any(|m| m.contains("la máquina se reinició a mitad del trabajo")),
        "{:?}",
        b.sink.job_messages()
    );
    // Se detuvo el envío: no se quemó mucho más, y el punto de control quedó para reanudar.
    let burned = b.emu.trace().total_length();
    assert!(burned < cut_length(&lines) * 0.9, "siguió enviando");
    b.sink.wait("posición sin confirmar", |s| {
        s.statuses()
            .last()
            .is_some_and(|x| x.position_unconfirmed == Some(true))
    });
    assert!(!h.is_homed());
    let loaded = b.store.load().expect("el punto de control se conserva");
    assert!(loaded.progress.is_some());
    // Y se puede continuar como tras un corte de luz.
    let view = continue_after_interruption(&mut b, &lines);
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
    let view = resume::view(&b.store, &loaded, &PlanOptions::default(), None).unwrap();
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
    assert!(resume::summarize(&b.store, &loaded, None).can_resume);
    let view = resume::view(&b.store, &loaded, &PlanOptions::default(), None).unwrap();
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
    b.wait_checkpoint();
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
    let info = resume::summarize(&b.store, &torn, None);
    assert!(info.can_resume && !info.progress_known);
    // Sin avance legible se propone volver a empezar y se avisa; se puede elegir el punto a mano.
    let view = resume::view(&b.store, &torn, &PlanOptions::default(), None).unwrap();
    assert_eq!(view.suggested.line, 0);
    assert!(!view.warnings.is_empty());
    // Un nuevo trabajo reemplaza lo anterior sin arrastrar el avance dañado.
    let h = b.connect();
    b.start(&h, &lines);
    b.wait_burned(b.emu.trace().total_length() + 20.0);
    thread::sleep(Duration::from_millis(100));
    b.wait_checkpoint();
    let fresh = b.store.load().unwrap();
    assert_ne!(fresh.meta.id, torn.meta.id);
}

// ---------------------------------------------------------------------------
// La app muere y la placa sigue sola con lo que tenía en su memoria
// ---------------------------------------------------------------------------

/// Cuadrado de 80 mm con lados largos y lentos: las diez líneas caben en la memoria de la placa.
fn slow_square() -> Vec<String> {
    "G90\nG21\nM4 S0\nG0 X100 Y100\nG1 X180 S500 F600\nG1 Y180\nG1 X100\nG1 Y100\nM5\nG0 X0 Y0"
        .lines()
        .map(str::to_string)
        .collect()
}

/// Espera a que el punto de control sepa que ya se entregaron `n` líneas.
fn wait_sent(b: &Bench, n: u32) {
    let end = Instant::now() + Duration::from_secs(30);
    while b
        .store
        .load()
        .and_then(|l| l.progress)
        .is_none_or(|p| p.sent < n)
    {
        assert!(
            Instant::now() < end,
            "el punto de control no vio las líneas entregadas"
        );
        thread::sleep(Duration::from_millis(5));
    }
    // Y que se escribió con la posición de la cabeza.
    thread::sleep(Duration::from_millis(100));
}

/// La app muere sin avisar: el cable se suelta (nada le llega a la placa, ni siquiera la parada
/// del cierre) y la placa sigue con lo que ya tenía en su búfer.
fn kill_app(b: &mut Bench) {
    b.emu.inject(Trigger::Now, Fault::Disconnect);
    let end = Instant::now() + Duration::from_secs(10);
    while b.emu.is_connected() {
        assert!(Instant::now() < end, "el cable no se soltó");
        thread::sleep(Duration::from_millis(5));
    }
    b.conn.take().unwrap().close();
    assert!(!b.emu.is_connected());
}

/// Espera a que la placa vacíe su memoria y quede en reposo.
pub(super) fn wait_machine_idle(b: &Bench) {
    let end = Instant::now() + Duration::from_secs(60);
    while !(b.emu.state() == tlaku_emu::StateKind::Idle && b.emu.planner_used() == 0) {
        assert!(Instant::now() < end, "la máquina no terminó sola");
        thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn a_job_that_fit_in_the_buffer_finishes_alone_and_the_resume_recommends_discarding() {
    let mut b = Bench::new("alone-done", "virtual:falcon2@4", DeviceKind::Grbl);
    let lines = slow_square();
    let h = b.connect();
    b.start(&h, &lines);
    b.wait_burned(30.0);
    wait_sent(&b, 10);
    kill_app(&mut b);
    // Sin la app, la máquina termina lo suyo y vuelve a (0, 0).
    wait_machine_idle(&b);
    assert!(b.emu.trace().total_length() > 300.0);
    let loaded = b.store.load().expect("quedó el punto de control");
    // Lo que se guardó: unos mm del primer lado, aunque casi todas las líneas tenían `ok`.
    let before = resume::summarize(&b.store, &loaded, None);
    assert!(before.percent < 20.0, "{}", before.percent);
    assert_eq!(before.machine, tlaku_doc::ResumeMachine::NotChecked);
    assert!(before.ahead_lines >= 5, "{before:?}");
    assert!(before.warning.unwrap().contains("pudo seguir ejecutando"));

    // Al reconectar se compara la posición de ahora con la guardada.
    let h = b.connect();
    let now = start::current_for(&h, &loaded).expect("en reposo");
    let view = resume::view(&b.store, &loaded, &PlanOptions::default(), Some(&now)).unwrap();
    assert_eq!(view.job.machine, tlaku_doc::ResumeMachine::ProbablyFinished);
    assert!(view.job.recommend_discard);
    assert!(view.job.percent > 99.0, "{}", view.job.percent);
    assert!(
        view.warnings[0].contains("parece haber terminado")
            && view.warnings[0].contains("Revisa la pieza"),
        "{:?}",
        view.warnings
    );
    let info = resume::summarize(&b.store, &loaded, Some(&now));
    assert!(info.recommend_discard && info.warning.is_some());
}

/// 150 filas de 5 mm (zig-zag, 750 mm de corte): más líneas que memoria tiene la placa.
pub(super) fn long_job() -> Vec<String> {
    let mut v: Vec<String> = ["G90", "G21", "M4 S0", "G0 X10 Y10"]
        .map(String::from)
        .to_vec();
    for i in 0..150 {
        let y = 10.0 + f64::from(i) * 0.5;
        let (from, to) = if i % 2 == 0 { (10, 15) } else { (15, 10) };
        v.push(format!("G0 X{from} Y{y}"));
        v.push(if i == 0 {
            format!("G1 X{to} S500 F1500")
        } else {
            format!("G1 X{to}")
        });
    }
    v.extend(["M5", "G0 X0 Y0"].map(String::from));
    v
}

#[test]
fn a_job_longer_than_the_buffer_is_relocated_with_the_position_of_now() {
    let mut b = Bench::new("alone-mid", "virtual:falcon2@4", DeviceKind::Grbl);
    let lines = long_job();
    let h = b.connect();
    b.start(&h, &lines);
    b.wait_burned(cut_length(&lines) * 0.2);
    kill_app(&mut b);
    wait_machine_idle(&b);
    let burned_alone = b.emu.trace().total_length();
    let loaded = b.store.load().expect("quedó el punto de control");
    let saved = resume::summarize(&b.store, &loaded, None);
    assert!(
        burned_alone < cut_length(&lines) * 0.95,
        "no debió terminar"
    );

    let h = b.connect();
    let now = start::current_for(&h, &loaded).expect("en reposo");
    let view = resume::view(&b.store, &loaded, &PlanOptions::default(), Some(&now)).unwrap();
    assert_eq!(
        view.job.machine,
        tlaku_doc::ResumeMachine::Moved,
        "{:?}",
        view.job
    );
    assert!(!view.job.recommend_discard);
    assert!(
        view.job.percent > saved.percent,
        "{} > {}",
        view.job.percent,
        saved.percent
    );
    assert!(
        view.warnings[0].contains("por su cuenta"),
        "{:?}",
        view.warnings
    );
    // El punto propuesto está donde quedó la cabeza, no donde estaba al guardarse el punto.
    let head = b.emu.mpos();
    let start_pt = view.suggested.at.unwrap_or_else(|| {
        resume::walker::modal_at(&lines, view.suggested.line as usize, false)
            .1
            .unwrap()
    });
    let d = (start_pt[0] - head[0]).hypot(start_pt[1] - head[1]);
    assert!(
        d < 6.0,
        "el punto está a {d} mm de la cabeza: {start_pt:?} vs {head:?}"
    );

    // Y se puede continuar desde ahí sin volver a pasar por lo que la máquina hizo sola.
    h.command("$H").unwrap();
    start::frame(&h, &b.store, &loaded, 6000.0).unwrap();
    b.sink.wait("encuadre", |s| {
        s.logs().iter().any(|(_, t)| t.contains("frame_done"))
    });
    // El homing y el encuadre no cambian lo que se vio al conectar: sigue siendo «siguió sola»
    // y el mismo punto (la MPos de después ya no es la de la máquina que terminó sola).
    assert!(h.position_altered());
    let after = resume::view(
        &b.store,
        &loaded,
        &PlanOptions::default(),
        start::current_for(&h, &loaded).as_ref(),
    )
    .unwrap();
    assert_eq!(after.job.machine, tlaku_doc::ResumeMachine::Moved);
    assert_eq!(after.suggested, view.suggested);
    let args = args(&loaded, &view);
    start::resume_now(&h, b.store.clone(), &loaded, &args(false, true, true)).unwrap();
    b.sink.wait("trabajo terminado", |s| {
        s.jobs().contains(&JobState::Finished)
    });
    assert_eq!(uncovered(&lines, &b.emu, 0.3), 0, "quedaron huecos");
    let overlap = b.emu.trace().total_length() - cut_length(&lines);
    assert!(overlap < 6.0, "solape de {overlap} mm");
}

#[test]
fn after_a_power_cut_the_board_at_zero_is_not_taken_for_a_finished_job() {
    // Sin homing la placa vuelve en reposo y sin alarma, como la Falcon2 de verdad.
    let mut b = Bench::new("alone-power", "virtual:grbl11@4", DeviceKind::Grbl);
    let lines = slow_square();
    let h = b.connect();
    b.start(&h, &lines);
    b.wait_burned(30.0);
    wait_sent(&b, 10);
    b.emu.inject(Trigger::Now, Fault::PowerLoss);
    b.sink.wait("conexión perdida", |s| {
        s.connections().iter().any(|c| !c.connected)
    });
    b.conn = None;
    let loaded = b.store.load().expect("quedó el punto de control");
    let h = b.connect();
    let now = start::current_for(&h, &loaded).expect("en reposo");
    assert_eq!(now.parser_feed, Some(0.0), "{now:?}");
    let info = resume::summarize(&b.store, &loaded, Some(&now));
    assert!(info.can_resume && !info.recommend_discard, "{info:?}");
    assert_ne!(info.machine, tlaku_doc::ResumeMachine::ProbablyFinished);
    // Se conserva el comportamiento de siempre, con el aviso.
    assert!(info.warning.unwrap().contains("pudo seguir ejecutando"));
    let view = resume::view(&b.store, &loaded, &PlanOptions::default(), Some(&now)).unwrap();
    assert!(view.suggested.line < 8, "{:?}", view.suggested);
}

#[test]
fn connecting_to_a_machine_still_running_says_so_instead_of_blaming_the_port() {
    let mut b = Bench::new("alone-run", "virtual:falcon2@4", DeviceKind::Grbl);
    let lines = slow_square();
    let h = b.connect();
    b.start(&h, &lines);
    b.wait_burned(20.0);
    kill_app(&mut b);
    // La placa sigue con su trabajo: `$I` no obtiene respuesta de reposo, `?` sí.
    let vm = b.emu.reconnect();
    let sink = Arc::new(Collect::default());
    let err = Connection::start_with(
        Box::new(vm),
        &b.port,
        sink,
        Timing {
            identify_timeout: Duration::from_millis(600),
            ..fast()
        },
        Some(b.emu.clone()),
    )
    .err()
    .expect("no debe conectar");
    assert!(
        err.contains("ejecutando un trabajo") && err.contains("estado Run"),
        "{err}"
    );
    assert!(!err.contains("puerto correcto"), "{err}");
    // Una placa muda sigue dando el mensaje de siempre.
    let (vm, emu) = virtual_port::build("virtual:falcon2@4").unwrap();
    emu.inject(
        Trigger::Now,
        Fault::Silence {
            duration: Duration::from_secs(30),
            drop_output: true,
        },
    );
    let err = Connection::start_with(
        Box::new(vm),
        "virtual:falcon2@4",
        Arc::new(Collect::default()),
        Timing {
            identify_timeout: Duration::from_millis(300),
            ..fast()
        },
        Some(emu),
    )
    .err()
    .expect("no debe conectar");
    assert!(err.contains("puerto correcto"), "{err}");
}

// ---------------------------------------------------------------------------
// Punto de control de otra sesión (la prueba de la Falcon2 Pro): la máquina se usó para otras
// cosas y volvió a (0, 0); homing y encuadre no deben tomarse por «terminó sola».
// ---------------------------------------------------------------------------

/// El trabajo de la prueba real, con las dos líneas de comentario de la cabecera: 10 líneas que
/// se envían en 12 del archivo. Cuadrado de 80 mm en 100..180 y vuelta al origen.
fn yesterday_job() -> Vec<String> {
    "; Tlaku\n; pieza\nG90\nG21\nM4 S0\nG0 X100 Y100\nG1 X180 S500 F600\nG1 Y180\nG1 X100\nG1 Y100\nM5\nG0 X0 Y0"
        .lines()
        .map(str::to_string)
        .collect()
}

/// Escribe en la carpeta del punto de control lo que dejó la app hace `age_ms`: ocho `ok`, la
/// última línea con `ok` es `G1 Y100` (línea 9 del archivo) y la cabeza iba en (167,725, 100),
/// 67,7 mm dentro del primer lado, con la memoria de la placa casi llena.
fn write_old_checkpoint(b: &Bench, lines: &[String], age_ms: u64) {
    use resume::store::{CheckpointProgress, VERSION, now_ms};
    let mut meta = b.plan(lines).meta;
    let then = now_ms() - age_ms;
    meta.started_at_ms = then - 60_000;
    b.store.begin(&meta, Some(lines)).unwrap();
    let head = Some([167.725, 100.0]);
    let progress = CheckpointProgress {
        version: VERSION,
        seq: 1,
        acked: 8,
        sent_line: Some(9),
        sent: 10,
        sent_to_line: Some(11),
        mpos: head,
        wpos: head,
        wco: Some([0.0, 0.0]),
        planner_free: Some(1),
        rx_free: Some(100),
        planner_cap: Some(15),
        rx_cap: Some(127),
        modal: tlaku_doc::ResumeModal {
            spindle: tlaku_doc::ResumeSpindle::M4,
            s: Some(500.0),
            f: Some(600.0),
            ..Default::default()
        },
        t_ms: then,
    };
    b.store.write_progress(&progress).unwrap();
}

fn wait_frame_done(b: &Bench, n: usize) {
    b.sink.wait("encuadre", move |s| {
        s.logs()
            .iter()
            .filter(|(_, t)| t.contains("frame_done"))
            .count()
            >= n
    });
}

#[test]
fn an_old_checkpoint_is_not_taken_for_a_finished_job_after_homing_and_framing() {
    const DAY: u64 = 24 * 3600 * 1000;
    let mut b = Bench::new("yesterday", "virtual:falcon2@4", DeviceKind::Grbl);
    let lines = yesterday_job();
    write_old_checkpoint(&b, &lines, DAY);

    // Hoy la máquina se usó para otras cosas: quedó en (0, 0) y con el avance de otro trabajo
    // (`F` distinto de cero), así que no parece una placa recién reiniciada.
    let h = b.connect();
    h.command("G1 X0 Y0 F1000").unwrap();
    b.conn.take().unwrap().close();
    assert!(b.emu.mpos()[0].abs() < 0.01 && b.emu.mpos()[1].abs() < 0.01);

    // Secuencia de la app: conectar → aviso → homing → encuadre → continuar.
    let h = b.connect();
    assert!(!h.position_altered());
    let loaded = b.store.load().expect("el punto de ayer sigue ahí");
    let now = start::current_for(&h, &loaded);
    assert!(now.is_none(), "un punto de ayer no se compara: {now:?}");
    let info = resume::summarize(&b.store, &loaded, now.as_ref());
    assert_eq!(
        info.machine,
        tlaku_doc::ResumeMachine::NotChecked,
        "{info:?}"
    );
    assert!(!info.recommend_discard, "{info:?}");
    assert!(
        info.percent > 15.0 && info.percent < 30.0,
        "{}",
        info.percent
    );
    let view0 = resume::view(&b.store, &loaded, &PlanOptions::default(), now.as_ref()).unwrap();
    // La pieza son los 80 mm de 100 a 180, no el viaje al origen.
    assert_eq!(view0.bounds, [[100.0, 100.0], [180.0, 180.0]]);
    // Se reanuda en el primer lado, ~1,5 mm antes de donde iba la cabeza.
    let at = view0.suggested.at.expect("a mitad del primer lado");
    assert!(
        (at[0] - 166.2).abs() < 0.5 && (at[1] - 100.0).abs() < 0.01,
        "{:?}",
        view0.suggested
    );
    assert!(!view0.suggested.mismatch);

    h.command("$H").unwrap();
    assert!(h.position_altered());
    // Encuadre: solo la pieza; la cabeza termina en la esquina de la pieza y no en (0, 0).
    let mark = b.emu.received_lines().len();
    start::frame(&h, &b.store, &loaded, 3000.0).unwrap();
    // Con el encuadre en curso, continuar dice que la máquina está ocupada: no pide confirmar
    // que «no se movió» (la máquina tiene homing y el mensaje no habla de eso).
    let ready = start::readiness(&h);
    assert!(
        !ready.settings_read && ready.state == tlaku_doc::MachineState::Run,
        "{ready:?}"
    );
    let make = args(&loaded, &view0);
    let e = start::resume_now(&h, b.store.clone(), &loaded, &make(true, true, true)).unwrap_err();
    assert!(e.contains("ocupada"), "{e}");
    // El estado va traducido como en la barra de estado, no el nombre interno («run»).
    assert!(e.contains("(En marcha)") && !e.contains("(run)"), "{e}");
    assert!(!e.contains("no tiene homing"), "{e}");
    wait_frame_done(&b, 1);
    let framed = b.gcode_since(mark);
    assert!(
        !framed.iter().any(|l| l.contains("X0Y0")),
        "el encuadre no debe ir al origen: {framed:?}"
    );
    let end = b.emu.mpos();
    assert!(
        (end[0] - 100.0).abs() < 0.5 && (end[1] - 100.0).abs() < 0.5,
        "{end:?}"
    );

    // Con la cabeza en la esquina de la pieza la sugerencia es la misma de antes.
    let now = start::current_for(&h, &loaded);
    assert!(now.is_none());
    let info = resume::summarize(&b.store, &loaded, now.as_ref());
    assert_eq!(info.machine, tlaku_doc::ResumeMachine::NotChecked);
    assert!(!info.recommend_discard);
    let view1 = resume::view(&b.store, &loaded, &PlanOptions::default(), now.as_ref()).unwrap();
    assert_eq!(view1.suggested, view0.suggested);
    assert_eq!(view1.bounds, view0.bounds);

    // Continuar con esa sugerencia.
    let sent_mark = b.emu.received_lines().len();
    start::resume_now(&h, b.store.clone(), &loaded, &make(false, true, true)).unwrap();
    b.sink.wait("trabajo terminado", |s| {
        s.jobs().contains(&JobState::Finished)
    });
    // El registro numera las líneas del archivo y nunca pasa del total.
    let log = b
        .sink
        .logs()
        .into_iter()
        .map(|(_, t)| t)
        .find(|t| t.contains("Continuando el trabajo desde la línea"))
        .expect("el registro dice desde dónde continúa");
    let nums: Vec<u32> = log
        .split(|c: char| !c.is_ascii_digit())
        .filter_map(|n| n.parse().ok())
        .collect();
    assert_eq!(nums.len(), 2, "{log}");
    assert_eq!(nums[1], 12, "{log}");
    assert_eq!(nums[0], view0.suggested.line + 1, "{log}");
    assert!(nums[0] <= nums[1], "{log}");

    // Empezó a grabar donde iba (166,2, 100) y grabó lo que faltaba, sin saltarse nada.
    let trace = b.emu.trace();
    let first = trace.segments().first().expect("grabó algo");
    assert!(
        (first.start[0] - 166.2).abs() < 0.6 && (first.start[1] - 100.0).abs() < 0.3,
        "{first:?}"
    );
    let left = 180.0 - 166.2 + 3.0 * 80.0;
    assert!(
        (trace.total_length() - left).abs() < 3.0,
        "grabó {} mm y faltaban {left}",
        trace.total_length()
    );
    let missing = uncovered_where(&lines, &b.emu, 0.3, |p| p[0] > 166.5 || p[1] > 100.5);
    assert_eq!(missing, 0, "quedaron huecos");
    assert!(b.emu.trace().burned_during_rapids().is_empty());
    let sent = b.gcode_since(sent_mark);
    assert_eq!(sent.first().map(String::as_str), Some("M5"), "{sent:?}");
    assert!(b.store.load().is_none());
}

#[test]
fn a_recent_checkpoint_is_not_compared_either_once_the_head_has_moved() {
    // Un punto de hace un minuto sí se compara al conectar, pero si antes de preguntar ya se
    // hizo homing, la MPos es la del homing y no la del trabajo.
    let mut b = Bench::new("moved-first", "virtual:falcon2@4", DeviceKind::Grbl);
    let lines = yesterday_job();
    write_old_checkpoint(&b, &lines, 60_000);
    let h = b.connect();
    h.command("G1 X0 Y0 F1000").unwrap();
    b.conn.take().unwrap().close();
    let h = b.connect();
    h.command("$H").unwrap();
    let loaded = b.store.load().unwrap();
    assert!(start::current_for(&h, &loaded).is_none());
    let info = resume::summarize(&b.store, &loaded, None);
    assert_eq!(info.machine, tlaku_doc::ResumeMachine::NotChecked);
    assert!(!info.recommend_discard);

    // En cambio, sin tocar la cabeza se compara (la máquina volvió a (0, 0), con `F` de otro
    // trabajo: parece haber terminado sola).
    b.conn.take().unwrap().close();
    let h = b.connect();
    assert!(!h.position_altered());
    let now = start::current_for(&h, &loaded);
    assert!(now.is_some());
    let info = resume::summarize(&b.store, &loaded, now.as_ref());
    assert_eq!(
        info.machine,
        tlaku_doc::ResumeMachine::ProbablyFinished,
        "{info:?}"
    );
    // Y esa decisión no cambia por hacer homing después.
    h.command("$H").unwrap();
    assert_eq!(start::current_for(&h, &loaded), now);
}
