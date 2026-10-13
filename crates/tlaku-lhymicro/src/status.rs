//! Estado de la placa: consulta de 1 byte (`0xA0`) y respuesta de 6 bytes.
//!
//! El segundo byte de la respuesta (índice 1) es el código de estado. Valores (MeerK40t,
//! `lihuiyu/controller.py`, MIT; coinciden con EduTech y el foro de MakerForums):
//!
//! | Código | Significado |
//! |---|---|
//! | 206 (`0xCE`) | listo: acepta un paquete |
//! | 238 (`0xEE`) | ocupado: búfer lleno, no enviar |
//! | 207 (`0xCF`) | error de CRC: el paquete se rechazó y hay que reenviarlo |
//! | 236 (`0xEC`) | terminó: búfer vacío tras un trabajo |
//! | 239 (`0xEF`) | energía insuficiente (paro de emergencia, fuente apagada) |
//! | 204 (`0xCC`) | fin con confirmación de número de serie (solo M3 Nano) |
//! | 0 | sin lectura (fallo de USB) |
//!
//! Los códigos son las líneas del puerto paralelo que el microcontrolador de la placa maneja a
//! través del CH341 (inferido de los nombres de bits que usa MeerK40t): el bit 0 es ERR (CRC), el
//! bit 1 es PEMP (búfer vacío cuando vale 0) y el bit 5 es BUSY. De ahí el criterio de
//! "terminó" de MeerK40t (`código & 0x02 == 0`).
//!
//! La **Moshiboard** comparte VID/PID con la M2 Nano pero usa otros códigos (205 listo, 207
//! procesando, 237 error, 239 reinicio; MeerK40t `moshi/controller.py`): 205 y 237 no existen en
//! la M2 Nano y delatan una Moshiboard (inferido).

pub const STATUS_OK: u8 = 206;
pub const STATUS_BUSY: u8 = 238;
pub const STATUS_CRC_ERROR: u8 = 207;
pub const STATUS_FINISH: u8 = 236;
pub const STATUS_LOW_POWER: u8 = 239;
pub const STATUS_M3_FINISH: u8 = 204;
/// Códigos exclusivos de la Moshiboard (no los emite la M2 Nano).
pub const MOSHI_OK: u8 = 205;
pub const MOSHI_ERROR: u8 = 237;

/// Largo de la respuesta de estado.
pub const STATUS_LEN: usize = 6;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// 206: listo para un paquete.
    Ok,
    /// 238: ocupado (búfer lleno).
    Busy,
    /// 207: el último paquete falló el CRC.
    CrcError,
    /// 236: terminó, búfer vacío.
    Finished,
    /// 239: energía insuficiente.
    LowPower,
    /// 204: fin con confirmación de serie (M3).
    M3Finished,
    /// 0: la lectura no devolvió estado.
    NoReply,
    /// Cualquier otro valor.
    Unknown(u8),
}

impl Status {
    pub fn from_code(code: u8) -> Status {
        match code {
            STATUS_OK => Status::Ok,
            STATUS_BUSY => Status::Busy,
            STATUS_CRC_ERROR => Status::CrcError,
            STATUS_FINISH => Status::Finished,
            STATUS_LOW_POWER => Status::LowPower,
            STATUS_M3_FINISH => Status::M3Finished,
            0 => Status::NoReply,
            other => Status::Unknown(other),
        }
    }

    pub fn code(self) -> u8 {
        match self {
            Status::Ok => STATUS_OK,
            Status::Busy => STATUS_BUSY,
            Status::CrcError => STATUS_CRC_ERROR,
            Status::Finished => STATUS_FINISH,
            Status::LowPower => STATUS_LOW_POWER,
            Status::M3Finished => STATUS_M3_FINISH,
            Status::NoReply => 0,
            Status::Unknown(c) => c,
        }
    }

    /// La placa puede recibir un paquete: lista, o terminada y en reposo.
    pub fn accepts_packets(self) -> bool {
        matches!(self, Status::Ok | Status::Finished | Status::M3Finished)
    }

    /// Búfer vacío (bit PEMP en 0): el criterio con el que MeerK40t decide que un trabajo
    /// terminó. Cierto en 236 y 204.
    pub fn is_empty(self) -> bool {
        matches!(self, Status::Finished | Status::M3Finished)
    }

    pub fn describe(self) -> String {
        match self {
            Status::Ok => "lista (206)".into(),
            Status::Busy => "ocupada (238)".into(),
            Status::CrcError => "rechazó el paquete por CRC (207)".into(),
            Status::Finished => "terminó (236)".into(),
            Status::LowPower => {
                "energía insuficiente (239): ¿paro de emergencia o fuente apagada?".into()
            }
            Status::M3Finished => "terminó (204, M3)".into(),
            Status::NoReply => "sin respuesta (0)".into(),
            Status::Unknown(c) => format!("código desconocido ({c})"),
        }
    }
}

/// Lectura de estado: los 6 bytes y el código interpretado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StatusReport {
    pub raw: [u8; STATUS_LEN],
    pub status: Status,
}

impl StatusReport {
    /// Interpreta la respuesta. Con menos de 2 bytes no hay código (`NoReply`).
    pub fn parse(bytes: &[u8]) -> StatusReport {
        let mut raw = [0u8; STATUS_LEN];
        let n = bytes.len().min(STATUS_LEN);
        raw[..n].copy_from_slice(&bytes[..n]);
        let status = if n >= 2 {
            Status::from_code(raw[1])
        } else {
            Status::NoReply
        };
        StatusReport { raw, status }
    }

    pub fn code(&self) -> u8 {
        self.raw[1]
    }
}

/// El código solo lo emite una Moshiboard (no una M2 Nano).
pub fn looks_like_moshiboard(code: u8) -> bool {
    matches!(code, MOSHI_OK | MOSHI_ERROR)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_round_trip() {
        for code in [206u8, 238, 207, 236, 239, 204, 0, 17, 205] {
            assert_eq!(Status::from_code(code).code(), code);
        }
        assert_eq!(Status::from_code(206), Status::Ok);
        assert_eq!(Status::from_code(238), Status::Busy);
        assert_eq!(Status::from_code(207), Status::CrcError);
        assert_eq!(Status::from_code(236), Status::Finished);
        assert_eq!(Status::from_code(239), Status::LowPower);
        assert_eq!(Status::from_code(204), Status::M3Finished);
        assert_eq!(Status::from_code(0), Status::NoReply);
        assert_eq!(Status::from_code(77), Status::Unknown(77));
    }

    #[test]
    fn readiness_and_emptiness() {
        assert!(Status::Ok.accepts_packets());
        assert!(Status::Finished.accepts_packets());
        assert!(!Status::Busy.accepts_packets());
        assert!(!Status::CrcError.accepts_packets());
        assert!(!Status::LowPower.accepts_packets());
        assert!(!Status::Unknown(5).accepts_packets());
        assert!(Status::Finished.is_empty() && Status::M3Finished.is_empty());
        assert!(!Status::Ok.is_empty() && !Status::Busy.is_empty());
        // El criterio por bits de MeerK40t (código & 0x02 == 0) coincide con `is_empty`.
        for code in [206u8, 238, 207, 236, 239, 204] {
            assert_eq!(
                Status::from_code(code).is_empty(),
                code & 0x02 == 0,
                "{code}"
            );
        }
    }

    #[test]
    fn six_byte_reply_is_parsed() {
        let r = StatusReport::parse(&[255, 206, 0, 0, 0, 1]);
        assert_eq!(r.status, Status::Ok);
        assert_eq!(r.code(), 206);
        assert_eq!(StatusReport::parse(&[255]).status, Status::NoReply);
        assert_eq!(StatusReport::parse(&[]).status, Status::NoReply);
        assert_eq!(StatusReport::parse(&[255, 238]).status, Status::Busy);
    }

    #[test]
    fn moshiboard_codes_are_recognized() {
        assert!(looks_like_moshiboard(205));
        assert!(looks_like_moshiboard(237));
        for code in [206u8, 238, 207, 236, 239, 204] {
            assert!(!looks_like_moshiboard(code));
        }
    }
}
