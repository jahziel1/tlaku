//! Cámara virtual para pruebas: renderiza un tablero de ajedrez con proyección y
//! distorsión conocidas (suavizado por supermuestreo adaptativo, desenfoque óptico
//! y ruido gaussiano determinista).

use crate::calibration::{LensCalibration, LensModel};
use crate::imgproc::{FloatImage, gaussian_blur};
use image::GrayImage;
use nalgebra::{Matrix3, Rotation3, Vector3};
use tlaku_core::Point;

pub const DARK: f32 = 30.0;
pub const LIGHT: f32 = 225.0;

pub fn truth_camera() -> LensCalibration {
    LensCalibration {
        width: 640,
        height: 480,
        fx: 620.0,
        fy: 627.0,
        cx: 322.5,
        cy: 238.0,
        k1: -0.20,
        k2: 0.05,
        p1: 0.0006,
        p2: -0.0005,
        k3: 0.0,
        model: LensModel::BrownConrady,
        k4: 0.0,
        view_zoom: 1.0,
        rms_error_px: 0.0,
        views_used: 0,
    }
}

#[derive(Clone, Copy)]
pub struct Pose {
    pub r: Matrix3<f64>,
    pub t: Vector3<f64>,
}

impl Pose {
    /// Giros en grados (x, y, z = alabeo) alrededor del centro del tablero, que queda
    /// en `center` (mm, coordenadas de la cámara).
    pub fn about_center(
        board: [u32; 2],
        sq: f64,
        deg: (f64, f64, f64),
        center: (f64, f64, f64),
    ) -> Self {
        let d = std::f64::consts::PI / 180.0;
        let r = Rotation3::from_euler_angles(deg.0 * d, deg.1 * d, deg.2 * d)
            .matrix()
            .to_owned();
        let c = Vector3::new(
            (board[0] - 1) as f64 * sq / 2.0,
            (board[1] - 1) as f64 * sq / 2.0,
            0.0,
        );
        let t = Vector3::new(center.0, center.1, center.2) - r * c;
        Self { r, t }
    }
}

/// Proyecta un punto del plano del tablero (mm) a píxeles, con distorsión.
pub fn project(cam: &LensCalibration, pose: &Pose, x: f64, y: f64) -> Point {
    let p = pose.r * Vector3::new(x, y, 0.0) + pose.t;
    let (xd, yd) = cam.distort_normalized(p.x / p.z, p.y / p.z);
    Point::new(cam.fx * xd + cam.cx, cam.fy * yd + cam.cy)
}

pub fn board_corners(cam: &LensCalibration, pose: &Pose, board: [u32; 2], sq: f64) -> Vec<Point> {
    let mut v = Vec::new();
    for j in 0..board[1] {
        for i in 0..board[0] {
            v.push(project(cam, pose, f64::from(i) * sq, f64::from(j) * sq));
        }
    }
    v
}

/// Generador xorshift con ruido gaussiano (Box-Muller).
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1)
    }
    pub fn uniform(&mut self) -> f64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        ((x >> 11) as f64 + 0.5) / (1u64 << 53) as f64
    }
    pub fn gauss(&mut self) -> f64 {
        let (a, b) = (self.uniform(), self.uniform());
        (-2.0 * a.ln()).sqrt() * (2.0 * std::f64::consts::PI * b).cos()
    }
}

/// Renderiza el tablero. Cuadrado oscuro en el índice (0, 0) (el contiguo al origen
/// hacia +x, +y); margen blanco alrededor.
pub fn render_board(
    cam: &LensCalibration,
    pose: &Pose,
    board: [u32; 2],
    sq: f64,
    noise: f32,
    seed: u64,
) -> GrayImage {
    let (w, h) = (cam.width as usize, cam.height as usize);
    // Plano -> imagen sin distorsión: K [r1 r2 t]; se invierte para el trazado inverso.
    let hmat = Matrix3::from_columns(&[pose.r.column(0).into(), pose.r.column(1).into(), pose.t]);
    let hinv = hmat.try_inverse().expect("pose degenerada");
    let intensity_at = |u: f64, v: f64| -> f32 {
        let (xd, yd) = ((u - cam.cx) / cam.fx, (v - cam.cy) / cam.fy);
        let (x, y) = cam.undistort_normalized(xd, yd);
        let q = hinv * Vector3::new(x, y, 1.0);
        let (bx, by) = (q.x / q.z, q.y / q.z);
        let (ci, cj) = ((bx / sq).floor(), (by / sq).floor());
        let inside = ci >= -1.0
            && ci <= f64::from(board[0]) - 1.0
            && cj >= -1.0
            && cj <= f64::from(board[1]) - 1.0;
        if inside && ((ci + cj) as i64).rem_euclid(2) == 0 {
            DARK
        } else {
            LIGHT
        }
    };
    // Rejilla de muestras en las esquinas de los píxeles.
    let gw = w + 1;
    let mut grid = vec![0.0f32; gw * (h + 1)];
    for j in 0..=h {
        for i in 0..=w {
            grid[j * gw + i] = intensity_at(i as f64 - 0.5, j as f64 - 0.5);
        }
    }
    let mut data = vec![0.0f32; w * h];
    for j in 0..h {
        for i in 0..w {
            let c = [
                grid[j * gw + i],
                grid[j * gw + i + 1],
                grid[(j + 1) * gw + i],
                grid[(j + 1) * gw + i + 1],
            ];
            data[j * w + i] = if c.iter().all(|&v| v == c[0]) {
                c[0]
            } else {
                let n = 4;
                let mut s = 0.0;
                for sj in 0..n {
                    for si in 0..n {
                        let u = i as f64 - 0.5 + (f64::from(si) + 0.5) / f64::from(n);
                        let v = j as f64 - 0.5 + (f64::from(sj) + 0.5) / f64::from(n);
                        s += intensity_at(u, v);
                    }
                }
                s / (n * n) as f32
            };
        }
    }
    let blurred = gaussian_blur(&FloatImage { w, h, data }, 0.7);
    let mut rng = Rng::new(seed);
    let pixels: Vec<u8> = blurred
        .data
        .iter()
        .map(|&v| (v + noise * rng.gauss() as f32).round().clamp(0.0, 255.0) as u8)
        .collect();
    GrayImage::from_raw(cam.width, cam.height, pixels).unwrap()
}

/// Poses variadas para calibrar con un tablero de 9x6 de 25 mm.
pub fn standard_poses(board: [u32; 2], sq: f64) -> Vec<Pose> {
    type Spec = ((f64, f64, f64), (f64, f64, f64));
    let specs: [Spec; 10] = [
        ((25.0, -20.0, 10.0), (-60.0, -40.0, 520.0)),
        ((-30.0, 15.0, -15.0), (70.0, 50.0, 480.0)),
        ((10.0, 35.0, 25.0), (0.0, 0.0, 560.0)),
        ((-20.0, -30.0, 170.0), (-80.0, 60.0, 600.0)),
        ((35.0, 5.0, -40.0), (40.0, -40.0, 510.0)),
        ((-10.0, -25.0, 90.0), (-30.0, 40.0, 520.0)),
        ((20.0, 25.0, -100.0), (40.0, -20.0, 620.0)),
        ((30.0, -35.0, 5.0), (-50.0, -50.0, 500.0)),
        ((-35.0, 30.0, 12.0), (50.0, 60.0, 540.0)),
        ((5.0, -10.0, -8.0), (0.0, 10.0, 450.0)),
    ];
    specs
        .iter()
        .map(|&(deg, c)| Pose::about_center(board, sq, deg, c))
        .collect()
}

/// Cámara virtual de ojo de pez (Kannala-Brandt) como las de 5 MP en la tapa de un CO2: unos
/// 140° de campo en la diagonal, de modo que el borde está a 70° del eje y `tan` no es útil.
pub fn fisheye_camera() -> LensCalibration {
    LensCalibration {
        width: 640,
        height: 480,
        fx: 330.0,
        fy: 331.0,
        cx: 321.0,
        cy: 241.0,
        k1: -0.04,
        k2: 0.012,
        p1: 0.0,
        p2: 0.0,
        k3: 0.0,
        model: LensModel::Fisheye,
        k4: 0.0,
        view_zoom: 1.0,
        rms_error_px: 0.0,
        views_used: 0,
    }
}

/// Poses de un tablero de 9x6 que recorren el campo del ojo de pez hasta unos 60° del eje: la
/// normal apunta hacia la cámara más una inclinación y un giro propios de cada vista.
pub fn fisheye_poses(board: [u32; 2], sq: f64) -> Vec<Pose> {
    // (centro en mm, inclinación°, giro°)
    let specs: [((f64, f64, f64), f64, f64); 16] = [
        ((0.0, 0.0, 330.0), 20.0, 5.0),
        ((-210.0, -110.0, 330.0), 25.0, 20.0),
        ((230.0, -100.0, 340.0), -25.0, -15.0),
        ((-220.0, 120.0, 330.0), 30.0, 70.0),
        ((210.0, 130.0, 330.0), -20.0, 100.0),
        ((0.0, -150.0, 300.0), 35.0, 0.0),
        ((20.0, 160.0, 310.0), -30.0, 160.0),
        ((-330.0, 10.0, 360.0), 15.0, 40.0),
        ((340.0, 20.0, 360.0), -15.0, -50.0),
        ((-90.0, 60.0, 280.0), 30.0, -90.0),
        ((120.0, -70.0, 290.0), -35.0, 30.0),
        ((-150.0, -190.0, 400.0), 10.0, 120.0),
        ((-360.0, -230.0, 360.0), 15.0, 30.0),
        ((360.0, -230.0, 360.0), -15.0, -30.0),
        ((-360.0, 230.0, 360.0), 20.0, 60.0),
        ((360.0, 230.0, 360.0), -20.0, 110.0),
    ];
    specs
        .iter()
        .map(|&((x, y, z), tilt, roll)| {
            let c = Vector3::new(x, y, z);
            let n = -c.normalize();
            let base =
                Rotation3::rotation_between(&Vector3::z(), &n).unwrap_or_else(Rotation3::identity);
            let d = std::f64::consts::PI / 180.0;
            let r = (base * Rotation3::from_euler_angles(tilt * d, 0.0, roll * d))
                .matrix()
                .to_owned();
            let local_center = Vector3::new(
                (board[0] - 1) as f64 * sq / 2.0,
                (board[1] - 1) as f64 * sq / 2.0,
                0.0,
            );
            Pose {
                r,
                t: c - r * local_center,
            }
        })
        .collect()
}
