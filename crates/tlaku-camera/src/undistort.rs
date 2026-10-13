//! Corrección de distorsión de lente con tabla de remapeo precalculada.
//!
//! La tabla se calcula una sola vez por calibración y tamaño de salida; aplicarla a
//! cada cuadro es un remapeo bilineal en aritmética entera (apto para cámara en vivo).
//!
//! Cámara virtual de salida: la focal de la calibración por `view_zoom` (1 salvo en ojo de pez)
//! y el mismo punto principal, escalados por `out_size / tamaño_calibrado`. Con distorsión de barril el
//! resultado queda recortado (sin bordes negros); con cojín aparecen bordes negros.
//! La alineación con la cama debe calcularse sobre esta misma imagen corregida.

use crate::calibration::LensCalibration;
use image::{ImageBuffer, Pixel};
use std::path::{Path, PathBuf};
use tlaku_core::Point;

const OUTSIDE: u32 = u32::MAX;

#[derive(Clone, Copy)]
struct Entry {
    /// Índice del píxel (x0, y0) en la imagen de origen, o `OUTSIDE`.
    idx: u32,
    /// Pesos bilineales en 1/256 (0..=256).
    wx: u16,
    wy: u16,
}

/// Tabla de remapeo de la imagen cruda (con distorsión) a la imagen corregida.
pub struct UndistortMap {
    calib: LensCalibration,
    src_size: (u32, u32),
    out_size: (u32, u32),
    /// Cámara de salida: fx, fy, cx, cy.
    new_k: [f64; 4],
    table: Vec<Entry>,
}

impl UndistortMap {
    pub fn new(calib: &LensCalibration, out_size: (u32, u32)) -> Result<Self, String> {
        calib.validate()?;
        if out_size.0 == 0 || out_size.1 == 0 {
            return Err("el tamaño de salida de la corrección no puede ser 0".into());
        }
        if calib.width < 2 || calib.height < 2 {
            return Err("la imagen calibrada es demasiado pequeña".into());
        }
        let sx = f64::from(out_size.0) / f64::from(calib.width);
        let sy = f64::from(out_size.1) / f64::from(calib.height);
        let (zfx, zfy) = calib.corrected_focal();
        let new_k = [zfx * sx, zfy * sy, calib.cx * sx, calib.cy * sy];
        let (sw, sh) = (f64::from(calib.width), f64::from(calib.height));
        let mut table = Vec::with_capacity(out_size.0 as usize * out_size.1 as usize);
        for v in 0..out_size.1 {
            let y = (f64::from(v) - new_k[3]) / new_k[1];
            for u in 0..out_size.0 {
                let x = (f64::from(u) - new_k[2]) / new_k[0];
                let (xd, yd) = calib.distort_normalized(x, y);
                let (px, py) = (calib.fx * xd + calib.cx, calib.fy * yd + calib.cy);
                let inside = px.is_finite()
                    && py.is_finite()
                    && px >= 0.0
                    && py >= 0.0
                    && px <= sw - 1.0
                    && py <= sh - 1.0;
                table.push(if inside {
                    let x0 = (px.floor() as u32).min(calib.width - 2);
                    let y0 = (py.floor() as u32).min(calib.height - 2);
                    Entry {
                        idx: y0 * calib.width + x0,
                        wx: (((px - f64::from(x0)) * 256.0).round() as u16).min(256),
                        wy: (((py - f64::from(y0)) * 256.0).round() as u16).min(256),
                    }
                } else {
                    Entry {
                        idx: OUTSIDE,
                        wx: 0,
                        wy: 0,
                    }
                });
            }
        }
        Ok(Self {
            calib: calib.clone(),
            src_size: (calib.width, calib.height),
            out_size,
            new_k,
            table,
        })
    }

    pub fn out_size(&self) -> (u32, u32) {
        self.out_size
    }

    /// Cámara de la imagen corregida: `[fx, fy, cx, cy]`.
    pub fn new_camera(&self) -> [f64; 4] {
        self.new_k
    }

    /// Corrige una imagen (`GrayImage`, `RgbImage`, `RgbaImage`...). Debe tener el
    /// tamaño con que se calibró.
    pub fn undistort<P>(
        &self,
        img: &ImageBuffer<P, Vec<u8>>,
    ) -> Result<ImageBuffer<P, Vec<u8>>, String>
    where
        P: Pixel<Subpixel = u8>,
    {
        if (img.width(), img.height()) != self.src_size {
            return Err(format!(
                "la imagen mide {}x{} pero la calibración es de {}x{}",
                img.width(),
                img.height(),
                self.src_size.0,
                self.src_size.1
            ));
        }
        let ch = P::CHANNEL_COUNT as usize;
        let src = img.as_raw();
        let mut out = vec![0u8; self.table.len() * ch];
        // Versiones con número de canales constante para que el compilador desenrolle.
        match ch {
            1 => self.remap::<1>(src, &mut out),
            2 => self.remap::<2>(src, &mut out),
            3 => self.remap::<3>(src, &mut out),
            4 => self.remap::<4>(src, &mut out),
            _ => return Err("número de canales no soportado".into()),
        }
        ImageBuffer::from_raw(self.out_size.0, self.out_size.1, out)
            .ok_or_else(|| "tamaño de búfer inconsistente al corregir la imagen".to_string())
    }

    fn remap<const CH: usize>(&self, src: &[u8], out: &mut [u8]) {
        let stride = self.src_size.0 as usize * CH;
        for (e, dst) in self.table.iter().zip(out.chunks_exact_mut(CH)) {
            if e.idx == OUTSIDE {
                continue;
            }
            let i00 = e.idx as usize * CH;
            let i10 = i00 + stride;
            let (wx, wy) = (u32::from(e.wx), u32::from(e.wy));
            let top = &src[i00..i00 + 2 * CH];
            let bot = &src[i10..i10 + 2 * CH];
            for c in 0..CH {
                let t = u32::from(top[c]) * (256 - wx) + u32::from(top[CH + c]) * wx;
                let b = u32::from(bot[c]) * (256 - wx) + u32::from(bot[CH + c]) * wx;
                dst[c] = ((t * (256 - wy) + b * wy + 32768) >> 16) as u8;
            }
        }
    }

    /// Píxel de la imagen cruda a píxel de la imagen corregida (para llevar marcas
    /// detectadas en la imagen original al sistema de la alineación).
    pub fn raw_to_corrected(&self, p: Point) -> Point {
        let (x, y) = self.calib.pixel_to_normalized(p);
        Point::new(
            self.new_k[0] * x + self.new_k[2],
            self.new_k[1] * y + self.new_k[3],
        )
    }

    /// Inversa de [`Self::raw_to_corrected`].
    pub fn corrected_to_raw(&self, p: Point) -> Point {
        let x = (p.x - self.new_k[2]) / self.new_k[0];
        let y = (p.y - self.new_k[3]) / self.new_k[1];
        let (xd, yd) = self.calib.distort_normalized(x, y);
        Point::new(
            self.calib.fx * xd + self.calib.cx,
            self.calib.fy * yd + self.calib.cy,
        )
    }
}

/// Ruta por defecto de la imagen corregida: `<nombre>_undistorted.png` junto a la original.
pub fn undistorted_path(input: &Path) -> PathBuf {
    let stem = input
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "frame".into());
    input.with_file_name(format!("{stem}_undistorted.png"))
}

/// Lee `input`, corrige la distorsión y guarda un PNG en `output`.
pub fn undistort_file(input: &Path, output: &Path, calib: &LensCalibration) -> Result<(), String> {
    let img = image::open(input)
        .map_err(|e| format!("no se pudo abrir {}: {e}", input.display()))?
        .to_rgb8();
    let size = (img.width(), img.height());
    // Si la imagen tiene otra resolución que la calibración, se reescala el modelo.
    let calib = if size == (calib.width, calib.height) {
        calib.clone()
    } else {
        rescaled(calib, size.0, size.1)
    };
    let map = UndistortMap::new(&calib, size)?;
    let out = map.undistort(&img)?;
    out.save(output)
        .map_err(|e| format!("no se pudo guardar {}: {e}", output.display()))
}

/// Misma lente a otra resolución (mismo campo de visión).
fn rescaled(c: &LensCalibration, w: u32, h: u32) -> LensCalibration {
    let (sx, sy) = (
        f64::from(w) / f64::from(c.width),
        f64::from(h) / f64::from(c.height),
    );
    LensCalibration {
        width: w,
        height: h,
        fx: c.fx * sx,
        fy: c.fy * sy,
        cx: c.cx * sx,
        cy: c.cy * sy,
        ..c.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chessboard::find_chessboard;
    use crate::testutil::*;
    use image::{GrayImage, RgbImage};

    const BOARD: [u32; 2] = [9, 6];
    const SQ: f64 = 25.0;

    /// Mayor desviación de las esquinas de cada fila y columna respecto de su recta
    /// de mínimos cuadrados (ortogonal).
    fn max_line_deviation(pts: &[Point]) -> f64 {
        let (cols, rows) = (BOARD[0] as usize, BOARD[1] as usize);
        let mut lines: Vec<Vec<Point>> = Vec::new();
        for j in 0..rows {
            lines.push((0..cols).map(|i| pts[j * cols + i]).collect());
        }
        for i in 0..cols {
            lines.push((0..rows).map(|j| pts[j * cols + i]).collect());
        }
        let mut worst: f64 = 0.0;
        for l in lines {
            let n = l.len() as f64;
            let (mx, my) = (
                l.iter().map(|p| p.x).sum::<f64>() / n,
                l.iter().map(|p| p.y).sum::<f64>() / n,
            );
            let (mut sxx, mut sxy, mut syy) = (0.0, 0.0, 0.0);
            for p in &l {
                sxx += (p.x - mx).powi(2);
                sxy += (p.x - mx) * (p.y - my);
                syy += (p.y - my).powi(2);
            }
            let theta = 0.5 * (2.0 * sxy).atan2(sxx - syy);
            let (nx, ny) = (-theta.sin(), theta.cos());
            for p in &l {
                worst = worst.max(((p.x - mx) * nx + (p.y - my) * ny).abs());
            }
        }
        worst
    }

    #[test]
    fn la_imagen_corregida_recupera_lineas_rectas() {
        // Barril fuerte (el de una webcam gran angular).
        let cam = LensCalibration {
            k1: -0.35,
            k2: 0.10,
            ..truth_camera()
        };
        let pose = Pose::about_center(BOARD, SQ, (15.0, -20.0, 8.0), (0.0, 0.0, 330.0));
        let img = render_board(&cam, &pose, BOARD, SQ, 1.0, 7);
        let raw = find_chessboard(&img, BOARD).unwrap();
        let before = max_line_deviation(&raw);

        let map = UndistortMap::new(&cam, (640, 480)).unwrap();
        // Por puntos.
        let pts: Vec<Point> = raw.iter().map(|&p| map.raw_to_corrected(p)).collect();
        let after_pts = max_line_deviation(&pts);
        // Por imagen: se vuelve a detectar el tablero en la imagen corregida.
        let fixed = map.undistort(&img).unwrap();
        let found = find_chessboard(&fixed, BOARD).unwrap();
        let after_img = max_line_deviation(&found);
        eprintln!(
            "desviación de rectas: antes {before:.2} px, puntos {after_pts:.3} px, imagen {after_img:.3} px"
        );
        assert!(
            before > 2.0,
            "la imagen cruda debería estar curvada ({before:.2})"
        );
        assert!(after_pts < 0.2, "puntos corregidos: {after_pts:.3}");
        assert!(after_img < 0.35, "imagen corregida: {after_img:.3}");
        // Ida y vuelta de puntos.
        for &p in raw.iter().take(10) {
            assert!(map.corrected_to_raw(map.raw_to_corrected(p)).dist(p) < 1e-6);
        }
    }

    #[test]
    fn el_ojo_de_pez_corregido_recupera_lineas_rectas() {
        // Ojo de pez de 140° con el tablero lejos del centro, donde la curvatura es fuerte.
        let mut cam = fisheye_camera();
        cam.view_zoom = 0.62;
        let pose = Pose::about_center(BOARD, SQ, (20.0, 10.0, 5.0), (120.0, -60.0, 270.0));
        let img = render_board(&cam, &pose, BOARD, SQ, 1.0, 17);
        let map = UndistortMap::new(&cam, (640, 480)).unwrap();
        let truth_raw = board_corners(&cam, &pose, BOARD, SQ);
        let before = max_line_deviation(&truth_raw);
        let pts: Vec<Point> = truth_raw.iter().map(|&p| map.raw_to_corrected(p)).collect();
        let after_pts = max_line_deviation(&pts);
        let fixed = map.undistort(&img).unwrap();
        let found = find_chessboard(&fixed, BOARD).expect("el tablero corregido se detecta");
        let after_img = max_line_deviation(&found);
        eprintln!(
            "ojo de pez, desviación de rectas: antes {before:.2} px, puntos {after_pts:.3} px, imagen {after_img:.3} px"
        );
        assert!(
            before > 3.0,
            "la imagen cruda debería estar curvada ({before:.2})"
        );
        assert!(after_pts < 0.05, "puntos corregidos: {after_pts:.3}");
        assert!(after_img < 0.5, "imagen corregida: {after_img:.3}");
        // Ida y vuelta de puntos.
        for &p in truth_raw.iter().take(10) {
            assert!(map.corrected_to_raw(map.raw_to_corrected(p)).dist(p) < 1e-6);
        }
        // La cámara de salida usa la focal reducida por view_zoom.
        let k = map.new_camera();
        assert!((k[0] - cam.fx * 0.62).abs() < 1e-9 && (k[2] - cam.cx).abs() < 1e-9);
    }

    #[test]
    fn sin_distorsion_es_la_identidad() {
        let cam = LensCalibration {
            k1: 0.0,
            k2: 0.0,
            p1: 0.0,
            p2: 0.0,
            k3: 0.0,
            ..truth_camera()
        };
        let src = GrayImage::from_fn(640, 480, |x, y| {
            image::Luma([((x * 7 + y * 3) % 251) as u8])
        });
        let map = UndistortMap::new(&cam, (640, 480)).unwrap();
        let out = map.undistort(&src).unwrap();
        assert_eq!(out.as_raw(), src.as_raw());
    }

    #[test]
    fn rgb_y_validaciones() {
        let cam = truth_camera();
        let map = UndistortMap::new(&cam, (320, 240)).unwrap();
        let rgb = RgbImage::from_pixel(640, 480, image::Rgb([10, 200, 90]));
        let out = map.undistort(&rgb).unwrap();
        assert_eq!((out.width(), out.height()), (320, 240));
        assert_eq!(*out.get_pixel(160, 120), image::Rgb([10, 200, 90]));
        let small = RgbImage::new(100, 100);
        assert!(map.undistort(&small).unwrap_err().contains("calibración"));
        let bad = LensCalibration {
            fx: 0.0,
            ..cam.clone()
        };
        assert!(UndistortMap::new(&bad, (10, 10)).is_err());
        assert!(UndistortMap::new(&cam, (0, 10)).is_err());
    }

    #[test]
    fn archivo_a_archivo() {
        let cam = truth_camera();
        let dir = std::env::temp_dir().join(format!("tlaku_cam_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let input = dir.join("frame.png");
        RgbImage::from_pixel(640, 480, image::Rgb([200, 100, 50]))
            .save(&input)
            .unwrap();
        let output = undistorted_path(&input);
        assert!(output.ends_with("frame_undistorted.png"));
        undistort_file(&input, &output, &cam).unwrap();
        assert_eq!(image::open(&output).unwrap().width(), 640);
        assert!(
            undistort_file(&dir.join("no_existe.png"), &output, &cam)
                .unwrap_err()
                .contains("no se pudo abrir")
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    /// Objetivo de rendimiento: < 15 ms por cuadro de 1280x720 en release.
    /// `cargo test -p tlaku-camera --release -- --ignored --nocapture`
    #[test]
    #[ignore = "medición de rendimiento: ejecutar en release"]
    fn rendimiento_1280x720() {
        let cam = LensCalibration {
            width: 1280,
            height: 720,
            fx: 1100.0,
            fy: 1100.0,
            cx: 640.0,
            cy: 360.0,
            ..truth_camera()
        };
        let t0 = std::time::Instant::now();
        let map = UndistortMap::new(&cam, (1280, 720)).unwrap();
        let build = t0.elapsed();
        let rgb = RgbImage::from_fn(1280, 720, |x, y| {
            image::Rgb([(x % 256) as u8, (y % 256) as u8, ((x + y) % 256) as u8])
        });
        let gray = GrayImage::from_fn(1280, 720, |x, y| image::Luma([((x ^ y) % 256) as u8]));
        let _ = map.undistort(&rgb).unwrap();
        let n = 20;
        let t1 = std::time::Instant::now();
        for _ in 0..n {
            std::hint::black_box(map.undistort(&rgb).unwrap());
        }
        let per_rgb = t1.elapsed().as_secs_f64() * 1000.0 / f64::from(n);
        let t2 = std::time::Instant::now();
        for _ in 0..n {
            std::hint::black_box(map.undistort(&gray).unwrap());
        }
        let per_gray = t2.elapsed().as_secs_f64() * 1000.0 / f64::from(n);
        eprintln!(
            "tabla: {:.1} ms; RGB: {per_rgb:.2} ms/cuadro; gris: {per_gray:.2} ms/cuadro",
            build.as_secs_f64() * 1000.0
        );
        assert!(per_rgb < 15.0, "RGB {per_rgb:.2} ms");
    }
}
