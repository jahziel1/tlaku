//! Referencia de las coordenadas: `D8 10` (máquina), `D8 11` (origen del usuario) y `D8 12`
//! (posición actual), ruida.md 12.15.

use super::*;
use crate::cmd::ProcessOp;
use crate::safety::{self, SafetyError};

/// Trabajo con el ancla en (0, 0): el cuadrado queda a ambos lados, como con el ancla al centro.
fn centered() -> Toolpath {
    let c = |x: f64, y: f64| Move::Cut {
        to: Point::new(x, y),
        power_pct: 20.0,
    };
    Toolpath {
        blocks: vec![ToolpathBlock {
            speed_mm_min: 3000.0,
            moves: vec![
                Move::Travel {
                    to: Point::new(-10.0, -5.0),
                },
                c(10.0, -5.0),
                c(10.0, 5.0),
                c(-10.0, 5.0),
                c(-10.0, -5.0),
            ],
            ..Default::default()
        }],
    }
}

fn with(reference: RefMode, home: HomeCorner) -> RuidaJobOptions {
    RuidaJobOptions {
        reference,
        home,
        ..Default::default()
    }
}

fn points(cmds: &[Command]) -> Vec<(i64, i64)> {
    let mut out = Vec::new();
    let (mut x, mut y) = (0i64, 0i64);
    for c in cmds {
        match *c {
            Command::MoveAbs { x: a, y: b } | Command::CutAbs { x: a, y: b } => (x, y) = (a, b),
            Command::MoveRel { dx, dy } | Command::CutRel { dx, dy } => {
                x += i64::from(dx);
                y += i64::from(dy);
            }
            _ => continue,
        }
        out.push((x, y));
    }
    out
}

#[test]
fn each_mode_writes_its_own_reference_command() {
    for (mode, op) in [
        (RefMode::Machine, ProcessOp::RefPoint),
        (RefMode::UserOrigin, ProcessOp::RefOrigin),
        (RefMode::Current, ProcessOp::RefCurrent),
    ] {
        let tp = if mode.is_relative() {
            centered()
        } else {
            Toolpath {
                blocks: vec![rect_block(10.0)],
            }
        };
        let cmds = build_commands(&tp, &with(mode, HomeCorner::BottomLeft)).unwrap();
        assert_eq!(cmds[0], Command::Process(op));
        assert_eq!(cmds[1], Command::SetReference);
        assert_eq!(cmds[4], Command::Process(ProcessOp::Start));
    }
}

#[test]
fn relative_modes_keep_the_anchor_at_zero_and_allow_negative_coordinates() {
    for mode in [RefMode::Current, RefMode::UserOrigin] {
        let cmds = build_commands(&centered(), &with(mode, HomeCorner::BottomLeft)).unwrap();
        let pts = points(&cmds);
        assert_eq!(pts[0], (-10_000, -5_000));
        assert_eq!(pts[2], (10_000, 5_000));
        // Las cajas del trabajo y del documento van en el mismo sistema relativo.
        let enc = |c: &Command| c.to_bytes().unwrap();
        let box_min = cmds
            .iter()
            .find(|c| enc(c).starts_with(&[0xE7, 0x03]))
            .unwrap();
        let expect = Command::Point2 {
            op: 0xE7,
            sub: 0x03,
            x: -10_000,
            y: -5_000,
        };
        assert_eq!(*box_min, expect);
    }
}

#[test]
fn relative_modes_flip_the_axes_by_the_home_corner_without_the_bed_size() {
    // Home arriba a la izquierda: Y del archivo = -Y de Tlaku (hacia abajo), X igual.
    let cmds = build_commands(&centered(), &with(RefMode::Current, HomeCorner::TopLeft)).unwrap();
    let pts = points(&cmds);
    assert_eq!(pts[0], (-10_000, 5_000));
    assert_eq!(pts[2], (10_000, -5_000));
    // Home abajo a la derecha: X al revés.
    let cmds = build_commands(
        &centered(),
        &with(RefMode::Current, HomeCorner::BottomRight),
    )
    .unwrap();
    let pts = points(&cmds);
    assert_eq!(pts[0], (10_000, -5_000));
}

#[test]
fn machine_mode_still_adds_the_bed_size() {
    let o = with(RefMode::Machine, HomeCorner::TopLeft);
    let tp = Toolpath {
        blocks: vec![rect_block(10.0)],
    };
    let pts = points(&build_commands(&tp, &o).unwrap());
    assert_eq!(pts[0], (281_424, 400_000 - 35_158));
}

#[test]
fn relative_job_is_checked_by_its_extent_not_by_absolute_position() {
    // Coordenadas negativas: válidas en modo relativo, fuera de la cama en modo máquina.
    assert!(build_rd(&centered(), &with(RefMode::Current, HomeCorner::BottomLeft)).is_ok());
    assert!(matches!(
        build_rd(&centered(), &with(RefMode::Machine, HomeCorner::BottomLeft)),
        Err(RdError::Safety(SafetyError::OutOfBed { .. }))
    ));
    // Un trabajo más ancho que la cama no cabe, vaya donde vaya el cabezal.
    let wide = Toolpath {
        blocks: vec![ToolpathBlock {
            speed_mm_min: 3000.0,
            moves: vec![
                Move::Travel {
                    to: Point::new(-100.0, 0.0),
                },
                Move::Cut {
                    to: Point::new(550.0, 0.0),
                    power_pct: 10.0,
                },
            ],
            ..Default::default()
        }],
    };
    assert!(matches!(
        build_rd(&wide, &with(RefMode::Current, HomeCorner::BottomLeft)),
        Err(RdError::Safety(SafetyError::OutOfBed { .. }))
    ));
}

#[test]
fn stream_check_uses_the_span_when_the_stream_is_relative() {
    let limits = safety::SafetyLimits::default();
    let go = |x: i64| {
        [
            Command::Process(ProcessOp::RefCurrent),
            Command::MoveAbs { x: -x, y: 0 },
            Command::CutAbs { x, y: 0 },
        ]
    };
    // 2 x 250 mm de recorrido en una cama de 600: cabe.
    safety::check_commands(&go(250_000), &limits, false).unwrap();
    // 2 x 350 mm = 700 mm: no cabe.
    assert!(matches!(
        safety::check_commands(&go(350_000), &limits, false),
        Err(SafetyError::StreamOutOfBed { .. })
    ));
    // Sin `D8 11/12` rige la regla absoluta: un negativo no vale.
    let abs = [Command::MoveAbs { x: -1_000, y: 0 }];
    assert!(matches!(
        safety::check_commands(&abs, &limits, false),
        Err(SafetyError::StreamOutOfBed { .. })
    ));
}
