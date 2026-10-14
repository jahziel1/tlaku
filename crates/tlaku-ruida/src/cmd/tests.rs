//! Pruebas de codificación, decodificación y texto de los comandos.

use super::*;

fn hex(s: &str) -> Vec<u8> {
    let s: String = s.split_whitespace().collect();
    (0..s.len() / 2)
        .map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap())
        .collect()
}

fn one(s: &str) -> Command {
    let b = hex(s);
    let (c, used) = decode_one(&b).unwrap();
    assert_eq!(used, b.len(), "{s}");
    assert_eq!(c.to_bytes().unwrap(), b, "ida y vuelta de {s}: {c}");
    c
}

#[test]
fn movement_and_cut() {
    assert_eq!(
        one("88 00000b5112 0000021258"),
        Command::MoveAbs {
            x: 190_610,
            y: 35_160
        }
    );
    assert_eq!(
        one("a8 00000b5112 0000021256"),
        Command::CutAbs {
            x: 190_610,
            y: 35_158
        }
    );
    assert_eq!(one("89 7f7f 0001"), Command::MoveRel { dx: -1, dy: 1 });
    assert_eq!(
        one("a9 3f7f 4000"),
        Command::CutRel {
            dx: 8191,
            dy: -8192
        }
    );
    assert_eq!(one("8a 7f7f"), Command::MoveRelX(-1));
    assert_eq!(one("8b 0001"), Command::MoveRelY(1));
    assert_eq!(one("aa 0001"), Command::CutRelX(1));
    assert_eq!(one("ab 7f7f"), Command::CutRelY(-1));
}

#[test]
fn power_and_speed() {
    assert_eq!(
        one("c6 31 00 1319"),
        Command::LayerPower {
            laser: 1,
            limit: Limit::Min,
            layer: 0,
            raw: 2457
        }
    );
    assert_eq!(
        one("c6 42 03 2632"),
        Command::LayerPower {
            laser: 2,
            limit: Limit::Max,
            layer: 3,
            raw: 4914
        }
    );
    assert_eq!(
        one("c6 08 2632"),
        Command::LaserPower {
            laser: 4,
            limit: Limit::Max,
            raw: 4914
        }
    );
    assert_eq!(
        one("c9 04 00 000006 0d20"),
        Command::LayerSpeed {
            layer: 0,
            um_s: 100_000
        }
    );
    assert_eq!(one("c9 02 000006 0d20"), Command::Speed(100_000));
    assert_eq!(
        one("c6 15 0000000000"),
        Command::LaserTime {
            sub: 0x15,
            micros: 0
        }
    );
    assert_eq!(
        one("c6 60 01 02 0000000a14"),
        Command::LayerFrequency {
            laser: 1,
            layer: 2,
            hz: 1300
        }
    );
}

#[test]
fn layers_and_air() {
    assert_eq!(one("ca 01 00"), Command::EndLayer);
    assert_eq!(one("ca 01 12"), Command::AirOff);
    assert_eq!(one("ca 01 13"), Command::AirOn);
    assert_eq!(one("ca 01 30"), Command::LayerFlag(0x30));
    assert_eq!(one("ca 02 05"), Command::LayerSelect(5));
    assert_eq!(
        one("ca 06 01 000000017f"),
        Command::LayerColor {
            layer: 1,
            color: 0xFF
        }
    );
    assert_eq!(one("ca 22 00"), Command::MaxLayer(0));
    assert_eq!(one("ca 41 00 00"), Command::LayerMode { layer: 0, mode: 0 });
    assert_eq!(
        one("ca 03 0d"),
        Command::Small {
            op: 0xCA,
            sub: 3,
            n: 0x0D
        }
    );
    assert_eq!(one("ca 05 0000000000"), Command::Color(0));
}

#[test]
fn file_and_process() {
    assert_eq!(one("d7"), Command::EndOfFile);
    assert_eq!(one("e7 00"), Command::EndBlock);
    assert_eq!(one("d8 00"), Command::Process(ProcessOp::Start));
    assert_eq!(one("d8 02"), Command::Process(ProcessOp::Pause));
    assert_eq!(one("d8 03"), Command::Process(ProcessOp::Resume));
    assert_eq!(one("d8 01"), Command::Process(ProcessOp::Stop));
    assert_eq!(one("d8 10"), Command::Process(ProcessOp::RefPoint));
    assert_eq!(one("d8 55"), Command::Process(ProcessOp::Other(0x55)));
    assert_eq!(one("e6 01"), Command::Absolute);
    assert_eq!(one("f0"), Command::SetReference);
    assert_eq!(one("eb"), Command::Marker(0xEB));
    assert_eq!(one("e5 05 0000000123"), Command::FileChecksum(163));
    assert_eq!(one("e7 01 41 42 00"), Command::FileName("AB".into()));
}

#[test]
fn memory_read_uses_two_seven_bit_bytes() {
    // 0x2FE = 5*128 + 0x7E -> `05 7E`.
    assert_eq!(one("da 00 05 7e"), Command::MemRead { addr: 0x2FE });
    assert_eq!(
        one("da 01 06 20 0000000247 0000000247"),
        Command::MemWrite {
            addr: 0x320,
            values: vec![327, 327]
        }
    );
}

#[test]
fn structure_commands() {
    assert_eq!(
        one("e7 03 00000b5112 0000021258"),
        Command::Point2 {
            op: 0xE7,
            sub: 3,
            x: 190_610,
            y: 35_160
        }
    );
    assert_eq!(
        one("e7 52 00 00000b5112 0000021258"),
        Command::PartPoint {
            sub: 0x52,
            part: 0,
            x: 190_610,
            y: 35_160
        }
    );
    assert_eq!(
        one("e7 54 01 0000030650"),
        Command::PartAbs {
            sub: 0x54,
            n: 1,
            v: 50_000
        }
    );
    // Los "siete valores" que EduTech decodifica como 1,1,-1,-710,-7808,4,7597 son una
    // matriz de 1 columna x 1 fila con pasos de -90,813 mm y 73,133 mm.
    assert_eq!(
        one("e7 08 0001 0001 7f7f7a3a43 0000043b2d"),
        Command::Copies {
            op: 0xE7,
            sub: 8,
            cols: 1,
            rows: 1,
            step_x: -90_813,
            step_y: 73_133
        }
    );
    assert_eq!(
        one("f1 03 0000030650 0000030650"),
        Command::Point2 {
            op: 0xF1,
            sub: 3,
            x: 50_000,
            y: 50_000
        }
    );
    assert!(matches!(
        one("f2 02 052a391c41046a150820"),
        Command::ElementName(_)
    ));
    assert_eq!(one("ea 00"), Command::ShortArg { op: 0xEA, n: 0 });
}

#[test]
fn real_lightburn_file_checksum_message() {
    // `E5 05` capturado de LightBurn, del README de ruida-pa (licencia MIT, Copyright (c)
    // 2025 Steve Isaacs; ver THIRD_PARTY.md): la suma es 9 763 961 = 0x94FC79.
    assert_eq!(
        one("e5 05 00 04 53 78 79"),
        Command::FileChecksum(9_763_961)
    );
    let mut b = Vec::new();
    Command::FileChecksum(9_763_961).encode(&mut b).unwrap();
    assert_eq!(b, hex("e5050004537879"));
}

#[test]
fn immediate_power_rapid_moves_and_home() {
    assert_eq!(one("c7 13 19"), Command::ImmPower(2457));
    assert_eq!(one("c8 7f 7f"), Command::EndPower(0x3FFF));
    assert_eq!(one("d8 2a"), Command::Process(ProcessOp::HomeXY));
    assert_eq!(one("d8 11"), Command::Process(ProcessOp::RefOrigin));
    assert_eq!(one("d8 12"), Command::Process(ProcessOp::RefCurrent));
    // Salto XY desde el cero de la máquina (opciones 00) y relativo (02).
    let xy = Command::RapidXY {
        rel: RapidRef::Machine,
        x: 190_610,
        y: 35_160,
    };
    assert_eq!(
        xy.to_bytes().unwrap(),
        hex("d9 10 00 00000b5112 0000021258")
    );
    assert_eq!(one("d9 10 00 00000b5112 0000021258"), xy);
    assert_eq!(
        one("d9 10 02 7f7f7f7f7f 0000000001"),
        Command::RapidXY {
            rel: RapidRef::Current,
            x: -1,
            y: 1
        }
    );
    assert_eq!(
        one("d9 00 02 0000000064"),
        Command::RapidX {
            rel: RapidRef::Current,
            v: 100
        }
    );
    assert_eq!(
        one("d9 01 00 0000000064"),
        Command::RapidY {
            rel: RapidRef::Machine,
            v: 100
        }
    );
    // Con el bit del haz de luz (opciones 01 y 03) no se interpreta: queda como Unknown.
    for opt in ["01", "03"] {
        assert!(matches!(
            one(&format!("d9 10 {opt} 0000000000 0000000000")),
            Command::Unknown { op: 0xD9, .. }
        ));
    }
}

#[test]
fn copies_matrix_fields() {
    let c = Command::Copies {
        op: 0xF2,
        sub: 5,
        cols: 3,
        rows: 2,
        step_x: 1_000,
        step_y: -2_000,
    };
    let b = c.to_bytes().unwrap();
    assert_eq!(b.len(), 16);
    assert_eq!(decode_one(&b).unwrap().0, c);
}

#[test]
fn unknown_opcodes_are_preserved() {
    assert_eq!(
        one("e8 02"),
        Command::Unknown {
            op: 0xE8,
            data: vec![2]
        }
    );
    assert_eq!(
        one("c9 03 01 02"),
        Command::Unknown {
            op: 0xC9,
            data: vec![3, 1, 2]
        }
    );
}

#[test]
fn bad_input_is_reported() {
    // Datos al inicio.
    assert!(matches!(
        decode_stream(&[0x00, 0x88]),
        Err(DecodeError::NotAnOpcode { offset: 0, .. })
    ));
    // Longitud incorrecta.
    assert!(matches!(
        decode_stream(&hex("88 00000b5112")),
        Err(DecodeError::BadLength { op: 0x88, .. })
    ));
    // Truncado en medio de un comando seguido de otro.
    assert!(matches!(
        decode_stream(&hex("a8 00 d7")),
        Err(DecodeError::BadLength { op: 0xA8, .. })
    ));
    assert!(matches!(
        decode_stream(&hex("e7 01 41 42")),
        Err(DecodeError::Invalid { .. })
    ));
}

#[test]
fn encode_validates_values() {
    let mut v = Vec::new();
    assert!(Command::MoveRel { dx: 9000, dy: 0 }.encode(&mut v).is_err());
    assert!(v.is_empty(), "un error no deja bytes a medias");
    assert!(
        Command::LaserPower {
            laser: 5,
            limit: Limit::Max,
            raw: 0
        }
        .encode(&mut v)
        .is_err()
    );
    assert!(Command::LayerSelect(128).encode(&mut v).is_err());
    assert!(Command::FileName("ñ".into()).encode(&mut v).is_err());
    assert!(
        Command::MemWrite {
            addr: 1,
            values: vec![]
        }
        .encode(&mut v)
        .is_err()
    );
    assert!(v.is_empty());
}

#[test]
fn stream_framing_and_disassembly() {
    let bytes = hex("d8 10 f0 e6 01 f1 02 00 d8 00");
    let cmds = decode_stream(&bytes).unwrap();
    assert_eq!(cmds.len(), 5);
    assert_eq!(encode_stream(&cmds).unwrap(), bytes);
    let text = disassemble(&bytes).unwrap();
    assert!(text.contains("PROCESS Start"));
    assert_eq!(text.lines().count(), 5);
}
