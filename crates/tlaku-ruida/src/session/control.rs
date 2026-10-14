//! Control del proceso (pausa, reanudar, parar, home), saltos y encuadre.

use super::*;
use crate::cmd::{Command, ProcessOp, RapidRef};
use crate::link::PacketLink;
use crate::num;
use crate::safety;
use std::time::{Duration, Instant};
use tlaku_core::Point;

impl<L: PacketLink> RuidaSession<L> {
    // ------------------------------------------------------------------------------------
    // Control del proceso

    /// Pausa el trabajo (`D8 02`).
    pub fn pause(&mut self) -> Result<(), SessionError> {
        self.command(&[Command::Process(ProcessOp::Pause)], self.cfg.ack_timeout)
    }

    /// Reanuda el trabajo (`D8 03`).
    pub fn resume(&mut self) -> Result<(), SessionError> {
        self.command(&[Command::Process(ProcessOp::Resume)], self.cfg.ack_timeout)
    }

    /// Detiene el trabajo (`D8 01`).
    pub fn stop(&mut self) -> Result<(), SessionError> {
        self.command(&[Command::Process(ProcessOp::Stop)], self.cfg.ack_timeout)
    }

    /// Lleva el cabezal al home (`D8 2A`) y espera a que termine.
    pub fn home(&mut self) -> Result<(), SessionError> {
        self.ensure_idle()?;
        self.command(
            &[Command::Process(ProcessOp::HomeXY)],
            self.cfg.long_timeout,
        )?;
        self.wait_idle(self.cfg.long_timeout)
    }

    pub(super) fn ensure_idle(&mut self) -> Result<(), SessionError> {
        if self.cfg.require_idle {
            let st = self.status()?;
            if !st.is_idle() {
                return Err(SessionError::Busy(st));
            }
        }
        Ok(())
    }

    /// Espera hasta que la máquina esté en reposo (consulta el estado cada 100 ms).
    pub fn wait_idle(&mut self, timeout: Duration) -> Result<(), SessionError> {
        let deadline = Instant::now() + timeout;
        loop {
            if self.status()?.is_idle() {
                return Ok(());
            }
            if Instant::now() >= deadline {
                return Err(SessionError::Timeout {
                    what: "fin del movimiento",
                });
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }

    // ------------------------------------------------------------------------------------
    // Saltos y encuadre (siempre con el láser apagado: opciones `D9` 00 y 02)

    /// Lleva el cabezal a un punto (mm, sistema de Tlaku) con el láser apagado.
    pub fn jog_to(&mut self, target: Point) -> Result<(), SessionError> {
        self.ensure_idle()?;
        self.jog_unchecked(target)
    }

    fn jog_unchecked(&mut self, target: Point) -> Result<(), SessionError> {
        let bed = self.bed_mm();
        let (mx, my) = self.cfg.home.to_machine(target, bed);
        self.jog_machine_unchecked(mx, my)
    }

    /// Lleva el cabezal a (`x`, `y`) mm del sistema de la máquina (distancias desde el home hacia
    /// dentro de la cama, las que informa [`MachineStatus::pos_um`]) con el láser apagado.
    pub fn jog_machine(&mut self, x: f64, y: f64) -> Result<(), SessionError> {
        self.ensure_idle()?;
        self.jog_machine_unchecked(x, y)
    }

    fn jog_machine_unchecked(&mut self, mx: f64, my: f64) -> Result<(), SessionError> {
        let bed = self.bed_mm();
        let (x, y) = (
            num::mm_to_um(mx).map_err(num_err)?,
            num::mm_to_um(my).map_err(num_err)?,
        );
        let speed = num::speed_from_mm_min(self.cfg.jog_speed_mm_s * 60.0).map_err(num_err)?;
        let cmds = [
            Command::Speed(speed),
            Command::RapidXY {
                rel: RapidRef::Machine,
                x,
                y,
            },
        ];
        let mut limits = self.cfg.limits;
        limits.bed_mm = bed;
        limits.max_speed_mm_min = limits.max_speed_mm_min.max(self.cfg.jog_speed_mm_s * 60.0);
        safety::check_commands(&cmds, &limits, false)?;
        self.command(&cmds, self.cfg.ack_timeout)
    }

    /// Desplaza el cabezal `dx`, `dy` mm (ejes de Tlaku) desde donde está.
    pub fn jog_by(&mut self, dx: f64, dy: f64) -> Result<(), SessionError> {
        self.ensure_idle()?;
        let st = self.status()?;
        let target = Point::new(st.pos_mm.x + dx, st.pos_mm.y + dy);
        self.jog_unchecked(target)
    }

    /// Recorre las cuatro esquinas del rectángulo (mm) con el láser apagado, esperando a que el
    /// cabezal llegue a cada una. No hay un comando de encuadre documentado (`A5 53 00` del
    /// panel remoto no se conoce): se hace con saltos.
    pub fn frame(&mut self, min: Point, max: Point) -> Result<(), SessionError> {
        self.ensure_idle()?;
        let corners = [
            Point::new(min.x, min.y),
            Point::new(max.x, min.y),
            Point::new(max.x, max.y),
            Point::new(min.x, max.y),
            Point::new(min.x, min.y),
        ];
        for c in corners {
            self.jog_unchecked(c)?;
            self.wait_idle(self.cfg.long_timeout)?;
        }
        Ok(())
    }
}
