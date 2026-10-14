//! Texto legible de los comandos (`Display`).

use super::{Command, Limit};
use crate::num;
use std::fmt;

// ----------------------------------------------------------------------------------------
// Texto legible

fn mm(um: i64) -> String {
    format!("{:.3}mm", um as f64 / 1000.0)
}

impl fmt::Display for Command {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use Command::*;
        match self {
            MoveAbs { x, y } => write!(f, "MOVE_ABS x={} y={}", mm(*x), mm(*y)),
            CutAbs { x, y } => write!(f, "CUT_ABS x={} y={}", mm(*x), mm(*y)),
            MoveRel { dx, dy } => write!(f, "MOVE_REL dx={} dy={}", mm(*dx as i64), mm(*dy as i64)),
            CutRel { dx, dy } => write!(f, "CUT_REL dx={} dy={}", mm(*dx as i64), mm(*dy as i64)),
            MoveRelX(d) => write!(f, "MOVE_REL_X dx={}", mm(*d as i64)),
            MoveRelY(d) => write!(f, "MOVE_REL_Y dy={}", mm(*d as i64)),
            CutRelX(d) => write!(f, "CUT_REL_X dx={}", mm(*d as i64)),
            CutRelY(d) => write!(f, "CUT_REL_Y dy={}", mm(*d as i64)),
            LaserPower { laser, limit, raw } => write!(
                f,
                "POWER laser={laser} {} {:.3}%",
                lim(*limit),
                num::power_to_pct(*raw)
            ),
            LayerPower {
                laser,
                limit,
                layer,
                raw,
            } => write!(
                f,
                "LAYER_POWER laser={laser} layer={layer} {} {:.3}%",
                lim(*limit),
                num::power_to_pct(*raw)
            ),
            ImmPower(raw) => write!(f, "IMD_POWER_1 {:.3}%", num::power_to_pct(*raw)),
            EndPower(raw) => write!(f, "END_POWER_1 {:.3}%", num::power_to_pct(*raw)),
            Speed(v) => write!(f, "SPEED {:.3}mm/s", *v as f64 / 1000.0),
            LayerSpeed { layer, um_s } => {
                write!(
                    f,
                    "LAYER_SPEED layer={layer} {:.3}mm/s",
                    *um_s as f64 / 1000.0
                )
            }
            LaserTime { sub, micros } => write!(f, "LASER_TIME sub=0x{sub:02X} {micros}us"),
            LayerFrequency { laser, layer, hz } => {
                write!(f, "LAYER_FREQ laser={laser} layer={layer} {hz}Hz")
            }
            EndLayer => write!(f, "END_LAYER"),
            AirOff => write!(f, "AIR_OFF"),
            AirOn => write!(f, "AIR_ON"),
            LayerFlag(x) => write!(f, "LAYER_FLAG 0x{x:02X}"),
            LayerSelect(l) => write!(f, "LAYER {l}"),
            Color(c) => write!(f, "COLOR 0x{c:06X}"),
            LayerColor { layer, color } => write!(f, "LAYER_COLOR layer={layer} bgr=0x{color:06X}"),
            MaxLayer(l) => write!(f, "MAX_LAYER {l}"),
            LayerMode { layer, mode } => write!(f, "LAYER_MODE layer={layer} mode={mode}"),
            Process(p) => write!(f, "PROCESS {p:?}"),
            EndBlock => write!(f, "END_BLOCK"),
            EndOfFile => write!(f, "END_OF_FILE"),
            Absolute => write!(f, "ABSOLUTE"),
            SetReference => write!(f, "SET_REFERENCE"),
            FileChecksum(s) => write!(f, "FILE_CHECKSUM {s}"),
            FileName(n) => write!(f, "FILE_NAME {n:?}"),
            MemRead { addr } => write!(f, "MEM_READ 0x{addr:03X}"),
            MemWrite { addr, values } => write!(f, "MEM_WRITE 0x{addr:03X} {values:?}"),
            Point2 { op, sub, x, y } => {
                write!(f, "POINT2 {op:02X}.{sub:02X} x={} y={}", mm(*x), mm(*y))
            }
            PartPoint { sub, part, x, y } => write!(
                f,
                "PART_POINT E7.{sub:02X} part={part} x={} y={}",
                mm(*x),
                mm(*y)
            ),
            PartAbs { sub, n, v } => write!(f, "PART_ABS E7.{sub:02X} n={n} v={}", mm(*v)),
            Copies {
                op,
                sub,
                cols,
                rows,
                step_x,
                step_y,
            } => write!(
                f,
                "COPIES {op:02X}.{sub:02X} {cols}x{rows} step_x={} step_y={}",
                mm(*step_x),
                mm(*step_y)
            ),
            Small { op, sub, n } => write!(f, "SMALL {op:02X}.{sub:02X} {n}"),
            Marker(0xEB) => write!(f, "ARRAY_END"),
            Marker(op) => write!(f, "MARKER {op:02X}"),
            ShortArg { op: 0xEA, n } => write!(f, "ARRAY_START {n}"),
            ShortArg { op, n } => write!(f, "SHORT {op:02X} {n}"),
            ElementName(b) => write!(f, "ELEMENT_NAME {:?}", crate::rd::decode_element_name(b)),
            RapidX { rel, v } => write!(f, "RAPID_X {rel:?} {}", mm(*v)),
            RapidY { rel, v } => write!(f, "RAPID_Y {rel:?} {}", mm(*v)),
            RapidXY { rel, x, y } => write!(f, "RAPID_XY {rel:?} x={} y={}", mm(*x), mm(*y)),
            Unknown { op, data } => write!(f, "UNKNOWN {op:02X} {data:02x?}"),
        }
    }
}

fn lim(l: Limit) -> &'static str {
    match l {
        Limit::Min => "min",
        Limit::Max => "max",
    }
}
