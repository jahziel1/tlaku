//! Inicio de un trabajo o encuadre: comprobaciones previas y entrega del puerto al `Streamer`.

use super::*;
use tlaku_i18n::tr;

impl Worker {
    pub(super) fn try_start(
        &mut self,
        mut t: Box<dyn Transport>,
        lines: Vec<String>,
        link: LinkProfile,
        s_max: Option<f64>,
        kind: RunKind,
        load: Option<LinkLoad>,
    ) -> Result<Run, StartFail> {
        // El perfil de la máquina debe corresponder a la controladora conectada.
        if link.family != self.link_family {
            let msg = tr!(
                "desktop.worker.profile_mismatch",
                profile = link.family_name(),
                connected = match self.link_family {
                    LinkFamily::Grbl => "GRBL",
                    LinkFamily::Marlin => "Marlin",
                    LinkFamily::Smoothie => "Smoothieware",
                }
            );
            return Err(StartFail::Refused(t, msg));
        }
        // Con el `EMERGENCY_PARSER` informado, la pausa de Marlin también manda `M0` y `M108`.
        let link = if link.family == LinkFamily::Marlin {
            link.with_emergency_parser(self.emergency_parser)
        } else {
            link
        };
        // Mismo orden que `tlaku send`: validar, comprobar el estado y solo entonces enviar.
        if let Err(e) = validate_lines(&lines, &link) {
            return Err(StartFail::Refused(t, e));
        }
        self.reader.clear();
        let mut shown = Vec::new();
        // El encuadre va con el láser apagado (M5, S0): no necesita el modo láser.
        let expect = JobExpect {
            require_laser_mode: kind == RunKind::Job,
            s_max,
            check_parameters: true,
        };
        // El aviso de enlace saturado (solo en trabajos) no bloquea el envío.
        let checked = preflight_link_with_load(
            &mut t,
            &self.timing.preflight,
            Advice::Console,
            &expect,
            &link,
            load.as_ref().filter(|_| kind == RunKind::Job),
            &mut shown,
        );
        // Los avisos ("Aviso: $30...") no bloquean, pero se dejan en el registro.
        for w in String::from_utf8_lossy(&shown)
            .lines()
            .filter(|l| tlaku_grbl::preflight::is_warning_line(l))
        {
            self.log(LogDir::In, w);
        }
        if let Err(msg) = checked {
            self.log(LogDir::In, &first_line(&msg));
            // Un fallo de E/S en la consulta no se distingue aquí de una máquina muda: el
            // sondeo normal detectará la pérdida si es real.
            return Err(StartFail::Refused(t, msg));
        }
        self.last_poll = None;
        let total = lines.len();
        let config = StreamerConfig {
            silence_timeout: self.timing.silence_timeout,
            home_timeout: self.timing.home_timeout,
            hold_timeout: self.timing.hold_timeout,
            reset_wait: self.timing.reset_wait,
            ..StreamerConfig::for_link(&link)
        };
        match Streamer::with_link(t, config, link).spawn(&lines) {
            Ok(job) => {
                match kind {
                    RunKind::Job => {
                        self.log(
                            LogDir::Out,
                            &tr!("desktop.worker.log_job_lines", total = total),
                        );
                        self.job_event(
                            JobState::Started,
                            &tr!("desktop.worker.job_sending", total = total),
                        );
                        self.sink.emit(Emit::Progress(ProgressEvent {
                            acked: 0,
                            total: total.try_into().unwrap_or(u32::MAX),
                        }));
                    }
                    RunKind::Frame => self.log(LogDir::Out, &tr!("desktop.common.log_frame_start")),
                }
                self.set_reported_state(MachineState::Run);
                Ok(Run {
                    job,
                    kind,
                    last_progress: None,
                    cancelling: false,
                })
            }
            // Las líneas ya se validaron: no debería pasar, pero el puerto se habría perdido.
            Err(e) => Err(StartFail::Lost(e.to_string())),
        }
    }
}
