//! Composición de fotos sobre la cama: de una sola foto corregida (captura de la cama) o de
//! varias tomadas con la cámara montada en el cabezal (mosaico por posición conocida).
//!
//! Cada foto llega con su transformación píxel -> milímetros de la cama (`to_bed`, con el origen
//! de píxel en el borde de la imagen, como el resto de la alineación). El resultado es una imagen
//! alineada con los ejes de la cama: su fila de arriba es la `Y` mayor, así que, colocada en el
//! documento con la esquina inferior izquierda en `origin_mm`, queda exactamente donde estaba.
//!
//! Mezcla en los bordes: cada foto aporta con un peso que baja a casi cero hacia su borde (rampa
//! suave de `feather_px`), de modo que en la zona de traslape se pasa de una a otra sin escalón
//! aunque la exposición cambie. Un margen `crop_px` descarta la franja del borde, que en una
//! cámara de cabezal es la más borrosa y deformada.

use crate::homography::Homography;
use image::RgbImage;
use tlaku_core::Point;

/// Una foto con su posición en la cama.
pub struct Tile<'a> {
    pub image: &'a RgbImage,
    /// Píxel (origen en el borde de la imagen) -> mm de la cama.
    pub to_bed: Homography,
}

#[derive(Debug, Clone, Copy)]
pub struct MosaicOptions {
    /// Rectángulo a componer, en mm de la cama: `[x_min, y_min, x_max, y_max]`.
    pub rect: [f64; 4],
    /// Resolución de salida.
    pub px_per_mm: f64,
    /// Ancho (px de la foto) de la rampa de mezcla en los bordes.
    pub feather_px: f64,
    /// Franja (px de la foto) que se descarta en cada borde.
    pub crop_px: f64,
    /// Color donde ninguna foto llega.
    pub background: [u8; 3],
}

impl MosaicOptions {
    pub fn new(rect: [f64; 4], px_per_mm: f64) -> Self {
        Self {
            rect,
            px_per_mm,
            feather_px: 24.0,
            crop_px: 0.0,
            background: [255, 255, 255],
        }
    }
}

#[derive(Debug, Clone)]
pub struct Mosaic {
    pub image: RgbImage,
    /// Esquina inferior izquierda de la imagen en la cama (mm).
    pub origin_mm: [f64; 2],
    /// Tamaño de la imagen en la cama (mm); es exactamente `píxeles / px_per_mm`.
    pub size_mm: [f64; 2],
    pub px_per_mm: f64,
    /// Fracción de píxeles cubierta por alguna foto (0..1).
    pub coverage: f64,
}

/// Límite de píxeles de salida (imágenes enormes agotarían la memoria sin aportar nada).
const MAX_PIXELS: f64 = 60e6;

/// Rectángulo (mm) que cubren las fotos, recortado a la cama `[0, bed.0] x [0, bed.1]`. `None`
/// si no cae nada dentro de la cama.
pub fn covered_rect(tiles: &[Tile], bed: [f64; 2]) -> Option<[f64; 4]> {
    let mut r = [f64::MAX, f64::MAX, f64::MIN, f64::MIN];
    for t in tiles {
        let (w, h) = (f64::from(t.image.width()), f64::from(t.image.height()));
        for (u, v) in [(0.0, 0.0), (w, 0.0), (w, h), (0.0, h)] {
            let p = t.to_bed.apply(Point::new(u, v));
            if !p.x.is_finite() || !p.y.is_finite() {
                return None;
            }
            r = [r[0].min(p.x), r[1].min(p.y), r[2].max(p.x), r[3].max(p.y)];
        }
    }
    let clipped = [
        r[0].max(0.0),
        r[1].max(0.0),
        r[2].min(bed[0]),
        r[3].min(bed[1]),
    ];
    (clipped[2] - clipped[0] > 1e-6 && clipped[3] - clipped[1] > 1e-6).then_some(clipped)
}

/// Escala aproximada de una foto en el centro (píxeles por mm).
pub fn tile_scale_px_per_mm(t: &Tile) -> f64 {
    let (w, h) = (f64::from(t.image.width()), f64::from(t.image.height()));
    let c = Point::new(w / 2.0, h / 2.0);
    let (a, b) = (
        t.to_bed.apply(c),
        t.to_bed.apply(Point::new(c.x + 10.0, c.y)),
    );
    let (d, e) = (
        t.to_bed.apply(c),
        t.to_bed.apply(Point::new(c.x, c.y + 10.0)),
    );
    let mm_x = a.dist(b) / 10.0;
    let mm_y = d.dist(e) / 10.0;
    let mm = (mm_x * mm_y).sqrt();
    if mm > 0.0 { 1.0 / mm } else { 0.0 }
}

fn smooth(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Muestreo bilineal de una imagen RGB en coordenadas de centro de píxel.
fn sample(img: &RgbImage, x: f64, y: f64) -> [f32; 3] {
    let (w, h) = (img.width() as usize, img.height() as usize);
    let x0 = (x.floor() as usize).min(w.saturating_sub(2));
    let y0 = (y.floor() as usize).min(h.saturating_sub(2));
    let (fx, fy) = ((x - x0 as f64) as f32, (y - y0 as f64) as f32);
    let (x1, y1) = ((x0 + 1).min(w - 1), (y0 + 1).min(h - 1));
    let raw = img.as_raw();
    let px = |i: usize, j: usize, c: usize| f32::from(raw[(j * w + i) * 3 + c]);
    let mut out = [0.0; 3];
    for (c, o) in out.iter_mut().enumerate() {
        let a = px(x0, y0, c) * (1.0 - fx) + px(x1, y0, c) * fx;
        let b = px(x0, y1, c) * (1.0 - fx) + px(x1, y1, c) * fx;
        *o = a * (1.0 - fy) + b * fy;
    }
    out
}

/// Compone las fotos en una imagen alineada con la cama.
pub fn compose(tiles: &[Tile], opts: &MosaicOptions) -> Result<Mosaic, String> {
    if tiles.is_empty() {
        return Err("no hay ninguna foto que componer".into());
    }
    let [x0, y0, x1, y1] = opts.rect;
    if !(opts.rect.iter().all(|v| v.is_finite()) && x1 > x0 && y1 > y0) {
        return Err("el rectángulo a componer no es válido".into());
    }
    if !(opts.px_per_mm.is_finite() && opts.px_per_mm > 0.0) {
        return Err("la resolución de la imagen debe ser mayor que 0".into());
    }
    let ppm = opts.px_per_mm;
    let (wpx, hpx) = (((x1 - x0) * ppm).ceil(), ((y1 - y0) * ppm).ceil());
    if wpx * hpx > MAX_PIXELS {
        return Err(format!(
            "la imagen sería de {wpx} x {hpx} píxeles; baja la resolución"
        ));
    }
    let (wpx, hpx) = (wpx.max(1.0) as usize, hpx.max(1.0) as usize);
    // La imagen cuelga de la esquina superior izquierda (x0, y1).
    let size_mm = [wpx as f64 / ppm, hpx as f64 / ppm];
    let origin_mm = [x0, y1 - size_mm[1]];

    let mut acc = vec![0.0f32; wpx * hpx * 3];
    let mut weight = vec![0.0f32; wpx * hpx];
    for t in tiles {
        let (tw, th) = (t.image.width() as usize, t.image.height() as usize);
        if tw < 2 || th < 2 {
            continue;
        }
        let inv = t.to_bed.inverse()?;
        // Caja de la foto en la cama: solo se recorren los píxeles de salida que la tocan.
        let (mut bx0, mut by0, mut bx1, mut by1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
        for (u, v) in [
            (0.0, 0.0),
            (tw as f64, 0.0),
            (tw as f64, th as f64),
            (0.0, th as f64),
        ] {
            let p = t.to_bed.apply(Point::new(u, v));
            if !(p.x.is_finite() && p.y.is_finite()) {
                return Err("una foto se proyecta al infinito: revisa su alineación".into());
            }
            bx0 = bx0.min(p.x);
            by0 = by0.min(p.y);
            bx1 = bx1.max(p.x);
            by1 = by1.max(p.y);
        }
        let i_lo = (((bx0 - x0) * ppm).floor().max(0.0)) as usize;
        let i_hi = ((((bx1 - x0) * ppm).ceil()).max(0.0) as usize).min(wpx);
        let j_lo = (((y1 - by1) * ppm).floor().max(0.0)) as usize;
        let j_hi = ((((y1 - by0) * ppm).ceil()).max(0.0) as usize).min(hpx);
        let crop = opts.crop_px.max(0.0);
        let feather = opts.feather_px.max(1.0);
        for j in j_lo..j_hi {
            let by = y1 - (j as f64 + 0.5) / ppm;
            for i in i_lo..i_hi {
                let bx = x0 + (i as f64 + 0.5) / ppm;
                let src = inv.apply(Point::new(bx, by));
                // Coordenadas de centro de píxel de la foto.
                let (u, v) = (src.x - 0.5, src.y - 0.5);
                if !(u.is_finite() && v.is_finite()) {
                    continue;
                }
                let d = u.min(v).min(tw as f64 - 1.0 - u).min(th as f64 - 1.0 - v) - crop;
                if d < 0.0 {
                    continue;
                }
                // Peso mínimo > 0: con una sola foto el borde no deja huecos.
                let wgt = (0.02 + 0.98 * smooth(d / feather)) as f32;
                let rgb = sample(t.image, u, v);
                let k = j * wpx + i;
                for c in 0..3 {
                    acc[k * 3 + c] += rgb[c] * wgt;
                }
                weight[k] += wgt;
            }
        }
    }
    let mut out = RgbImage::from_pixel(wpx as u32, hpx as u32, image::Rgb(opts.background));
    let mut covered = 0usize;
    for (k, px) in out.pixels_mut().enumerate() {
        if weight[k] > 0.0 {
            covered += 1;
            for c in 0..3 {
                px.0[c] = (acc[k * 3 + c] / weight[k]).round().clamp(0.0, 255.0) as u8;
            }
        }
    }
    Ok(Mosaic {
        image: out,
        origin_mm,
        size_mm,
        px_per_mm: ppm,
        coverage: covered as f64 / (wpx * hpx) as f64,
    })
}

/// Pasa la imagen a gris (luma Rec. 601) conservando los tres canales.
pub fn to_gray(img: &mut RgbImage) {
    for p in img.pixels_mut() {
        let g = (0.299 * f32::from(p.0[0]) + 0.587 * f32::from(p.0[1]) + 0.114 * f32::from(p.0[2]))
            .round()
            .clamp(0.0, 255.0) as u8;
        p.0 = [g, g, g];
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Suelo sintético (mm -> color): ondas suaves más un disco brillante en (`DISC`).
    const DISC: (f64, f64, f64) = (150.0, 90.0, 8.0);

    fn ground(x: f64, y: f64) -> [u8; 3] {
        let base = 120.0 + 50.0 * (x / 17.0).sin() * (y / 23.0).cos();
        let (dx, dy) = (x - DISC.0, y - DISC.1);
        let on = (dx * dx + dy * dy).sqrt() < DISC.2;
        let v = if on { 250.0 } else { base };
        [v as u8, (v * 0.9) as u8, (v * 0.8) as u8]
    }

    /// Foto de `w x h` px de la zona con esquina superior izquierda en (`cx`, `cy`) mm
    /// (con Y hacia arriba), a `ppm` px/mm, girada `deg` grados. Devuelve imagen y `to_bed`.
    fn shoot(
        cx: f64,
        cy: f64,
        w: u32,
        h: u32,
        ppm: f64,
        deg: f64,
        gain: f64,
    ) -> (RgbImage, Homography) {
        let (s, c) = deg.to_radians().sin_cos();
        // píxel (u, v) -> mm: x = cx + (c u + s v)/ppm ; y = cy - (-s u + c v)/ppm
        let m = [
            [c / ppm, s / ppm, cx],
            [s / ppm, -c / ppm, cy],
            [0.0, 0.0, 1.0],
        ];
        let to_bed = Homography { m };
        let img = RgbImage::from_fn(w, h, |i, j| {
            let p = to_bed.apply(Point::new(f64::from(i) + 0.5, f64::from(j) + 0.5));
            let g = ground(p.x, p.y);
            image::Rgb(g.map(|v| (f64::from(v) * gain).clamp(0.0, 255.0) as u8))
        });
        (img, to_bed)
    }

    #[test]
    fn una_foto_cae_exactamente_donde_estaba() {
        // Foto de 200x150 px a 2 px/mm: cubre 100x75 mm con la esquina en (100, 140).
        let (img, to_bed) = shoot(100.0, 140.0, 200, 150, 2.0, 0.0, 1.0);
        let tiles = [Tile {
            image: &img,
            to_bed,
        }];
        let rect = covered_rect(&tiles, [300.0, 200.0]).unwrap();
        assert_eq!(rect, [100.0, 65.0, 200.0, 140.0]);
        let m = compose(&tiles, &MosaicOptions::new(rect, 2.0)).unwrap();
        assert_eq!(m.image.dimensions(), (200, 150));
        assert!((m.size_mm[0] - 100.0).abs() < 1e-9 && (m.size_mm[1] - 75.0).abs() < 1e-9);
        assert!((m.origin_mm[0] - 100.0).abs() < 1e-9 && (m.origin_mm[1] - 65.0).abs() < 1e-9);
        assert!((m.coverage - 1.0).abs() < 1e-9);
        // Cada píxel coincide con el suelo en su posición (tolerancia: interpolación).
        let mut worst = 0i32;
        for j in 0..150 {
            for i in 0..200 {
                let bx = m.origin_mm[0] + (f64::from(i) + 0.5) / 2.0;
                let by = m.origin_mm[1] + m.size_mm[1] - (f64::from(j) + 0.5) / 2.0;
                let g = ground(bx, by);
                let p = m.image.get_pixel(i, j).0;
                worst = worst.max((i32::from(g[0]) - i32::from(p[0])).abs());
            }
        }
        assert!(worst <= 2, "diferencia máxima {worst}");
    }

    /// Centroide (mm) de los píxeles muy brillantes del mosaico.
    fn disc_centroid(m: &Mosaic) -> (f64, f64) {
        let (mut sx, mut sy, mut n) = (0.0, 0.0, 0.0);
        for (i, j, p) in m.image.enumerate_pixels() {
            if p.0[0] >= 245 {
                sx += m.origin_mm[0] + (f64::from(i) + 0.5) / m.px_per_mm;
                sy += m.origin_mm[1] + m.size_mm[1] - (f64::from(j) + 0.5) / m.px_per_mm;
                n += 1.0;
            }
        }
        (sx / n, sy / n)
    }

    #[test]
    fn el_mosaico_de_una_cuadricula_con_giros_pequenos_conserva_posiciones() {
        // 3x2 fotos de 160x120 px a 2 px/mm (80x60 mm) con 25 % de traslape y un giro de 0.4°.
        let (w, h, ppm) = (160u32, 120u32, 2.0);
        let mut shots = Vec::new();
        for r in 0..2 {
            for c in 0..3 {
                let cx = 60.0 + f64::from(c) * 60.0;
                let cy = 160.0 - f64::from(r) * 45.0;
                shots.push(shoot(cx, cy, w, h, ppm, 0.4, 1.0));
            }
        }
        let tiles: Vec<Tile> = shots
            .iter()
            .map(|(img, to_bed)| Tile {
                image: img,
                to_bed: *to_bed,
            })
            .collect();
        let full = covered_rect(&tiles, [300.0, 200.0]).unwrap();
        // El margen descartado de cada foto deja un borde sin cobertura en el contorno del
        // mosaico (3 mm) y los vértices de las fotos giradas; se compone sin ese contorno.
        let rect = [full[0] + 4.0, full[1] + 4.0, full[2] - 4.0, full[3] - 4.0];
        let mut opts = MosaicOptions::new(rect, 2.0);
        opts.crop_px = 6.0;
        let m = compose(&tiles, &opts).unwrap();
        assert!(m.coverage > 0.999, "cobertura {}", m.coverage);
        let (cx, cy) = disc_centroid(&m);
        eprintln!(
            "disco: ({cx:.3}, {cy:.3}) mm, verdadero ({}, {})",
            DISC.0, DISC.1
        );
        assert!((cx - DISC.0).abs() < 0.15 && (cy - DISC.1).abs() < 0.15);
    }

    #[test]
    fn la_mezcla_en_el_traslape_suaviza_el_cambio_de_exposicion() {
        // Dos fotos que se traslapan 40 mm; la segunda sale 25 % más clara.
        let (a, ha) = shoot(0.0, 100.0, 200, 100, 2.0, 0.0, 1.0);
        let (b, hb) = shoot(60.0, 100.0, 200, 100, 2.0, 0.0, 1.25);
        let tiles = [
            Tile {
                image: &a,
                to_bed: ha,
            },
            Tile {
                image: &b,
                to_bed: hb,
            },
        ];
        let mut opts = MosaicOptions::new([0.0, 50.0, 160.0, 100.0], 2.0);
        opts.feather_px = 60.0;
        let m = compose(&tiles, &opts).unwrap();
        // Fila central: sin escalones grandes entre vecinos aunque los dos tonos difieran.
        let y = m.image.height() / 2;
        let mut max_step = 0i32;
        for x in 1..m.image.width() {
            let p = i32::from(m.image.get_pixel(x, y).0[0]);
            let q = i32::from(m.image.get_pixel(x - 1, y).0[0]);
            max_step = max_step.max((p - q).abs());
        }
        // Sin mezcla, el escalón sería del orden de 25 niveles (25 % de ~120).
        let mut hard = opts;
        hard.feather_px = 1.0;
        let h = compose(&tiles, &hard).unwrap();
        let mut hard_step = 0i32;
        for x in 1..h.image.width() {
            let p = i32::from(h.image.get_pixel(x, y).0[0]);
            let q = i32::from(h.image.get_pixel(x - 1, y).0[0]);
            hard_step = hard_step.max((p - q).abs());
        }
        eprintln!("escalón máximo: mezclado {max_step}, sin mezclar {hard_step}");
        assert!(max_step < hard_step, "{max_step} vs {hard_step}");
    }

    #[test]
    fn valida_entradas() {
        let (img, to_bed) = shoot(0.0, 50.0, 40, 40, 1.0, 0.0, 1.0);
        let t = [Tile {
            image: &img,
            to_bed,
        }];
        assert!(compose(&[], &MosaicOptions::new([0.0, 0.0, 10.0, 10.0], 1.0)).is_err());
        assert!(compose(&t, &MosaicOptions::new([5.0, 0.0, 5.0, 10.0], 1.0)).is_err());
        assert!(compose(&t, &MosaicOptions::new([0.0, 0.0, 10.0, 10.0], 0.0)).is_err());
        assert!(compose(&t, &MosaicOptions::new([0.0, 0.0, 1e5, 1e5], 10.0)).is_err());
        // Una foto fuera de la cama no aporta rectángulo.
        assert!(covered_rect(&t, [30.0, 5.0]).is_none());
        let scale = tile_scale_px_per_mm(&t[0]);
        assert!((scale - 1.0).abs() < 1e-9);
        let mut g = img.clone();
        to_gray(&mut g);
        let p = g.get_pixel(3, 3).0;
        assert!(p[0] == p[1] && p[1] == p[2]);
    }
}
