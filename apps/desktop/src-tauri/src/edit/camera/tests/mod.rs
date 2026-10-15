//! Pruebas del área `camera`: apoyos compartidos (tablero sintético, carpeta temporal).

use super::*;
use image::ImageFormat;
use std::io::Cursor;
use std::sync::atomic::{AtomicU32, Ordering};

const BOARD: [u32; 2] = [9, 6];
const SQ: f64 = 25.0;

type Mat = [[f64; 3]; 3];

fn mul(a: &Mat, b: &Mat) -> Mat {
    let mut m = [[0.0; 3]; 3];
    for (i, row) in m.iter_mut().enumerate() {
        for (j, v) in row.iter_mut().enumerate() {
            *v = (0..3).map(|k| a[i][k] * b[k][j]).sum();
        }
    }
    m
}

fn rot(rx: f64, ry: f64, rz: f64) -> Mat {
    let (sx, cx) = rx.to_radians().sin_cos();
    let (sy, cy) = ry.to_radians().sin_cos();
    let (sz, cz) = rz.to_radians().sin_cos();
    let x = [[1.0, 0.0, 0.0], [0.0, cx, -sx], [0.0, sx, cx]];
    let y = [[cy, 0.0, sy], [0.0, 1.0, 0.0], [-sy, 0.0, cy]];
    let z = [[cz, -sz, 0.0], [sz, cz, 0.0], [0.0, 0.0, 1.0]];
    mul(&mul(&z, &y), &x)
}

/// Cámara de la prueba: 640x480 con distorsión radial moderada.
struct Cam {
    fx: f64,
    fy: f64,
    cx: f64,
    cy: f64,
    k1: f64,
}

const CAM: Cam = Cam {
    fx: 620.0,
    fy: 625.0,
    cx: 320.0,
    cy: 240.0,
    k1: -0.12,
};
const W: u32 = 640;
const H: u32 = 480;

/// Pose del tablero: `r` columnas = ejes del tablero en la cámara, `t` = origen del tablero.
struct Pose {
    r: Mat,
    t: [f64; 3],
}

fn pose(deg: (f64, f64, f64), center: (f64, f64, f64)) -> Pose {
    let r = rot(deg.0, deg.1, deg.2);
    // El centro del patrón de esquinas internas queda en `center`.
    let c = [
        f64::from(BOARD[0] - 1) * SQ / 2.0,
        f64::from(BOARD[1] - 1) * SQ / 2.0,
        0.0,
    ];
    let t = [
        center.0 - (r[0][0] * c[0] + r[0][1] * c[1]),
        center.1 - (r[1][0] * c[0] + r[1][1] * c[1]),
        center.2 - (r[2][0] * c[0] + r[2][1] * c[1]),
    ];
    Pose { r, t }
}

fn distort(x: f64, y: f64) -> (f64, f64) {
    let q = 1.0 + CAM.k1 * (x * x + y * y);
    (x * q, y * q)
}

/// Inversa de `distort` por punto fijo (basta con una distorsión moderada).
fn undistort(xd: f64, yd: f64) -> (f64, f64) {
    let (mut x, mut y) = (xd, yd);
    for _ in 0..30 {
        let q = 1.0 + CAM.k1 * (x * x + y * y);
        x = xd / q;
        y = yd / q;
    }
    (x, y)
}

/// Píxel donde cae el punto (x, y) del plano del tablero (mm).
fn project(p: &Pose, x: f64, y: f64) -> [f64; 2] {
    let c = |i: usize| p.r[i][0] * x + p.r[i][1] * y + p.t[i];
    let (xd, yd) = distort(c(0) / c(2), c(1) / c(2));
    [CAM.fx * xd + CAM.cx, CAM.fy * yd + CAM.cy]
}

fn truth_corners(p: &Pose) -> Vec<[f64; 2]> {
    let mut v = Vec::new();
    for j in 0..BOARD[1] {
        for i in 0..BOARD[0] {
            v.push(project(p, f64::from(i) * SQ, f64::from(j) * SQ));
        }
    }
    v
}

/// Dibuja el tablero (10x7 cuadros, margen blanco) por trazado inverso con 3x3 muestras por píxel.
fn render(p: &Pose) -> GrayImage {
    let normal = [p.r[0][2], p.r[1][2], p.r[2][2]];
    let dist = normal[0] * p.t[0] + normal[1] * p.t[1] + normal[2] * p.t[2];
    let shade = |u: f64, v: f64| -> f64 {
        let (x, y) = undistort((u - CAM.cx) / CAM.fx, (v - CAM.cy) / CAM.fy);
        let ray = [x, y, 1.0];
        let denom = normal[0] * ray[0] + normal[1] * ray[1] + normal[2];
        let s = dist / denom;
        let q = [ray[0] * s - p.t[0], ray[1] * s - p.t[1], s - p.t[2]];
        // Coordenadas en el plano del tablero: proyección sobre sus ejes.
        let bx = p.r[0][0] * q[0] + p.r[1][0] * q[1] + p.r[2][0] * q[2];
        let by = p.r[0][1] * q[0] + p.r[1][1] * q[1] + p.r[2][1] * q[2];
        let (ci, cj) = ((bx / SQ).floor(), (by / SQ).floor());
        let inside = ci >= -1.0
            && ci <= f64::from(BOARD[0]) - 1.0
            && cj >= -1.0
            && cj <= f64::from(BOARD[1]) - 1.0;
        if inside && ((ci + cj) as i64).rem_euclid(2) == 0 {
            30.0
        } else {
            225.0
        }
    };
    let n = 3;
    GrayImage::from_fn(W, H, |i, j| {
        let mut acc = 0.0;
        for sj in 0..n {
            for si in 0..n {
                let u = f64::from(i) - 0.5 + (f64::from(si) + 0.5) / f64::from(n);
                let v = f64::from(j) - 0.5 + (f64::from(sj) + 0.5) / f64::from(n);
                acc += shade(u, v);
            }
        }
        image::Luma([(acc / f64::from(n * n)).round() as u8])
    })
}

fn to_b64(img: &GrayImage) -> String {
    let mut buf = Cursor::new(Vec::new());
    img.write_to(&mut buf, ImageFormat::Png).unwrap();
    STANDARD.encode(buf.into_inner())
}

fn views() -> Vec<Pose> {
    [
        ((20.0, -15.0, 8.0), (-50.0, -30.0, 480.0)),
        ((-25.0, 12.0, -10.0), (60.0, 40.0, 450.0)),
        ((8.0, 30.0, 20.0), (0.0, 0.0, 520.0)),
        ((-15.0, -25.0, 95.0), (-70.0, 50.0, 540.0)),
        ((30.0, 5.0, -35.0), (40.0, -40.0, 470.0)),
        ((-30.0, 25.0, 10.0), (45.0, 55.0, 500.0)),
        ((12.0, -30.0, 4.0), (-40.0, -45.0, 460.0)),
        ((0.0, 8.0, -6.0), (0.0, 10.0, 430.0)),
    ]
    .into_iter()
    .map(|(d, c)| pose(d, c))
    .collect()
}

static COUNTER: AtomicU32 = AtomicU32::new(0);

struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("tlaku-camera-{}-{n}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        Self(dir)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn sample_setup() -> CameraSetup {
    CameraSetup {
        calibration: Some(LensCalibration {
            width: W,
            height: H,
            fx: 620.0,
            fy: 625.0,
            cx: 320.0,
            cy: 240.0,
            k1: -0.12,
            k2: 0.01,
            p1: 0.0,
            p2: 0.0,
            k3: 0.0,
            model: tlaku_camera::LensModel::BrownConrady,
            k4: 0.0,
            view_zoom: 1.0,
            rms_error_px: 0.31,
            views_used: 8,
        }),
        // Hoja carta (279.4 x 215.9 mm) vista casi de frente en 640x480.
        alignment: Some(CameraAlignment {
            image_size: [W, H],
            image_px: [[100.0, 400.0], [540.0, 410.0], [530.0, 60.0], [110.0, 70.0]],
            bed_mm: [[10.0, 10.0], [289.4, 10.0], [289.4, 225.9], [10.0, 225.9]],
            to_bed: [[0.0; 3]; 3],
            rms_mm: 0.0,
            method: AlignmentMethod::Manual,
            reference_height_mm: 0.0,
            samples: Vec::new(),
            height: None,
        }),
        ..CameraSetup::default()
    }
}

mod bed_image;
mod board;
mod head_align;
mod height;
mod marks;
mod network;
mod setup;
