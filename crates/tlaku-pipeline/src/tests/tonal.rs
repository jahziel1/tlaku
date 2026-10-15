//! Vista previa «por tonos»: la dosis sale de la trayectoria real y la curva del material la
//! vuelve color.

use super::*;
use tlaku_doc::{JobOptions, LayerSettings, Overscan, PowerMode, ToneCurve, tone_preset};

fn wood() -> ToneCurve {
    tone_preset("light_wood").unwrap()
}

fn options(max_px: u32) -> TonalOptions {
    TonalOptions {
        curve: wood(),
        spot_mm: 0.1,
        region_mm: None,
        max_px,
    }
}

/// Cuadrado de 10 x 10 mm en (10, 10) como línea con los ajustes dados.
fn square_job(power: f64, speed: f64, passes: u32, device: &DeviceProfile) -> ComputedJob {
    let layer = LayerSettings {
        power,
        speed,
        passes,
        ..line_layer(RED)
    };
    let d = doc(
        vec![rect_object("a", RED, 10.0, 10.0, 10.0, 10.0)],
        vec![layer],
    );
    compute(&d, device, &JobOptions::default()).unwrap()
}

fn rgba(img: &TonalImage) -> image::RgbaImage {
    image::load_from_memory(&img.png).unwrap().to_rgba8()
}

/// Color del píxel que contiene el punto `(x, y)` mm.
fn at(img: &TonalImage, x: f64, y: f64) -> [u8; 4] {
    let im = rgba(img);
    let col = ((x - img.covered_mm[0]) / img.px_mm) as u32;
    let row = ((img.covered_mm[3] - y) / img.px_mm) as u32;
    im.get_pixel(col.min(im.width() - 1), row.min(im.height() - 1))
        .0
}

fn luma(c: [u8; 4]) -> f64 {
    0.299 * f64::from(c[0]) + 0.587 * f64::from(c[1]) + 0.114 * f64::from(c[2])
}

#[test]
fn more_power_or_less_speed_or_more_passes_is_darker() {
    let dev = device();
    let dose = |power, speed, passes| {
        render_tonal(
            &square_job(power, speed, passes, &dev),
            &dev,
            &options(1200),
        )
        .unwrap()
        .max_dose
    };
    let base = dose(40.0, 1200.0, 1);
    assert!(base > 0.0);
    // La dosis es lineal en la potencia, en 1/velocidad y en las pasadas.
    assert!((dose(80.0, 1200.0, 1) / base - 2.0).abs() < 0.05);
    assert!((dose(40.0, 600.0, 1) / base - 2.0).abs() < 0.05);
    assert!((dose(40.0, 1200.0, 3) / base - 3.0).abs() < 0.05);
    // Y se nota en el color del trazo.
    let img = |p, v, n| render_tonal(&square_job(p, v, n, &dev), &dev, &options(1200)).unwrap();
    let l = |i: &TonalImage| luma(at(i, 10.0, 15.0));
    assert!(l(&img(80.0, 1200.0, 1)) < l(&img(40.0, 1200.0, 1)));
    assert!(l(&img(40.0, 600.0, 1)) < l(&img(40.0, 1200.0, 1)));
    assert!(l(&img(40.0, 1200.0, 3)) < l(&img(40.0, 1200.0, 1)));
}

#[test]
fn tone_saturates_at_the_burn_color_and_stays_base_elsewhere() {
    let dev = device();
    let img = render_tonal(&square_job(100.0, 60.0, 8, &dev), &dev, &options(1200)).unwrap();
    let c = at(&img, 10.0, 15.0);
    let burn = wood().burn;
    for i in 0..3 {
        assert!(
            (i16::from(c[i]) - i16::from(burn[i])).abs() <= 2,
            "{c:?} {burn:?}"
        );
    }
    // El centro del cuadrado no se grabó: es el color del material.
    let inside = at(&img, 15.0, 15.0);
    assert_eq!(&inside[..3], &wood().base);
    assert_eq!(inside[3], 255);
    // La hoja es la caja grabada más el margen.
    assert_eq!(
        img.sheet_mm,
        [10.0 - 3.0, 10.0 - 3.0, 20.0 + 3.0, 20.0 + 3.0]
    );
}

#[test]
fn constant_power_burns_more_where_the_head_starts_and_stops() {
    let mut m4 = device();
    m4.power_mode = PowerMode::Dynamic;
    let mut m3 = device();
    m3.power_mode = PowerMode::Constant;
    let job = square_job(30.0, 6000.0, 1, &m4);
    let dose = |dev: &DeviceProfile, x: f64, y: f64| {
        let img = render_tonal(&job, dev, &options(1600)).unwrap();
        // Dosis recuperada del PNG no hace falta: se compara el color, más oscuro = más dosis.
        luma(at(&img, x, y))
    };
    // En la mitad de un lado (a velocidad plena) las dos máquinas dan lo mismo...
    let mid4 = dose(&m4, 15.0, 10.0);
    let mid3 = dose(&m3, 15.0, 10.0);
    assert!((mid4 - mid3).abs() < 1.5, "{mid4} {mid3}");
    // ...y en el punto de partida del cuadrado (esquina 10,10) M3 deja más quemadura.
    let start4 = dose(&m4, 10.0, 10.0);
    let start3 = dose(&m3, 10.0, 10.0);
    assert!(start3 < start4 - 3.0, "{start3} {start4}");
    // La dosis máxima sube con M3 y no con M4.
    let max = |dev: &DeviceProfile| render_tonal(&job, dev, &options(1600)).unwrap().max_dose;
    assert!(max(&m3) > max(&m4) * 1.5);
}

#[test]
fn fills_accumulate_by_interval() {
    let dev = device();
    let fill = |interval: f64| {
        let layer = LayerSettings {
            interval,
            power: 40.0,
            speed: 3000.0,
            overscan: Overscan::Mm(0.0),
            ..fill_layer(RED)
        };
        let d = doc(
            vec![rect_object("a", RED, 10.0, 10.0, 10.0, 10.0)],
            vec![layer],
        );
        let job = compute(&d, &dev, &JobOptions::default()).unwrap();
        // Un punto de 5 mm reparte las líneas en una dosis pareja, sea cual sea el píxel.
        let opts = TonalOptions {
            spot_mm: 5.0,
            ..options(400)
        };
        render_tonal(&job, &dev, &opts).unwrap()
    };
    let fine = fill(0.1);
    let coarse = fill(0.2);
    // Con píxeles más gruesos que el intervalo, la dosis media es potencia / (velocidad x intervalo).
    let expected = |interval: f64| 0.4 / (50.0 * interval);
    assert!(
        (fine.max_dose - expected(0.1)).abs() / expected(0.1) < 0.15,
        "{}",
        fine.max_dose
    );
    assert!(
        (coarse.max_dose - expected(0.2)).abs() / expected(0.2) < 0.15,
        "{}",
        coarse.max_dose
    );
    assert!(luma(at(&fine, 15.0, 15.0)) < luma(at(&coarse, 15.0, 15.0)));
}

#[test]
fn a_dithered_photo_shows_its_tones_and_dots() {
    let dir = workdir("tonal-photo");
    let bmp = write_gradient(&dir); // negro a la izquierda, claro a la derecha
    let layer = LayerSettings {
        overscan: Overscan::Mm(0.0),
        interval: 0.1,
        power: 100.0,
        speed: 1000.0,
        ..LayerSettings::for_images()
    };
    let d = doc(
        vec![image_object(
            "foto",
            &bmp,
            [16.0, 16.0],
            affine::translate(20.0, 20.0),
        )],
        vec![layer],
    );
    let dev = device();
    let job = compute(&d, &dev, &JobOptions::default()).unwrap();
    // Vista completa: el lado oscuro de la foto se ve más oscuro que el claro.
    let img = render_tonal(&job, &dev, &options(600)).unwrap();
    let left = luma(at(&img, 21.0, 28.0));
    let right = luma(at(&img, 35.0, 28.0));
    assert!(left < right - 20.0, "{left} {right}");
    // Zona acercada: se ven los puntos del tramado, píxeles de dos tonos bien distintos.
    let opts = TonalOptions {
        region_mm: Some([27.0, 27.0, 29.0, 29.0]),
        ..options(200)
    };
    let zoom = render_tonal(&job, &dev, &opts).unwrap();
    assert_eq!(zoom.size_px, [200, 200]);
    assert_eq!(zoom.covered_mm, [27.0, 27.0, 29.0, 29.0]);
    let im = rgba(&zoom);
    let lumas: Vec<f64> = im.pixels().map(|p| luma(p.0)).collect();
    let dark = lumas.iter().filter(|&&l| l < 120.0).count();
    let light = lumas.iter().filter(|&&l| l > 170.0).count();
    assert!(dark > 500 && light > 500, "dark={dark} light={light}");
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn zoom_region_and_errors() {
    let dev = device();
    let job = square_job(40.0, 1200.0, 1, &dev);
    let img = render_tonal(&job, &dev, &options(500)).unwrap();
    assert!(img.size_px[0] <= 500 && img.size_px[1] <= 500);
    assert!(img.covered_mm[0] <= img.sheet_mm[0] + 1e-9);
    // Una zona que se sale de la hoja se recorta a ella.
    let opts = TonalOptions {
        region_mm: Some([0.0, 0.0, 12.0, 12.0]),
        ..options(240)
    };
    let part = render_tonal(&job, &dev, &opts).unwrap();
    assert!((part.covered_mm[0] - 7.0).abs() < 1e-9 && (part.covered_mm[1] - 7.0).abs() < 1e-9);
    // Fuera de la hoja: error; curva rota o punto inválido: error.
    let far = TonalOptions {
        region_mm: Some([100.0, 100.0, 120.0, 120.0]),
        ..options(240)
    };
    assert!(render_tonal(&job, &dev, &far).is_err());
    let mut bad = options(240);
    bad.curve.dose_char = 0.0;
    assert!(render_tonal(&job, &dev, &bad).is_err());
    let mut bad_spot = options(240);
    bad_spot.spot_mm = f64::NAN;
    assert!(render_tonal(&job, &dev, &bad_spot).is_err());
}

#[test]
fn nothing_to_burn_is_an_error() {
    let dev = device();
    let layer = LayerSettings {
        power: 0.0,
        ..line_layer(RED)
    };
    let d = doc(
        vec![rect_object("a", RED, 10.0, 10.0, 10.0, 10.0)],
        vec![layer],
    );
    if let Ok(job) = compute(&d, &dev, &JobOptions::default()) {
        assert!(render_tonal(&job, &dev, &options(200)).is_err());
    }
}

/// Medición con un trabajo de imagen grande: `cargo test -p tlaku-pipeline --release -- --ignored
/// --nocapture large_image_job`. Imprime los tiempos de calcular el trabajo y de la vista por tonos.
#[test]
#[ignore = "medición de rendimiento"]
fn large_image_job_renders_fast() {
    use std::time::Instant;
    let dir = workdir("tonal-large");
    // 2400 x 2400 px de ruido con degradado, estirados a 240 x 240 mm.
    let path = dir.join("grande.bmp");
    std::fs::write(
        &path,
        gray_bmp(2400, 2400, |x, y| {
            let n = (x.wrapping_mul(2654435761) ^ y.wrapping_mul(40503)) % 64;
            ((x * 255 / 2400) as u8).saturating_add(n as u8 / 4)
        }),
    )
    .unwrap();
    let layer = LayerSettings {
        interval: 0.1,
        power: 50.0,
        ..LayerSettings::for_images()
    };
    let d = doc(
        vec![image_object(
            "foto",
            &path,
            [240.0, 240.0],
            affine::translate(20.0, 20.0),
        )],
        vec![layer],
    );
    let dev = device();
    let t = Instant::now();
    let job = compute(&d, &dev, &JobOptions::default()).unwrap();
    let t_job = t.elapsed();
    let moves: usize = job.toolpath.blocks.iter().map(|b| b.moves.len()).sum();
    let t = Instant::now();
    let img = render_tonal(&job, &dev, &options(2048)).unwrap();
    let t_full = t.elapsed();
    let mut m3 = device();
    m3.power_mode = PowerMode::Constant;
    let t = Instant::now();
    render_tonal(&job, &m3, &options(2048)).unwrap();
    let t_m3 = t.elapsed();
    let t = Instant::now();
    let zoom = render_tonal(
        &job,
        &dev,
        &TonalOptions {
            region_mm: Some([100.0, 100.0, 130.0, 130.0]),
            ..options(2048)
        },
    )
    .unwrap();
    let t_zoom = t.elapsed();
    println!(
        "trabajo: {moves} movimientos en {t_job:?}; tonos completos {:?} px en {t_full:?} (con M3 {t_m3:?}); zoom {:?} px en {t_zoom:?}",
        img.size_px, zoom.size_px
    );
    std::fs::remove_dir_all(dir).unwrap();
}
