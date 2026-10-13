//! Tabla de colores ACI (AutoCAD Color Index) a RGB.

use tlaku_core::Rgb;

/// Color por defecto de CAD (ACI 7) cuando no hay nada que resolver.
pub const DEFAULT_COLOR: Rgb = Rgb(0, 0, 0);

/// Convierte un índice ACI a RGB.
///
/// - 1-9: valores exactos de la tabla estándar, salvo el 7: en la tabla es blanco,
///   pero CAD lo dibuja negro sobre fondo claro y en un lienzo blanco sería invisible.
/// - 10-249: 24 tonos (cada 15 grados) por 10 variantes, calculados con la
///   estructura de la tabla estándar: brillo 100/80/60/50/30 % y, en los
///   índices impares, saturación 50 %. Coincide con los valores conocidos de la
///   tabla (p. ej. 10 = FF0000, 11 = FF7F7F, 20 = FF3F00); es una aproximación
///   por truncado, no una copia de la tabla.
/// - 250-255: grises estándar.
/// - 0 (BYBLOCK) no es un color real; se devuelve el color por defecto.
pub fn aci_to_rgb(index: u8) -> Rgb {
    match index {
        1 => Rgb(255, 0, 0),
        2 => Rgb(255, 255, 0),
        3 => Rgb(0, 255, 0),
        4 => Rgb(0, 255, 255),
        5 => Rgb(0, 0, 255),
        6 => Rgb(255, 0, 255),
        7 => DEFAULT_COLOR,
        8 => Rgb(65, 65, 65),
        9 => Rgb(128, 128, 128),
        10..=249 => hsv_entry(index - 10),
        250 => Rgb(51, 51, 51),
        251 => Rgb(80, 80, 80),
        252 => Rgb(105, 105, 105),
        253 => Rgb(130, 130, 130),
        254 => Rgb(190, 190, 190),
        255 => Rgb(255, 255, 255),
        0 => DEFAULT_COLOR,
    }
}

fn hsv_entry(offset: u8) -> Rgb {
    const VALUES: [f64; 5] = [1.0, 0.8, 0.6, 0.5, 0.3];
    let hue = f64::from(offset / 10) * 15.0;
    let k = usize::from(offset % 10);
    let saturation = if k % 2 == 1 { 0.5 } else { 1.0 };
    let max = (VALUES[k / 2] * 255.0 + 1e-9).floor();
    let min = (max * (1.0 - saturation) + 1e-9).floor();

    let sector = (hue / 60.0).floor();
    let frac = hue / 60.0 - sector;
    let rising = (min + (max - min) * frac + 1e-9).floor();
    let falling = (min + (max - min) * (1.0 - frac) + 1e-9).floor();
    let (r, g, b) = match sector as u8 {
        0 => (max, rising, min),
        1 => (falling, max, min),
        2 => (min, max, rising),
        3 => (min, falling, max),
        4 => (rising, min, max),
        _ => (max, min, falling),
    };
    Rgb(r as u8, g as u8, b as u8)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_colors_are_exact() {
        assert_eq!(aci_to_rgb(1), Rgb(255, 0, 0));
        assert_eq!(aci_to_rgb(2), Rgb(255, 255, 0));
        assert_eq!(aci_to_rgb(3), Rgb(0, 255, 0));
        assert_eq!(aci_to_rgb(4), Rgb(0, 255, 255));
        assert_eq!(aci_to_rgb(5), Rgb(0, 0, 255));
        assert_eq!(aci_to_rgb(6), Rgb(255, 0, 255));
        assert_eq!(aci_to_rgb(7), Rgb(0, 0, 0));
        assert_eq!(aci_to_rgb(8), Rgb(65, 65, 65));
        assert_eq!(aci_to_rgb(9), Rgb(128, 128, 128));
    }

    #[test]
    fn known_entries_of_the_hue_ramp() {
        assert_eq!(aci_to_rgb(10), Rgb(255, 0, 0));
        assert_eq!(aci_to_rgb(11), Rgb(255, 127, 127));
        assert_eq!(aci_to_rgb(12), Rgb(204, 0, 0));
        assert_eq!(aci_to_rgb(14), Rgb(153, 0, 0));
        assert_eq!(aci_to_rgb(16), Rgb(127, 0, 0));
        assert_eq!(aci_to_rgb(18), Rgb(76, 0, 0));
        assert_eq!(aci_to_rgb(20), Rgb(255, 63, 0));
        assert_eq!(aci_to_rgb(30), Rgb(255, 127, 0));
        assert_eq!(aci_to_rgb(50), Rgb(255, 255, 0));
        assert_eq!(aci_to_rgb(90), Rgb(0, 255, 0));
        assert_eq!(aci_to_rgb(130), Rgb(0, 255, 255));
        assert_eq!(aci_to_rgb(170), Rgb(0, 0, 255));
        assert_eq!(aci_to_rgb(210), Rgb(255, 0, 255));
    }

    #[test]
    fn grays_and_total_function() {
        assert_eq!(aci_to_rgb(250), Rgb(51, 51, 51));
        assert_eq!(aci_to_rgb(255), Rgb(255, 255, 255));
        for i in 0..=255u8 {
            let _ = aci_to_rgb(i);
        }
    }
}
