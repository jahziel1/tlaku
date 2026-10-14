//! Comandos tipados: solo existe un tipo para cada comando que Tlaku envía (`Policy::Sent`).
//!
//! No hay forma de construir desde aquí escritura de EEPROM, de la tabla de corrección ni de
//! parámetros de configuración de la tarjeta: sencillamente no tienen variante. Lo que llega del
//! cable con otro código se decodifica como `Unknown` y `safety` lo rechaza.

use crate::codes::{imm, list};
use crate::wire::Raw;
use std::fmt;

/// Retardo en microsegundos con signo: el valor absoluto va en el primer parámetro y
/// `0x8000` en el segundo si es negativo (galvoplotter y MeerK40t).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Delay {
    pub us: u16,
    pub negative: bool,
}

impl Delay {
    /// Convierte microsegundos, con saturación a ±65535.
    pub fn from_us(us: i64) -> Delay {
        Delay {
            us: us.unsigned_abs().min(u16::MAX as u64) as u16,
            negative: us < 0,
        }
    }

    pub fn signed_us(&self) -> i32 {
        if self.negative {
            -(self.us as i32)
        } else {
            self.us as i32
        }
    }

    fn params(&self) -> [u16; 5] {
        [self.us, if self.negative { 0x8000 } else { 0 }, 0, 0, 0]
    }

    fn from_raw(r: &Raw) -> Delay {
        Delay {
            us: r.p[0],
            negative: r.p[1] & 0x8000 != 0,
        }
    }
}

/// Comando de lista.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListOp {
    /// `listReadyMark`: primer comando de toda lista nueva.
    Ready,
    /// Salto con el láser apagado. `dist` en unidades galvo (saturada).
    JumpTo {
        x: u16,
        y: u16,
        dist: u16,
    },
    /// Marca: movimiento con el láser encendido.
    MarkTo {
        x: u16,
        y: u16,
        dist: u16,
    },
    /// Dispara en el punto actual durante `ticks` x 10 us.
    LaserOnPoint {
        ticks: u16,
    },
    /// Espera de `ticks` x 10 us.
    DelayTime {
        ticks: u16,
    },
    /// Unidades galvo por ms.
    JumpSpeed(u16),
    MarkSpeed(u16),
    LaserOnDelay(Delay),
    LaserOffDelay(Delay),
    JumpDelay(Delay),
    /// Retardo de polígono en unidades de 10 us (ver la discrepancia en `codes`).
    PolygonDelay(u16),
    /// Máscara de salidas.
    WritePort(u16),
    /// Potencia de fibra (12 bits).
    MarkCurrent(u16),
    /// Periodo del Q-switch de fibra.
    QSwitchPeriod(u16),
    /// Periodo de frecuencia de CO2 y UV.
    MarkFreq(u16),
    /// Razón de potencia de CO2 y UV.
    PowerRatio(u16),
    /// First pulse killer de CO2 y UV.
    Co2Fpk(u16),
    /// Ancho de pulso de MOPA (ns, inferido).
    PulseWidth(u16),
    EndOfList,
    /// Cualquier otro código de lista: `safety` lo rechaza.
    Unknown(Raw),
}

impl ListOp {
    pub fn to_raw(&self) -> Raw {
        let r = |code: u16, p: [u16; 5]| Raw::new(code, p);
        let one = |code: u16, v: u16| Raw::new(code, [v, 0, 0, 0, 0]);
        match *self {
            ListOp::Ready => Raw::bare(list::READY_MARK),
            ListOp::JumpTo { x, y, dist } => r(list::JUMP_TO, [x, y, 0, dist, 0]),
            ListOp::MarkTo { x, y, dist } => r(list::MARK_TO, [x, y, 0, dist, 0]),
            ListOp::LaserOnPoint { ticks } => one(list::LASER_ON_POINT, ticks),
            ListOp::DelayTime { ticks } => one(list::DELAY_TIME, ticks),
            ListOp::JumpSpeed(v) => one(list::JUMP_SPEED, v),
            ListOp::MarkSpeed(v) => one(list::MARK_SPEED, v),
            ListOp::LaserOnDelay(d) => r(list::LASER_ON_DELAY, d.params()),
            ListOp::LaserOffDelay(d) => r(list::LASER_OFF_DELAY, d.params()),
            ListOp::JumpDelay(d) => r(list::JUMP_DELAY, d.params()),
            ListOp::PolygonDelay(v) => one(list::POLYGON_DELAY, v),
            ListOp::WritePort(v) => one(list::WRITE_PORT, v),
            ListOp::MarkCurrent(v) => one(list::MARK_CURRENT, v),
            ListOp::QSwitchPeriod(v) => one(list::QSWITCH_PERIOD, v),
            ListOp::MarkFreq(v) => one(list::MARK_FREQ, v),
            ListOp::PowerRatio(v) => one(list::MARK_POWER_RATIO, v),
            ListOp::Co2Fpk(v) => r(list::SET_CO2_FPK, [v, v, 0, 0, 0]),
            ListOp::PulseWidth(v) => one(list::FIBER_YLPM_PULSE_WIDTH, v),
            ListOp::EndOfList => Raw::bare(list::END_OF_LIST),
            ListOp::Unknown(raw) => raw,
        }
    }

    /// Interpreta un comando de 12 bytes. Un comando conocido con parámetros extra distintos de
    /// cero no es un `ListOp` exacto: se devuelve como `Unknown` para que `safety` lo vea.
    pub fn decode(raw: &Raw) -> ListOp {
        let p = raw.p;
        let only = |n: usize| p[n..].iter().all(|&v| v == 0);
        let op = match raw.code {
            list::READY_MARK if only(0) => ListOp::Ready,
            list::JUMP_TO if p[2] == 0 && p[4] == 0 => ListOp::JumpTo {
                x: p[0],
                y: p[1],
                dist: p[3],
            },
            list::MARK_TO if p[2] == 0 && p[4] == 0 => ListOp::MarkTo {
                x: p[0],
                y: p[1],
                dist: p[3],
            },
            list::LASER_ON_POINT if only(1) => ListOp::LaserOnPoint { ticks: p[0] },
            list::DELAY_TIME if only(1) => ListOp::DelayTime { ticks: p[0] },
            list::JUMP_SPEED if only(1) => ListOp::JumpSpeed(p[0]),
            list::MARK_SPEED if only(1) => ListOp::MarkSpeed(p[0]),
            list::LASER_ON_DELAY if only(2) => ListOp::LaserOnDelay(Delay::from_raw(raw)),
            list::LASER_OFF_DELAY if only(2) => ListOp::LaserOffDelay(Delay::from_raw(raw)),
            list::JUMP_DELAY if only(2) => ListOp::JumpDelay(Delay::from_raw(raw)),
            list::POLYGON_DELAY if only(1) => ListOp::PolygonDelay(p[0]),
            list::WRITE_PORT if only(1) => ListOp::WritePort(p[0]),
            list::MARK_CURRENT if only(1) => ListOp::MarkCurrent(p[0]),
            list::QSWITCH_PERIOD if only(1) => ListOp::QSwitchPeriod(p[0]),
            list::MARK_FREQ if only(1) => ListOp::MarkFreq(p[0]),
            list::MARK_POWER_RATIO if only(1) => ListOp::PowerRatio(p[0]),
            list::SET_CO2_FPK if p[0] == p[1] && only(2) => ListOp::Co2Fpk(p[0]),
            list::FIBER_YLPM_PULSE_WIDTH if only(1) => ListOp::PulseWidth(p[0]),
            list::END_OF_LIST if only(0) => ListOp::EndOfList,
            _ => ListOp::Unknown(*raw),
        };
        // Un código conocido con parámetros inesperados deja de serlo: así `Unknown` los marca.
        if op.to_raw() == *raw {
            op
        } else {
            ListOp::Unknown(*raw)
        }
    }
}

/// Comando inmediato. Solo los que Tlaku envía.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Imm {
    DisableLaser,
    EnableLaser,
    ExecuteList,
    GetVersion,
    GetSerialNo,
    GetListStatus,
    GetPositionXY,
    GotoXY {
        x: u16,
        y: u16,
        dist: u16,
    },
    ResetList,
    SetEndOfList(u16),
    StopExecute,
    /// Salidas; `safety` solo deja pasar 0, el bit del láser o el del puntero rojo.
    WritePort(u16),
    ReadPort,
    /// 0 cierra y 1 abre el oscilador maestro (fibra).
    FiberSetMo(u16),
    /// Cualquier otro código inmediato: nunca se envía.
    Unknown(Raw),
}

impl Imm {
    pub fn to_raw(&self) -> Raw {
        let one = |code: u16, v: u16| Raw::new(code, [v, 0, 0, 0, 0]);
        match *self {
            Imm::DisableLaser => Raw::bare(imm::DISABLE_LASER),
            Imm::EnableLaser => Raw::bare(imm::ENABLE_LASER),
            Imm::ExecuteList => Raw::bare(imm::EXECUTE_LIST),
            Imm::GetVersion => Raw::new(imm::GET_VERSION, [1, 0, 0, 0, 0]),
            Imm::GetSerialNo => Raw::bare(imm::GET_SERIAL_NO),
            Imm::GetListStatus => Raw::bare(imm::GET_LIST_STATUS),
            Imm::GetPositionXY => Raw::bare(imm::GET_POSITION_XY),
            Imm::GotoXY { x, y, dist } => Raw::new(imm::GOTO_XY, [x, y, 0, dist, 0]),
            Imm::ResetList => Raw::bare(imm::RESET_LIST),
            Imm::SetEndOfList(v) => one(imm::SET_END_OF_LIST, v),
            Imm::StopExecute => Raw::bare(imm::STOP_EXECUTE),
            Imm::WritePort(v) => one(imm::WRITE_PORT, v),
            Imm::ReadPort => Raw::bare(imm::READ_PORT),
            Imm::FiberSetMo(v) => one(imm::FIBER_SET_MO, v),
            Imm::Unknown(raw) => raw,
        }
    }

    pub fn decode(raw: &Raw) -> Imm {
        let p = raw.p;
        let only = |n: usize| p[n..].iter().all(|&v| v == 0);
        let op = match raw.code {
            imm::DISABLE_LASER if only(0) => Imm::DisableLaser,
            imm::ENABLE_LASER if only(0) => Imm::EnableLaser,
            imm::EXECUTE_LIST if only(0) => Imm::ExecuteList,
            imm::GET_VERSION if p[0] == 1 && only(1) => Imm::GetVersion,
            imm::GET_SERIAL_NO if only(0) => Imm::GetSerialNo,
            imm::GET_LIST_STATUS if only(0) => Imm::GetListStatus,
            imm::GET_POSITION_XY if only(0) => Imm::GetPositionXY,
            imm::GOTO_XY if p[2] == 0 && p[4] == 0 => Imm::GotoXY {
                x: p[0],
                y: p[1],
                dist: p[3],
            },
            imm::RESET_LIST if only(0) => Imm::ResetList,
            imm::SET_END_OF_LIST if only(1) => Imm::SetEndOfList(p[0]),
            imm::STOP_EXECUTE if only(0) => Imm::StopExecute,
            imm::WRITE_PORT if only(1) => Imm::WritePort(p[0]),
            imm::READ_PORT if only(0) => Imm::ReadPort,
            imm::FIBER_SET_MO if only(1) => Imm::FiberSetMo(p[0]),
            _ => Imm::Unknown(*raw),
        };
        if op.to_raw() == *raw {
            op
        } else {
            Imm::Unknown(*raw)
        }
    }
}

/// Una línea legible de un comando, para volcados y mensajes (`0x8005 listMarkTo 8000 8000 ...`).
pub struct Describe<'a>(pub &'a Raw);

impl fmt::Display for Describe<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let r = self.0;
        write!(f, "{:04x} {:<24}", r.code, crate::codes::name(r.code))?;
        let last = r.p.iter().rposition(|&v| v != 0).map_or(0, |i| i + 1);
        for v in &r.p[..last] {
            write!(f, " {v:04x}")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all_list_ops() -> Vec<ListOp> {
        let d = |us| Delay::from_us(us);
        vec![
            ListOp::Ready,
            ListOp::JumpTo {
                x: 0x8000,
                y: 1,
                dist: 99,
            },
            ListOp::MarkTo {
                x: 65535,
                y: 0,
                dist: 65535,
            },
            ListOp::LaserOnPoint { ticks: 60000 },
            ListOp::DelayTime { ticks: 5 },
            ListOp::JumpSpeed(2383),
            ListOp::MarkSpeed(1),
            ListOp::LaserOnDelay(d(100)),
            ListOp::LaserOffDelay(d(-200)),
            ListOp::JumpDelay(d(0)),
            ListOp::PolygonDelay(10),
            ListOp::WritePort(1),
            ListOp::MarkCurrent(0xFFF),
            ListOp::QSwitchPeriod(667),
            ListOp::MarkFreq(333),
            ListOp::PowerRatio(120),
            ListOp::Co2Fpk(67),
            ListOp::PulseWidth(4),
            ListOp::EndOfList,
        ]
    }

    #[test]
    fn every_list_op_round_trips() {
        for op in all_list_ops() {
            let raw = op.to_raw();
            assert_eq!(raw.code & 0xFF00, 0x8000, "{op:?}");
            assert_eq!(ListOp::decode(&raw), op, "{op:?}");
        }
    }

    #[test]
    fn every_immediate_round_trips() {
        for op in [
            Imm::DisableLaser,
            Imm::EnableLaser,
            Imm::ExecuteList,
            Imm::GetVersion,
            Imm::GetSerialNo,
            Imm::GetListStatus,
            Imm::GetPositionXY,
            Imm::GotoXY {
                x: 1,
                y: 2,
                dist: 3,
            },
            Imm::ResetList,
            Imm::SetEndOfList(0),
            Imm::StopExecute,
            Imm::WritePort(0x100),
            Imm::ReadPort,
            Imm::FiberSetMo(1),
        ] {
            let raw = op.to_raw();
            assert_eq!(raw.code >> 8, 0, "{op:?}");
            assert_eq!(Imm::decode(&raw), op, "{op:?}");
            assert!(crate::codes::is_sent(raw.code), "{op:?}");
        }
    }

    #[test]
    fn delays_carry_their_sign_in_the_second_word() {
        let neg = ListOp::LaserOnDelay(Delay::from_us(-200)).to_raw();
        assert_eq!(neg.p[..2], [200, 0x8000]);
        let pos = ListOp::LaserOffDelay(Delay::from_us(150)).to_raw();
        assert_eq!(pos.p[..2], [150, 0]);
        assert_eq!(Delay::from_us(-70_000).us, u16::MAX);
        assert_eq!(Delay::from_us(-5).signed_us(), -5);
    }

    #[test]
    fn unknown_codes_and_extra_parameters_stay_unknown() {
        // Un cuarto parámetro en un salto no existe en el protocolo conocido.
        let odd = Raw::new(list::JUMP_TO, [1, 2, 3, 4, 5]);
        assert!(matches!(ListOp::decode(&odd), ListOp::Unknown(_)));
        let eeprom_like = Raw::bare(0x0042);
        assert!(matches!(Imm::decode(&eeprom_like), Imm::Unknown(_)));
        let cor = Raw::new(imm::WRITE_COR_TABLE, [1, 0, 0, 0, 0]);
        assert!(matches!(Imm::decode(&cor), Imm::Unknown(_)));
        assert!(matches!(
            ListOp::decode(&Raw::bare(list::FLY_ENABLE)),
            ListOp::Unknown(_)
        ));
    }

    #[test]
    fn describe_prints_name_and_parameters() {
        let r = ListOp::MarkTo {
            x: 0x8000,
            y: 0x7FFF,
            dist: 0x10,
        }
        .to_raw();
        let text = Describe(&r).to_string();
        assert_eq!(
            text.split_whitespace().collect::<Vec<_>>(),
            ["8005", "listMarkTo", "8000", "7fff", "0000", "0010"]
        );
    }
}
