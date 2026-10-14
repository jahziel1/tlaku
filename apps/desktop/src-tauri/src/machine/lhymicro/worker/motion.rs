//! Home, saltos, encuadre y parada en reposo.

use super::*;

impl Worker {
    pub(super) fn jog(&mut self, dx: f64, dy: f64) -> Result<(), String> {
        if !(dx.is_finite() && dy.is_finite()) {
            return Err("El desplazamiento no es válido.".into());
        }
        let Some([x, y]) = self.machine_pos() else {
            return Err(UNKNOWN_POSITION.into());
        };
        self.jog_to(x + dx, y + dy)
    }

    /// Salto absoluto a un punto en mm del sistema de la máquina.
    pub(super) fn jog_to(&mut self, x: f64, y: f64) -> Result<(), String> {
        if !(x.is_finite() && y.is_finite()) {
            return Err("La posición no es válida.".into());
        }
        let target = self.home.from_machine_mm((x, y), self.bed);
        self.publish(MachineState::Jog);
        let r = self.session.jog_to(target);
        self.publish(MachineState::Idle);
        if r.is_err() && self.machine_pos().is_none() {
            self.log(
                LogDir::In,
                "; AVISO: el salto falló y la posición del cabezal quedó desconocida: haz home",
            );
        }
        r.map_err(op_error)
    }

    pub(super) fn frame(&mut self, corners: Vec<(f64, f64)>, reply: Reply<()>) {
        self.shared.cancel.store(false, Ordering::SeqCst);
        if self.machine_pos().is_none() {
            self.finish(reply, Err(UNKNOWN_POSITION.into()));
            return;
        }
        if corners.is_empty() {
            self.finish(
                reply,
                Err("No hay nada que encuadrar: el trabajo está vacío.".into()),
            );
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
                Err(format!(
                    "El encuadre pasa por ({x:.2}, {y:.2}) mm, que queda fuera de la cama."
                )),
            );
            return;
        }
        self.log(
            LogDir::Out,
            "; encuadre: recorrido del rectángulo con el láser apagado",
        );
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
                    &format!("; encuadre interrumpido: {}", op_error(e)),
                );
                aborted = true;
                break;
            }
        }
        if self.cancelled() {
            self.log(LogDir::In, "; encuadre detenido por el usuario");
            let _ = self.stop_idle();
        } else if aborted {
            self.publish(MachineState::Idle);
        } else {
            self.publish(MachineState::Idle);
            self.log(LogDir::In, "; encuadre terminado");
        }
        self.shared.cancel.store(false, Ordering::SeqCst);
    }

    /// Detiene la placa sin trabajo en marcha (`I` e `IUS1P`). La posición queda desconocida.
    pub(super) fn stop_idle(&mut self) -> Result<(), String> {
        let outcome = self.session.stop();
        self.publish(MachineState::Idle);
        match outcome {
            StopOutcome::Acked => {
                self.log(
                    LogDir::Out,
                    "; detener (I, IUS1P): la placa lo confirmó y el láser quedó apagado. La \
                     posición del cabezal se perdió: haz home antes de seguir",
                );
                Ok(())
            }
            StopOutcome::Unconfirmed => {
                self.log(
                    LogDir::In,
                    "; AVISO: la placa no confirmó detenerse ni apagar el láser: usa el paro de \
                     emergencia o abre la tapa",
                );
                Err(format!(
                    "{UNCONFIRMED_MARK} de que la placa se detuviera ni de que el láser se \
                     apagara. Usa el paro de emergencia o abre la tapa."
                ))
            }
        }
    }
}
