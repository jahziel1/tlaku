//! Pruebas de saltos, encuadre y home.

use super::*;

#[test]
fn jog_and_frame_are_laser_off_and_checked() {
    let (mut s, _l, emu) = mem_session(EmuConfig::default(), fast_cfg());
    s.probe().unwrap();
    s.jog_to(Point::new(100.0, 50.0)).unwrap();
    assert_eq!(emu.lock().unwrap().pos_um, (100_000, 50_000));
    assert_eq!(s.status().unwrap().pos_mm, Point::new(100.0, 50.0));
    s.jog_by(5.0, -10.0).unwrap();
    assert_eq!(emu.lock().unwrap().pos_um, (105_000, 40_000));
    // Fuera de la cama: se rechaza sin enviar el salto.
    let before = emu.lock().unwrap().jogs.len();
    assert!(matches!(
        s.jog_to(Point::new(700.0, 10.0)),
        Err(SessionError::Safety(_))
    ));
    assert_eq!(emu.lock().unwrap().jogs.len(), before);
    s.frame(Point::new(10.0, 10.0), Point::new(60.0, 40.0))
        .unwrap();
    let e = emu.lock().unwrap();
    assert_eq!(e.pos_um, (10_000, 10_000));
    // Todos los saltos son D9 con las opciones 00 (los de láser encendido no existen).
    for j in &e.jogs {
        let b = j.to_bytes().unwrap();
        assert_eq!(b[0], 0xD9);
        assert_eq!(b[2] & 1, 0, "el bit del haz de luz nunca se activa");
    }
}

#[test]
fn home_corner_maps_the_reported_position() {
    let cfg = SessionConfig {
        home: HomeCorner::TopRight,
        ..fast_cfg()
    };
    let (mut s, _l, emu) = mem_session(EmuConfig::default(), cfg);
    s.probe().unwrap();
    // Home arriba a la derecha: la máquina en (0,0) es la esquina (600, 400) de Tlaku.
    emu.lock().unwrap().pos_um = (0, 0);
    assert_eq!(s.status().unwrap().pos_mm, Point::new(600.0, 400.0));
    s.jog_to(Point::new(500.0, 300.0)).unwrap();
    assert_eq!(emu.lock().unwrap().pos_um, (100_000, 100_000));
    s.home().unwrap();
    assert_eq!(emu.lock().unwrap().pos_um, (0, 0));
}
