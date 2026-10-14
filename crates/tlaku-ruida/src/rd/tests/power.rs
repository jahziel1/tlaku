//! Pruebas de potencia variable, movimientos relativos y esperas.

use super::*;
use crate::cmd::{Command, Limit};
use crate::num;
use tlaku_core::Point;
use tlaku_core::toolpath::{Move, Toolpath};

/// Dos filas de un ráster: overscan a potencia 0, tramos de potencia distinta y overscan.
fn raster_block() -> ToolpathBlock {
    let cut = |x: f64, y: f64, p: f64| Move::Cut {
        to: Point::new(x, y),
        power_pct: p,
    };
    let travel = |x: f64, y: f64| Move::Travel {
        to: Point::new(x, y),
    };
    ToolpathBlock {
        label: "ráster".into(),
        speed_mm_min: 6000.0,
        moves: vec![
            travel(8.0, 10.0),
            cut(10.0, 10.0, 0.0),
            cut(11.0, 10.0, 40.0),
            cut(12.0, 10.0, 80.0),
            cut(14.0, 10.0, 0.0),
            travel(14.0, 10.1),
            cut(12.0, 10.1, 0.0),
            cut(11.0, 10.1, 80.0),
            cut(10.0, 10.1, 40.0),
            cut(8.0, 10.1, 0.0),
        ],
        ..Default::default()
    }
}

#[test]
fn variable_power_uses_imd_power_and_zero_power_travels() {
    let tp = Toolpath {
        blocks: vec![raster_block()],
    };
    let cmds = build_commands(&tp, &dflt()).unwrap();
    // Capa con mín 0 y máx = la mayor potencia (80 %).
    let hi = num::power_from_pct(80.0).unwrap();
    let lp = |limit: Limit| {
        cmds.iter()
            .find_map(|c| match c {
                Command::LayerPower {
                    laser: 1,
                    limit: l,
                    raw,
                    ..
                } if *l == limit => Some(*raw),
                _ => None,
            })
            .unwrap()
    };
    assert_eq!((lp(Limit::Min), lp(Limit::Max)), (0, hi));
    // C7 solo cuando cambia y nunca para potencia 0 (los tramos de overscan son saltos).
    let imm: Vec<u16> = cmds
        .iter()
        .filter_map(|c| match c {
            Command::ImmPower(r) => Some(*r),
            _ => None,
        })
        .collect();
    let (p40, p80) = (
        num::power_from_pct(40.0).unwrap(),
        num::power_from_pct(80.0).unwrap(),
    );
    assert_eq!(imm, [p40, p80, p40]);
    // Cada corte va precedido por el C7 vigente: se reconstruye la potencia de cada tramo.
    let (mut cur, mut got) = (None, Vec::new());
    for c in &cmds {
        match c {
            Command::ImmPower(r) => cur = Some(*r),
            Command::CutRel { .. } | Command::CutAbs { .. } => got.push(cur.unwrap()),
            _ => {}
        }
    }
    assert_eq!(got, [p40, p80, p80, p40]);
    // Los tramos a potencia 0 salen como saltos.
    let travels = cmds
        .iter()
        .filter(|c| matches!(c, Command::MoveAbs { .. } | Command::MoveRel { .. }))
        .count();
    assert_eq!(travels, 6);
    // Con `ZeroPower::Cut` el tramo se corta con C7 0.
    let o = RuidaJobOptions {
        zero_power: ZeroPower::Cut,
        ..dflt()
    };
    let cmds = build_commands(&tp, &o).unwrap();
    assert!(cmds.contains(&Command::ImmPower(0)));
    // Solo quedan los dos saltos de verdad (el primero y el del cambio de fila).
    let moves = cmds
        .iter()
        .filter(|c| matches!(c, Command::MoveAbs { .. } | Command::MoveRel { .. }))
        .count();
    assert_eq!(moves, 2);
    build_rd(&tp, &o).unwrap();
    build_rd(&tp, &dflt()).unwrap();
}

#[test]
fn constant_power_blocks_do_not_use_imd_power() {
    let tp = Toolpath {
        blocks: vec![rect_block(50.0)],
    };
    let cmds = build_commands(&tp, &dflt()).unwrap();
    assert!(!cmds.iter().any(|c| matches!(c, Command::ImmPower(_))));
}

#[test]
fn relative_moves_used_when_they_fit_and_never_drift() {
    // Una polilínea con saltos de 3 mm y uno de 20 mm; todo en µm exactos.
    let pts = [
        (10.0, 10.0),
        (13.0, 10.0),
        (13.0, 13.0),
        (33.0, 13.0),
        (33.0123, 13.0456),
        (30.0, 10.0),
    ];
    let moves: Vec<Move> = pts
        .iter()
        .enumerate()
        .map(|(i, &(x, y))| {
            let to = Point::new(x, y);
            if i == 0 {
                Move::Travel { to }
            } else {
                Move::Cut {
                    to,
                    power_pct: 20.0,
                }
            }
        })
        .collect();
    let tp = Toolpath {
        blocks: vec![ToolpathBlock {
            speed_mm_min: 1200.0,
            moves,
            ..Default::default()
        }],
    };
    let cmds = build_commands(&tp, &dflt()).unwrap();
    let body: Vec<&Command> = cmds
        .iter()
        .filter(|c| {
            matches!(
                c,
                Command::MoveAbs { .. }
                    | Command::CutAbs { .. }
                    | Command::MoveRel { .. }
                    | Command::CutRel { .. }
            )
        })
        .collect();
    assert!(matches!(body[0], Command::MoveAbs { .. }));
    assert!(matches!(body[1], Command::CutRel { dx: 3000, dy: 0 }));
    assert!(matches!(body[2], Command::CutRel { dx: 0, dy: 3000 }));
    // 20 mm no cabe en 14 bits (+-8,191 mm): absoluto.
    assert!(matches!(
        body[3],
        Command::CutAbs {
            x: 33_000,
            y: 13_000
        }
    ));
    assert!(matches!(body[4], Command::CutRel { dx: 12, dy: 46 }));
    // Reconstrucción por suma: llega exactamente al último punto en µm.
    let (mut x, mut y) = (0i64, 0i64);
    for c in &body {
        match c {
            Command::MoveAbs { x: a, y: b } | Command::CutAbs { x: a, y: b } => (x, y) = (*a, *b),
            Command::MoveRel { dx, dy } | Command::CutRel { dx, dy } => {
                x += i64::from(*dx);
                y += i64::from(*dy);
            }
            _ => unreachable!(),
        }
    }
    assert_eq!((x, y), (30_000, 10_000));
    // Sin relativos si se pide.
    let abs_only = RuidaJobOptions {
        use_relative: false,
        ..dflt()
    };
    let cmds = build_commands(&tp, &abs_only).unwrap();
    assert!(!cmds.iter().any(|c| matches!(c, Command::CutRel { .. })));
}

#[test]
fn axis_shortcuts_are_optional() {
    let tp = Toolpath {
        blocks: vec![ToolpathBlock {
            speed_mm_min: 1200.0,
            moves: vec![
                Move::Travel {
                    to: Point::new(1.0, 1.0),
                },
                Move::Cut {
                    to: Point::new(2.0, 1.0),
                    power_pct: 10.0,
                },
                Move::Cut {
                    to: Point::new(2.0, 2.0),
                    power_pct: 10.0,
                },
            ],
            ..Default::default()
        }],
    };
    let o = RuidaJobOptions {
        use_axis_shortcuts: true,
        ..dflt()
    };
    let cmds = build_commands(&tp, &o).unwrap();
    assert!(cmds.contains(&Command::CutRelX(1000)));
    assert!(cmds.contains(&Command::CutRelY(1000)));
    let cmds = build_commands(&tp, &dflt()).unwrap();
    assert!(cmds.contains(&Command::CutRel { dx: 1000, dy: 0 }));
}

#[test]
fn dwells_are_skipped_because_rd_has_no_pause() {
    let line = |dwells: bool| {
        let mut moves = vec![Move::Travel {
            to: Point::new(1.0, 1.0),
        }];
        if dwells {
            moves.push(Move::Dwell {
                to: Point::new(1.0, 1.0),
                ms: 50.0,
                power_pct: 80.0,
            });
        }
        moves.push(Move::Cut {
            to: Point::new(2.0, 1.0),
            power_pct: 10.0,
        });
        if dwells {
            moves.push(Move::Dwell {
                to: Point::new(2.0, 1.0),
                ms: 50.0,
                power_pct: 10.0,
            });
        }
        Toolpath {
            blocks: vec![ToolpathBlock {
                speed_mm_min: 1200.0,
                moves,
                ..Default::default()
            }],
        }
    };
    assert_eq!(
        build_commands(&line(true), &dflt()).unwrap(),
        build_commands(&line(false), &dflt()).unwrap()
    );
}
