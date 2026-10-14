//! Pruebas contra el ejemplo 1 de EduTech y el ida y vuelta del swizzle.

use super::*;
use crate::cmd::{self, Command, Limit};
use crate::swizzle::Magic;
use tlaku_core::toolpath::Toolpath;

#[test]
fn example_1_decodes_and_reencodes_byte_for_byte() {
    let bytes = hex(EXAMPLE_1);
    let cmds = cmd::decode_stream(&bytes).unwrap();
    assert!(
        cmds.iter().all(|c| !matches!(c, Command::Unknown { .. })),
        "todo el ejemplo debe estar tipado"
    );
    assert_eq!(cmd::encode_stream(&cmds).unwrap(), bytes);
    // Valores que EduTech decodifica a mano.
    assert!(cmds.contains(&Command::LayerSpeed {
        layer: 0,
        um_s: 100_000
    }));
    assert!(cmds.contains(&Command::AirOn));
    assert_eq!(cmds.last(), Some(&Command::EndOfFile));
}

#[test]
fn example_1_regenerated_matches_except_for_documented_differences() {
    let expected = example_with_tlaku_choices();
    let tp = Toolpath {
        blocks: vec![rect_block(pct(3112))],
    };
    let ours = build_commands(&tp, &example_options()).unwrap();
    // Mismos comandos, mismo orden, mismo número.
    assert_eq!(ours.len(), expected.len());
    let mut diffs = Vec::new();
    for (i, (a, b)) in expected.iter().zip(&ours).enumerate() {
        if a != b {
            diffs.push((i, a.clone(), b.clone()));
        }
    }
    // Las únicas diferencias son las cajas: EduTech las trae con +-1 a 4 µm de diferencia
    // respecto a la trayectoria que dibuja (redondeo propio de RDWorks en milésimas de
    // pulgada). Aquí las calculamos exactamente desde las coordenadas. Ver ruida.md 5.
    for (i, a, b) in &diffs {
        let ok = |c: &Command| matches!(c, Command::Point2 { .. } | Command::PartPoint { .. });
        assert!(ok(a) && ok(b), "diferencia inesperada en {i}: {a} vs {b}");
    }
    // 4 + 4 (caja de proceso y de documento) + 8 (cajas de capa) + 4 (elemento) + 2 (E7 13/17)
    // menos los que coinciden por casualidad.
    assert!(!diffs.is_empty());
    println!("diferencias con el ejemplo 1: {}", diffs.len());
    for (i, a, b) in &diffs {
        println!("  #{i}: ejemplo {a}   nuestro {b}");
    }
    // La trayectoria, la velocidad, las potencias, el aire y el pie son idénticos.
    let tail = |v: &[Command]| v[v.len() - 12..].to_vec();
    assert_eq!(tail(&expected), tail(&ours));
}

#[test]
fn example_with_exact_example_boxes_is_identical_in_bytes() {
    // Si la trayectoria y las cajas coinciden con las del ejemplo, los bytes son idénticos.
    let ex = example_with_tlaku_choices();
    let tp = Toolpath {
        blocks: vec![rect_block(pct(3112))],
    };
    let mut cmds = build_commands(&tp, &example_options()).unwrap();
    for (a, b) in cmds.iter_mut().zip(&ex) {
        if matches!(a, Command::Point2 { .. } | Command::PartPoint { .. }) {
            *a = b.clone();
        }
    }
    assert_eq!(
        cmd::encode_stream(&cmds).unwrap(),
        cmd::encode_stream(&ex).unwrap()
    );
}

#[test]
fn rd_is_valid_stream_and_roundtrips_through_swizzle() {
    let tp = Toolpath {
        blocks: vec![rect_block(40.0)],
    };
    let plain = build_rd(&tp, &dflt()).unwrap();
    assert!(plain.iter().filter(|&&b| b >= 0x80).count() > 20);
    let cmds = cmd::decode_stream(&plain).unwrap();
    assert!(cmds.iter().all(|c| !matches!(c, Command::Unknown { .. })));
    assert_eq!(*plain.last().unwrap(), 0xD7);
    // No hay escritura de memoria por defecto.
    assert!(!cmds.iter().any(|c| matches!(c, Command::MemWrite { .. })));
    let wire = to_swizzled(&plain, Magic::default());
    assert_ne!(wire, plain);
    assert_eq!(from_swizzled(&wire, Magic::default()), plain);
    assert_eq!(*wire.last().unwrap(), 0x60);
}

#[test]
fn default_secondary_lasers_are_off_and_power_constant() {
    let tp = Toolpath {
        blocks: vec![rect_block(50.0)],
    };
    let cmds = build_commands(&tp, &dflt()).unwrap();
    let raw_of = |laser: u8, limit: Limit| {
        cmds.iter()
            .find_map(|c| match c {
                Command::LaserPower {
                    laser: l,
                    limit: li,
                    raw,
                } if *l == laser && *li == limit => Some(*raw),
                _ => None,
            })
            .unwrap()
    };
    assert_eq!(raw_of(1, Limit::Min), 8191);
    assert_eq!(raw_of(1, Limit::Max), 8191);
    assert_eq!(raw_of(2, Limit::Max), 0);
    assert_eq!(raw_of(4, Limit::Min), 0);
}
