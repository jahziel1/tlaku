//! Pruebas de límites de la cama, esquinas de home, marcos y posición inicial.

use super::*;

#[test]
fn out_of_bed_nan_and_bad_speed_are_errors_before_any_output() {
    let c = cfg();
    let e = encode(
        &tp(vec![block(
            1200.0,
            vec![travel(10.0, 190.0), cut(301.0, 190.0)],
        )]),
        &c,
    )
    .unwrap_err();
    assert!(matches!(e, EncodeError::OutOfBed { index: 1, .. }), "{e}");
    let e = encode(&tp(vec![block(1200.0, vec![travel(-1.0, 190.0)])]), &c).unwrap_err();
    assert!(matches!(e, EncodeError::OutOfBed { .. }));
    let e = encode(&tp(vec![block(1200.0, vec![travel(10.0, 201.0)])]), &c).unwrap_err();
    assert!(matches!(e, EncodeError::OutOfBed { .. }));
    let e = encode(&tp(vec![block(1200.0, vec![travel(f64::NAN, 1.0)])]), &c).unwrap_err();
    assert!(matches!(e, EncodeError::NonFinite { .. }));
    // Velocidad: cero, NaN, por encima del máximo y demasiado lenta.
    for sp in [0.0, f64::NAN, -5.0] {
        let e = encode(&tp(vec![block(sp, vec![travel(1.0, 1.0)])]), &c).unwrap_err();
        assert!(matches!(e, EncodeError::NonFinite { .. }), "{sp}: {e}");
    }
    let e = encode(&tp(vec![block(60.0 * 141.0, vec![travel(1.0, 1.0)])]), &c).unwrap_err();
    assert!(matches!(e, EncodeError::Speed { .. }));
    let e = encode(&tp(vec![block(60.0 * 0.1, vec![travel(1.0, 1.0)])]), &c).unwrap_err();
    assert!(matches!(e, EncodeError::Speed { .. }), "{e}");
    // Un bloque inválido tras uno válido: no se produce nada.
    let bad = tp(vec![
        block(1200.0, vec![travel(10.0, 190.0), cut(20.0, 190.0)]),
        block(1200.0, vec![cut(500.0, 10.0)]),
    ]);
    assert!(encode(&bad, &c).is_err());
}

#[test]
fn bed_edge_with_rounding_is_accepted() {
    let c = cfg();
    let p = encode(
        &tp(vec![block(
            1200.0,
            vec![travel(0.0, 200.0), cut(300.0, 200.0), cut(300.0, 0.0)],
        )]),
        &c,
    )
    .unwrap();
    assert_eq!(p.end_mil, (mm_to_mil(300.0), mm_to_mil(200.0)));
}

#[test]
fn home_corner_flips_the_axes() {
    for (corner, start_tlaku, end_tlaku, want_end) in [
        (
            HomeCorner::TopLeft,
            (0.0, 200.0),
            (50.0, 150.0),
            (50.0, 50.0),
        ),
        (
            HomeCorner::BottomLeft,
            (0.0, 0.0),
            (50.0, 50.0),
            (50.0, 50.0),
        ),
        (
            HomeCorner::TopRight,
            (300.0, 200.0),
            (250.0, 150.0),
            (50.0, 50.0),
        ),
        (
            HomeCorner::BottomRight,
            (300.0, 0.0),
            (250.0, 50.0),
            (50.0, 50.0),
        ),
    ] {
        let mut c = cfg();
        c.home = corner;
        let p = encode(
            &tp(vec![block(
                1200.0,
                vec![
                    travel(start_tlaku.0, start_tlaku.1),
                    travel(end_tlaku.0, end_tlaku.1),
                ],
            )]),
            &c,
        )
        .unwrap();
        let want = (mm_to_mil(want_end.0), mm_to_mil(want_end.1));
        assert_eq!(p.end_mil, want, "{}", corner.name());
        let back = p.end_mm();
        assert!((back.x - end_tlaku.0).abs() < 0.02 && (back.y - end_tlaku.1).abs() < 0.02);
    }
    assert_eq!(HomeCorner::parse("Top-Left"), Some(HomeCorner::TopLeft));
    assert_eq!(HomeCorner::parse("centro"), None);
}

#[test]
fn autolock_off_appends_unlock_after_each_rapid() {
    let mut c = cfg();
    c.autolock = false;
    let p = encode(&tp(vec![block(1200.0, vec![travel(50.0, 100.0)])]), &c).unwrap();
    assert!(s(&p).ends_with("S1P\nIS2P\n"), "{}", s(&p));
    let p = encode(&tp(vec![block(1200.0, vec![travel(50.0, 100.0)])]), &cfg()).unwrap();
    assert!(s(&p).ends_with("S1P\n") && !s(&p).contains("S2P"));
}

#[test]
fn frames_have_valid_bytes_and_wait_only_at_program_exit() {
    let path = tp(vec![block(
        1200.0,
        vec![
            travel(10.0, 190.0),
            cut(60.0, 190.0),
            cut(60.0, 130.0),
            cut(10.0, 130.0),
            cut(10.0, 190.0),
        ],
    )]);
    let p = encode(&path, &cfg()).unwrap();
    let frames = p.frames().unwrap();
    assert!(frames.len() >= 3);
    let waits: Vec<_> = frames.iter().filter(|f| f.wait_finish).collect();
    assert_eq!(waits.len(), 1);
    assert!(frames.last().unwrap().wait_finish);
    assert!(frames.last().unwrap().text().ends_with("FNSE"));
}

#[test]
fn egv_file_has_the_header_of_the_lhymicro_gl_format() {
    let p = encode(&tp(vec![block(1200.0, vec![travel(50.0, 100.0)])]), &cfg()).unwrap();
    let f = String::from_utf8(p.to_egv_file("Tlaku test")).unwrap();
    assert!(f.starts_with("Document type : LHYMICRO-GL file\nFile version: 1.0.01\n"));
    assert!(f.contains("Creator-Software: Tlaku test\n\n%0%0%0%0%\nI"));
}

#[test]
fn empty_toolpath_gives_an_empty_program() {
    let p = encode(&Toolpath::default(), &cfg()).unwrap();
    assert!(p.stream.is_empty());
    assert_eq!(p.end_mil, p.start_mil);
    assert!(p.frames().unwrap().is_empty());
}

#[test]
fn start_position_is_the_origin_of_the_first_rapid() {
    let mut c = cfg();
    c.start_mil = (100, 50);
    let p = encode(&tp(vec![block(1200.0, vec![travel(0.0, 200.0)])]), &c).unwrap();
    // De (100, 50) a (0, 0): izquierda 100 y arriba 50.
    assert_eq!(s(&p), "IT100L|yS1P\n");
}
