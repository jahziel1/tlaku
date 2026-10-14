//! Ola 6E: las funciones nuevas del galvo (aplicar un `.cor` en el anfitrión, el asistente de nueve
//! puntos, la corrección de cilindro y el marcado dividido por formas) no pueden mandar nunca un
//! comando de escritura de configuración.
//!
//! La garantía tiene tres capas, y cada una se prueba aquí:
//!
//! 1. la lista blanca (`codes::TABLE`): lo que escribe configuración, la tabla de corrección o la
//!    EEPROM es `NeverSent` y no existe en `cmd`;
//! 2. el código nuevo (`cor.rs`, `calib.rs`) no menciona esos códigos fuera de comentarios;
//! 3. una sesión completa con un `.cor` aplicado (marcado, marcado de prueba, encuadre y
//!    movimiento) contra la tarjeta simulada: lo que llega a la tarjeta es solo lo permitido.

use tlaku_core::Point;
use tlaku_core::toolpath::{Move, PowerMode, Toolpath, ToolpathBlock};
use tlaku_doc::GalvoDevice;
use tlaku_galvo::codes::{self, Class, Policy, imm};
use tlaku_galvo::compile::params_from_device;
use tlaku_galvo::emu::{EmuConfig, emulator, lock};
use tlaku_galvo::session::{GalvoSession, JobControl, SessionConfig};
use tlaku_galvo::wire::Raw;

/// Códigos que escriben configuración, tablas, EEPROM o tocan ejes y marcado al vuelo.
const CONFIG_WRITES: &[u16] = &[
    imm::WRITE_COR_TABLE,
    imm::WRITE_COR_LINE,
    imm::SET_CONTROL_MODE,
    imm::SET_DELAY_MODE,
    imm::SET_MAX_POLY_DELAY,
    imm::SET_FIRST_PULSE_KILLER,
    imm::SET_LASER_MODE,
    imm::SET_TIMING,
    imm::SET_STANDBY,
    imm::SET_PWM_HALF_PERIOD,
    imm::SET_PWM_PULSE_WIDTH,
    imm::SET_AXIS_MOTION_PARAM,
    imm::SET_AXIS_ORIGIN_PARAM,
    imm::SET_FLY_RES,
    imm::SET_FPK_PARAM,
    imm::SET_FPK_PARAM2,
    imm::SET_Z_DATA,
    imm::LASER_SIGNAL_ON,
    imm::RESET,
    imm::STOP_LIST,
    // Marcado al vuelo y entradas por encoder.
    codes::list::FLY_ENABLE,
    codes::list::FLY_DELAY,
    codes::list::FLY_WAIT_INPUT,
    codes::list::FLY_ENCODER_COUNT,
];

fn device() -> GalvoDevice {
    GalvoDevice {
        field_mm: 100.0,
        freq_min_khz: 20.0,
        freq_max_khz: 80.0,
        ..GalvoDevice::default()
    }
}

fn write_cor(dir: &std::path::Path, dx: f64, dy: f64) -> String {
    let mut b: Vec<u8> = "LMC1COR_1.0"
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect();
    b.extend([0, 0]);
    b.extend(std::iter::repeat_n(0u8, 63 * 8));
    for _ in 0..65 * 65 {
        b.extend(dx.to_le_bytes());
        b.extend(dy.to_le_bytes());
    }
    let path = dir.join("lente.cor");
    std::fs::write(&path, b).unwrap();
    path.to_string_lossy().into_owned()
}

fn square(dev: &GalvoDevice) -> Toolpath {
    let cut = |x: f64, y: f64| Move::Cut {
        to: Point::new(x, y),
        power_pct: 30.0,
    };
    Toolpath {
        blocks: vec![ToolpathBlock {
            label: "cuadro".into(),
            speed_mm_min: 60_000.0,
            air_assist: false,
            power_mode: PowerMode::Constant,
            moves: vec![
                Move::Travel {
                    to: Point::new(-10.0, -10.0),
                },
                cut(10.0, -10.0),
                cut(10.0, 10.0),
                cut(-10.0, 10.0),
                cut(-10.0, -10.0),
            ],
            z_mm: None,
            galvo: Some(params_from_device(dev, 30.0, None)),
        }],
    }
}

#[test]
fn every_config_write_is_never_sent_and_has_no_constructor() {
    for &code in CONFIG_WRITES {
        let info = codes::lookup(code).unwrap_or_else(|| panic!("{code:#06x} no está en la tabla"));
        assert!(
            matches!(info.policy, Policy::NeverSent(_)),
            "{} ({code:#06x}) no debe poder enviarse",
            info.name
        );
        assert!(!codes::is_sent(code), "{}", info.name);
    }
    // Todo lo clasificado como configuración o señal directa es NeverSent, sin excepciones.
    for c in codes::TABLE {
        if matches!(c.class, Class::Config | Class::Direct) {
            assert!(matches!(c.policy, Policy::NeverSent(_)), "{}", c.name);
        }
    }
    // Y no hay forma de armar uno: decodificado es `Unknown` y la lista blanca lo rechaza.
    for &code in CONFIG_WRITES.iter().filter(|c| **c < 0x8000) {
        let imm = tlaku_galvo::cmd::Imm::decode(&Raw::bare(code));
        assert!(
            matches!(imm, tlaku_galvo::cmd::Imm::Unknown(_)),
            "{code:#06x} se decodifica como un comando permitido"
        );
    }
}

/// El código de lectura y aplicación de la tabla no puede ni nombrar los comandos de escritura.
#[test]
fn the_cor_code_does_not_reference_any_write_command() {
    for (name, src) in [
        ("cor.rs", include_str!("../src/cor.rs")),
        ("calib.rs", include_str!("../src/calib.rs")),
    ] {
        // Solo el código: sin comentarios (`//`, `///`, `//!`) y sin el módulo de pruebas.
        let code = src.split("#[cfg(test)]").next().unwrap();
        for line in code.lines().filter(|l| !l.trim_start().starts_with("//")) {
            for banned in [
                "WRITE_COR",
                "WriteCor",
                "write_cor",
                "SET_FLY",
                "WRITE_PORT",
                "EEPROM",
                "UsbTransport",
                "Transport",
                "std::fs::write",
                "File::create",
                "OpenOptions",
            ] {
                assert!(!line.contains(banned), "{name}: «{banned}» en «{line}»");
            }
        }
    }
}

#[test]
fn a_session_with_a_cor_file_sends_only_whitelisted_commands() {
    let dir = std::env::temp_dir().join(format!("tlaku-6e-safety-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut dev = device();
    dev.correction.cor_file = Some(write_cor(&dir, 300.0, -200.0));

    let (t, h) = emulator(EmuConfig {
        source: dev.source,
        field_mm: dev.field_mm,
        ..EmuConfig::default()
    });
    let mut cfg = SessionConfig::new(dev.clone()).unwrap();
    cfg.poll_interval = std::time::Duration::ZERO;
    let mut s = GalvoSession::new(t, cfg).unwrap();
    s.probe().unwrap();
    let tp = square(&dev);
    let ctl = JobControl::new();
    s.mark(&tp, false, &ctl, &mut |_| {}).unwrap();
    s.mark(&tp, true, &ctl, &mut |_| {}).unwrap();
    s.frame(
        Point::new(-10.0, -10.0),
        Point::new(10.0, 10.0),
        &ctl,
        Some(1),
        &mut |_| {},
    )
    .unwrap();
    s.jog_to(Point::new(5.0, 5.0)).unwrap();

    let st = lock(&h);
    assert!(st.violations.is_empty(), "{:?}", st.violations);
    assert!(
        st.forbidden_commands().is_empty(),
        "llegó a la tarjeta: {:?}",
        st.forbidden_commands()
            .iter()
            .map(|c| format!("{c:#06x}"))
            .collect::<Vec<_>>()
    );
    for &code in CONFIG_WRITES {
        let seen = st
            .immediates
            .iter()
            .chain(&st.list_commands)
            .filter(|r| r.code == code)
            .count();
        assert_eq!(seen, 0, "{code:#06x} llegó a la tarjeta");
    }
    // La tabla se aplicó (el trazo no está donde estaría sin ella) y todo terminó apagado.
    assert!(st.laser_safe());
    let marks = st.emitted();
    assert!(!marks.is_empty());
    // Sin tabla, la primera marca iría de (-10, -10) a (10, -10): en x de 0x8000 - 6553,6 = 26214
    // a 39322. La tabla suma 300 unidades.
    let first = marks[0];
    assert_eq!(first.from.0, 26_214 + 300, "{first:?}");
    assert_eq!(first.to.0, 39_322 + 300, "{first:?}");
    let _ = std::fs::remove_dir_all(&dir);
}
