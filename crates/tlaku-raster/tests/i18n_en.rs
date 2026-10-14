//! Los errores del procesado de imágenes salen en el idioma activo (español por omisión).

use tlaku_i18n::{Locale, with_locale};
use tlaku_raster::{ImageAdjust, RasterBitmap};

fn bad_adjust() -> ImageAdjust {
    ImageAdjust {
        brightness: 2.0,
        ..ImageAdjust::default()
    }
}

#[test]
fn adjust_error_follows_the_locale() {
    assert_eq!(
        bad_adjust().validate().unwrap_err(),
        "brillo fuera de -1..1: 2"
    );
    let en = with_locale(Locale::En, || bad_adjust().validate().unwrap_err());
    assert_eq!(en, "brightness outside -1..1: 2");
}

#[test]
fn bitmap_errors_take_their_arguments_in_english() {
    let en = with_locale(Locale::En, || {
        RasterBitmap::new(2, 2, 0.1, 0.1, vec![0.0; 3]).unwrap_err()
    });
    assert_eq!(en, "expected 4 dots but got 3");
    let es = RasterBitmap::new(2, 2, 0.1, 0.1, vec![0.0; 3]).unwrap_err();
    assert_eq!(es, "se esperaban 4 puntos y llegaron 3");
}
