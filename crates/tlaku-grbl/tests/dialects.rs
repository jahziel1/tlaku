//! Los vectores de prueba de `docs/hardware/dialectos-gcode.md` (sección 10) como dorados: el
//! mismo trabajo (un cuadrado de 10 mm a 50 %) escrito con el dialecto de cada firmware. El V1
//! (GRBL 1.1) es la salida de siempre; los demás son los vectores hechos a mano del documento,
//! con las diferencias que se anotan en cada prueba. Regenera con `UPDATE_GOLDEN=1 cargo test`
//! y revisa el diff.

use std::path::Path;
use tlaku_core::Point;
use tlaku_core::toolpath::{Move, PowerMode, Toolpath, ToolpathBlock};
use tlaku_doc::{CommentStyle, DeviceKind, DwellFormat, GcodeDialect};
use tlaku_grbl::gcode::{GcodeOptions, generate};

fn p(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

/// El trabajo común: `Travel(0,0)`, `Cut` a 50 % por (10,0), (10,10), (0,10) y (0,0), con aire.
fn square(mode: PowerMode) -> Toolpath {
    let cut = |x, y| Move::Cut {
        to: p(x, y),
        power_pct: 50.0,
    };
    Toolpath {
        blocks: vec![ToolpathBlock {
            label: "Cuadrado".into(),
            speed_mm_min: 1000.0,
            air_assist: true,
            power_mode: mode,
            moves: vec![
                Move::Travel { to: p(0.0, 0.0) },
                cut(10.0, 0.0),
                cut(10.0, 10.0),
                cut(0.0, 10.0),
                cut(0.0, 0.0),
            ],
            z_mm: None,
        }],
    }
}

/// Opciones del vector: dialecto, `S` máxima y la velocidad de G0 de los vectores de Marlin.
fn opts(dialect: GcodeDialect, s_max: f64) -> GcodeOptions {
    GcodeOptions {
        dialect,
        s_max,
        rapid_feed: Some(6000.0),
        ..GcodeOptions::default()
    }
}

fn check(name: &str, actual: &str) {
    let golden = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/dialects")
        .join(format!("{name}.gcode"));
    if std::env::var_os("UPDATE_GOLDEN").is_some() {
        std::fs::write(&golden, actual).unwrap();
    }
    let expected = std::fs::read_to_string(&golden)
        .unwrap_or_else(|_| panic!("falta {}; ejecuta con UPDATE_GOLDEN=1", golden.display()));
    assert_eq!(actual, expected, "el G-code de '{name}' cambió");
}

fn render(kind: DeviceKind, s_max: f64) -> String {
    generate(
        &square(PowerMode::Dynamic),
        &opts(GcodeDialect::for_kind(kind), s_max),
    )
}

/// V1: GRBL 1.1. Es la salida exacta que producía `generate()` antes de los dialectos.
#[test]
fn v1_grbl11() {
    let g = render(DeviceKind::Grbl, 1000.0);
    check("v1_grbl11", &g);
    // No regresión: el dialecto por defecto y las opciones por defecto dan lo mismo.
    assert_eq!(
        g,
        generate(&square(PowerMode::Dynamic), &GcodeOptions::default())
    );
}

/// V2: grblHAL, igual que V1 byte a byte (cambia solo el enlace).
#[test]
fn v2_grblhal_is_v1() {
    assert_eq!(
        render(DeviceKind::GrblHal, 1000.0),
        render(DeviceKind::Grbl, 1000.0)
    );
}

/// V3: FluidNC con `speed_map` 0-1000, igual que V1; con 255 sale `S128` (o `S127.5` con
/// decimales).
#[test]
fn v3_fluidnc() {
    assert_eq!(
        render(DeviceKind::FluidNc, 1000.0),
        render(DeviceKind::Grbl, 1000.0)
    );
    let g255 = render(DeviceKind::FluidNc, 255.0);
    assert!(g255.contains("G1 X10 S127.5 F1000\n"), "{g255}");
    let whole = GcodeDialect {
        s_decimals: 0,
        ..GcodeDialect::for_kind(DeviceKind::FluidNc)
    };
    let g = generate(&square(PowerMode::Dynamic), &opts(whole, 255.0));
    assert!(g.contains("G1 X10 S128 F1000\n"), "{g}");
    check("v3_fluidnc_255", &g);
}

/// V4: GRBL-LPC con `$30 = 1.0` (el valor de fábrica): `S0.5`.
#[test]
fn v4_grbl_lpc() {
    let g = render(DeviceKind::GrblLpc, 1.0);
    assert!(g.contains("G1 X10 S0.5 F1000\n"), "{g}");
    check("v4_grbl_lpc", &g);
    // Con `$30 = 1000` sale como V1.
    assert_eq!(
        render(DeviceKind::GrblLpc, 1000.0),
        render(DeviceKind::Grbl, 1000.0)
    );
}

/// V5: GRBL-M3 (0.9/1.0), `S` máxima 255 y sin `M4` aunque el bloque sea dinámico.
#[test]
fn v5_grbl_m3() {
    let g = render(DeviceKind::GrblM3, 255.0);
    assert!(!g.contains("M4"));
    check("v5_grbl_m3", &g);
}

/// V5 con más de una forma: como `G0` no apaga el láser, cada desplazamiento lleva `S0`.
#[test]
fn v5_grbl_m3_travels_carry_s0() {
    let mut tp = square(PowerMode::Dynamic);
    tp.blocks[0].moves.extend([
        Move::Travel { to: p(20.0, 0.0) },
        Move::Cut {
            to: p(30.0, 0.0),
            power_pct: 50.0,
        },
    ]);
    let g = generate(
        &tp,
        &opts(GcodeDialect::for_kind(DeviceKind::GrblM3), 255.0),
    );
    assert!(g.contains("G0 X20 S0\n"), "{g}");
    // Y el siguiente corte vuelve a pedir su potencia.
    assert!(g.contains("G1 X30 S128\n"), "{g}");
    // En GRBL 1.1 no hace falta.
    let g = generate(&tp, &opts(GcodeDialect::for_kind(DeviceKind::Grbl), 1000.0));
    assert!(g.contains("G0 X20\n"), "{g}");
}

/// V6: Smoothieware con `Maximum S-value = 1.0`. Diferencia con el documento: termina con
/// `M400` (el dialecto lo emite para que el último `ok` signifique "trabajo terminado").
#[test]
fn v6_smoothie() {
    let g = render(DeviceKind::Smoothie, 1.0);
    assert!(
        g.contains("G1 X10 S0.5 F1000\n")
            && g.starts_with("; Generado por Tlaku\nG90\nG21\nM3 S0\n"),
        "{g}"
    );
    assert!(g.ends_with("G0 X0 Y0\nM400\n"), "{g}");
    check("v6_smoothie", &g);
    // Con `Maximum S-value = 100` la línea sale con S50.
    assert!(render(DeviceKind::Smoothie, 100.0).contains("S50 F1000"));
}

/// V7: Marlin 2.x inline. Con `M400` al final (ver V6).
#[test]
fn v7_marlin_inline() {
    let g = render(DeviceKind::Marlin, 255.0);
    check("v7_marlin_inline", &g);
    // El primer G1 repite F1000 y el G0 final reescribe F6000: F es compartida.
    assert!(g.contains("G0 X0 Y0 F6000\n") && g.contains("G1 X10 S128 F1000\n"));
    // Con CUTTER_POWER_UNIT = PERCENT sale S50.
    assert!(render(DeviceKind::Marlin, 100.0).contains("G1 X10 S50 F1000"));
}

/// V8: Marlin sin inline. Diferencias con el documento: sin la línea `M3 S0` inicial (la primera
/// `M3 S128` ya enciende el láser, la otra sería redundante) y con `M400` al final.
#[test]
fn v8_marlin_standard() {
    let g = generate(
        &square(PowerMode::Dynamic),
        &opts(GcodeDialect::marlin_standard(), 255.0),
    );
    check("v8_marlin_standard", &g);
    assert_eq!(g.matches("M3 S").count(), 1, "{g}");
}

/// V8 con varias formas: cada `G0` apaga el láser, así que se repite `M3 S128` antes de cada
/// trozo; un overscan (`S0`) con el láser apagado no escribe nada.
#[test]
fn v8_marlin_standard_repeats_the_power_after_each_travel() {
    let mut tp = square(PowerMode::Dynamic);
    tp.blocks[0].moves.extend([
        Move::Travel { to: p(20.0, 0.0) },
        Move::Cut {
            to: p(21.0, 0.0),
            power_pct: 0.0,
        },
        Move::Cut {
            to: p(30.0, 0.0),
            power_pct: 50.0,
        },
    ]);
    let g = generate(&tp, &opts(GcodeDialect::marlin_standard(), 255.0));
    assert_eq!(g.matches("M3 S128\n").count(), 2, "{g}");
    assert!(!g.contains("S0\n"), "{g}");
}

/// V9: Marlin heredado con el láser en la salida del ventilador. Diferencia con el documento:
/// al final `M9` va antes del apagado `M107 P1` (el mismo orden que en los demás dialectos:
/// aire y luego láser) y se añade `M400`.
#[test]
fn v9_marlin_fan() {
    let g = generate(
        &square(PowerMode::Dynamic),
        &opts(GcodeDialect::marlin_fan(), 255.0),
    );
    check("v9_marlin_fan", &g);
    assert_eq!(g.matches("M107 P1").count(), 1, "{g}");
}

/// V9 con varias formas: `G0` no apaga el láser, así que se apaga antes de cada desplazamiento
/// y no se repite.
#[test]
fn v9_marlin_fan_turns_the_laser_off_before_each_travel() {
    let mut tp = square(PowerMode::Dynamic);
    tp.blocks[0].moves.extend([
        Move::Travel { to: p(20.0, 0.0) },
        Move::Cut {
            to: p(30.0, 0.0),
            power_pct: 50.0,
        },
    ]);
    let g = generate(&tp, &opts(GcodeDialect::marlin_fan(), 255.0));
    let lines: Vec<&str> = g.lines().collect();
    let at = lines.iter().position(|l| l.starts_with("G0 X20")).unwrap();
    assert_eq!(lines[at - 1], "M107 P1", "{g}");
    assert_eq!(g.matches("M106 P1 S128").count(), 2, "{g}");
}

/// V10: V7 con el aire por salidas de ventilador (el perfil Snapmaker de LightBurn).
#[test]
fn v10_marlin_inline_with_fan_air() {
    let dialect = GcodeDialect {
        air_on: Some("M106 P2 S255".into()),
        air_off: Some("M107 P2".into()),
        ..GcodeDialect::marlin_inline()
    };
    let g = generate(&square(PowerMode::Dynamic), &opts(dialect, 255.0));
    check("v10_marlin_fan_air", &g);
    let v7 = render(DeviceKind::Marlin, 255.0);
    assert_eq!(
        g,
        v7.replace("M8\n", "M106 P2 S255\n")
            .replace("M9\n", "M107 P2\n")
    );
}

#[test]
fn custom_gcode_starts_as_grbl_and_every_field_is_editable() {
    assert_eq!(
        GcodeDialect::for_kind(DeviceKind::CustomGcode),
        GcodeDialect::default()
    );
    // Sin aire en el dialecto no hay M8/M9; comentarios entre paréntesis; sin coordenadas con
    // más de 1 decimal.
    let d = GcodeDialect {
        air_on: None,
        air_off: None,
        comment: CommentStyle::Parentheses,
        decimals: 1,
        ..GcodeDialect::default()
    };
    let mut tp = square(PowerMode::Constant);
    tp.blocks[0].label = "Cuadrado (uno)".into();
    tp.blocks[0].moves[1] = Move::Cut {
        to: p(10.26, 0.0),
        power_pct: 50.0,
    };
    let g = generate(&tp, &opts(d, 1000.0));
    assert!(
        g.starts_with("(Generado por Tlaku)\nG90\nG21\nM3 S0\n(Cuadrado uno)\n"),
        "{g}"
    );
    assert!(!g.contains("M8") && !g.contains("M9"));
    assert!(g.contains("G1 X10.3 S500 F1000\n"), "{g}");
}

#[test]
fn comments_never_exceed_the_line_limit() {
    let mut tp = square(PowerMode::Dynamic);
    tp.blocks[0].label = "capa muy larga ".repeat(20);
    let g = generate(
        &tp,
        &opts(GcodeDialect::for_kind(DeviceKind::Marlin), 255.0),
    );
    assert!(g.lines().all(|l| l.len() < 96), "{g}");
    let comment = g.lines().nth(4).unwrap();
    assert!(comment.starts_with("; capa muy larga"), "{comment}");
    // Sin límite (0) la etiqueta entra completa.
    let d = GcodeDialect {
        max_line_len: 0,
        ..GcodeDialect::default()
    };
    let g = generate(&tp, &opts(d, 1000.0));
    assert!(g.lines().any(|l| l.len() > 200));
}

/// Propiedades de todos los dialectos: ninguna línea pasa de `max_line_len`, `S` no pasa de
/// `s_max` y, si `G0` no apaga el láser, ningún `G0` sale con el láser encendido.
#[test]
fn every_preset_respects_its_own_limits() {
    let kinds = [
        (DeviceKind::Grbl, 1000.0),
        (DeviceKind::GrblM3, 255.0),
        (DeviceKind::GrblHal, 1000.0),
        (DeviceKind::FluidNc, 1000.0),
        (DeviceKind::GrblLpc, 1.0),
        (DeviceKind::Smoothie, 1.0),
        (DeviceKind::Marlin, 255.0),
        (DeviceKind::CustomGcode, 1000.0),
    ];
    let mut tp = square(PowerMode::Dynamic);
    tp.blocks[0].moves.extend([
        Move::Travel { to: p(20.0, 5.0) },
        Move::Cut {
            to: p(30.0, 5.0),
            power_pct: 100.0,
        },
    ]);
    for (kind, s_max) in kinds {
        let d = GcodeDialect::for_kind(kind);
        let g = generate(&tp, &opts(d.clone(), s_max));
        for l in g.lines() {
            if d.max_line_len > 0 {
                assert!(l.len() < d.max_line_len as usize, "{kind:?}: {l}");
            }
            if let Some(s) = l.split(' ').find_map(|w| w.strip_prefix('S')) {
                assert!(s.parse::<f64>().unwrap() <= s_max, "{kind:?}: {l}");
            }
        }
        if !d.g0_laser_off {
            // Antes de cada G0 con corte previo la S vigente es 0.
            let mut hot = false;
            for l in g.lines() {
                if l == d.laser_off {
                    hot = false;
                } else if l.starts_with("G0") {
                    assert!(!hot || l.contains(" S0"), "{kind:?}: {l}");
                    hot = false;
                } else if l.starts_with("G1") {
                    hot = l
                        .split(' ')
                        .find_map(|w| w.strip_prefix('S'))
                        .is_none_or(|s| s != "0")
                        || hot;
                }
            }
        }
    }
}

/// Un punto (`Move::Dwell`) de 250 ms con el dialecto de cada firmware: `G4 P` va en segundos en
/// GRBL y familia y en milisegundos en Marlin y Smoothieware; `S` siempre en segundos.
#[test]
fn dwell_unit_follows_each_dialect() {
    let dot = |ms| Toolpath {
        blocks: vec![ToolpathBlock {
            label: "Punto".into(),
            speed_mm_min: 1000.0,
            air_assist: false,
            power_mode: PowerMode::Constant,
            moves: vec![
                Move::Travel { to: p(5.0, 5.0) },
                Move::Dwell {
                    to: p(5.0, 5.0),
                    ms,
                    power_pct: 50.0,
                },
            ],
            z_mm: None,
        }],
    };
    let g4 = |d: GcodeDialect, ms: f64, s_max: f64| -> String {
        generate(&dot(ms), &opts(d, s_max))
            .lines()
            .find(|l| l.starts_with("G4"))
            .unwrap_or("")
            .to_string()
    };
    for (kind, s_max, want) in [
        (DeviceKind::Grbl, 1000.0, "G4 P0.25"),
        (DeviceKind::GrblM3, 255.0, "G4 P0.25"),
        (DeviceKind::GrblHal, 1000.0, "G4 P0.25"),
        (DeviceKind::FluidNc, 1000.0, "G4 P0.25"),
        (DeviceKind::GrblLpc, 1.0, "G4 P0.25"),
        (DeviceKind::CustomGcode, 1000.0, "G4 P0.25"),
        (DeviceKind::Smoothie, 1.0, "G4 P250"),
        (DeviceKind::Marlin, 255.0, "G4 P250"),
    ] {
        assert_eq!(
            g4(GcodeDialect::for_kind(kind), 250.0, s_max),
            want,
            "{kind:?}"
        );
    }
    // Las variantes de Marlin heredan los milisegundos.
    assert_eq!(g4(GcodeDialect::marlin_standard(), 250.0, 255.0), "G4 P250");
    assert_eq!(g4(GcodeDialect::marlin_fan(), 250.0, 255.0), "G4 P250");
    // Formato propio: segundos en la letra S.
    let s_secs = GcodeDialect {
        dwell: DwellFormat::SSeconds,
        ..GcodeDialect::default()
    };
    assert_eq!(g4(s_secs, 250.0, 1000.0), "G4 S0.25");
    // En milisegundos el valor es entero y una pausa de menos de medio ms no se emite.
    let ms = GcodeDialect::marlin_inline();
    assert_eq!(g4(ms.clone(), 1234.6, 255.0), "G4 P1235");
    assert_eq!(g4(ms, 0.4, 255.0), "");
}
