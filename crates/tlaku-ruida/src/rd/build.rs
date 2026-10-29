//! Construcción del flujo de comandos: cabecera, capas, cuerpo y pie.

use super::plan::{LayerPlan, plan_layers};
use super::{ELEMENT_NAME, FileChecksum, RdError, RuidaJobOptions, encode_element_name};
use crate::cmd::{Command, Limit, ProcessOp};
use crate::num::{self, NumError};
use crate::safety;
use tlaku_core::toolpath::Toolpath;

fn point2(op: u8, sub: u8, p: (i64, i64)) -> Command {
    Command::Point2 {
        op,
        sub,
        x: p.0,
        y: p.1,
    }
}

fn part_point(sub: u8, part: u8, p: (i64, i64)) -> Command {
    Command::PartPoint {
        sub,
        part,
        x: p.0,
        y: p.1,
    }
}

/// Matriz de una sola copia (1 columna x 1 fila). RDWorks escribe el tamaño del dibujo como paso
/// (con signos que cambian de un archivo a otro), pero con 1x1 el paso no se usa; MeerK40t
/// escribe pasos arbitrarios y funciona. Aquí 0.
fn single_copy(op: u8, sub: u8) -> Command {
    Command::Copies {
        op,
        sub,
        cols: 1,
        rows: 1,
        step_x: 0,
        step_y: 0,
    }
}

/// Potencia mínima y máxima de cada uno de los cuatro láseres para una capa: la de la capa en el
/// láser que usa y la secundaria en los demás (0 % por defecto, para que un láser no elegido no
/// dispare).
fn laser_powers(p: &LayerPlan, sec: u16) -> [(u8, u16, u16); 4] {
    let own = (p.power_min, p.power_max);
    let other = (sec, sec);
    let (l1, l2) = if p.laser2 { (other, own) } else { (own, other) };
    [
        (1, l1.0, l1.1),
        (2, l2.0, l2.1),
        (3, sec, sec),
        (4, sec, sec),
    ]
}

/// Instrucciones de movimiento de una capa: relativas si el salto cabe, absolutas si no. La
/// primera de cada capa es siempre absoluta y la posición se lleva en enteros (sin deriva).
/// En las capas de potencia variable, `C7 P` precede a cada corte cuya potencia cambia.
fn emit_moves(plan: &LayerPlan, opts: &RuidaJobOptions, out: &mut Vec<Command>) {
    let mut pos: Option<(i64, i64)> = None;
    let mut imm: Option<u16> = None;
    for s in &plan.steps {
        if plan.variable && s.cut && imm != Some(s.power) {
            out.push(Command::ImmPower(s.power));
            imm = Some(s.power);
        }
        let rel = match pos {
            Some((px, py)) if opts.use_relative => {
                let (dx, dy) = (s.x - px, s.y - py);
                let fits =
                    |d: i64| (i64::from(num::REL_MIN)..=i64::from(num::REL_MAX)).contains(&d);
                (fits(dx) && fits(dy)).then_some((dx as i32, dy as i32))
            }
            _ => None,
        };
        out.push(match (rel, s.cut) {
            (Some((dx, 0)), cut) if opts.use_axis_shortcuts => {
                if cut {
                    Command::CutRelX(dx)
                } else {
                    Command::MoveRelX(dx)
                }
            }
            (Some((0, dy)), cut) if opts.use_axis_shortcuts => {
                if cut {
                    Command::CutRelY(dy)
                } else {
                    Command::MoveRelY(dy)
                }
            }
            (Some((dx, dy)), true) => Command::CutRel { dx, dy },
            (Some((dx, dy)), false) => Command::MoveRel { dx, dy },
            (None, true) => Command::CutAbs { x: s.x, y: s.y },
            (None, false) => Command::MoveAbs { x: s.x, y: s.y },
        });
        pos = Some((s.x, s.y));
    }
}

/// Longitud de corte en mm (parte entera) a partir de los pasos de todas las capas.
fn cut_length_mm(plans: &[LayerPlan]) -> u64 {
    let mut total = 0.0f64;
    let mut pos: Option<(i64, i64)> = None;
    for p in plans {
        for s in &p.steps {
            if let (true, Some((px, py))) = (s.cut, pos) {
                total += ((s.x - px) as f64).hypot((s.y - py) as f64);
            }
            pos = Some((s.x, s.y));
        }
    }
    (total / 1000.0).floor() as u64
}

/// Genera la lista de comandos del trabajo (sin bytes ni swizzle).
#[allow(clippy::vec_init_then_push)]
pub fn build_commands(tp: &Toolpath, opts: &RuidaJobOptions) -> Result<Vec<Command>, RdError> {
    if opts.reference.is_relative() {
        safety::check_toolpath_relative(tp, &opts.limits)?;
    } else {
        safety::check_toolpath(tp, &opts.limits)?;
    }
    let sec = num::power_from_pct(opts.secondary_laser_power_pct)?;
    if opts.laser_tube_start > 0x7F {
        return Err(RdError::BadOption("laser_tube_start debe caber en 7 bits"));
    }
    let name = encode_element_name(ELEMENT_NAME).expect("nombre válido");
    let planned = plan_layers(tp, opts)?;
    planned.check_limit()?;
    let plans = planned.layers;
    let mut job_box = plans[0].bbox;
    for p in &plans[1..] {
        job_box.union(&p.bbox);
    }
    let mut c: Vec<Command> = Vec::new();

    // 1. Arranque. `D8 00` va siempre: está en todos los archivos de RDWorks, en la captura
    // de Nova 35 y en lo que envían MeerK40t y ruida-pa (verificado en RDC6442S).
    // `D8 10` (cero de la máquina), `D8 11` (origen del usuario) o `D8 12` (posición actual).
    c.push(Command::Process(opts.reference.process_op()));
    c.push(Command::SetReference);
    c.push(Command::Absolute);
    c.push(Command::Small {
        op: 0xF1,
        sub: 0x02,
        n: 0,
    });
    c.push(Command::Process(ProcessOp::Start));

    // 2. Cajas del trabajo.
    c.push(point2(0xE7, 0x06, (0, 0)));
    c.push(point2(0xE7, 0x03, job_box.min));
    c.push(point2(0xE7, 0x07, job_box.max));
    c.push(point2(0xE7, 0x50, job_box.min));
    c.push(point2(0xE7, 0x51, job_box.max));
    c.push(single_copy(0xE7, 0x04));
    c.push(Command::Small {
        op: 0xE7,
        sub: 0x05,
        n: 0,
    });

    // 3. Cabecera de cada capa.
    for p in &plans {
        let l = p.index;
        c.push(Command::LayerSpeed {
            layer: l,
            um_s: p.speed_um_s,
        });
        for (laser, mn, mx) in laser_powers(p, sec) {
            // Orden del ejemplo: 1 mín, 1 máx, 2 mín, 2 máx, 3 mín, 3 máx, 4 mín, 4 máx.
            c.push(Command::LayerPower {
                laser,
                limit: Limit::Min,
                layer: l,
                raw: mn,
            });
            c.push(Command::LayerPower {
                laser,
                limit: Limit::Max,
                layer: l,
                raw: mx,
            });
        }
        c.push(Command::LayerColor {
            layer: l,
            color: p.color,
        });
        c.push(Command::LayerMode { layer: l, mode: 0 });
        c.push(part_point(0x52, l, p.bbox.min));
        c.push(part_point(0x53, l, p.bbox.max));
        c.push(part_point(0x61, l, p.bbox.min));
        c.push(part_point(0x62, l, p.bbox.max));
    }

    // 4. Máximo de capa, offsets (todos a 0) y bloque de elemento.
    c.push(Command::MaxLayer(plans.len() as u8 - 1));
    for sub in [0x54, 0x55] {
        for n in 0..2u8 {
            c.push(Command::PartAbs { sub, n, v: 0 });
        }
    }
    c.push(point2(0xF1, 0x03, (0, 0)));
    for (op, sub) in [(0xF1, 0x00), (0xF1, 0x01), (0xF2, 0x00), (0xF2, 0x01)] {
        c.push(Command::Small { op, sub, n: 0 });
    }
    c.push(Command::ElementName(name));
    c.push(point2(0xF2, 0x03, job_box.min));
    c.push(point2(0xF2, 0x04, job_box.max));
    c.push(point2(0xF2, 0x06, (0, 0)));
    c.push(Command::Small {
        op: 0xF2,
        sub: 0x07,
        n: 0,
    });
    c.push(single_copy(0xF2, 0x05));

    // 5. Bloque de elemento del cuerpo.
    c.push(Command::ShortArg { op: 0xEA, n: 0 });
    c.push(Command::Small {
        op: 0xE7,
        sub: 0x60,
        n: 0,
    });
    c.push(point2(0xE7, 0x13, job_box.min));
    c.push(point2(0xE7, 0x17, job_box.max));
    c.push(point2(0xE7, 0x23, (0, 0)));
    c.push(Command::Small {
        op: 0xE7,
        sub: 0x24,
        n: 0,
    });
    c.push(single_copy(0xE7, 0x08));

    // 6. Cuerpo de cada capa.
    for p in &plans {
        c.push(Command::EndLayer);
        c.push(Command::LayerSelect(p.index));
        c.push(Command::LayerFlag(0x30));
        // `CA 01 10` / `CA 01 11`: dispositivo (láser) de la capa, 0 o 1 ("Layer Device" en
        // EduTech, `LASER_DEVICE_0/1` en ruida-pa y MeerK40t; no marcado como verificado).
        c.push(Command::LayerFlag(if p.laser2 { 0x11 } else { 0x10 }));
        c.push(if p.air {
            Command::AirOn
        } else {
            Command::AirOff
        });
        c.push(Command::Speed(p.speed_um_s));
        // Retardos de encendido y apagado del láser (`C6 12/13`), a 0: los que escriben
        // MeerK40t y los archivos de RDWorks. El ejemplo de EduTech usa `C6 15/16`.
        c.push(Command::LaserTime {
            sub: 0x12,
            micros: 0,
        });
        c.push(Command::LaserTime {
            sub: 0x13,
            micros: 0,
        });
        for (laser, mn, mx) in laser_powers(p, sec) {
            c.push(Command::LaserPower {
                laser,
                limit: Limit::Min,
                raw: mn,
            });
            c.push(Command::LaserPower {
                laser,
                limit: Limit::Max,
                raw: mx,
            });
        }
        // `CA 03 n`: máscara de tubos activos (1 = solo el láser 1 en MeerK40t; 0x0F en RDWorks
        // con cuatro). Una capa del láser 2 añade el bit 1; es una inferencia de la forma de
        // máscara, sin verificar en una máquina de dos láseres.
        c.push(Command::Small {
            op: 0xCA,
            sub: 0x03,
            n: if p.laser2 {
                opts.laser_tube_start | 0x02
            } else {
                opts.laser_tube_start
            },
        });
        c.push(Command::Small {
            op: 0xCA,
            sub: 0x10,
            n: 0x00,
        });
        emit_moves(p, opts, &mut c);
    }

    // 7. Pie.
    c.push(Command::Marker(0xEB));
    c.push(Command::EndBlock);
    if opts.footer_stats {
        let len = cut_length_mm(&plans);
        c.push(Command::MemWrite {
            addr: 0x0320,
            values: vec![len, len],
        });
    }
    if opts.file_checksum == FileChecksum::SumAll {
        let sum = sum_bytes(&c)? + 0xD7;
        c.push(Command::FileChecksum(sum));
    }
    c.push(Command::EndOfFile);
    Ok(c)
}

/// Suma de los bytes de los comandos (sin lecturas `DA 00`, que no entran en la suma).
fn sum_bytes(cmds: &[Command]) -> Result<u64, NumError> {
    let mut sum = 0u64;
    for c in cmds {
        if matches!(c, Command::MemRead { .. }) {
            continue;
        }
        sum += c.to_bytes()?.iter().map(|&b| u64::from(b)).sum::<u64>();
    }
    Ok(sum)
}
