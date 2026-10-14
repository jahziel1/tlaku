//! Los errores de la cámara salen en el idioma activo (español por omisión).

use image::GrayImage;
use tlaku_camera::{MarkLayout, calibrate_lens, find_chessboard};
use tlaku_i18n::{Locale, with_locale};

#[test]
fn calibration_errors_follow_the_locale() {
    let es = calibrate_lens(&[], [9, 6], 10.0).unwrap_err();
    assert_eq!(es, "no se indicó ninguna imagen para calibrar");
    let en = with_locale(Locale::En, || {
        calibrate_lens(&[], [9, 6], 10.0).unwrap_err()
    });
    assert_eq!(en, "no image was given to calibrate with");
}

#[test]
fn board_and_layout_errors_take_their_arguments_in_english() {
    let img = GrayImage::new(20, 20);
    let en = with_locale(Locale::En, || find_chessboard(&img, [9, 6]).unwrap_err());
    assert_eq!(en, "the image is too small (20x20)");
    let en = with_locale(Locale::En, || {
        MarkLayout::new([400.0, 400.0], 5).unwrap_err()
    });
    assert_eq!(en, "4 or 9 marks can be generated (5 were requested)");
    let es = MarkLayout::new([400.0, 400.0], 5).unwrap_err();
    assert_eq!(es, "se pueden generar 4 o 9 marcas (se pidieron 5)");
}
