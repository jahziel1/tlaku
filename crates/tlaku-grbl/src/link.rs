//! Perfil de enlace: cómo se transmite el G-code a cada familia de firmware.
//!
//! El generador (`gcode.rs`) decide *qué* se escribe; este módulo decide *cómo viaja*: control
//! de flujo, consulta de estado, pausa, cancelación, detección del fin del trabajo y saludo de
//! reinicio. Son datos puros que lee el [`Streamer`](crate::streamer::Streamer); ver
//! `docs/hardware/dialectos-gcode.md`, secciones 2.2, 3.2, 8 y 9.
//!
//! - **GRBL** (GRBL 1.1, GRBL-M3, grblHAL, FluidNC, GRBL-LPC y `CustomGcode`): conteo de
//!   caracteres sobre un búfer RX, `?` periódico, `!`/`~` en tiempo real y `0x18` para cancelar.
//!   Es el comportamiento de siempre y no cambia.
//! - **Marlin**: ping-pong (una línea, esperar su `ok`), sin `?` durante el trabajo (sondear
//!   `M114` provoca tirones), pausa = dejar de enviar, cancelación = `M410` + el apagado del
//!   láser del dialecto, fin = `M400` + `ok`. Opcionalmente líneas con `N…*checksum` y `Resend`.
//! - **Smoothieware**: ping-pong estricto, pausa = dejar de enviar, cancelación = `M112` +
//!   `M999` + apagado del láser, fin = `M400` + `ok`.

use tlaku_doc::{DeviceKind, DeviceProfile, GcodeDialect};

/// Familia de protocolo del enlace.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkFamily {
    /// GRBL 1.1 y derivados: conteo de caracteres, tiempo real, `$`.
    Grbl,
    /// Marlin 2.x (RepRap): una línea por `ok`.
    Marlin,
    /// Smoothieware (RepRap con `?`): una línea por `ok`.
    Smoothie,
}

/// Control de flujo hacia la controladora.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlowControl {
    /// Conteo de caracteres: solo se envía otra línea mientras quepa en el búfer RX de la
    /// controladora (127 bytes en GRBL). Cada `ok` libera la línea más antigua.
    CharCounting { rx_buffer: usize },
    /// Ventana de líneas: como mucho `max_lines` sin responder. Con 1 es ping-pong. La
    /// longitud máxima de línea (con su salto) es `max_line_len`; 0 = sin dato.
    Window {
        max_lines: usize,
        max_line_len: usize,
    },
}

impl FlowControl {
    /// Una línea por vez: se envía la siguiente cuando llega el `ok` de la anterior.
    pub const fn ping_pong(max_line_len: usize) -> Self {
        Self::Window {
            max_lines: 1,
            max_line_len,
        }
    }

    /// Bytes máximos de una línea con su `\n` (`usize::MAX` si no hay límite conocido).
    pub fn max_line_len(self) -> usize {
        match self {
            Self::CharCounting { rx_buffer } => rx_buffer,
            Self::Window {
                max_line_len: 0, ..
            } => usize::MAX,
            Self::Window { max_line_len, .. } => max_line_len,
        }
    }

    pub fn is_ping_pong(self) -> bool {
        matches!(self, Self::Window { max_lines: 1, .. })
    }
}

/// Cómo se reconoce la confirmación de una línea.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AckMatch {
    /// La línea es exactamente `ok` (GRBL).
    Exact,
    /// `ok` o `ok` seguido de un espacio y texto (Marlin con `ADVANCED_OK`, `ok T:...`).
    Prefix,
}

/// Cómo se consulta el estado de la máquina.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StatusQuery {
    /// Byte en tiempo real (`?`).
    Realtime(u8),
    /// Línea de G-code (`M114`): lleva su propio `ok` y se encola detrás del trabajo.
    GcodeLine(String),
    /// No hay consulta.
    None,
}

/// Una acción de control (pausa, reanudar, cancelar).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ControlAction {
    /// Byte en tiempo real, fuera del conteo.
    Realtime(u8),
    /// Línea de G-code. Lleva su `ok`, que el streamer no cuenta como avance del trabajo.
    Line(String),
    /// Solo del lado del anfitrión: dejar de enviar (o seguir enviando).
    HostSide,
}

/// Cómo se detiene la máquina al cancelar o ante un error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CancelStyle {
    /// GRBL: feed hold, esperar a que frene (`Hold:0`) y soft reset `0x18`.
    SafeStopReset,
    /// Firmware sin reinicio en caliente: se deja de enviar y se manda esta secuencia de
    /// líneas (parada rápida y apagado del láser). Se esperan sus `ok`.
    Sequence(Vec<ControlAction>),
}

/// Cómo se sabe que la máquina terminó de moverse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FinishDetection {
    /// Informe `Idle` tras el último `ok` (los `ok` de GRBL llegan al interpretar, no al ejecutar).
    StatusIdle,
    /// Se envía esta línea (`M400`) y se espera su `ok`: llega cuando acaba el último movimiento.
    /// Si la última línea del trabajo ya es esa orden, se usa su `ok`.
    SyncCommand(String),
}

/// Saludo que indica que la controladora se reinició.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Banner {
    /// La línea empieza por este texto (sin distinguir mayúsculas).
    Prefix(String),
    /// La línea es exactamente este texto (sin distinguir mayúsculas).
    Exact(String),
}

/// Líneas numeradas con checksum (`N<n> <línea>*<xor>`): el firmware pide reenvío
/// (`Resend: n`) si fallan. Marlin exige `N` y `*` juntos ("ambos o ninguno"), así que no hay
/// versión sin checksum.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LineNumbering;

/// Todo lo que el streamer necesita saber del enlace de un firmware.
#[derive(Debug, Clone, PartialEq)]
pub struct LinkProfile {
    pub family: LinkFamily,
    pub flow: FlowControl,
    pub ack: AckMatch,
    /// Consulta durante el trabajo. `None` desactiva el sondeo (Marlin: provoca tirones).
    pub status_job: StatusQuery,
    /// Consulta con la máquina en reposo (fuera de un trabajo): `?` o `M114`.
    pub status_idle: StatusQuery,
    /// Qué hacer al pausar. `HostSide` = dejar de enviar líneas nuevas.
    pub pause: Vec<ControlAction>,
    pub resume: Vec<ControlAction>,
    pub cancel: CancelStyle,
    pub finish: FinishDetection,
    /// Saludos de reinicio de la controladora.
    pub reset_banners: Vec<Banner>,
    /// Quita todos los espacios de la línea (`G1 X10` -> `G1X10`). Solo GRBL: en Marlin y
    /// Smoothie rompería `M3 S0 I` y comandos como `G28.2`.
    pub strip_spaces: bool,
    /// Rechaza `?`, `!` y `~` fuera de comentarios: GRBL (y Smoothie) los toman como tiempo real.
    pub reject_realtime_chars: bool,
    /// `None` = sin numerar (por defecto en todos). Solo Marlin lo admite.
    pub line_numbering: Option<LineNumbering>,
    /// Escribe en EEPROM (`$x=`, `G10`, `G28.1`...): se envían solas, esperando su `ok`. Solo GRBL.
    pub eeprom_sync: bool,
    /// El firmware tiene `EMERGENCY_PARSER` (Marlin): `M410`/`M112`/`M108` actúan al instante
    /// aunque la cola esté llena.
    pub emergency_parser: bool,
    /// Velocidad serie habitual del firmware.
    pub default_baud: u32,
}

impl LinkProfile {
    /// GRBL con el búfer RX por defecto (127 bytes). Es lo que usa `Streamer::new`.
    pub fn grbl() -> Self {
        Self::grbl_with_buffer(127)
    }

    /// GRBL con un búfer RX dado (127 en GRBL clásico, más en grblHAL y la Falcon2).
    pub fn grbl_with_buffer(rx_buffer: usize) -> Self {
        Self {
            family: LinkFamily::Grbl,
            flow: FlowControl::CharCounting { rx_buffer },
            ack: AckMatch::Exact,
            status_job: StatusQuery::Realtime(b'?'),
            status_idle: StatusQuery::Realtime(b'?'),
            pause: vec![ControlAction::Realtime(b'!')],
            resume: vec![ControlAction::Realtime(b'~')],
            cancel: CancelStyle::SafeStopReset,
            finish: FinishDetection::StatusIdle,
            reset_banners: vec![Banner::Prefix("grbl".into())],
            strip_spaces: true,
            reject_realtime_chars: true,
            line_numbering: None,
            eeprom_sync: true,
            emergency_parser: false,
            default_baud: 115_200,
        }
    }

    /// Marlin 2.x. La cancelación es `M410` y el apagado del láser del dialecto (`M5`,
    /// `M107 P1`...): Marlin no tiene reinicio en caliente y el láser seguiría como estaba.
    pub fn marlin(dialect: &GcodeDialect) -> Self {
        let mut cancel = vec![ControlAction::HostSide, ControlAction::Line("M410".into())];
        if !dialect.laser_off.trim().is_empty() {
            cancel.push(ControlAction::Line(dialect.laser_off.trim().to_string()));
        }
        Self {
            family: LinkFamily::Marlin,
            flow: FlowControl::ping_pong(dialect.max_line_len as usize),
            ack: AckMatch::Prefix,
            // `M114` durante el trabajo provoca tirones (doc de M154): el progreso sale de los `ok`.
            status_job: StatusQuery::None,
            status_idle: StatusQuery::GcodeLine("M114".into()),
            pause: vec![ControlAction::HostSide],
            resume: vec![ControlAction::HostSide],
            cancel: CancelStyle::Sequence(cancel),
            finish: FinishDetection::SyncCommand("M400".into()),
            reset_banners: vec![Banner::Exact("start".into())],
            strip_spaces: false,
            reject_realtime_chars: false,
            line_numbering: None,
            eeprom_sync: false,
            emergency_parser: false,
            default_baud: 250_000,
        }
    }

    /// Smoothieware. El formato de respuesta de `?` no está confirmado por la documentación
    /// (`dialectos-gcode.md`, 3.2), así que durante el trabajo no se sondea: ping-pong estricto,
    /// y la cancelación es `M112` + `M999` (limpia el estado de paro) + apagado del láser.
    pub fn smoothie(dialect: &GcodeDialect) -> Self {
        let mut cancel = vec![
            ControlAction::HostSide,
            ControlAction::Line("M112".into()),
            ControlAction::Line("M999".into()),
        ];
        if !dialect.laser_off.trim().is_empty() {
            cancel.push(ControlAction::Line(dialect.laser_off.trim().to_string()));
        }
        Self {
            family: LinkFamily::Smoothie,
            flow: FlowControl::ping_pong(dialect.max_line_len as usize),
            ack: AckMatch::Prefix,
            status_job: StatusQuery::None,
            status_idle: StatusQuery::GcodeLine("M114".into()),
            pause: vec![ControlAction::HostSide],
            resume: vec![ControlAction::HostSide],
            cancel: CancelStyle::Sequence(cancel),
            finish: FinishDetection::SyncCommand("M400".into()),
            reset_banners: vec![Banner::Exact("smoothie".into())],
            strip_spaces: false,
            reject_realtime_chars: true,
            line_numbering: None,
            eeprom_sync: false,
            emergency_parser: false,
            default_baud: 115_200,
        }
    }

    /// Enlace de un tipo de firmware con su dialecto. `rx_buffer` solo importa en la familia
    /// GRBL (conteo de caracteres); Marlin y Smoothie usan `dialect.max_line_len`.
    pub fn for_kind(kind: DeviceKind, dialect: &GcodeDialect, rx_buffer: usize) -> Self {
        match kind {
            DeviceKind::Marlin => Self::marlin(dialect),
            DeviceKind::Smoothie => Self::smoothie(dialect),
            DeviceKind::Grbl
            | DeviceKind::GrblM3
            | DeviceKind::GrblHal
            | DeviceKind::FluidNc
            | DeviceKind::GrblLpc
            | DeviceKind::CustomGcode => Self::grbl_with_buffer(rx_buffer),
            // No usan este enlace (tienen su propio driver); se deja el de GRBL por completitud.
            DeviceKind::Ruida | DeviceKind::LhyMicro | DeviceKind::Galvo => {
                Self::grbl_with_buffer(rx_buffer)
            }
        }
    }

    /// Enlace de un perfil de máquina (`kind`, `dialect` y `rx_buffer` del perfil).
    pub fn for_device(device: &DeviceProfile) -> Self {
        Self::for_kind(device.kind, &device.dialect, device.rx_buffer as usize)
    }

    /// Activa (o no) el `EMERGENCY_PARSER` de Marlin. Con él, pausar también manda `M0` y
    /// reanudar `M108` (`M0` encolada se ignora sin él, ver 2.2 de `dialectos-gcode.md`).
    pub fn with_emergency_parser(mut self, yes: bool) -> Self {
        self.emergency_parser = yes;
        if self.family == LinkFamily::Marlin {
            if yes {
                self.pause = vec![ControlAction::HostSide, ControlAction::Line("M0".into())];
                self.resume = vec![ControlAction::Line("M108".into()), ControlAction::HostSide];
            } else {
                self.pause = vec![ControlAction::HostSide];
                self.resume = vec![ControlAction::HostSide];
            }
        }
        self
    }

    /// Numera las líneas (`N<n>`) con checksum y atiende los `Resend:`. Solo Marlin; en las
    /// demás familias no hace nada. Por USB suma ~10 bytes por línea sin beneficio: úsalo con
    /// enlaces ruidosos (UART real, cables largos).
    pub fn with_line_numbering(mut self) -> Self {
        if self.family == LinkFamily::Marlin {
            self.line_numbering = Some(LineNumbering);
        }
        self
    }

    /// Cambia el tamaño del búfer RX (solo con conteo de caracteres).
    pub fn with_rx_buffer(mut self, rx_buffer: usize) -> Self {
        if let FlowControl::CharCounting { rx_buffer: b } = &mut self.flow {
            *b = rx_buffer;
        }
        self
    }

    /// ¿La línea recibida confirma una línea enviada?
    pub fn is_ack(&self, line: &str) -> bool {
        match self.ack {
            AckMatch::Exact => line == "ok",
            AckMatch::Prefix => line == "ok" || line.starts_with("ok "),
        }
    }

    /// ¿La línea es el saludo de un reinicio de la controladora?
    pub fn is_reset_banner(&self, line: &str) -> bool {
        self.reset_banners.iter().any(|b| match b {
            Banner::Prefix(p) => line
                .as_bytes()
                .get(..p.len())
                .is_some_and(|h| h.eq_ignore_ascii_case(p.as_bytes())),
            Banner::Exact(p) => line.trim().eq_ignore_ascii_case(p),
        })
    }

    /// ¿Se consulta el estado con un byte en tiempo real durante el trabajo?
    pub fn polls_during_job(&self) -> bool {
        matches!(self.status_job, StatusQuery::Realtime(_))
    }

    /// Nombre corto de la familia para los mensajes.
    pub fn family_name(&self) -> &'static str {
        match self.family {
            LinkFamily::Grbl => "GRBL",
            LinkFamily::Marlin => "Marlin",
            LinkFamily::Smoothie => "Smoothieware",
        }
    }
}

/// Checksum de una línea numerada: XOR de todos sus bytes (incluido el `N<n>`) antes del `*`.
pub fn line_checksum(body: &str) -> u8 {
    body.bytes().fold(0, |acc, b| acc ^ b)
}

/// Línea numerada lista para enviar (sin `\n`): `N<n> <texto>*<xor>`.
pub fn numbered_line(n: usize, text: &str, _numbering: LineNumbering) -> String {
    let body = format!("N{n} {text}");
    let cs = line_checksum(&body);
    format!("{body}*{cs}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grbl_link_matches_the_historical_behaviour() {
        let l = LinkProfile::grbl();
        assert_eq!(l.flow, FlowControl::CharCounting { rx_buffer: 127 });
        assert!(l.polls_during_job() && l.strip_spaces && l.eeprom_sync);
        assert!(l.is_ack("ok") && !l.is_ack("ok T:20") && !l.is_ack("okay"));
        assert!(l.is_reset_banner("Grbl 1.1h ['$' for help]") && l.is_reset_banner("GrblHAL 1.1f"));
        assert!(!l.is_reset_banner("grb") && !l.is_reset_banner("\u{fffd}\u{fffd}\u{fffd}x"));
        assert_eq!(l.cancel, CancelStyle::SafeStopReset);
        assert_eq!(l.finish, FinishDetection::StatusIdle);
    }

    #[test]
    fn marlin_link_is_ping_pong_without_polling() {
        let d = GcodeDialect::marlin_inline();
        let l = LinkProfile::marlin(&d);
        assert!(l.flow.is_ping_pong());
        assert_eq!(l.flow.max_line_len(), 96);
        assert!(!l.polls_during_job());
        assert_eq!(l.status_idle, StatusQuery::GcodeLine("M114".into()));
        assert!(!l.strip_spaces);
        assert!(l.is_ack("ok") && l.is_ack("ok N3 P15 B3") && !l.is_ack("okay"));
        assert!(l.is_reset_banner("start") && !l.is_reset_banner("echo:start"));
        assert_eq!(l.finish, FinishDetection::SyncCommand("M400".into()));
        // La cancelación usa el apagado del dialecto.
        assert_eq!(
            l.cancel,
            CancelStyle::Sequence(vec![
                ControlAction::HostSide,
                ControlAction::Line("M410".into()),
                ControlAction::Line("M5".into())
            ])
        );
        let fan = LinkProfile::marlin(&GcodeDialect::marlin_fan());
        assert!(matches!(&fan.cancel, CancelStyle::Sequence(s)
            if s.last() == Some(&ControlAction::Line("M107 P1".into()))));
    }

    #[test]
    fn smoothie_link_is_strict_ping_pong_with_no_line_limit() {
        let d = GcodeDialect::for_kind(DeviceKind::Smoothie);
        let l = LinkProfile::for_kind(DeviceKind::Smoothie, &d, 127);
        assert!(l.flow.is_ping_pong());
        assert_eq!(l.flow.max_line_len(), usize::MAX);
        assert!(l.is_reset_banner("Smoothie") && !l.is_reset_banner("Smoothieware"));
        assert!(matches!(&l.cancel, CancelStyle::Sequence(s)
            if s[1] == ControlAction::Line("M112".into()) && s[2] == ControlAction::Line("M999".into())));
    }

    #[test]
    fn every_kind_has_a_link() {
        use DeviceKind::*;
        for k in [
            Grbl,
            GrblM3,
            GrblHal,
            FluidNc,
            GrblLpc,
            Smoothie,
            Marlin,
            CustomGcode,
        ] {
            let d = GcodeDialect::for_kind(k);
            let l = LinkProfile::for_kind(k, &d, 255);
            let grbl = !matches!(k, Smoothie | Marlin);
            assert_eq!(l.family == LinkFamily::Grbl, grbl, "{k:?}");
            if grbl {
                assert_eq!(l.flow, FlowControl::CharCounting { rx_buffer: 255 });
            }
            assert_eq!(
                l.default_baud,
                if k == Marlin { 250_000 } else { 115_200 },
                "{k:?}"
            );
        }
    }

    #[test]
    fn emergency_parser_adds_m0_and_m108() {
        let l = LinkProfile::marlin(&GcodeDialect::marlin_inline()).with_emergency_parser(true);
        assert!(l.pause.contains(&ControlAction::Line("M0".into())));
        assert!(l.resume.contains(&ControlAction::Line("M108".into())));
        let g = LinkProfile::grbl().with_emergency_parser(true);
        assert_eq!(g.pause, vec![ControlAction::Realtime(b'!')]);
    }

    #[test]
    fn numbered_lines_match_the_reprap_checksum_rule() {
        // Ejemplos de la wiki de RepRap (oráculo de la regla XOR).
        assert_eq!(line_checksum("N3 T0"), 57);
        assert_eq!(line_checksum("N4 G92 E0"), 67);
        assert_eq!(line_checksum("N5 G28"), 22);
        assert_eq!(line_checksum("N6 G1 F1500.0"), 82);
        let l = numbered_line(5, "G1 X10", LineNumbering);
        let (body, cs) = l.split_once('*').unwrap();
        assert_eq!(body, "N5 G1 X10");
        assert_eq!(cs.parse::<u8>().unwrap(), line_checksum(body));
        // Solo Marlin numera.
        assert!(
            LinkProfile::grbl()
                .with_line_numbering()
                .line_numbering
                .is_none()
        );
        assert!(
            LinkProfile::marlin(&GcodeDialect::marlin_inline())
                .with_line_numbering()
                .line_numbering
                .is_some()
        );
    }
}
