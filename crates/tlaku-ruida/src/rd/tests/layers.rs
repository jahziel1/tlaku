//! Pruebas de capas, esquina de home, seguridad y pie del archivo.

use super::*;
use crate::cmd::{self, Command, ProcessOp};
use crate::safety::SafetyError;
use tlaku_core::Point;
use tlaku_core::toolpath::{Move, Toolpath};

#[test]
fn each_block_is_a_layer_and_empty_blocks_are_skipped() {
    let mut b2 = rect_block(30.0);
    b2.speed_mm_min = 3000.0;
    b2.air_assist = false;
    let empty = ToolpathBlock {
        speed_mm_min: 100.0,
        moves: vec![Move::Travel {
            to: Point::new(1.0, 1.0),
        }],
        ..Default::default()
    };
    let tp = Toolpath {
        blocks: vec![rect_block(50.0), empty, b2],
    };
    let o = RuidaJobOptions {
        layers: vec![
            LayerOptions::default(),
            LayerOptions::default(),
            LayerOptions {
                color: [255, 0, 0],
                min_power_pct: None,
            },
        ],
        ..Default::default()
    };
    let cmds = build_commands(&tp, &o).unwrap();
    assert!(cmds.contains(&Command::MaxLayer(1)));
    assert!(cmds.contains(&Command::LayerSelect(0)));
    assert!(cmds.contains(&Command::LayerSelect(1)));
    assert!(cmds.contains(&Command::LayerSpeed {
        layer: 1,
        um_s: 50_000
    }));
    assert!(cmds.contains(&Command::LayerColor {
        layer: 1,
        color: 0xFF
    }));
    let air: Vec<&Command> = cmds
        .iter()
        .filter(|c| matches!(c, Command::AirOn | Command::AirOff))
        .collect();
    assert_eq!(air, [&Command::AirOn, &Command::AirOff]);
}

#[test]
fn home_corner_maps_the_axes_and_offset() {
    let tp = Toolpath {
        blocks: vec![ToolpathBlock {
            speed_mm_min: 1200.0,
            moves: vec![Move::Cut {
                to: Point::new(10.0, 30.0),
                power_pct: 10.0,
            }],
            ..Default::default()
        }],
    };
    // Cama por defecto 600 x 400.
    for (home, x, y) in [
        (HomeCorner::TopLeft, 11_000, 370_500),
        (HomeCorner::TopRight, 591_000, 370_500),
        (HomeCorner::BottomLeft, 11_000, 30_500),
        (HomeCorner::BottomRight, 591_000, 30_500),
    ] {
        let o = RuidaJobOptions {
            home,
            offset_mm: [1.0, 0.5],
            ..Default::default()
        };
        let cmds = build_commands(&tp, &o).unwrap();
        // x' = 10 (o 600 - 10) + 1; y' = 400 - 30 (o 30) + 0.5
        let want = if matches!(home, HomeCorner::TopRight | HomeCorner::BottomRight) {
            x
        } else {
            11_000
        };
        assert!(cmds.contains(&Command::CutAbs { x: want, y }), "{home:?}");
    }
}

#[test]
fn home_corner_roundtrip_and_parse() {
    let bed = [600.0, 400.0];
    for home in [
        HomeCorner::TopLeft,
        HomeCorner::TopRight,
        HomeCorner::BottomLeft,
        HomeCorner::BottomRight,
    ] {
        let p = Point::new(123.4, 56.7);
        let (x, y) = home.to_machine(p, bed);
        let back = home.from_machine(x, y, bed);
        assert!((back.x - p.x).abs() < 1e-9 && (back.y - p.y).abs() < 1e-9);
        assert_eq!(HomeCorner::parse(home.name()), Some(home));
    }
    assert_eq!(HomeCorner::parse("TOP_LEFT"), Some(HomeCorner::TopLeft));
    assert_eq!(HomeCorner::parse("centro"), None);
}

#[test]
fn element_name_matches_every_rdworks_file() {
    // Los bytes de f202... del ejemplo 1 de EduTech y de los demás archivos de RDWorks.
    let bytes = [0x05, 0x2A, 0x39, 0x1C, 0x41, 0x04, 0x6A, 0x15, 0x08, 0x20];
    assert_eq!(encode_element_name(ELEMENT_NAME), Some(bytes));
    assert_eq!(decode_element_name(&bytes), "UNNAMED ");
    assert_eq!(encode_element_name("corto"), None);
    assert_eq!(encode_element_name("ñññññññ1"), None);
}

#[test]
fn rejects_empty_unsafe_and_z() {
    let o = dflt();
    assert_eq!(
        build_rd(&Toolpath::default(), &o).unwrap_err(),
        RdError::EmptyJob
    );
    let mut b = rect_block(120.0);
    assert!(matches!(
        build_rd(
            &Toolpath {
                blocks: vec![b.clone()]
            },
            &o
        ),
        Err(RdError::Safety(SafetyError::Power { .. }))
    ));
    b = rect_block(10.0);
    b.moves.push(Move::Cut {
        to: Point::new(f64::NAN, 1.0),
        power_pct: 10.0,
    });
    assert!(matches!(
        build_rd(&Toolpath { blocks: vec![b] }, &o),
        Err(RdError::Safety(SafetyError::NonFinite { .. }))
    ));
    let mut b = rect_block(10.0);
    b.z_mm = Some(2.0);
    assert!(matches!(
        build_rd(&Toolpath { blocks: vec![b] }, &o),
        Err(RdError::Safety(SafetyError::ZUnsupported { .. }))
    ));
    // Un offset que saca el trabajo de la cama lo detecta la comprobación del flujo.
    let tp = Toolpath {
        blocks: vec![rect_block(10.0)],
    };
    let off = RuidaJobOptions {
        offset_mm: [400.0, 0.0],
        ..Default::default()
    };
    assert!(matches!(
        build_rd(&tp, &off),
        Err(RdError::Safety(SafetyError::StreamOutOfBed { .. }))
    ));
}

#[test]
fn too_many_layers() {
    let blocks = (0..129).map(|_| rect_block(10.0)).collect();
    assert!(matches!(
        build_rd(&Toolpath { blocks }, &dflt()),
        Err(RdError::TooManyLayers { .. })
    ));
}

#[test]
fn min_power_above_max_is_rejected() {
    let o = RuidaJobOptions {
        layers: vec![LayerOptions {
            color: [0; 3],
            min_power_pct: Some(60.0),
        }],
        ..Default::default()
    };
    assert!(matches!(
        build_rd(
            &Toolpath {
                blocks: vec![rect_block(50.0)]
            },
            &o
        ),
        Err(RdError::MinPowerAboveMax { block: 0 })
    ));
}

#[test]
fn memory_write_footer_only_when_requested() {
    let tp = Toolpath {
        blocks: vec![rect_block(10.0)],
    };
    let on = RuidaJobOptions {
        footer_stats: true,
        ..Default::default()
    };
    let cmds = build_commands(&tp, &on).unwrap();
    assert!(
        cmds.iter().any(
            |c| matches!(c, Command::MemWrite { addr: 0x320, values } if values == &[327, 327])
        )
    );
    build_rd(&tp, &on).unwrap();
}

#[test]
fn file_checksum_is_optional() {
    let tp = Toolpath {
        blocks: vec![rect_block(10.0)],
    };
    let none = build_commands(&tp, &dflt()).unwrap();
    assert!(!none.iter().any(|c| matches!(c, Command::FileChecksum(_))));
    let o = RuidaJobOptions {
        file_checksum: FileChecksum::SumAll,
        footer_stats: true,
        ..Default::default()
    };
    let cmds = build_commands(&tp, &o).unwrap();
    let i = cmds
        .iter()
        .position(|c| matches!(c, Command::FileChecksum(_)))
        .unwrap();
    // Va justo antes de D7 y después del pie.
    assert!(matches!(cmds[i - 1], Command::MemWrite { .. }));
    assert_eq!(cmds[i + 1], Command::EndOfFile);
    let Command::FileChecksum(s) = cmds[i] else {
        unreachable!()
    };
    // Fórmula de MeerK40t: suma de todos los bytes anteriores más 0xD7 del fin de archivo.
    let expect: u64 = cmd::encode_stream(&cmds[..i])
        .unwrap()
        .iter()
        .map(|&b| u64::from(b))
        .sum::<u64>()
        + 0xD7;
    assert_eq!(s, expect);
}

#[test]
fn process_start_is_always_written_after_the_reference_setup() {
    let tp = Toolpath {
        blocks: vec![rect_block(10.0)],
    };
    let cmds = build_commands(&tp, &dflt()).unwrap();
    // D8 10, F0, E6 01, F1 02 00 y D8 00 (captura de Nova 35, RDWorks, MeerK40t).
    assert_eq!(cmds[0], Command::Process(ProcessOp::RefPoint));
    assert_eq!(cmds[4], Command::Process(ProcessOp::Start));
    assert_eq!(
        cmds.iter()
            .filter(|c| **c == Command::Process(ProcessOp::Start))
            .count(),
        1
    );
}

#[test]
fn laser_tube_mask_and_delays() {
    let tp = Toolpath {
        blocks: vec![rect_block(10.0)],
    };
    let cmds = build_commands(&tp, &dflt()).unwrap();
    assert!(cmds.contains(&Command::Small {
        op: 0xCA,
        sub: 3,
        n: 1
    }));
    assert!(cmds.contains(&Command::LaserTime {
        sub: 0x12,
        micros: 0
    }));
    assert!(cmds.contains(&Command::LaserTime {
        sub: 0x13,
        micros: 0
    }));
    let o = RuidaJobOptions {
        laser_tube_start: 200,
        ..Default::default()
    };
    assert!(matches!(
        build_commands(&tp, &o),
        Err(RdError::BadOption(_))
    ));
}
