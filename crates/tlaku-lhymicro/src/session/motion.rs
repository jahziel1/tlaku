//! Saltos, encuadre y home.

use super::{JobControl, JobState, LhySession, SessionError, secs};
use crate::encode::{RAPID_ESTIMATE_MM_S, mil_to_mm, mm_to_mil, rapid_text};
use crate::packet::frame_stream;
use crate::status::Status;
use crate::transport::Transport;
use tlaku_core::Point;

impl<T: Transport> LhySession<T> {
    // ------------------------------------------------------------------------------------
    // Saltos y home

    fn require_idle(&mut self) -> Result<(), SessionError> {
        if self.control.state() != JobState::Idle {
            return Err(SessionError::JobActive);
        }
        let st = self.status()?;
        match st.status {
            Status::LowPower => Err(SessionError::LowPower),
            s @ (Status::NoReply | Status::Unknown(_)) => Err(SessionError::NoStatus(s)),
            s if s.accepts_packets() || s == Status::CrcError => Ok(()),
            s => Err(SessionError::Busy(s)),
        }
    }

    /// Manda un flujo corto de saltos (sin `-`) y confirma cada paquete.
    fn send_short(&mut self, text: &str, ctl: &JobControl) -> Result<(), SessionError> {
        self.last_confirmed = None;
        let frames = frame_stream(text.as_bytes())?;
        for (i, f) in frames.iter().enumerate() {
            self.send_frame(f, i, ctl)?;
        }
        Ok(())
    }

    /// Lleva el cabezal al home (finales de carrera) y fija la posición en (0, 0).
    pub fn home(&mut self) -> Result<(), SessionError> {
        self.require_idle()?;
        let ctl = JobControl::new();
        self.send_short("IPP\n", &ctl)?;
        // El cabezal puede estar moviéndose: hasta que la placa vacíe su búfer o venza el tiempo.
        self.wait_empty(&ctl, self.cfg.home_timeout, false)?;
        self.pos = Some((0, 0));
        Ok(())
    }

    /// Salto con el láser apagado a una posición en mils de la placa.
    fn rapid_to_mil(&mut self, target: (i64, i64)) -> Result<(), SessionError> {
        let from = self.pos.ok_or(SessionError::UnknownPosition)?;
        let (dx, dy) = (target.0 - from.0, target.1 - from.1);
        if dx == 0 && dy == 0 {
            return Ok(());
        }
        let ctl = JobControl::new();
        let text = rapid_text(dx, dy, self.cfg.autolock);
        if let Err(e) = self.send_short(&text, &ctl) {
            // No se sabe si el salto llegó a ejecutarse.
            self.pos = None;
            return Err(e);
        }
        let dist_mm = mil_to_mm(dx.abs().max(dy.abs()));
        let limit = secs(
            dist_mm / RAPID_ESTIMATE_MM_S * 1.5 + self.cfg.motion_margin_s,
            self.cfg.busy_timeout,
        );
        match self.wait_empty(&ctl, limit, false) {
            Ok(_) => {
                self.pos = Some(target);
                Ok(())
            }
            Err(e) => {
                self.pos = None;
                Err(e)
            }
        }
    }

    fn machine_mil(&self, p: Point) -> Result<(i64, i64), SessionError> {
        let m = self.cfg.home.to_machine_mm(p, self.cfg.bed_mm);
        let eps = crate::speed::MM_PER_MIL;
        if !(m.0.is_finite() && m.1.is_finite())
            || m.0 < -eps
            || m.1 < -eps
            || m.0 > self.cfg.bed_mm[0] + eps
            || m.1 > self.cfg.bed_mm[1] + eps
        {
            return Err(SessionError::OutOfBed {
                x_mm: p.x,
                y_mm: p.y,
            });
        }
        Ok((mm_to_mil(m.0).max(0), mm_to_mil(m.1).max(0)))
    }

    /// Lleva el cabezal a un punto (mm, sistema de Tlaku) con el láser apagado.
    pub fn jog_to(&mut self, target: Point) -> Result<(), SessionError> {
        self.require_idle()?;
        let t = self.machine_mil(target)?;
        self.rapid_to_mil(t)
    }

    /// Desplaza el cabezal `dx`, `dy` mm (ejes de Tlaku) con el láser apagado.
    pub fn jog_by(&mut self, dx: f64, dy: f64) -> Result<(), SessionError> {
        self.require_idle()?;
        let here = self.position_mm().ok_or(SessionError::UnknownPosition)?;
        let t = self.machine_mil(Point::new(here.x + dx, here.y + dy))?;
        self.rapid_to_mil(t)
    }

    /// Recorre las cuatro esquinas del rectángulo (mm) con el láser apagado.
    pub fn frame(&mut self, min: Point, max: Point) -> Result<(), SessionError> {
        self.require_idle()?;
        let corners = [
            Point::new(min.x, min.y),
            Point::new(max.x, min.y),
            Point::new(max.x, max.y),
            Point::new(min.x, max.y),
            Point::new(min.x, min.y),
        ];
        // Se valida todo el recorrido antes de mover nada.
        let targets: Vec<(i64, i64)> = corners
            .iter()
            .map(|&c| self.machine_mil(c))
            .collect::<Result<_, _>>()?;
        for t in targets {
            self.rapid_to_mil(t)?;
        }
        Ok(())
    }
}
