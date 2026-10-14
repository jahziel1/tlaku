//! Configuración del [`Streamer`] y política ante `error:N`.

use std::time::Duration;

use tlaku_doc::DeviceProfile;

use crate::link::{FlowControl, LinkProfile};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorPolicy {
    /// Un `error:N` detiene el trabajo: feed hold, espera a que frene, soft reset y fin.
    /// GRBL sigue ejecutando lo que ya tiene en el búfer cuando una línea falla (y la línea
    /// fallida no aplicó su S, F o M3/M4), así que no basta con dejar de enviar.
    Stop,
    /// Se registra el error y el trabajo sigue con la línea siguiente.
    Continue,
}

#[derive(Debug, Clone)]
pub struct StreamerConfig {
    /// Bytes que se pueden tener pendientes en la controladora. GRBL declara 128,
    /// pero su búfer circular deja libre un byte, así que se usa 127 por defecto;
    /// grblHAL admite bastante más.
    pub rx_buffer: usize,
    pub error_policy: ErrorPolicy,
    /// Cada cuánto se consulta `?`; `None` desactiva el sondeo (y con él la
    /// espera de reposo y el aviso por silencio).
    pub status_interval: Option<Duration>,
    /// Sin recibir ningún byte durante este tiempo (con sondeo activo) se da
    /// la conexión por perdida.
    ///
    /// Regla del silencio: `?` puede no contestarse durante el homing. Si hay una línea de
    /// homing (`$H`...) sin responder, el plazo es [`home_timeout`](Self::home_timeout) en
    /// lugar de este. Las alarmas (`ALARM:N`) terminan el trabajo al instante, así que no
    /// hay silencio posterior que vigilar aquí.
    pub silence_timeout: Duration,
    /// Plazo de silencio mientras una línea `$H` espera su `ok`.
    pub home_timeout: Duration,
    /// Terminar solo cuando la máquina informe `Idle` tras el último `ok`
    /// (los `ok` llegan al interpretar la línea, no al ejecutarla).
    pub wait_idle: bool,
    /// Margen tras el soft reset: tras ver el saludo (`Grbl ...`) se sigue leyendo este
    /// tiempo para recoger un `ALARM:3` que lo acompañe; luego se descarta lo que quede.
    pub cancel_delay: Duration,
    /// Máximo que se espera, tras el feed hold, a que la máquina termine de frenar
    /// (`Hold:0`) antes de resetear de todos modos. Un reset en `Hold:1` (todavía frenando)
    /// provoca `ALARM:3` y pierde la posición. Con el sondeo desactivado
    /// (`status_interval: None`) no hay forma de preguntar y solo se espera `cancel_delay`.
    pub hold_timeout: Duration,
    /// Cada cuánto se consulta `?` mientras se espera el fin del feed hold.
    pub hold_poll_interval: Duration,
    /// Máximo que se espera el saludo tras enviar el soft reset.
    pub reset_wait: Duration,
}

impl StreamerConfig {
    /// Configuración para un enlace: el búfer RX de GRBL y el sondeo `?` solo con GRBL; en
    /// Marlin y Smoothie no se sondea durante el trabajo.
    pub fn for_link(link: &LinkProfile) -> Self {
        let mut c = Self::default();
        if let FlowControl::CharCounting { rx_buffer } = link.flow {
            c.rx_buffer = rx_buffer;
        }
        if !link.polls_during_job() {
            c.status_interval = None;
        }
        c
    }

    /// Configuración para un perfil de máquina (ver [`LinkProfile::for_device`]).
    pub fn for_device(device: &DeviceProfile) -> Self {
        Self::for_link(&LinkProfile::for_device(device))
    }
}

impl Default for StreamerConfig {
    fn default() -> Self {
        Self {
            rx_buffer: 127,
            error_policy: ErrorPolicy::Stop,
            status_interval: Some(Duration::from_millis(200)),
            silence_timeout: Duration::from_secs(5),
            home_timeout: Duration::from_secs(120),
            wait_idle: true,
            cancel_delay: Duration::from_millis(50),
            hold_timeout: Duration::from_secs(2),
            hold_poll_interval: Duration::from_millis(100),
            reset_wait: Duration::from_millis(500),
        }
    }
}
