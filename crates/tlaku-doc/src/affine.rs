//! Transformaciones afines `[a b c d e f]`: `x' = a·x + c·y + e`, `y' = b·x + d·y + f`.

pub type Transform = [f64; 6];

pub const IDENTITY: Transform = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];

pub fn translate(dx: f64, dy: f64) -> Transform {
    [1.0, 0.0, 0.0, 1.0, dx, dy]
}

pub fn scale(sx: f64, sy: f64) -> Transform {
    [sx, 0.0, 0.0, sy, 0.0, 0.0]
}

/// Rotación antihoraria (Y hacia arriba) de `radians` alrededor del origen.
pub fn rotate(radians: f64) -> Transform {
    let (s, c) = radians.sin_cos();
    [c, s, -s, c, 0.0, 0.0]
}

pub fn is_identity(t: &Transform) -> bool {
    *t == IDENTITY
}

pub fn apply(t: &Transform, p: [f64; 2]) -> [f64; 2] {
    [
        t[0] * p[0] + t[2] * p[1] + t[4],
        t[1] * p[0] + t[3] * p[1] + t[5],
    ]
}

/// Transformación que aplica primero `inner` y después `outer`.
pub fn compose(outer: &Transform, inner: &Transform) -> Transform {
    [
        outer[0] * inner[0] + outer[2] * inner[1],
        outer[1] * inner[0] + outer[3] * inner[1],
        outer[0] * inner[2] + outer[2] * inner[3],
        outer[1] * inner[2] + outer[3] * inner[3],
        outer[0] * inner[4] + outer[2] * inner[5] + outer[4],
        outer[1] * inner[4] + outer[3] * inner[5] + outer[5],
    ]
}

pub fn is_finite(t: &Transform) -> bool {
    t.iter().all(|v| v.is_finite())
}
