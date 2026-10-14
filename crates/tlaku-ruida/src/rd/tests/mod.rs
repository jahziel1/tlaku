//! Pruebas del generador de `.rd`, agrupadas por tema.

mod example;
mod layers;
mod power;

use super::*;
use crate::cmd::{self, Command};
use tlaku_core::Point;
use tlaku_core::toolpath::Move;
use tlaku_core::toolpath::ToolpathBlock;

/// Ejemplo 1 de EduTech (ruida.md sección 5), bytes sin swizzle.
const EXAMPLE_1: &str = "
        d810 f0 e601 f10200 d800
        e70600000000000000000000
        e70300000b51120000021258
        e707000011164c0000064e02
        e75000000b51120000021258
        e751000011164c0000064e02
        e7040001000100000000000000000000
        e70500
        c904000000060d20
        c631001319 c632001828
        c641002632 c642002632
        c635002632 c636002632 c637002632 c638002632
        ca06000000000000
        ca410000
        e7520000000b51120000021258 e75300000011164c0000064e02
        e7610000000b51120000021258 e76200000011164c0000064e02
        ca2200
        e754000000030650 e754010000030650
        e755000000000000 e755010000000000
        f10300000306500000030650
        f10000 f10100 f20000 f20100
        f202052a391c41046a150820
        f20300000b51120000021255 f20400001116500000064e03
        f20600000000000000000000 f20700
        f205000100017f7f7a3a430000043b2d
        ea00 e76000 e71300000b51120000021255 e71700001116500000064e03
        e72300000000000000000000 e72400
        e708000100017f7f7a3a430000043b2d
        ca0100 ca0200 ca0130 ca0110 ca0113
        c9020000060d20
        c6150000000000 c6160000000000
        c6011319 c6021828 c6212632 c6222632 c6052632 c6062632 c6072632 c6082632
        ca030d ca1000
        8800001116500000021256
        a800000b51120000021256
        a800000b51120000064e03
        a800001116500000064e03
        a800001116500000021256
        eb e700
        da01062000000002470000000247
        d7";

fn hex(s: &str) -> Vec<u8> {
    let s: String = s.split_whitespace().collect();
    (0..s.len() / 2)
        .map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap())
        .collect()
}

/// Opciones sin cambio de ejes (home abajo a la izquierda): coordenadas = las del `Toolpath`.
fn dflt() -> RuidaJobOptions {
    RuidaJobOptions {
        home: HomeCorner::BottomLeft,
        ..Default::default()
    }
}

fn pct(raw: u16) -> f64 {
    f64::from(raw) * 100.0 / 16383.0
}

fn rect_block(power: f64) -> ToolpathBlock {
    let c = |x: f64, y: f64| Move::Cut {
        to: Point::new(x, y),
        power_pct: power,
    };
    ToolpathBlock {
        label: "rectángulo".into(),
        speed_mm_min: 6000.0,
        air_assist: true,
        moves: vec![
            Move::Travel {
                to: Point::new(281.424, 35.158),
            },
            c(190.610, 35.158),
            c(190.610, 108.291),
            c(281.424, 108.291),
            c(281.424, 35.158),
        ],
        ..Default::default()
    }
}

fn example_options() -> RuidaJobOptions {
    RuidaJobOptions {
        layers: vec![LayerOptions {
            color: [0, 0, 0],
            min_power_pct: Some(pct(2457)),
        }],
        secondary_laser_power_pct: pct(4914),
        footer_stats: true,
        ..dflt()
    }
}

/// El ejemplo 1 con las diferencias **intencionadas** de Tlaku aplicadas, para poder
/// compararlo con lo que genera (ver ruida.md sección 12): offsets de pluma y de pantalla
/// a 0, retardos `C6 12/13` en vez de `C6 15/16`, `CA 03 01` en vez de `0D` y matrices 1x1
/// con pasos 0.
fn example_with_tlaku_choices() -> Vec<Command> {
    cmd::decode_stream(&hex(EXAMPLE_1))
        .unwrap()
        .into_iter()
        .map(|c| match c {
            Command::PartAbs { sub, n, .. } => Command::PartAbs { sub, n, v: 0 },
            Command::Point2 {
                op: 0xF1,
                sub: 0x03,
                ..
            } => Command::Point2 {
                op: 0xF1,
                sub: 0x03,
                x: 0,
                y: 0,
            },
            Command::LaserTime { sub, micros } => Command::LaserTime {
                sub: sub - 3,
                micros,
            },
            Command::Small {
                op: 0xCA,
                sub: 0x03,
                ..
            } => Command::Small {
                op: 0xCA,
                sub: 0x03,
                n: 1,
            },
            Command::Copies {
                op,
                sub,
                cols,
                rows,
                ..
            } if sub != 0x04 => Command::Copies {
                op,
                sub,
                cols,
                rows,
                step_x: 0,
                step_y: 0,
            },
            other => other,
        })
        .collect()
}
