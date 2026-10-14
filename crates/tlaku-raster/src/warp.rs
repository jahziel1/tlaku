//! Giro y deformación afín de una imagen sobre una rejilla alineada a los ejes.
//!
//! Lo usa Print & Cut: si el registro gira el trabajo, la imagen se vuelve a muestrear sobre la
//! caja que ocupa ya girada y luego se traza como cualquier otra imagen. Se hace sobre el gris,
//! antes de ajustar y tramar, para que el tramado actúe sobre los puntos finales.

use crate::gray::GrayF32;
use tlaku_i18n::tr;

/// Transformación afín `[a, b, c, d, e, f]`: `x' = a·x + c·y + e`, `y' = b·x + d·y + f`
/// (el mismo orden que `tlaku_doc::affine::Transform`).
pub type Affine = [f64; 6];

/// Inversa de `t`; `None` si no es invertible o tiene valores que no son números.
pub fn invert(t: &Affine) -> Option<Affine> {
    let det = t[0] * t[3] - t[1] * t[2];
    if !det.is_finite() || det.abs() < 1e-12 || !t.iter().all(|v| v.is_finite()) {
        return None;
    }
    let (a, b, c, d) = (t[3] / det, -t[1] / det, -t[2] / det, t[0] / det);
    Some([a, b, c, d, -(a * t[4] + c * t[5]), -(b * t[4] + d * t[5])])
}

/// Muestra bilineal de `img` en la coordenada de píxel continua `(u, v)` (el centro del píxel
/// `(i, j)` es `(i, j)`), con los bordes repetidos.
fn sample(img: &GrayF32, u: f64, v: f64) -> f32 {
    let max_x = (img.width - 1) as f64;
    let max_y = (img.height - 1) as f64;
    let (u, v) = (u.clamp(0.0, max_x), v.clamp(0.0, max_y));
    let (x0, y0) = (u.floor() as usize, v.floor() as usize);
    let (x1, y1) = ((x0 + 1).min(img.width - 1), (y0 + 1).min(img.height - 1));
    let (fx, fy) = ((u - x0 as f64) as f32, (v - y0 as f64) as f32);
    let top = img.get(x0, y0) * (1.0 - fx) + img.get(x1, y0) * fx;
    let bottom = img.get(x0, y1) * (1.0 - fx) + img.get(x1, y1) * fx;
    top * (1.0 - fy) + bottom * fy
}

/// Vuelve a muestrear `src` (que cubre el rectángulo de esquina inferior izquierda `src_origin`
/// y tamaño `src_size`, en mm, con la fila 0 arriba) sobre una rejilla de `cols` x `rows` que
/// cubre `dst_origin` y `dst_size`, después de aplicar `forward` (de coordenadas de `src` a las
/// de la rejilla). Lo que cae fuera de la imagen vale `fill`.
#[allow(clippy::too_many_arguments)]
pub fn warp_affine(
    src: &GrayF32,
    src_origin: [f64; 2],
    src_size: [f64; 2],
    forward: &Affine,
    dst_origin: [f64; 2],
    dst_size: [f64; 2],
    cols: usize,
    rows: usize,
    fill: f32,
) -> Result<GrayF32, String> {
    let inv = invert(forward).ok_or_else(|| tr!("raster.warp.not_invertible"))?;
    if cols == 0 || rows == 0 {
        return Err(tr!("raster.warp.zero_grid"));
    }
    let (px, py) = (dst_size[0] / cols as f64, dst_size[1] / rows as f64);
    let (kx, ky) = (
        src.width as f64 / src_size[0],
        src.height as f64 / src_size[1],
    );
    let mut data = Vec::with_capacity(cols * rows);
    for row in 0..rows {
        // Centro de la fila: la fila 0 es la de arriba.
        let y = dst_origin[1] + dst_size[1] - (row as f64 + 0.5) * py;
        for col in 0..cols {
            let x = dst_origin[0] + (col as f64 + 0.5) * px;
            let sx = inv[0] * x + inv[2] * y + inv[4] - src_origin[0];
            let sy = inv[1] * x + inv[3] * y + inv[5] - src_origin[1];
            let inside = sx >= 0.0 && sx <= src_size[0] && sy >= 0.0 && sy <= src_size[1];
            data.push(if inside {
                sample(src, sx * kx - 0.5, (src_size[1] - sy) * ky - 0.5)
            } else {
                fill
            });
        }
    }
    GrayF32::new(cols, rows, data)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ramp(w: usize, h: usize) -> GrayF32 {
        GrayF32::from_fn(w, h, |x, y| (x + y * w) as f32 / (w * h) as f32)
    }

    const ID: Affine = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];

    #[test]
    fn invert_round_trips() {
        let t = [0.9, 0.3, -0.2, 1.1, 5.0, -7.0];
        let i = invert(&t).unwrap();
        // t(i(p)) = p
        let p = [3.0, 4.0];
        let q = [
            i[0] * p[0] + i[2] * p[1] + i[4],
            i[1] * p[0] + i[3] * p[1] + i[5],
        ];
        let r = [
            t[0] * q[0] + t[2] * q[1] + t[4],
            t[1] * q[0] + t[3] * q[1] + t[5],
        ];
        assert!((r[0] - p[0]).abs() < 1e-12 && (r[1] - p[1]).abs() < 1e-12);
        assert!(invert(&[1.0, 2.0, 2.0, 4.0, 0.0, 0.0]).is_none());
        assert!(invert(&[f64::NAN, 0.0, 0.0, 1.0, 0.0, 0.0]).is_none());
    }

    #[test]
    fn identity_keeps_the_image() {
        let src = ramp(8, 6);
        let out = warp_affine(
            &src,
            [10.0, 20.0],
            [8.0, 6.0],
            &ID,
            [10.0, 20.0],
            [8.0, 6.0],
            8,
            6,
            1.0,
        )
        .unwrap();
        assert_eq!(out, src);
    }

    #[test]
    fn a_quarter_turn_moves_the_corners() {
        // Imagen de 4 x 2 mm con origen (0, 0): arriba a la izquierda negra, el resto blanco.
        let mut src = GrayF32::from_fn(4, 2, |_, _| 1.0);
        src.data[0] = 0.0;
        // Giro de 90 antihorario alrededor de (0, 0): (x, y) -> (-y, x). La caja destino es
        // x de -2 a 0 y y de 0 a 4. El punto de arriba a la izquierda (0, 2) pasa a (-2, 0), es
        // decir, abajo a la izquierda de la caja.
        let t: Affine = [0.0, 1.0, -1.0, 0.0, 0.0, 0.0];
        let out = warp_affine(
            &src,
            [0.0, 0.0],
            [4.0, 2.0],
            &t,
            [-2.0, 0.0],
            [2.0, 4.0],
            2,
            4,
            1.0,
        )
        .unwrap();
        assert_eq!((out.width, out.height), (2, 4));
        assert!(out.get(0, 3) < 0.01, "{:?}", out.data);
        assert!(out.data.iter().filter(|&&v| v < 0.01).count() == 1);
    }

    #[test]
    fn outside_the_source_takes_the_fill_value() {
        let src = GrayF32::from_fn(4, 4, |_, _| 0.0);
        // Caja destino el doble de grande que la imagen: el borde queda con `fill`.
        let out = warp_affine(
            &src,
            [0.0, 0.0],
            [4.0, 4.0],
            &ID,
            [-2.0, -2.0],
            [8.0, 8.0],
            8,
            8,
            0.75,
        )
        .unwrap();
        assert_eq!(out.get(0, 0), 0.75);
        assert_eq!(out.get(4, 4), 0.0);
        assert_eq!(out.get(7, 7), 0.75);
    }

    #[test]
    fn rejects_singular_transforms_and_empty_grids() {
        let src = ramp(2, 2);
        assert!(
            warp_affine(
                &src, [0.0; 2], [1.0; 2], &[0.0; 6], [0.0; 2], [1.0; 2], 2, 2, 1.0
            )
            .is_err()
        );
        assert!(warp_affine(&src, [0.0; 2], [1.0; 2], &ID, [0.0; 2], [1.0; 2], 0, 2, 1.0).is_err());
    }
}
