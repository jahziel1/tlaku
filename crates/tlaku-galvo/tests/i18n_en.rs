//! Mensajes del driver en inglés (`with_locale`) y en español (por omisión). La lógica de
//! seguridad no cambia con el idioma: solo el texto.

use tlaku_core::Point;
use tlaku_core::toolpath::{Move, PowerMode, Toolpath, ToolpathBlock};
use tlaku_doc::GalvoDevice;
use tlaku_galvo::cmd::Imm;
use tlaku_galvo::compile::{CompileOptions, compile, params_from_device};
use tlaku_galvo::emu::{EmuConfig, emulator};
use tlaku_galvo::safety::{ImmContext, SafetyLimits, check_immediate};
use tlaku_galvo::session::{
    GalvoSession, JobError, STOP_UNCONFIRMED_CODE, SessionConfig, SessionError, StopOutcome,
};
use tlaku_i18n::{Locale, with_locale};

fn device() -> GalvoDevice {
    GalvoDevice {
        field_mm: 100.0,
        freq_min_khz: 20.0,
        freq_max_khz: 80.0,
        max_power_pct: 60.0,
        ..GalvoDevice::default()
    }
}

#[test]
fn a_rejected_board_is_explained_in_english_and_spanish() {
    let dev = device();
    let reject = |locale: Locale| {
        with_locale(locale, || {
            let (t, _h) = emulator(EmuConfig {
                vendor_id: 0x9588,
                product_id: 0x9980,
                ..EmuConfig::default()
            });
            GalvoSession::new(t, SessionConfig::new(dev.clone()).unwrap())
                .err()
                .expect("must reject")
                .to_string()
        })
    };
    let en = reject(Locale::En);
    assert!(en.starts_with("board 9588:9980 is not supported: "), "{en}");
    assert!(en.contains("clone seen on a forum"), "{en}");
    let es = reject(Locale::Es);
    assert!(es.starts_with("tarjeta 9588:9980 no compatible: "), "{es}");
    assert!(es.contains("clon visto en un foro"), "{es}");
}

#[test]
fn an_unknown_vid_pid_is_refused_in_english() {
    let dev = device();
    let err = with_locale(Locale::En, || {
        let (t, _h) = emulator(EmuConfig {
            vendor_id: 0x1A86,
            product_id: 0x5512,
            ..EmuConfig::default()
        });
        GalvoSession::new(t, SessionConfig::new(dev.clone()).unwrap())
            .err()
            .expect("must reject")
            .to_string()
    });
    assert_eq!(err, "board 1A86:5512 is not supported: unknown VID:PID");
}

#[test]
fn a_safety_refusal_is_clear_in_english_and_keeps_its_logic() {
    let lim = SafetyLimits::from_device(&device()).unwrap();
    let ctx = ImmContext {
        arming_allowed: false,
        armed: false,
    };
    let refuse = |locale: Locale| {
        with_locale(locale, || {
            check_immediate(&Imm::EnableLaser, &lim, ctx)
                .unwrap_err()
                .to_string()
        })
    };
    let en = refuse(Locale::En);
    assert!(en.contains("refused"), "{en}");
    assert!(
        en.contains("the laser is only enabled during a real mark"),
        "{en}"
    );
    let es = refuse(Locale::Es);
    assert!(es.contains("rechazado: solo se habilita el láser durante un marcado real"));
}

#[test]
fn bad_limits_are_reported_in_english() {
    let mut dev = device();
    dev.overscan_mm = 99.0;
    let err = with_locale(Locale::En, || {
        SafetyLimits::from_device(&dev).unwrap_err().to_string()
    });
    assert_eq!(
        err,
        "invalid safety profile: the profile overscan must be between 0 and 20 mm"
    );
}

#[test]
fn compile_warnings_and_errors_come_out_in_english() {
    let dev = device();
    let tp = Toolpath {
        blocks: vec![ToolpathBlock {
            label: "test".into(),
            speed_mm_min: 60_000.0,
            air_assist: true,
            power_mode: PowerMode::Constant,
            moves: vec![
                Move::Travel {
                    to: Point::new(0.0, 0.0),
                },
                Move::Cut {
                    to: Point::new(5.0, 0.0),
                    power_pct: 30.0,
                },
            ],
            z_mm: None,
            galvo: Some(params_from_device(&dev, 30.0, None)),
        }],
    };
    let list = with_locale(Locale::En, || {
        compile(&tp, &dev, &CompileOptions::default()).unwrap()
    });
    assert_eq!(
        list.warnings,
        vec!["block 0: air assist is ignored on a galvo".to_string()]
    );
    let list_es = compile(&tp, &dev, &CompileOptions::default()).unwrap();
    assert_eq!(
        list_es.warnings,
        vec!["bloque 0: la asistencia de aire se ignora en galvo".to_string()]
    );

    let mut bad = tp.clone();
    bad.blocks[0].air_assist = false;
    bad.blocks[0].speed_mm_min = 0.0;
    let err = with_locale(Locale::En, || {
        compile(&bad, &dev, &CompileOptions::default())
            .unwrap_err()
            .to_string()
    });
    assert_eq!(err, "block 0: the mark speed must be greater than 0");
}

#[test]
fn an_unconfirmed_stop_is_loud_in_english_and_has_a_stable_code() {
    let e = JobError {
        cause: SessionError::Cancelled,
        batches_sent: 1,
        batches_total: 2,
        stop: StopOutcome::Unconfirmed,
    };
    assert_eq!(e.code(), Some(STOP_UNCONFIRMED_CODE));
    let en = with_locale(Locale::En, || e.to_string());
    assert!(en.contains("NO confirmation"), "{en}");
    assert!(en.contains("emergency stop"), "{en}");
    let es = with_locale(Locale::Es, || e.to_string());
    assert!(es.contains("NO hubo confirmación"), "{es}");
    let acked = JobError {
        stop: StopOutcome::Acked,
        ..e
    };
    assert_eq!(acked.code(), None);
}
