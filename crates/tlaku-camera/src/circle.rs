//! Detección de un círculo en una foto (buscador de centro): bordes con Sobel, transformada de
//! Hough con la dirección del gradiente para el centro, histograma de distancias para el radio
//! y ajuste por mínimos cuadrados (Kåsa) de los puntos de borde que concuerdan.
//!
//! Sirve para objetos redondos (tazas, medallas, tapas) fotografiados desde arriba. Devuelve
//! también una muestra de los puntos del borde: quien llama los lleva a milímetros y vuelve a
//! ajustar el círculo allí, porque la perspectiva convierte el círculo en una elipse y su
//! centro en la imagen no es el centro real.

use crate::imgproc::{FloatImage, gaussian_blur};
use image::GrayImage;
use nalgebra::{Matrix3, Vector3};
use serde::{Deserialize, Serialize};
use tlaku_core::Point;

/// Lo que el usuario sabe del círculo: un punto dentro (o cerca del centro) y, si quiere, el
/// radio aproximado.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CircleHint {
    pub center: [f64; 2],
    /// Radio aproximado en píxeles; si falta se busca entre un 5 % y un 60 % del lado menor.
    #[serde(default)]
    pub radius: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CircleFit {
    pub center: [f64; 2],
    pub radius: f64,
    /// Distancia rms (px) de los puntos de borde al círculo ajustado.
    pub rms_px: f64,
    /// Puntos de borde que concuerdan con el círculo.
    pub inliers: usize,
    /// Fracción del contorno visible (0..1), por sectores de 15 grados con algún punto.
    pub arc_coverage: f64,
    /// Muestra (hasta 48) de puntos de borde del círculo, repartida por ángulo (px).
    pub points: Vec<[f64; 2]>,
}

/// Lado mayor al que se reduce la imagen para buscar (la precisión final sale del ajuste con
/// los puntos de la imagen original).
const WORK_SIZE: f64 = 640.0;
const SECTORS: usize = 24;
/// Con menos contorno visible que esto no se acepta (un arco corto es ambiguo).
const MIN_COVERAGE: f64 = 0.45;

struct Edges {
    /// x, y, ux, uy (gradiente unitario).
    pts: Vec<[f32; 4]>,
}

fn sobel_edges(img: &GrayImage) -> Edges {
    let src = FloatImage::from_gray(img);
    let b = gaussian_blur(&src, 1.2);
    let (w, h) = (b.w, b.h);
    let mut mags = vec![0.0f32; w * h];
    let mut dirs = vec![(0.0f32, 0.0f32); w * h];
    for y in 1..h.saturating_sub(1) {
        for x in 1..w.saturating_sub(1) {
            let g = |dx: isize, dy: isize| {
                b.get((x as isize + dx) as usize, (y as isize + dy) as usize)
            };
            let gx = (g(1, -1) + 2.0 * g(1, 0) + g(1, 1)) - (g(-1, -1) + 2.0 * g(-1, 0) + g(-1, 1));
            let gy = (g(-1, 1) + 2.0 * g(0, 1) + g(1, 1)) - (g(-1, -1) + 2.0 * g(0, -1) + g(1, -1));
            let m = gx.hypot(gy);
            mags[y * w + x] = m;
            dirs[y * w + x] = (gx, gy);
        }
    }
    let mut sorted: Vec<f32> = mags.iter().copied().filter(|&m| m > 0.0).collect();
    sorted.sort_by(f32::total_cmp);
    let p99 = sorted.get(sorted.len() * 99 / 100).copied().unwrap_or(0.0);
    // Sobel suma 4 veces la diferencia: 24 equivale a ~6 niveles de gris de salto.
    let thr = (0.3 * p99).max(24.0);
    let mut pts = Vec::new();
    for y in 1..h.saturating_sub(1) {
        for x in 1..w.saturating_sub(1) {
            let k = y * w + x;
            let m = mags[k];
            if m < thr {
                continue;
            }
            // Supresión de no máximos simple: el mayor de sus dos vecinos en la dirección del gradiente.
            let (gx, gy) = dirs[k];
            let (ux, uy) = (gx / m, gy / m);
            let (nx, ny) = (
                (x as f32 + ux).round() as usize,
                (y as f32 + uy).round() as usize,
            );
            let (px, py) = (
                (x as f32 - ux).round() as usize,
                (y as f32 - uy).round() as usize,
            );
            if mags[ny * w + nx] > m || mags[py * w + px] > m {
                continue;
            }
            pts.push([x as f32, y as f32, ux, uy]);
        }
    }
    Edges { pts }
}

/// Ajuste algebraico (Kåsa) de un círculo a puntos: `x² + y² + a x + b y + c = 0`.
fn fit_circle(pts: &[[f64; 2]]) -> Option<(f64, f64, f64)> {
    if pts.len() < 3 {
        return None;
    }
    let n = pts.len() as f64;
    let (mx, my) = (
        pts.iter().map(|p| p[0]).sum::<f64>() / n,
        pts.iter().map(|p| p[1]).sum::<f64>() / n,
    );
    // Centrado para el condicionamiento.
    let (mut suu, mut suv, mut svv, mut suuu, mut svvv, mut suvv, mut svuu) =
        (0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
    for p in pts {
        let (u, v) = (p[0] - mx, p[1] - my);
        suu += u * u;
        suv += u * v;
        svv += v * v;
        suuu += u * u * u;
        svvv += v * v * v;
        suvv += u * v * v;
        svuu += v * u * u;
    }
    let a = Matrix3::new(suu, suv, 0.0, suv, svv, 0.0, 0.0, 0.0, 1.0);
    let det = suu * svv - suv * suv;
    if det.abs() < 1e-9 * (suu + svv).powi(2) || !det.is_finite() {
        return None;
    }
    let rhs = Vector3::new(0.5 * (suuu + suvv), 0.5 * (svvv + svuu), 0.0);
    let sol = a.try_inverse()? * rhs;
    let (uc, vc) = (sol.x, sol.y);
    let r = (uc * uc + vc * vc + (suu + svv) / n).sqrt();
    (r.is_finite() && r > 0.0).then_some((uc + mx, vc + my, r))
}

/// Busca el círculo más probable. `hint` acota la búsqueda (recomendado cuando hay varios
/// objetos); sin él se toma el de mayor apoyo en toda la imagen.
pub fn detect_circle(img: &GrayImage, hint: Option<CircleHint>) -> Result<CircleFit, String> {
    let (w0, h0) = (f64::from(img.width()), f64::from(img.height()));
    if img.width() < 32 || img.height() < 32 {
        return Err("la imagen es demasiado pequeña para buscar un círculo".into());
    }
    let scale = (WORK_SIZE / w0.max(h0)).min(1.0);
    let small;
    let work: &GrayImage = if scale < 1.0 {
        small = image::imageops::resize(
            img,
            (w0 * scale).round() as u32,
            (h0 * scale).round() as u32,
            image::imageops::FilterType::Triangle,
        );
        &small
    } else {
        img
    };
    let (w, h) = (work.width() as usize, work.height() as usize);
    let edges = sobel_edges(work);
    if edges.pts.len() < 30 {
        return Err("no se ven bordes en la imagen; mejora el contraste o la iluminación".into());
    }
    let min_side = w.min(h) as f64;
    let (rmin, rmax) = match hint.and_then(|h| h.radius) {
        Some(r) => ((0.6 * r * scale).max(4.0), 1.5 * r * scale),
        None => (0.05 * min_side, 0.6 * min_side),
    };
    let hint_c = hint.map(|h| (h.center[0] * scale, h.center[1] * scale));
    let hint_reach = hint.map(|h| h.radius.map_or(0.5 * min_side, |r| 0.5 * r * scale + 6.0));

    // Acumulador de centros: cada borde vota a lo largo de su gradiente, en los dos sentidos.
    let mut acc = FloatImage {
        w,
        h,
        data: vec![0.0; w * h],
    };
    let step = 1.0f32;
    for e in &edges.pts {
        for sign in [-1.0f32, 1.0] {
            let mut r = rmin as f32;
            while r <= rmax as f32 {
                let cx = e[0] + sign * e[2] * r;
                let cy = e[1] + sign * e[3] * r;
                let (ix, iy) = (cx.round(), cy.round());
                if ix >= 0.0 && iy >= 0.0 && (ix as usize) < w && (iy as usize) < h {
                    acc.data[iy as usize * w + ix as usize] += 1.0;
                }
                r += step;
            }
        }
    }
    let acc = gaussian_blur(&acc, 2.0);
    // Mejor candidato a centro (dentro del alcance del indicio, si lo hay).
    let mut best = (0.0f32, 0usize, 0usize);
    for y in 0..h {
        for x in 0..w {
            if let (Some((hx, hy)), Some(reach)) = (hint_c, hint_reach)
                && (x as f64 - hx).hypot(y as f64 - hy) > reach
            {
                continue;
            }
            let v = acc.get(x, y);
            if v > best.0 {
                best = (v, x, y);
            }
        }
    }
    if best.0 < 8.0 {
        return Err("no se encontró ningún círculo en la imagen".into());
    }
    let (c0x, c0y) = (best.1 as f64, best.2 as f64);

    // Radio: pico del histograma de distancias de los bordes cuyo gradiente apunta al centro.
    let nbins = rmax.ceil() as usize + 2;
    let mut hist = vec![0.0f32; nbins];
    for e in &edges.pts {
        let (dx, dy) = (f64::from(e[0]) - c0x, f64::from(e[1]) - c0y);
        let d = dx.hypot(dy);
        if d < rmin || d > rmax || d < 1e-9 {
            continue;
        }
        let align = ((dx * f64::from(e[2]) + dy * f64::from(e[3])) / d).abs();
        if align > 0.85 {
            hist[d.round() as usize] += 1.0;
        }
    }
    let mut r0 = 0usize;
    let mut best_h = 0.0f32;
    for i in 1..nbins - 1 {
        let s = hist[i - 1] + hist[i] + hist[i + 1];
        if s > best_h {
            best_h = s;
            r0 = i;
        }
    }
    if best_h < 8.0 || r0 == 0 {
        return Err("no se encontró ningún círculo en la imagen".into());
    }

    // Refinado en la imagen original: se repiten los bordes allí para no perder precisión.
    let full_edges = if scale < 1.0 {
        sobel_edges(img)
    } else {
        Edges {
            pts: edges.pts.clone(),
        }
    };
    let inv = 1.0 / scale;
    let (mut cx, mut cy, mut r) = (c0x * inv, c0y * inv, r0 as f64 * inv);
    let mut inliers: Vec<[f64; 2]> = Vec::new();
    for tol_k in [0.08, 0.04, 0.02, 0.015] {
        let tol = (tol_k * r).max(1.5);
        inliers = full_edges
            .pts
            .iter()
            .filter_map(|e| {
                let (dx, dy) = (f64::from(e[0]) - cx, f64::from(e[1]) - cy);
                let d = dx.hypot(dy);
                if d < 1e-9 || (d - r).abs() > tol {
                    return None;
                }
                let align = ((dx * f64::from(e[2]) + dy * f64::from(e[3])) / d).abs();
                (align > 0.8).then_some([f64::from(e[0]), f64::from(e[1])])
            })
            .collect();
        match fit_circle(&inliers) {
            Some((x, y, rr)) => {
                cx = x;
                cy = y;
                r = rr;
            }
            None => return Err("no se encontró ningún círculo en la imagen".into()),
        }
    }
    // Los puntos de borde de Sobel están en el centro del píxel; el origen de la alineación es
    // el borde de la imagen, así que se suma medio píxel al pasar a esas coordenadas.
    let rms = (inliers
        .iter()
        .map(|p| ((p[0] - cx).hypot(p[1] - cy) - r).powi(2))
        .sum::<f64>()
        / inliers.len().max(1) as f64)
        .sqrt();
    let mut sectors = [false; SECTORS];
    for p in &inliers {
        let a = (p[1] - cy).atan2(p[0] - cx);
        let k =
            (((a + std::f64::consts::PI) / (2.0 * std::f64::consts::PI)) * SECTORS as f64) as usize;
        sectors[k.min(SECTORS - 1)] = true;
    }
    let coverage = sectors.iter().filter(|&&s| s).count() as f64 / SECTORS as f64;
    if coverage < MIN_COVERAGE || inliers.len() < 24 {
        return Err(format!(
            "solo se ve el {:.0} % del contorno; el círculo debe verse casi completo",
            coverage * 100.0
        ));
    }
    // Muestra repartida por ángulo: un punto por sector como máximo, hasta 48.
    let mut by_sector: Vec<Option<[f64; 2]>> = vec![None; 48];
    for p in &inliers {
        let a = (p[1] - cy).atan2(p[0] - cx);
        let k = (((a + std::f64::consts::PI) / (2.0 * std::f64::consts::PI)) * 48.0) as usize;
        let k = k.min(47);
        let better = by_sector[k].is_none_or(|q| {
            ((p[0] - cx).hypot(p[1] - cy) - r).abs() < ((q[0] - cx).hypot(q[1] - cy) - r).abs()
        });
        if better {
            by_sector[k] = Some(*p);
        }
    }
    let points: Vec<[f64; 2]> = by_sector
        .into_iter()
        .flatten()
        .map(|p| [p[0] + 0.5, p[1] + 0.5])
        .collect();
    Ok(CircleFit {
        center: [cx + 0.5, cy + 0.5],
        radius: r,
        rms_px: rms,
        inliers: inliers.len(),
        arc_coverage: coverage,
        points,
    })
}

/// Círculo por tres puntos (circuncírculo). `None` si están casi alineados.
pub fn circle_from_three(a: Point, b: Point, c: Point) -> Option<(Point, f64)> {
    let d = 2.0 * (a.x * (b.y - c.y) + b.x * (c.y - a.y) + c.x * (a.y - b.y));
    let scale = a.dist(b).max(b.dist(c)).max(a.dist(c));
    if d.abs() < 1e-9 * scale * scale || !d.is_finite() {
        return None;
    }
    let (a2, b2, c2) = (
        a.x * a.x + a.y * a.y,
        b.x * b.x + b.y * b.y,
        c.x * c.x + c.y * c.y,
    );
    let ux = (a2 * (b.y - c.y) + b2 * (c.y - a.y) + c2 * (a.y - b.y)) / d;
    let uy = (a2 * (c.x - b.x) + b2 * (a.x - c.x) + c2 * (b.x - a.x)) / d;
    let center = Point::new(ux, uy);
    Some((center, center.dist(a)))
}

/// Ajusta un círculo a puntos de un borde (mínimos cuadrados, Kåsa): centro y radio.
pub fn fit_circle_points(pts: &[Point]) -> Option<(Point, f64)> {
    let v: Vec<[f64; 2]> = pts.iter().map(|p| [p.x, p.y]).collect();
    fit_circle(&v).map(|(x, y, r)| (Point::new(x, y), r))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::Rng;

    /// Disco oscuro (o claro) sobre fondo con degradado, suavizado por supermuestreo de 4x4.
    fn disc(w: u32, h: u32, c: (f64, f64), r: f64, dark: bool, noise: f64, seed: u64) -> GrayImage {
        let mut rng = Rng::new(seed);
        GrayImage::from_fn(w, h, |x, y| {
            let mut acc = 0.0;
            for sj in 0..4 {
                for si in 0..4 {
                    let px = f64::from(x) + (f64::from(si) + 0.5) / 4.0;
                    let py = f64::from(y) + (f64::from(sj) + 0.5) / 4.0;
                    let inside = (px - c.0).hypot(py - c.1) < r;
                    let bg = 190.0 + 20.0 * (px / w as f64);
                    let fg = if dark { 50.0 } else { 245.0 };
                    acc += if inside { fg } else { bg };
                }
            }
            image::Luma([(acc / 16.0 + noise * rng.gauss()).clamp(0.0, 255.0) as u8])
        })
    }

    #[test]
    fn encuentra_el_centro_de_un_disco_con_precision_subpixel() {
        let (c, r) = ((301.3, 212.7), 78.4);
        for (dark, seed) in [(true, 1), (false, 2)] {
            let img = disc(640, 480, c, r, dark, 2.0, seed);
            let f = detect_circle(&img, None).unwrap();
            eprintln!(
                "centro ({:.3}, {:.3}) r {:.3}  rms {:.3} cobertura {:.2} puntos {}",
                f.center[0],
                f.center[1],
                f.radius,
                f.rms_px,
                f.arc_coverage,
                f.points.len()
            );
            // El centro del píxel (i, j) es (i + 0.5, j + 0.5) en coordenadas de borde.
            assert!((f.center[0] - c.0).abs() < 0.35 && (f.center[1] - c.1).abs() < 0.35);
            assert!((f.radius - r).abs() < 0.5, "radio {}", f.radius);
            assert!(f.arc_coverage > 0.95 && f.points.len() >= 40);
        }
    }

    #[test]
    fn con_indicio_elige_el_circulo_pedido_y_tolera_un_borde_cortado() {
        // Dos discos: uno grande a la izquierda y uno pequeño pegado al borde derecho.
        let mut img = disc(640, 480, (200.0, 240.0), 110.0, true, 1.5, 3);
        let small = disc(640, 480, (600.0, 150.0), 50.0, true, 1.5, 4);
        for (x, y, p) in small.enumerate_pixels() {
            if (f64::from(x) - 600.0).hypot(f64::from(y) - 150.0) < 60.0 {
                img.put_pixel(x, y, *p);
            }
        }
        let hint = CircleHint {
            center: [600.0, 150.0],
            radius: Some(50.0),
        };
        let f = detect_circle(&img, Some(hint)).unwrap();
        assert!(
            (f.center[0] - 600.0).abs() < 0.6 && (f.center[1] - 150.0).abs() < 0.6,
            "{:?}",
            f.center
        );
        assert!((f.radius - 50.0).abs() < 0.8);
        // Sin indicio gana el grande.
        let big = detect_circle(&img, None).unwrap();
        assert!((big.center[0] - 200.0).abs() < 0.6, "{:?}", big.center);
        // Un círculo cortado por el borde de la imagen (la mitad visible) sigue funcionando.
        let cut = disc(640, 480, (630.0, 240.0), 100.0, true, 1.5, 5);
        let f = detect_circle(
            &cut,
            Some(CircleHint {
                center: [610.0, 240.0],
                radius: Some(100.0),
            }),
        );
        match f {
            Ok(f) => assert!((f.center[0] - 630.0).abs() < 1.0 && (f.radius - 100.0).abs() < 1.5),
            Err(e) => assert!(e.contains("contorno"), "{e}"),
        }
    }

    #[test]
    fn sin_circulo_da_error_claro() {
        let blank = GrayImage::from_pixel(320, 240, image::Luma([180]));
        assert!(detect_circle(&blank, None).unwrap_err().contains("bordes"));
        assert!(detect_circle(&GrayImage::new(10, 10), None).is_err());
        // Un rectángulo no es un círculo.
        let mut sq = GrayImage::from_pixel(320, 240, image::Luma([200]));
        for y in 60..180 {
            for x in 80..260 {
                sq.put_pixel(x, y, image::Luma([40]));
            }
        }
        assert!(detect_circle(&sq, None).is_err());
    }

    #[test]
    fn tres_puntos_y_ajuste_de_minimos_cuadrados() {
        let (c, r) = (Point::new(12.5, -7.25), 31.0);
        let at = |deg: f64| {
            Point::new(
                c.x + r * deg.to_radians().cos(),
                c.y + r * deg.to_radians().sin(),
            )
        };
        let (cc, rr) = circle_from_three(at(10.0), at(130.0), at(250.0)).unwrap();
        assert!(cc.dist(c) < 1e-9 && (rr - r).abs() < 1e-9);
        // Casi alineados: no hay círculo.
        assert!(
            circle_from_three(
                Point::new(0.0, 0.0),
                Point::new(1.0, 1.0),
                Point::new(2.0, 2.0)
            )
            .is_none()
        );
        // Con ruido, el ajuste con muchos puntos es mejor que con tres.
        let mut rng = Rng::new(9);
        let pts: Vec<Point> = (0..60)
            .map(|i| {
                let p = at(f64::from(i) * 6.0);
                Point::new(p.x + 0.05 * rng.gauss(), p.y + 0.05 * rng.gauss())
            })
            .collect();
        let (c2, r2) = fit_circle_points(&pts).unwrap();
        assert!(c2.dist(c) < 0.05 && (r2 - r).abs() < 0.05);
    }
}
