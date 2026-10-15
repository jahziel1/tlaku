//! Límites de la cama para el movimiento manual, lectura de `$$` y reinicios de la placa.
//!
//! Dos guardas de seguridad que viven en el trabajador (y no en la interfaz) para que las cumplan
//! también la automatización y el control remoto:
//!
//! - **Cama**: tras un homing exitoso en espacio positivo (la MPos queda cerca de (0, 0) y crece
//!   hacia dentro, como en la Falcon2) un `jog` o `jog_to` cuyo destino sale de la cama se
//!   rechaza. No se recorta en silencio. Sin homing, sin área de cama conocida o en espacio
//!   negativo no se bloquea: se avisa una vez en el registro.
//! - **Reinicio**: un saludo de la placa que Tlaku no provocó (no mandó `0x18` en los últimos
//!   segundos y no es el de la conexión) deja la posición «sin confirmar» hasta el próximo homing.

use super::*;
use tlaku_i18n::tr;

/// Cuánto se tolera pasar del borde de la cama (el homing deja la cabeza en ~0.025 mm).
const BED_TOLERANCE: f64 = 0.5;

/// Tras el homing, la MPos de cada eje debe estar entre `-HOME_NEAR_MIN` y `HOME_NEAR_MAX` para
/// tomar el espacio por positivo (el retroceso `$27` deja la cabeza a pocos mm del cero).
const HOME_NEAR_MIN: f64 = 0.1;
const HOME_NEAR_MAX: f64 = 10.0;

/// Tras mandar un `0x18` (parada segura, cancelar, reset manual) el saludo de la placa es
/// esperado durante este tiempo.
const OWN_RESET_WINDOW: Duration = Duration::from_secs(3);

/// Estado de las guardas del trabajador.
#[derive(Default)]
pub(super) struct Limits {
    /// `$130`/`$131` leídos de la placa (mm).
    settings_bed: [Option<f64>; 2],
    /// `$23` (dirección del homing) si se leyó: el bit de un eje activo = busca el final de carrera
    /// del lado negativo, o sea que el cero queda en esa esquina y la MPos crece hacia dentro.
    home_dir_mask: Option<u32>,
    /// `Some(true)` si tras el homing la MPos quedó cerca de (0, 0) y no es negativa.
    positive_space: Option<bool>,
    /// Hubo un `$H` y falta el primer reporte de estado para saber el espacio.
    awaiting_home_pos: bool,
    unchecked_logged: bool,
    soft_limits_logged: bool,
    /// Última vez que Tlaku mandó un reinicio a propósito.
    pub(super) last_reset_sent: Option<Instant>,
    last_boot_seen: Option<Instant>,
}

/// ¿El espacio de máquina es positivo (el cero en la esquina del homing, creciendo hacia dentro)?
/// La MPos tras el homing debe quedar cerca de (0, 0) sin signo negativo y, si se conoce `$23`,
/// X e Y deben buscar su final de carrera del lado negativo.
pub(in crate::machine) fn space_is_positive(mpos: [f64; 2], home_dir_mask: Option<u32>) -> bool {
    mpos.iter()
        .all(|v| (-HOME_NEAR_MIN..=HOME_NEAR_MAX).contains(v))
        && home_dir_mask.is_none_or(|m| m & 0b11 == 0b11)
}

/// `$130=400.000` -> `(130, 400.0)`.
fn parse_setting(line: &str) -> Option<(u32, f64)> {
    let rest = line.trim().strip_prefix('$')?;
    let (id, value) = rest.split_once('=')?;
    Some((id.trim().parse().ok()?, value.trim().parse().ok()?))
}

impl Worker {
    /// Área de la cama [ancho, alto] en los ejes de la máquina: la del perfil y, si no hay, la
    /// de `$130`/`$131`.
    fn bed_area(&self) -> Option<[f64; 2]> {
        let valid = |b: [f64; 2]| b[0] > 0.0 && b[1] > 0.0 && b[0].is_finite() && b[1].is_finite();
        if let Some(b) = self.shared.bed.lock().ok().and_then(|g| *g)
            && valid(b)
        {
            return Some(b);
        }
        match self.lim.settings_bed {
            [Some(x), Some(y)] if valid([x, y]) => Some([x, y]),
            _ => None,
        }
    }

    /// El área contra la que se revisa el movimiento, si hay certeza: GRBL, con homing hecho en
    /// esta sesión, espacio positivo y cama conocida.
    fn active_bed(&self) -> Option<[f64; 2]> {
        if self.link_family != LinkFamily::Grbl
            || !self.shared.homed.load(Ordering::SeqCst)
            || self.lim.positive_space != Some(true)
        {
            return None;
        }
        self.bed_area()
    }

    /// Si hay un homing reciente sin reporte posterior, espera uno (para saber el espacio).
    pub(super) fn settle_home_reference(&mut self, t: &mut dyn Transport) -> Result<(), Fail> {
        if !self.lim.awaiting_home_pos {
            return Ok(());
        }
        let before = self.statuses;
        t.write_all(&[Realtime::StatusQuery.byte()])?;
        self.last_poll = Some(Instant::now());
        let deadline = Instant::now() + Duration::from_millis(500);
        while self.statuses == before && Instant::now() < deadline {
            for l in self.pump(t, self.timing.tick)? {
                self.log(LogDir::In, &describe_line(self.fw, &l));
            }
        }
        Ok(())
    }

    /// Rechaza un destino (posición de máquina, mm) fuera de la cama. Sin certeza no bloquea y
    /// avisa una sola vez.
    pub(super) fn check_destination(&mut self, target: [f64; 2]) -> Result<(), Fail> {
        if self.link_family != LinkFamily::Grbl {
            return Ok(());
        }
        let Some(bed) = self.active_bed() else {
            if !self.lim.unchecked_logged {
                self.lim.unchecked_logged = true;
                self.log(
                    LogDir::In,
                    &format!("; {}", tr!("desktop.worker.jog_unchecked")),
                );
            }
            return Ok(());
        };
        let mut parts = Vec::new();
        for (i, axis) in ["X", "Y"].into_iter().enumerate() {
            if !(-BED_TOLERANCE..=bed[i] + BED_TOLERANCE).contains(&target[i]) {
                parts.push(tr!(
                    "desktop.worker.jog_outside_axis",
                    axis = axis,
                    value = format!("{:.2}", target[i]),
                    min = 0,
                    max = num(bed[i])
                ));
            }
        }
        if parts.is_empty() {
            Ok(())
        } else {
            Err(Fail::Msg(tr!(
                "desktop.worker.jog_outside_bed",
                detail = parts.join("; ")
            )))
        }
    }

    /// Posición de máquina a la que llegaría un movimiento relativo.
    pub(super) fn relative_target(&self, dx: f64, dy: f64) -> [f64; 2] {
        let m = self.last.machine_pos;
        [m[0] + dx, m[1] + dy]
    }

    /// Posición de máquina de un destino en coordenadas de trabajo.
    pub(super) fn absolute_target(&self, x: f64, y: f64) -> [f64; 2] {
        [x + self.wco[0], y + self.wco[1]]
    }

    /// La cabeza se mueve (o puede moverse) por orden de Tlaku: se anota en la conexión para que
    /// la reanudación no tome la MPos de ahora por la que dejó un trabajo anterior.
    pub(super) fn note_head_moves(&self) {
        self.shared.moved.store(true, Ordering::SeqCst);
    }

    /// Un `$H` terminó bien: la posición vuelve a estar confirmada y falta saber el espacio.
    pub(super) fn on_homed(&mut self) {
        self.shared.homed.store(true, Ordering::SeqCst);
        self.lim.awaiting_home_pos = true;
        self.lim.positive_space = None;
        if self.last.position_unconfirmed.is_some() {
            self.last.position_unconfirmed = None;
            self.publish_last();
        }
    }

    /// Primer reporte tras el homing: si la MPos quedó cerca de (0, 0) y sin signo negativo, el
    /// espacio es positivo (crece hacia dentro de la cama).
    pub(super) fn learn_space(&mut self, state: MachineState, mpos: [f64; 2]) {
        if !self.lim.awaiting_home_pos || matches!(state, MachineState::Home | MachineState::Alarm)
        {
            return;
        }
        self.lim.awaiting_home_pos = false;
        self.lim.positive_space = Some(space_is_positive(mpos, self.lim.home_dir_mask));
    }

    /// La posición dejó de ser fiable (reinicio de la placa): sin límites hasta el próximo homing.
    pub(super) fn mark_unconfirmed(&mut self) {
        self.shared.homed.store(false, Ordering::SeqCst);
        self.lim.awaiting_home_pos = false;
        self.lim.positive_space = None;
        if self.last.position_unconfirmed.is_none() {
            self.last.position_unconfirmed = Some(true);
            self.publish_last();
        }
    }

    /// Tlaku acaba de mandar (o mandará) un reinicio: el saludo que llegue es esperado.
    pub(super) fn note_own_reset(&mut self) {
        self.lim.last_reset_sent = Some(Instant::now());
    }

    /// Una línea recibida con el puerto en manos del trabajador (reposo, comandos): si es el
    /// arranque de la placa y no lo provocó Tlaku, la placa se reinició sola.
    pub(super) fn detect_reboot(&mut self, line: &str) {
        if self.link_family != LinkFamily::Grbl || !is_boot_banner(line) {
            return;
        }
        // Sin ningún reporte aún: es el saludo de la conexión, no un reinicio.
        if self.statuses == 0 {
            return;
        }
        let recent = |t: Option<Instant>| t.is_some_and(|t| t.elapsed() < OWN_RESET_WINDOW);
        // Esperado (lo provocó un `0x18` nuestro) o ya avisado (el saludo de Creality trae
        // varias líneas de arranque).
        if recent(self.lim.last_reset_sent) || recent(self.lim.last_boot_seen) {
            return;
        }
        self.lim.last_boot_seen = Some(Instant::now());
        self.mark_unconfirmed();
        self.log(
            LogDir::In,
            &format!("; {}", tr!("desktop.worker.rebooted_idle")),
        );
    }

    /// Lo que se aprende de la respuesta de `$$`: el tamaño de la cama (`$130`/`$131`) y el
    /// aviso de homing sin límites por software (`$22=1`, `$20=0`).
    pub(super) fn note_settings(&mut self, lines: &[String]) {
        let (mut s20, mut s22) = (None, None);
        for (id, v) in lines.iter().filter_map(|l| parse_setting(l)) {
            match id {
                20 => s20 = Some(v),
                22 => s22 = Some(v),
                23 => self.lim.home_dir_mask = Some(v as u32),
                130 => self.lim.settings_bed[0] = Some(v),
                131 => self.lim.settings_bed[1] = Some(v),
                _ => {}
            }
        }
        if s22.is_some_and(|v| v != 0.0) && s20 == Some(0.0) && !self.lim.soft_limits_logged {
            self.lim.soft_limits_logged = true;
            self.log(
                LogDir::In,
                &format!("; {}", tr!("desktop.worker.soft_limits_off")),
            );
        }
    }

    /// Antes de enviar una línea: `$RST` y `$bye` reinician la placa a propósito.
    pub(super) fn note_line_to_send(&mut self, line: &str) {
        let up = line.to_ascii_uppercase();
        if up.starts_with("$RST") || up.starts_with("$BYE") {
            self.note_own_reset();
        }
    }

    /// Una escritura de `$23`, `$130` o `$131` aceptada actualiza lo que se sabe de la máquina.
    pub(super) fn note_setting_written(&mut self, line: &str) {
        if let Some((id, v)) = parse_setting(line) {
            match id {
                23 => self.lim.home_dir_mask = Some(v as u32),
                130 => self.lim.settings_bed[0] = Some(v),
                131 => self.lim.settings_bed[1] = Some(v),
                _ => {}
            }
        }
    }
}
