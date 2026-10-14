use super::*;

fn snap(values: &[(u16, i64)]) -> SettingsSnapshot {
    SettingsSnapshot {
        card_id: 0x6510_6510,
        model: Some("RDC6442S".into()),
        values: WRITABLE
            .iter()
            .map(|d| SettingValue {
                addr: d.addr,
                key: d.key.to_string(),
                value: Some(
                    values
                        .iter()
                        .find(|(a, _)| *a == d.addr)
                        .map_or(d.min.max(1), |(_, v)| *v),
                ),
            })
            .collect(),
    }
}

#[test]
fn the_writable_list_is_unique_sorted_by_group_and_has_sane_ranges() {
    let mut addrs: Vec<u16> = WRITABLE.iter().map(|d| d.addr).collect();
    addrs.sort_unstable();
    addrs.dedup();
    assert_eq!(addrs.len(), WRITABLE.len(), "direcciones repetidas");
    let mut keys: Vec<&str> = WRITABLE.iter().map(|d| d.key).collect();
    keys.sort_unstable();
    keys.dedup();
    assert_eq!(keys.len(), WRITABLE.len(), "claves repetidas");
    for d in WRITABLE {
        assert!(d.min <= d.max, "{}", d.key);
        assert!(d.max < (1 << 34), "{}: no cabe en 35 bits con signo", d.key);
        assert!(d.signed || d.min >= 0, "{}", d.key);
        assert!(d.addr < 0x4000, "{}: cabe en 14 bits", d.key);
    }
}

#[test]
fn addresses_outside_the_closed_list_are_never_writable() {
    // Estado, posición, ID de tarjeta, bloqueo de tarjeta, bloqueos de PC, ajustes del sistema,
    // potencia del láser, modo de control, E/S y contadores: ninguno está en la lista.
    for a in [
        0x200u16,
        0x221,
        0x2FE,
        0x2FF,
        0x591,
        0x181,
        0x182,
        0x100,
        0x011,
        0x012,
        0x013,
        0x017,
        0x018,
        0x019,
        0x010,
        0x004,
        0x060,
        0x320,
        0x205,
        0x201,
        0x011F + 1,
        0x3FFF,
        0,
    ] {
        assert!(find(a).is_none(), "0x{a:03X} no debería ser escribible");
        assert_eq!(
            check_write(a, 1),
            Err(SettingsError::NotWritable { addr: a }),
            "0x{a:03X}"
        );
    }
}

#[test]
fn values_outside_the_range_are_rejected_at_both_ends() {
    let g0 = find(0x005).unwrap();
    assert!(check_write(0x005, g0.min).is_ok());
    assert!(check_write(0x005, g0.max).is_ok());
    assert!(matches!(
        check_write(0x005, g0.min - 1),
        Err(SettingsError::OutOfRange { .. })
    ));
    assert!(matches!(
        check_write(0x005, g0.max + 1),
        Err(SettingsError::OutOfRange { .. })
    ));
    // Con signo: el home offset admite negativos y se codifica en 35 bits.
    let raw = check_write(0x02A, -5000).unwrap();
    assert_eq!(raw, (1u64 << 35) - 5000);
    assert_eq!(find(0x02A).unwrap().decode(raw), -5000);
    // Sin signo: un negativo se rechaza.
    assert!(check_write(0x023, -1).is_err());
}

#[test]
fn plan_lists_only_real_differences_with_current_and_new() {
    let s = snap(&[(0x005, 200_000), (0x023, 400_000)]);
    let p = plan(
        &s,
        &[
            SettingChange {
                addr: 0x005,
                value: 200_000,
            },
            SettingChange {
                addr: 0x023,
                value: 300_000,
            },
        ],
    )
    .unwrap();
    assert_eq!(p.unchanged, 1);
    assert_eq!(
        p.changes,
        vec![PlannedChange {
            addr: 0x023,
            key: "x_max_speed".into(),
            current: 400_000,
            new: 300_000
        }]
    );
}

#[test]
fn plan_rejects_unlisted_duplicated_and_unknown_current_values() {
    let s = snap(&[]);
    assert_eq!(
        plan(
            &s,
            &[SettingChange {
                addr: 0x200,
                value: 1
            }]
        ),
        Err(SettingsError::NotWritable { addr: 0x200 })
    );
    let dup = [
        SettingChange {
            addr: 0x005,
            value: 2000,
        },
        SettingChange {
            addr: 0x005,
            value: 3000,
        },
    ];
    assert_eq!(
        plan(&s, &dup),
        Err(SettingsError::Duplicate { addr: 0x005 })
    );
    let mut blind = snap(&[]);
    blind.values[0].value = None;
    assert!(matches!(
        plan(
            &blind,
            &[SettingChange {
                addr: blind.values[0].addr,
                value: 5000
            }]
        ),
        Err(SettingsError::NoCurrentValue { .. })
    ));
    // Un paso sin valor actual (0: eje inexistente) no se escribe.
    let zero = snap(&[(0x041, 0)]);
    assert!(matches!(
        plan(
            &zero,
            &[SettingChange {
                addr: 0x041,
                value: 6000
            }]
        ),
        Err(SettingsError::NoCurrentValue { .. })
    ));
}

#[test]
fn a_step_cannot_jump_more_than_double_or_below_half() {
    let s = snap(&[(0x021, 6350)]);
    let try_ = |v| {
        plan(
            &s,
            &[SettingChange {
                addr: 0x021,
                value: v,
            }],
        )
    };
    assert!(try_(12_700).is_ok());
    assert!(try_(3_175).is_ok());
    assert!(matches!(try_(12_701), Err(SettingsError::StepJump { .. })));
    assert!(matches!(try_(3_174), Err(SettingsError::StepJump { .. })));
}

#[test]
fn calibration_scales_the_step_by_measured_over_commanded() {
    // Se pidieron 100 mm y se midieron 99: el paso nuevo es el 99 % del actual.
    let c = calibrate_step(6350, 100.0, 99.0).unwrap();
    assert_eq!(c.proposed, 6287); // 6350 x 0,99 = 6286,5 -> 6287
    assert!((c.ratio - 0.99).abs() < 1e-12);
    assert!((c.steps_per_mm_before - 1000.0 / 6350.0).abs() < 1e-12);
    assert!((c.steps_per_mm_after - 1000.0 / 6287.0).abs() < 1e-12);
    // Se midió de más: el paso crece.
    assert_eq!(calibrate_step(6350, 100.0, 101.0).unwrap().proposed, 6414);
}

#[test]
fn calibration_rejects_bad_measures_implausible_ratios_and_sub_resolution_changes() {
    assert!(matches!(
        calibrate_step(6350, 0.0, 1.0),
        Err(SettingsError::BadMeasure("commanded"))
    ));
    assert!(matches!(
        calibrate_step(6350, 100.0, f64::NAN),
        Err(SettingsError::BadMeasure("measured"))
    ));
    assert!(matches!(
        calibrate_step(0, 100.0, 99.0),
        Err(SettingsError::BadMeasure("current"))
    ));
    assert!(matches!(
        calibrate_step(6350, 100.0, 60.0),
        Err(SettingsError::ImplausibleRatio { .. })
    ));
    assert!(matches!(
        calibrate_step(6350, 100.0, 130.0),
        Err(SettingsError::ImplausibleRatio { .. })
    ));
    // Con un paso de 3 unidades, un 0,1 % no se puede corregir.
    assert!(matches!(
        calibrate_step(3, 100.0, 99.9),
        Err(SettingsError::BelowResolution { .. })
    ));
}

#[test]
fn the_backup_file_round_trips_and_names_the_card() {
    let s = snap(&[(0x005, 200_000), (0x02A, -1500)]);
    let f = SettingsFile::from_snapshot(&s, "2026-10-14T12:00:00Z");
    assert_eq!(f.format, FORMAT);
    assert_eq!(f.card_id, "0x65106510");
    let json = f.to_json();
    assert!(json.contains("\"x_home_offset\""));
    let back = parse_file(&json).unwrap();
    assert_eq!(back, f);
    assert_eq!(back.card().unwrap(), 0x6510_6510);
    let ch = back.changes();
    assert_eq!(ch.len(), WRITABLE.len());
    assert!(ch.contains(&SettingChange {
        addr: 0x02A,
        value: -1500
    }));
}

#[test]
fn parse_file_rejects_everything_that_is_not_a_clean_backup() {
    let ok = SettingsFile::from_snapshot(&snap(&[]), "t").to_json();
    let bad = |from: &str, to: &str| parse_file(&ok.replacen(from, to, 1)).unwrap_err();
    assert!(matches!(
        bad("tlaku-ruida-settings", "otro"),
        SettingsError::BadFile(_)
    ));
    assert!(matches!(
        bad("\"version\": 1", "\"version\": 2"),
        SettingsError::BadFile(_)
    ));
    assert!(matches!(
        bad("g0_speed", "system_control"),
        SettingsError::BadFile(_)
    ));
    assert!(matches!(
        bad("\"0x005\"", "\"0x200\""),
        SettingsError::BadFile(_)
    ));
    assert!(matches!(
        bad("\"0x65106510\"", "\"nada\""),
        SettingsError::BadFile(_)
    ));
    assert!(parse_file("no es json").is_err());
    assert!(parse_file(&" ".repeat(MAX_FILE_BYTES + 1)).is_err());
    // Un valor fuera de rango en el archivo se rechaza al leerlo, no al escribir.
    let huge = ok.replacen("\"value\": 1000\n", "\"value\": 9999999999\n", 1);
    assert_ne!(huge, ok);
    assert!(parse_file(&huge).is_err());
    // Claves repetidas.
    let mut f = SettingsFile::from_snapshot(&snap(&[]), "t");
    let first = f.settings[0].clone();
    f.settings.push(first);
    assert!(matches!(
        parse_file(&f.to_json()),
        Err(SettingsError::BadFile(_))
    ));
}

#[test]
fn utc_dates_and_backup_names() {
    assert_eq!(format_utc(0), "1970-01-01T00:00:00Z");
    assert_eq!(format_utc(1_700_000_000), "2023-11-14T22:13:20Z");
    // 29 de febrero de un año bisiesto.
    assert_eq!(format_utc(1_709_164_800), "2024-02-29T00:00:00Z");
    assert_eq!(
        backup_file_name(1_700_000_000),
        "ruida-20231114-221320.tlakuruida"
    );
}
