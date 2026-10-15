//! Cámara simulada para probar la alineación con el cabezal sin hardware.
//!
//! Una cama con textura (veta de madera, manchas, grano) vista desde una cámara fija con lente
//! (ojo de pez o Brown-Conrady) y un cabezal de cilindros que se mueve sobre ella. Cada foto lleva
//! ruido, un viñeteo y una exposición que cambia de una foto a otra, como una cámara real con
//! exposición automática. Se usa en las pruebas del motor y del flujo con la máquina virtual.

use crate::calibration::{LensCalibration, LensModel};
use crate::imgproc::{FloatImage, gaussian_blur};
use image::{GrayImage, Rgb, RgbImage};
use nalgebra::{Matrix3, Rotation3, Vector3};
use tlaku_core::Point;

/// Forma del cabezal: dos cilindros verticales centrados en el punto del láser (mm sobre la cama).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SimHead {
    pub body_radius_mm: f64,
    pub body_bottom_mm: f64,
    pub body_top_mm: f64,
    pub nozzle_radius_mm: f64,
    pub nozzle_bottom_mm: f64,
}

impl Default for SimHead {
    fn default() -> Self {
        Self {
            body_radius_mm: 22.0,
            body_bottom_mm: 20.0,
            body_top_mm: 46.0,
            nozzle_radius_mm: 6.0,
            nozzle_bottom_mm: 8.0,
        }
    }
}

/// Escena: cama, cámara y cabezal.
#[derive(Debug, Clone)]
pub struct SimScene {
    /// Lente de la cámara (tamaño de la foto, focal, distorsión).
    pub lens: LensCalibration,
    /// Posición del centro óptico (mm de la cama; Z hacia arriba).
    pub camera_mm: [f64; 3],
    /// Inclinación pequeña de la cámara (grados) sobre sus ejes x e y, y giro sobre el eje óptico.
    pub tilt_deg: [f64; 2],
    pub roll_deg: f64,
    pub head: SimHead,
    /// Desviación del ruido gaussiano por foto (niveles de gris).
    pub noise: f32,
    pub seed: u64,
    /// Pórtico (riel del eje X) que cruza toda la imagen, detrás del cabezal, y se mueve con su Y.
    pub gantry: bool,
    /// Marcas fijas de la cama (puntos oscuros de ~3 mm) visibles en todas las fotos.
    pub dots: Vec<[f64; 2]>,
}

impl SimScene {
    /// Cámara de ojo de pez de 800 x 600 sobre el centro de una cama de `bed` mm, a `height` mm
    /// (como la de la tapa de una CO2/diodo), con una inclinación y un giro pequeños.
    pub fn fisheye(bed: [f64; 2], height: f64) -> Self {
        let lens = LensCalibration {
            width: 800,
            height: 600,
            fx: 400.0,
            fy: 401.0,
            cx: 402.0,
            cy: 297.0,
            k1: -0.035,
            k2: 0.008,
            p1: 0.0,
            p2: 0.0,
            k3: 0.0,
            model: LensModel::Fisheye,
            k4: 0.0,
            view_zoom: 0.6,
            rms_error_px: 0.0,
            views_used: 0,
        };
        Self::with_lens(lens, bed, height)
    }

    /// Cámara con distorsión de barril suave (Brown-Conrady) de 800 x 600.
    pub fn barrel(bed: [f64; 2], height: f64) -> Self {
        let lens = LensCalibration {
            width: 800,
            height: 600,
            fx: 620.0,
            fy: 622.0,
            cx: 401.0,
            cy: 299.0,
            k1: -0.22,
            k2: 0.06,
            p1: 0.0,
            p2: 0.0,
            k3: 0.0,
            model: LensModel::BrownConrady,
            k4: 0.0,
            view_zoom: 1.0,
            rms_error_px: 0.0,
            views_used: 0,
        };
        Self::with_lens(lens, bed, height)
    }

    /// La misma escena con una cámara de `k` veces más píxeles por lado (misma vista).
    pub fn scaled(mut self, k: f64) -> Self {
        let l = &mut self.lens;
        l.width = (f64::from(l.width) * k).round() as u32;
        l.height = (f64::from(l.height) * k).round() as u32;
        l.fx *= k;
        l.fy *= k;
        l.cx = (l.cx + 0.5) * k - 0.5;
        l.cy = (l.cy + 0.5) * k - 0.5;
        self
    }

    fn with_lens(lens: LensCalibration, bed: [f64; 2], height: f64) -> Self {
        Self {
            lens,
            camera_mm: [bed[0] * 0.5 + 6.0, bed[1] * 0.5 - 4.0, height],
            tilt_deg: [1.2, -0.8],
            roll_deg: 1.5,
            head: SimHead::default(),
            noise: 2.0,
            seed: 7,
            gantry: true,
            dots: Vec::new(),
        }
    }

    fn rotation(&self) -> Matrix3<f64> {
        let d = std::f64::consts::PI / 180.0;
        Rotation3::from_euler_angles(
            self.tilt_deg[0] * d,
            self.tilt_deg[1] * d,
            self.roll_deg * d,
        )
        .into_inner()
    }

    /// Punto 3D de la cama (mm, Z arriba) al píxel de la foto cruda (centro del píxel `(i, j)` = `(i, j)`).
    pub fn project(&self, p: [f64; 3]) -> Point {
        let v = Vector3::new(
            p[0] - self.camera_mm[0],
            -(p[1] - self.camera_mm[1]),
            -(p[2] - self.camera_mm[2]),
        );
        let c = self.rotation() * v;
        let (xd, yd) = self.lens.distort_normalized(c.x / c.z, c.y / c.z);
        Point::new(
            self.lens.fx * xd + self.lens.cx,
            self.lens.fy * yd + self.lens.cy,
        )
    }

    /// Píxel de un punto del material (a Z = 0).
    pub fn ground_px(&self, bed_mm: [f64; 2]) -> Point {
        self.project([bed_mm[0], bed_mm[1], 0.0])
    }

    /// Altura a la que cae, en promedio, la silueta del cabezal: la del centro del cuerpo.
    pub fn head_mid_height(&self) -> f64 {
        (self.head.body_bottom_mm + self.head.body_top_mm) / 2.0
    }

    /// Foto con el cabezal en `head_xy` (mm de la cama) o sin cabezal. `frame` cambia el ruido y
    /// la exposición de cada foto.
    pub fn render(&self, head_xy: Option<[f64; 2]>, frame: u64) -> GrayImage {
        self.render_with(head_xy, head_xy.map(|h| h[1]), frame)
    }

    /// Como [`Self::render`] pero con el pórtico en `gantry_y` aunque el cabezal no se vea (el
    /// pórtico se mueve con la Y y el cabezal puede estar fuera del campo de la cámara).
    pub fn render_with(
        &self,
        head_xy: Option<[f64; 2]>,
        gantry_y: Option<f64>,
        frame: u64,
    ) -> GrayImage {
        let (w, h) = (self.lens.width as usize, self.lens.height as usize);
        let rot_t = self.rotation().transpose();
        let cam = Vector3::new(self.camera_mm[0], self.camera_mm[1], self.camera_mm[2]);
        let mut data = vec![0.0f32; w * h];
        for j in 0..h {
            for i in 0..w {
                let (xd, yd) = (
                    (i as f64 - self.lens.cx) / self.lens.fx,
                    (j as f64 - self.lens.cy) / self.lens.fy,
                );
                let (x, y) = self.lens.undistort_normalized(xd, yd);
                let dc = rot_t * Vector3::new(x, y, 1.0);
                // De la cámara al mundo: (dx, -dy, -dz).
                let d = Vector3::new(dc.x, -dc.y, -dc.z);
                let mut v = self.shade(cam, d, head_xy, gantry_y);
                let (u, vv) = (x, y);
                v *= (1.0 - 0.18 * (u * u + vv * vv)).max(0.3) as f32;
                data[j * w + i] = v;
            }
        }
        let blurred = gaussian_blur(&FloatImage { w, h, data }, 0.8);
        let mut rng = Lcg::new(self.seed.wrapping_mul(31).wrapping_add(frame));
        let gain = 1.0 + 0.05 * (frame as f32 * 1.7).sin();
        let offset = 3.0 * (frame as f32 * 0.9).cos();
        let pixels: Vec<u8> = blurred
            .data
            .iter()
            .map(|&v| {
                (v * gain + offset + self.noise * rng.gauss() as f32)
                    .round()
                    .clamp(0.0, 255.0) as u8
            })
            .collect();
        GrayImage::from_raw(w as u32, h as u32, pixels).expect("tamaño")
    }

    /// Como [`Self::render`] pero a color: tinte rojizo (ventana naranja de la tapa) y un LED verde
    /// en el cabezal, a 12 mm a la izquierda y 14 mm al frente de la boquilla.
    pub fn render_rgb(&self, head_xy: Option<[f64; 2]>, frame: u64) -> RgbImage {
        self.render_rgb_with(head_xy, head_xy.map(|h| h[1]), frame)
    }

    /// Como [`Self::render_rgb`] con el pórtico en `gantry_y` (ver [`Self::render_with`]).
    pub fn render_rgb_with(
        &self,
        head_xy: Option<[f64; 2]>,
        gantry_y: Option<f64>,
        frame: u64,
    ) -> RgbImage {
        let gray = self.render_with(head_xy, gantry_y, frame);
        let mut img = RgbImage::new(gray.width(), gray.height());
        for (o, p) in img.pixels_mut().zip(gray.pixels()) {
            let v = f32::from(p.0[0]);
            *o = Rgb([
                (v * 1.0).min(255.0) as u8,
                (v * 0.55) as u8,
                (v * 0.45) as u8,
            ]);
        }
        if let Some([hx, hy]) = head_xy {
            let c = self.project([hx - 12.0, hy - 14.0, self.head.body_bottom_mm + 6.0]);
            let r = 6i32;
            for dj in -r..=r {
                for di in -r..=r {
                    let (x, y) = (c.x.round() as i32 + di, c.y.round() as i32 + dj);
                    if x < 0 || y < 0 || x >= img.width() as i32 || y >= img.height() as i32 {
                        continue;
                    }
                    let d2 = (f64::from(x) - c.x).powi(2) + (f64::from(y) - c.y).powi(2);
                    let a = (-d2 / (2.0 * 1.6f64 * 1.6)).exp() as f32;
                    let px = img.get_pixel_mut(x as u32, y as u32);
                    let led = [80.0, 255.0, 140.0];
                    for (v, l) in px.0.iter_mut().zip(led) {
                        *v = (f32::from(*v) * (1.0 - a) + l * a) as u8;
                    }
                }
            }
        }
        img
    }

    /// Brillo de lo que ve el rayo `cam + t·d` (mundo, Z arriba).
    fn shade(
        &self,
        cam: Vector3<f64>,
        d: Vector3<f64>,
        head_xy: Option<[f64; 2]>,
        gantry_y: Option<f64>,
    ) -> f32 {
        if d.z >= -1e-9 {
            return 20.0;
        }
        let tg = -cam.z / d.z;
        let (gx, gy) = (cam.x + tg * d.x, cam.y + tg * d.y);
        let mut best_t = tg;
        let mut value = self.ground(gx, gy);
        if self.gantry
            && let Some(gy) = gantry_y
            && let Some(t) = hit_slab(cam, d, [gy + 26.0, gy + 56.0], 30.0, 62.0)
            && t < best_t
        {
            best_t = t;
            let p = cam + d * t;
            value = if p.z > 61.5 {
                78.0
            } else {
                62.0 + 8.0 * (p.x * 0.3).sin() as f32
            };
        }
        if let Some([hx, hy]) = head_xy {
            let h = &self.head;
            // Sombra suave del cabezal sobre la cama.
            let sd = ((gx - hx - 9.0).hypot(gy - hy + 9.0) - h.body_radius_mm) / 14.0;
            value *= (0.90 + 0.10 * sd.clamp(0.0, 1.0)) as f32;
            for (r, z0, z1, kind) in [
                (h.body_radius_mm, h.body_bottom_mm, h.body_top_mm, 0),
                (h.nozzle_radius_mm, h.nozzle_bottom_mm, h.body_bottom_mm, 1),
            ] {
                if let Some((t, top)) = hit_cylinder(cam, d, [hx, hy], r, z0, z1)
                    && t < best_t
                {
                    best_t = t;
                    let p = cam + d * t;
                    let rr = (p.x - hx).hypot(p.y - hy) / r;
                    value = match (kind, top) {
                        (0, true) => {
                            if rr < 0.4 {
                                96.0
                            } else {
                                58.0 + 10.0 * (p.x * 0.4).sin() as f32
                            }
                        }
                        (0, false) => 86.0,
                        _ => 34.0,
                    };
                }
            }
        }
        value
    }

    /// Textura de la cama en el punto (x, y) (mm): veta, manchas, grano y marcas fijas.
    fn ground(&self, x: f64, y: f64) -> f32 {
        let mut v = 150.0
            + 22.0 * (x / 37.0 + 1.3).sin() * (y / 53.0 + 0.4).cos()
            + 12.0 * ((x + 0.6 * y) / 11.0).sin()
            + 8.0 * (x / 5.3 - y / 7.7).sin();
        // Manchas oscuras repartidas por una rejilla irregular.
        let (cx, cy) = ((x / 90.0).floor(), (y / 90.0).floor());
        for (dx, dy) in [(0.0, 0.0), (1.0, 0.0), (0.0, 1.0), (1.0, 1.0)] {
            let (gx, gy) = (cx + dx, cy + dy);
            let hsh = hash2(gx as i64, gy as i64);
            let (px, py) = (
                (gx + 0.2 + 0.6 * (hsh & 0xff) as f64 / 255.0) * 90.0,
                (gy + 0.2 + 0.6 * ((hsh >> 8) & 0xff) as f64 / 255.0) * 90.0,
            );
            let r = 5.0 + 10.0 * ((hsh >> 16) & 0xff) as f64 / 255.0;
            let dd = (x - px).hypot(y - py);
            if dd < r {
                v -= 40.0 * (1.0 - (dd / r).powi(2));
            }
        }
        // Grano de 2 mm.
        let g = hash2((x / 2.0).floor() as i64, (y / 2.0).floor() as i64);
        v += 7.0 * ((g & 0xff) as f64 / 127.5 - 1.0);
        for dot in &self.dots {
            let dd = (x - dot[0]).hypot(y - dot[1]);
            if dd < 1.8 {
                v -= 90.0 * (1.0 - dd / 1.8);
            }
        }
        v as f32
    }
}

/// Primer choque con una losa horizontal infinita en X: `y` en `ys`, `z` en `[z0, z1]`.
fn hit_slab(o: Vector3<f64>, d: Vector3<f64>, ys: [f64; 2], z0: f64, z1: f64) -> Option<f64> {
    let span = |o: f64, d: f64, lo: f64, hi: f64| -> Option<(f64, f64)> {
        if d.abs() < 1e-12 {
            return (lo..=hi)
                .contains(&o)
                .then_some((f64::NEG_INFINITY, f64::INFINITY));
        }
        let (a, b) = ((lo - o) / d, (hi - o) / d);
        Some((a.min(b), a.max(b)))
    };
    let (ya, yb) = span(o.y, d.y, ys[0], ys[1])?;
    let (za, zb) = span(o.z, d.z, z0, z1)?;
    let (lo, hi) = (ya.max(za).max(0.0), yb.min(zb));
    (lo <= hi).then_some(lo)
}

/// Primer choque (y si es la tapa) de un rayo con un cilindro vertical finito.
fn hit_cylinder(
    o: Vector3<f64>,
    d: Vector3<f64>,
    c: [f64; 2],
    r: f64,
    z0: f64,
    z1: f64,
) -> Option<(f64, bool)> {
    let (ox, oy) = (o.x - c[0], o.y - c[1]);
    let a = d.x * d.x + d.y * d.y;
    let (t1, t2) = if a < 1e-12 {
        if ox * ox + oy * oy > r * r {
            return None;
        }
        (f64::NEG_INFINITY, f64::INFINITY)
    } else {
        let b = 2.0 * (ox * d.x + oy * d.y);
        let disc = b * b - 4.0 * a * (ox * ox + oy * oy - r * r);
        if disc < 0.0 {
            return None;
        }
        let s = disc.sqrt();
        ((-b - s) / (2.0 * a), (-b + s) / (2.0 * a))
    };
    // La altura baja con t (d.z < 0): entra por la tapa (z1) y sale por la base (z0).
    let (ta, tb) = ((z1 - o.z) / d.z, (z0 - o.z) / d.z);
    let (lo, hi) = (t1.max(ta).max(0.0), t2.min(tb));
    if lo > hi {
        return None;
    }
    Some((lo, (lo - ta).abs() < 1e-9))
}

fn hash2(x: i64, y: i64) -> u64 {
    let mut h = (x as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ (y as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F);
    h ^= h >> 29;
    h = h.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    h ^= h >> 32;
    h
}

struct Lcg(u64);

impl Lcg {
    fn new(seed: u64) -> Self {
        Self(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1)
    }
    fn uniform(&mut self) -> f64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        ((x >> 11) as f64 + 0.5) / (1u64 << 53) as f64
    }
    fn gauss(&mut self) -> f64 {
        let (a, b) = (self.uniform(), self.uniform());
        (-2.0 * a.ln()).sqrt() * (2.0 * std::f64::consts::PI * b).cos()
    }
}
