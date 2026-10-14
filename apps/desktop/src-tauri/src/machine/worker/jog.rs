//! Movimientos manuales (`jog`, `jog_to`) y overrides.

use super::*;

impl Worker {
    pub(super) fn jog(
        &mut self,
        t: &mut dyn Transport,
        dx: f64,
        dy: f64,
        speed: f64,
    ) -> Result<(), Fail> {
        if !(dx.is_finite() && dy.is_finite()) {
            return Err(Fail::Msg("El desplazamiento no es un número.".into()));
        }
        if !(speed.is_finite() && speed > 0.0) {
            return Err(Fail::Msg(
                "La velocidad del movimiento debe ser mayor que 0.".into(),
            ));
        }
        if self.link_family != LinkFamily::Grbl {
            return self.jog_reprap(t, dx, dy, speed);
        }
        let line = format!("$J=G91 G21 X{} Y{} F{}", num(dx), num(dy), num(speed));
        let reply = self.exchange(t, &line, self.timing.command_timeout)?;
        match reply.last().map(String::as_str) {
            Some("ok") => Ok(()),
            Some(e) => Err(Fail::Msg(jog_error(e))),
            None => Err(Fail::Msg("La máquina no respondió al movimiento.".into())),
        }
    }

    /// Movimiento absoluto: `$J=G90` en GRBL; en Marlin y Smoothieware `G90` y un `G1`.
    pub(super) fn jog_to(
        &mut self,
        t: &mut dyn Transport,
        x: f64,
        y: f64,
        speed: f64,
    ) -> Result<(), Fail> {
        if !(x.is_finite() && y.is_finite()) {
            return Err(Fail::Msg("La posición de destino no es un número.".into()));
        }
        if !(speed.is_finite() && speed > 0.0) {
            return Err(Fail::Msg(
                "La velocidad del movimiento debe ser mayor que 0.".into(),
            ));
        }
        // `M5` primero: el láser va apagado aunque una orden de la consola lo hubiera dejado
        // encendido (el jog no lo apaga).
        let lines = if self.link_family == LinkFamily::Grbl {
            vec![
                "M5".to_string(),
                format!("$J=G90 G21 X{} Y{} F{}", num(x), num(y), num(speed)),
            ]
        } else {
            vec![
                "M5".to_string(),
                "G90".to_string(),
                format!("G1 X{} Y{} F{}", num(x), num(y), num(speed)),
            ]
        };
        let mut failure = None;
        for line in &lines {
            let reply = self.exchange(t, line, self.timing.command_timeout)?;
            if failure.is_none()
                && let Some(last) = reply.last()
                && !is_ack(self.link_family, last)
            {
                failure = Some(last.clone());
            }
        }
        match failure {
            None => Ok(()),
            Some(e) => Err(Fail::Msg(jog_error(&e))),
        }
    }

    /// Plan de bytes para llevar el override de `kind` a `percent` desde lo último que informó
    /// la controladora. Falla fuera de GRBL, que no tiene overrides.
    pub(super) fn override_bytes(
        &self,
        kind: OverrideKind,
        percent: u32,
    ) -> Result<Vec<Realtime>, Fail> {
        if self.link_family != LinkFamily::Grbl {
            return Err(Fail::Msg(
                "Los ajustes en vivo de velocidad y potencia solo existen en controladoras GRBL:                  Marlin y Smoothieware no los tienen."
                    .into(),
            ));
        }
        let current = self.last.overrides.map(|o| match kind {
            OverrideKind::Feed => o.feed,
            OverrideKind::Spindle => o.spindle,
        });
        Ok(override_plan(kind, current, percent))
    }

    /// Registra el override y recuerda el valor pedido hasta el próximo `Ov:` de la controladora.
    pub(super) fn note_override(&mut self, kind: OverrideKind, percent: u32) {
        let percent = percent.clamp(streamer::OVERRIDE_MIN, streamer::OVERRIDE_MAX);
        let (name, what) = match kind {
            OverrideKind::Feed => ("velocidad", "feed"),
            OverrideKind::Spindle => ("potencia", "spindle"),
        };
        self.log(
            LogDir::Out,
            &format!("; override de {name} ({what}) al {percent} %"),
        );
        let mut o = self.last.overrides.unwrap_or(Overrides {
            feed: 100,
            rapid: 100,
            spindle: 100,
        });
        match kind {
            OverrideKind::Feed => o.feed = percent,
            OverrideKind::Spindle => o.spindle = percent,
        }
        self.last.overrides = Some(o);
        self.publish_last();
    }

    /// Override fuera de un trabajo: el trabajador escribe los bytes en el puerto.
    pub(super) fn override_idle(
        &mut self,
        t: &mut dyn Transport,
        kind: OverrideKind,
        percent: u32,
    ) -> Result<(), Fail> {
        let plan = self.override_bytes(kind, percent)?;
        for r in &plan {
            t.write_all(&[r.byte()])?;
        }
        self.note_override(kind, percent);
        Ok(())
    }

    /// Movimiento relativo sin `$J=` (solo GRBL): `G91`, el movimiento y vuelta a `G90`.
    pub(super) fn jog_reprap(
        &mut self,
        t: &mut dyn Transport,
        dx: f64,
        dy: f64,
        speed: f64,
    ) -> Result<(), Fail> {
        let lines = [
            "G91".to_string(),
            format!("G1 X{} Y{} F{}", num(dx), num(dy), num(speed)),
            "G90".to_string(),
        ];
        let mut failure = None;
        for line in &lines {
            let reply = self.exchange(t, line, self.timing.command_timeout)?;
            if failure.is_none()
                && let Some(last) = reply.last()
                && !is_ack(self.link_family, last)
            {
                failure = Some(last.clone());
            }
        }
        match failure {
            None => Ok(()),
            Some(e) => Err(Fail::Msg(jog_error(&e))),
        }
    }
}
