//! Constructores, validación y arranque del trabajo (`run`, `spawn`, `execute`).

use std::collections::VecDeque;
use std::io;
use std::sync::mpsc::{self};
use std::thread::{self};
use std::time::Instant;

use tlaku_doc::DeviceProfile;

use crate::link::{LinkFamily, LinkProfile, numbered_line};
use crate::serial::{LineReader, Transport};

use super::*;

impl<T: Transport> Streamer<T> {
    /// Streamer GRBL (conteo de caracteres con `config.rx_buffer`, `?`, `!`, `~`, `0x18`).
    pub fn new(transport: T, config: StreamerConfig) -> Self {
        let link = LinkProfile::grbl_with_buffer(config.rx_buffer);
        Self::with_link(transport, config, link)
    }

    /// Streamer para el enlace dado (GRBL, Marlin o Smoothieware). El búfer sale de
    /// `link.flow`; `config.rx_buffer` se ignora. Para Marlin y Smoothie conviene
    /// [`StreamerConfig::for_link`].
    pub fn with_link(transport: T, config: StreamerConfig, link: LinkProfile) -> Self {
        let (cmd_tx, cmd_rx) = mpsc::channel();
        let mut reader = LineReader::default();
        // Una respuesta pegada a otra cosa (`<Run|...|Ov:100,100,10ok`) solo se da en GRBL.
        reader.set_split_glued(link.family == LinkFamily::Grbl);
        Self {
            transport,
            config,
            link,
            cmd_tx,
            cmd_rx,
            reader,
            stop: None,
            abort: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Streamer con el enlace y la configuración que corresponden al perfil de máquina
    /// (`kind`, `dialect` y `rx_buffer`).
    pub fn for_device(transport: T, device: &DeviceProfile) -> Self {
        Self::with_link(
            transport,
            StreamerConfig::for_device(device),
            LinkProfile::for_device(device),
        )
    }

    /// Enlace en uso.
    pub fn link(&self) -> &LinkProfile {
        &self.link
    }

    /// Cómo se detuvo la máquina en el último trabajo, si hubo parada (cancelación, error con
    /// política `Stop`, falla de Creality o reset).
    pub fn last_stop(&self) -> Option<&StopInfo> {
        self.stop.as_ref()
    }

    pub fn control(&self) -> Control {
        Control {
            tx: self.cmd_tx.clone(),
            abort: self.abort.clone(),
        }
    }

    pub fn into_transport(self) -> T {
        self.transport
    }

    /// Envía un comando en tiempo real fuera de un trabajo.
    pub fn send_realtime(&mut self, cmd: Realtime) -> io::Result<()> {
        self.transport.write_all(&[cmd.byte()])
    }

    pub(super) fn prepare<S: AsRef<str>>(&self, lines: &[S]) -> Result<Vec<Prepared>, StreamError> {
        let max = self.link.flow.max_line_len();
        let mut job = Vec::new();
        // Con líneas numeradas, la primera fija el contador (`N0 M110 N0*cs`): así `N<n>` es el
        // índice de la línea en `job` y un `Resend: n` apunta a ella sin cuentas.
        if self.link.line_numbering.is_some() {
            job.push(Prepared {
                text: "M110 N0".into(),
                source_line: 0,
                sync: false,
                silent: true,
            });
        }
        for (i, raw) in lines.iter().enumerate() {
            let source_line = i + 1;
            let text = prepare_line_with(
                raw.as_ref(),
                self.link.strip_spaces,
                self.link.reject_realtime_chars,
            )
            .map_err(|ch| StreamError::InvalidChar { source_line, ch })?;
            if text.is_empty() {
                continue;
            }
            let len = self.wire_text_for(job.len(), &text).len() + 1;
            if len > max {
                return Err(StreamError::LineTooLong {
                    source_line,
                    len,
                    max,
                });
            }
            let sync = self.link.eeprom_sync && writes_eeprom(&text);
            job.push(Prepared {
                text,
                source_line,
                sync,
                silent: false,
            });
        }
        Ok(job)
    }

    /// Lo que viaja por el cable (sin `\n`) para la línea `index` del trabajo.
    pub(super) fn wire_text_for(&self, index: usize, text: &str) -> String {
        match self.link.line_numbering {
            Some(n) => numbered_line(index, text, n),
            None => text.to_string(),
        }
    }

    /// Ejecuta el trabajo en el hilo actual hasta que termine, informando cada
    /// evento a `on_event`. Las líneas se validan antes de enviar nada.
    pub fn run<S: AsRef<str>>(
        &mut self,
        lines: &[S],
        mut on_event: impl FnMut(Event),
    ) -> Result<Outcome, StreamError> {
        let job = self.prepare(lines)?;
        Ok(self.execute(&job, &mut on_event))
    }

    /// Como [`run`](Self::run), pero en un hilo propio; los eventos llegan por
    /// `Job::events` y el mando es `Job::control`.
    pub fn spawn<S: AsRef<str>>(self, lines: &[S]) -> Result<Job<T>, StreamError>
    where
        T: 'static,
    {
        let job = self.prepare(lines)?;
        let control = self.control();
        let (tx, events) = mpsc::channel();
        let handle = thread::spawn(move || {
            let mut streamer = self;
            let outcome = streamer.execute(&job, &mut |e| {
                let _ = tx.send(e);
            });
            (streamer, outcome)
        });
        Ok(Job {
            control,
            events,
            handle,
        })
    }

    fn execute(&mut self, job: &[Prepared], emit: &mut dyn FnMut(Event)) -> Outcome {
        while self.cmd_rx.try_recv().is_ok() {}
        self.reader.clear();
        self.stop = None;
        self.abort.store(false, Ordering::SeqCst);
        self.transport.set_abort(self.abort.clone());
        let mut st = RunState {
            next: 0,
            acked: 0,
            errors: 0,
            used: 0,
            pending: VecDeque::new(),
            paused: false,
            idle_seen: false,
            last_poll: None,
            last_rx: Instant::now(),
            silence_reported: None,
            last_state: None,
            hidden: job.iter().take_while(|p| p.silent).count(),
            sync_sent: false,
            resend_until: None,
            resends: 0,
        };
        let outcome = if job.len() == st.hidden {
            Outcome::Completed { errors: 0 }
        } else {
            loop {
                match self.tick(job, &mut st, emit) {
                    Ok(Some(outcome)) => break outcome,
                    Ok(None) => {}
                    // Una escritura abandonada porque el usuario canceló o cerró la conexión
                    // mientras la placa no leía (ver [`Transport::set_abort`]): no es una
                    // desconexión. Se intenta la parada de siempre (feed hold y reset); si el
                    // puerto sigue sin avanzar, sus escrituras también se abandonan enseguida.
                    Err(e)
                        if e.kind() == io::ErrorKind::ConnectionAborted
                            && self.abort.load(Ordering::SeqCst) =>
                    {
                        let _ = self.stop_machine(&mut st, emit);
                        break Outcome::Cancelled;
                    }
                    Err(e) => break Outcome::Disconnected(e.to_string()),
                }
            }
        };
        // Un aviso de silencio abierto se retira al terminar, sea cual sea el final.
        if st.silence_reported.take().is_some() {
            emit(Event::Silence { ms: 0 });
        }
        emit(Event::Finished(outcome.clone()));
        outcome
    }
}
