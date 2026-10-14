//! Solicitudes de la interfaz: saltos, encuadre, detener, información y archivos.

use super::*;
use tlaku_i18n::tr;

impl<L: PacketLink> Worker<L> {
    pub(super) fn handle(&mut self, req: Request) {
        match req {
            Request::Upload(up, reply) => {
                let r = self.upload(*up);
                let _ = reply.send(r);
            }
            Request::Frame {
                corners,
                speed_mm_min,
                reply,
            } => {
                let r = self.start_frame(corners, speed_mm_min);
                let _ = reply.send(r);
            }
            Request::Jog {
                dx,
                dy,
                speed_mm_min,
                reply,
            } => {
                let r = self.jog(dx, dy, speed_mm_min);
                let _ = reply.send(r);
            }
            Request::JogTo {
                x,
                y,
                speed_mm_min,
                reply,
            } => {
                let r = self.jog_to(x, y, speed_mm_min);
                let _ = reply.send(r);
            }
            Request::Pause(reply) => {
                let r = self.session.pause().map_err(op_error);
                if r.is_ok()
                    && let Some(j) = self.job.as_mut()
                    && !j.paused
                {
                    j.paused = true;
                    self.job_event(JobState::Paused, &tr!("desktop.ruida.job_paused"));
                }
                let _ = reply.send(r);
            }
            Request::Resume(reply) => {
                let r = self.session.resume().map_err(op_error);
                if r.is_ok()
                    && let Some(j) = self.job.as_mut()
                    && j.paused
                {
                    j.paused = false;
                    self.job_event(JobState::Started, &tr!("desktop.lhymicro.job_resumed"));
                }
                let _ = reply.send(r);
            }
            Request::Stop(reply) => {
                let r = self.stop();
                let _ = reply.send(r);
            }
            Request::Info(reply) => {
                let r = self.info();
                let _ = reply.send(r.map(Box::new));
            }
            Request::Files(reply) => {
                let r = self.files();
                let _ = reply.send(r);
            }
            Request::SettingsRead(reply) => {
                let r = self.settings_read();
                let _ = reply.send(r);
            }
            Request::SettingsPreview(changes, reply) => {
                let r = self.settings_preview(&changes);
                let _ = reply.send(r);
            }
            Request::SettingsPreviewFile(json, reply) => {
                let r = self.settings_preview_file(&json);
                let _ = reply.send(r);
            }
            Request::SettingsApply {
                changes,
                confirmed,
                dir,
                reply,
            } => {
                let r = self.settings_apply(&changes, confirmed, &dir);
                let _ = reply.send(r);
            }
            Request::SettingsRestore {
                json,
                confirmed,
                dir,
                reply,
            } => {
                let r = self.settings_restore(&json, confirmed, &dir);
                let _ = reply.send(r);
            }
            Request::SettingsExport(reply) => {
                let r = self.settings_export();
                let _ = reply.send(r);
            }
            Request::Calibrate {
                key,
                commanded_mm,
                measured_mm,
                reply,
            } => {
                let r = self.calibrate(&key, commanded_mm, measured_mm);
                let _ = reply.send(r);
            }
            Request::FileDelete {
                index,
                name,
                confirmed,
                reply,
            } => {
                let r = self.file_delete(index, &name, confirmed);
                let _ = reply.send(r);
            }
            Request::FileRun {
                index,
                name,
                confirmed,
                reply,
            } => {
                let r = self.file_run(index, &name, confirmed);
                let _ = reply.send(r);
            }
            Request::Disconnect => unreachable!("se atiende en el ciclo principal"),
        }
    }

    pub(super) fn busy_error(&self) -> Option<String> {
        if self.job.is_some() {
            Some(tr!("desktop.ruida.busy_job"))
        } else if self.frame.is_some() {
            Some(tr!("desktop.ruida.busy_frame"))
        } else {
            None
        }
    }

    pub(super) fn set_jog_speed(&mut self, speed_mm_min: f64) {
        if speed_mm_min.is_finite() && speed_mm_min > 0.0 {
            self.session.config_mut().jog_speed_mm_s = speed_mm_min / 60.0;
        }
    }

    pub(super) fn jog(&mut self, dx: f64, dy: f64, speed_mm_min: f64) -> Result<(), String> {
        if let Some(e) = self.busy_error() {
            return Err(e);
        }
        if !(dx.is_finite() && dy.is_finite()) {
            return Err(tr!("desktop.common.offset_invalid"));
        }
        self.set_jog_speed(speed_mm_min);
        let st = self.session.status().map_err(op_error)?;
        let (x, y) = (
            st.pos_um.0 as f64 / 1000.0 + dx,
            st.pos_um.1 as f64 / 1000.0 + dy,
        );
        self.session.jog_machine(x, y).map_err(op_error)
    }

    pub(super) fn jog_to(&mut self, x: f64, y: f64, speed_mm_min: f64) -> Result<(), String> {
        if let Some(e) = self.busy_error() {
            return Err(e);
        }
        if !(x.is_finite() && y.is_finite()) {
            return Err(tr!("desktop.common.position_invalid"));
        }
        self.set_jog_speed(speed_mm_min);
        self.session.jog_machine(x, y).map_err(op_error)
    }

    pub(super) fn start_frame(
        &mut self,
        corners: Vec<(f64, f64)>,
        speed_mm_min: f64,
    ) -> Result<(), String> {
        if let Some(e) = self.busy_error() {
            return Err(e);
        }
        let mut queue: VecDeque<(f64, f64)> = corners.into();
        let Some((x, y)) = queue.pop_front() else {
            return Err(tr!("desktop.common.nothing_to_frame"));
        };
        self.set_jog_speed(speed_mm_min);
        // La primera esquina comprueba que la máquina está en reposo y dentro de la cama.
        self.session.jog_machine(x, y).map_err(op_error)?;
        self.log(LogDir::Out, &tr!("desktop.common.log_frame_start"));
        self.frame = Some(FrameRun { corners: queue });
        Ok(())
    }

    pub(super) fn stop(&mut self) -> Result<(), String> {
        self.frame = None;
        let r = self.session.stop().map_err(op_error);
        // La subida (si la había) ya terminó: la orden de cancelar quedó atendida.
        self.shared.cancel.store(false, Ordering::SeqCst);
        if r.is_ok() {
            if let Some(sim) = self.sim.as_mut() {
                sim.remaining = None;
            }
            if self.job.take().is_some() {
                self.job_event(JobState::Cancelled, &tr!("desktop.ruida.job_stopped"));
            } else {
                self.log(LogDir::Out, &tr!("desktop.ruida.log_stop"));
            }
        }
        r
    }

    pub(super) fn info(&mut self) -> Result<RuidaInfo, String> {
        if self.job.is_some() {
            return Err(tr!("desktop.ruida.wait_for_job"));
        }
        self.log(LogDir::Out, &tr!("desktop.ruida.log_info"));
        self.session
            .controller_info()
            .map(to_info)
            .map_err(op_error)
    }

    pub(super) fn files(&mut self) -> Result<Vec<RuidaFile>, String> {
        if self.job.is_some() {
            return Err(tr!("desktop.ruida.wait_for_job"));
        }
        self.log(LogDir::Out, &tr!("desktop.ruida.log_files"));
        self.session
            .list_documents()
            .map(|v| {
                v.into_iter()
                    .map(|f| RuidaFile {
                        index: f.index,
                        name: f.name,
                    })
                    .collect()
            })
            .map_err(op_error)
    }
}
