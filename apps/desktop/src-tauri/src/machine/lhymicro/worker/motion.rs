//! Home, saltos, encuadre y parada en reposo.

use super::*;
use tlaku_i18n::tr;

impl Worker {
    pub(super) fn jog(&mut self, dx: f64, dy: f64) -> Result<(), String> {
        if !(dx.is_finite() && dy.is_finite()) {
            return Err(tr!("desktop.common.offset_invalid"));
        }
        let Some([x, y]) = self.machine_pos() else {
            return Err(unknown_position());
        };
        self.jog_to(x + dx, y + dy)
    }

    /// Salto absoluto a un punto en mm del sistema de la máquina.
    pub(super) fn jog_to(&mut self, x: f64, y: f64) -> Result<(), String> {
        if !(x.is_finite() && y.is_finite()) {
            return Err(tr!("desktop.common.position_invalid"));
        }
        let target = self.home.from_machine_mm((x, y), self.bed);
        self.publish(MachineState::Jog);
        let r = self.session.jog_to(target);
        self.publish(MachineState::Idle);
        if r.is_err() && self.machine_pos().is_none() {
            self.log(LogDir::In, &tr!("desktop.lhymicro.log_jog_failed"));
        }
        r.map_err(op_error)
    }

    pub(super) fn frame(&mut self, corners: Vec<(f64, f64)>, reply: Reply<()>) {
        self.shared.cancel.store(false, Ordering::SeqCst);
        if self.machine_pos().is_none() {
            self.finish(reply, Err(unknown_position()));
            return;
        }
        if corners.is_empty() {
            self.finish(reply, Err(tr!("desktop.common.nothing_to_frame")));
            return;
        }
        // Se valida todo el recorrido antes de mover nada.
        let eps = 0.05;
        if let Some(&(x, y)) = corners.iter().find(|&&(x, y)| {
            !(x.is_finite() && y.is_finite())
                || x < -eps
                || y < -eps
                || x > self.bed[0] + eps
                || y > self.bed[1] + eps
        }) {
            self.finish(
                reply,
                Err(tr!(
                    "desktop.lhymicro.frame_outside_bed",
                    x = format!("{x:.2}"),
                    y = format!("{y:.2}")
                )),
            );
            return;
        }
        self.log(LogDir::Out, &tr!("desktop.common.log_frame_start"));
        let _ = reply.send(Ok(()));
        self.publish(MachineState::Run);
        let mut aborted = false;
        for &(x, y) in &corners {
            if self.cancelled() {
                aborted = true;
                break;
            }
            let target = self.home.from_machine_mm((x, y), self.bed);
            if let Err(e) = self.session.jog_to(target) {
                self.log(
                    LogDir::In,
                    &tr!("desktop.lhymicro.log_frame_interrupted", err = op_error(e)),
                );
                aborted = true;
                break;
            }
        }
        if self.cancelled() {
            self.log(LogDir::In, &tr!("desktop.lhymicro.log_frame_stopped"));
            let _ = self.stop_idle();
        } else if aborted {
            self.publish(MachineState::Idle);
        } else {
            self.publish(MachineState::Idle);
            self.log(LogDir::In, &frame_done_log());
        }
        self.shared.cancel.store(false, Ordering::SeqCst);
    }

    /// Detiene la placa sin trabajo en marcha (`I` e `IUS1P`). La posición queda desconocida.
    pub(super) fn stop_idle(&mut self) -> Result<(), String> {
        let outcome = self.session.stop();
        self.publish(MachineState::Idle);
        match outcome {
            StopOutcome::Acked => {
                self.log(LogDir::Out, &tr!("desktop.lhymicro.log_stop_acked"));
                Ok(())
            }
            StopOutcome::Unconfirmed => {
                self.log(LogDir::In, &tr!("desktop.lhymicro.log_stop_unconfirmed"));
                Err(tlaku_i18n::coded(
                    STOP_UNCONFIRMED,
                    tr!("desktop.lhymicro.stop_unconfirmed"),
                ))
            }
        }
    }
}
