//! Ajuste del mapeo píxel a cama con las posiciones del cabezal.

use super::lensfit;
use crate::calibration::{LensCalibration, LensModel};
use crate::homography::{Homography, estimate_homography};
use serde::{Deserialize, Serialize};
use tlaku_core::Point;
use tlaku_i18n::tr;

/// Puntos mínimos para ajustar (la validación cruzada necesita uno más que los 4 de la homografía).
pub const MIN_POINTS: usize = 5;
/// Puntos por debajo de los cuales se avisa que el resultado se apoya en pocos datos.
pub const FEW_POINTS: usize = 8;
/// Error rms de validación cruzada hasta el cual el resultado es bueno / aceptable (mm).
pub const GOOD_LOO_MM: f64 = 1.0;
pub const FAIR_LOO_MM: f64 = 2.5;

/// Una foto con el cabezal localizado: dónde estaba la máquina y en qué píxel se vio.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct HeadObservation {
    /// Posición del láser (mm de documento).
    pub machine_mm: [f64; 2],
    /// Píxel del punto característico en la imagen de entrada (centro del píxel `(i, j)` = `(i, j)`).
    pub px: [f64; 2],
}

/// Alturas para corregir el paralaje: el punto del cabezal que se detecta está `feature_height_mm`
/// sobre el material y la cámara `camera_height_mm`. El punto del material que se ve en el mismo
/// píxel queda más lejos de la vertical de la cámara en la razón `cámara / (cámara - punto)`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Parallax {
    pub camera_height_mm: f64,
    pub feature_height_mm: f64,
}

/// Cómo ajustar.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct HeadAlignOptions {
    /// Tamaño (px) de la imagen donde se detectó el cabezal.
    pub image_size: [u32; 2],
    /// Calibración con que ya se corrigió esa imagen (la usa solo para ubicar la vertical de la
    /// cámara). `None` = imagen sin corregir.
    pub lens: Option<LensCalibration>,
    /// Sin calibración, estima una distorsión aproximada si mejora de verdad el ajuste.
    pub estimate_lens: bool,
    /// Corrección de paralaje; `None` = el punto del cabezal se toma tal cual.
    pub parallax: Option<Parallax>,
    /// Desfase fijo (mm) que se suma a la posición de la cama de cada píxel.
    pub offset_mm: [f64; 2],
    /// Un punto cuyo error pasa de este valor (mm) y de 4 veces el error típico se descarta.
    pub outlier_mm: f64,
}

impl Default for HeadAlignOptions {
    fn default() -> Self {
        Self {
            image_size: [0, 0],
            lens: None,
            estimate_lens: true,
            parallax: None,
            offset_mm: [0.0, 0.0],
            outlier_mm: 1.5,
        }
    }
}

/// Lo que pasó con un punto.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PointFit {
    pub machine_mm: [f64; 2],
    /// Píxel en la imagen corregida con el origen en el borde del píxel (el de la alineación).
    pub image_px: [f64; 2],
    /// Posición de la cama que el ajuste espera para ese punto (tras paralaje y desfase).
    pub bed_mm: [f64; 2],
    /// Error (mm): para un punto usado, el de ajustar sin él (validación cruzada); para uno
    /// descartado, el del ajuste final.
    pub error_mm: f64,
    /// Error del ajuste final sobre este punto (mm).
    pub residual_mm: f64,
    pub used: bool,
}

/// Valoración global.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Quality {
    Good,
    Fair,
    Poor,
}

/// Avisos del ajuste.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum HeadAlignIssue {
    /// El cabezal no se encontró en estas fotos (fuera de vista, poca luz...).
    NotDetected { count: usize },
    /// Estos puntos se descartaron por no concordar con el resto.
    Dropped { count: usize },
    /// Quedan pocos puntos: el resultado se apoya en poco.
    FewPoints { used: usize },
    /// La cámara solo vio el cabezal en parte del área segura: fuera de la caja de los puntos
    /// usados el mapeo extrapola.
    PartialCoverage {
        covered_mm: [f64; 4],
        area_mm: [f64; 4],
    },
    /// El error de validación cruzada es grande.
    HighError { loo_rms_mm: f64 },
    /// No había calibración de lente: se estimó una aproximada.
    LensEstimated {
        model: LensModel,
        rms_before_mm: f64,
        rms_after_mm: f64,
    },
}

/// Resultado del ajuste.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HeadAlignFit {
    /// Píxel de la imagen corregida (origen en el borde del píxel) a mm de la cama al nivel del
    /// material, por filas. Ya incluye paralaje y desfase.
    pub to_bed: [[f64; 3]; 3],
    /// Lente estimada, si no había calibración y mejoró el ajuste. La imagen corregida con ella
    /// es sobre la que vale `to_bed`.
    pub lens: Option<LensCalibration>,
    pub points: Vec<PointFit>,
    /// Error rms y máximo (mm) del ajuste sobre los puntos usados.
    pub rms_mm: f64,
    pub max_mm: f64,
    /// Lo mismo con validación cruzada (cada punto predicho sin él).
    pub loo_rms_mm: f64,
    pub loo_max_mm: f64,
    pub used: usize,
    pub dropped: usize,
    /// Caja `[x0, y0, x1, y1]` (mm de la máquina) de los puntos usados: la zona donde el ajuste
    /// está respaldado por datos; fuera de ella extrapola.
    pub covered_mm: [f64; 4],
    pub quality: Quality,
    /// Desfase total aplicado (mm), con el de las opciones y el de la referencia.
    pub offset_mm: [f64; 2],
    /// Razón de paralaje aplicada (1 = ninguna) y vertical de la cámara en la cama (mm).
    pub parallax_scale: f64,
    pub nadir_mm: [f64; 2],
    pub issues: Vec<HeadAlignIssue>,
}

fn pts(v: impl Iterator<Item = [f64; 2]>) -> Vec<Point> {
    v.map(|p| Point::new(p[0], p[1])).collect()
}

fn dlt(px: &[Point], mm: &[Point]) -> Result<Homography, String> {
    let pairs: Vec<(Point, Point)> = px.iter().copied().zip(mm.iter().copied()).collect();
    let h = estimate_homography(&pairs)?;
    if h.m.iter().flatten().any(|v| !v.is_finite()) {
        return Err(tr!("camera.head_align.fit.degenerate"));
    }
    Ok(h)
}

fn dist(a: Point, b: Point) -> f64 {
    (a.x - b.x).hypot(a.y - b.y)
}

fn median(v: &[f64]) -> f64 {
    let mut s = v.to_vec();
    s.sort_by(f64::total_cmp);
    if s.is_empty() { 0.0 } else { s[s.len() / 2] }
}

/// Subconjunto `used` de los pares.
fn pick<T: Copy>(v: &[T], used: &[bool]) -> Vec<T> {
    v.iter()
        .zip(used)
        .filter(|(_, u)| **u)
        .map(|(p, _)| *p)
        .collect()
}

/// Ajusta el mapeo píxel a cama con las observaciones del cabezal (ver el módulo).
pub fn fit_head_alignment(
    obs: &[HeadObservation],
    opts: &HeadAlignOptions,
) -> Result<HeadAlignFit, String> {
    let n = obs.len();
    if n < MIN_POINTS {
        return Err(tr!(
            "camera.head_align.fit.few_points",
            n = n,
            min = MIN_POINTS
        ));
    }
    if opts.image_size[0] == 0 || opts.image_size[1] == 0 {
        return Err(tr!("camera.head_align.fit.zero_size"));
    }
    if obs
        .iter()
        .flat_map(|o| o.machine_mm.iter().chain(&o.px))
        .any(|v| !v.is_finite())
        || opts.offset_mm.iter().any(|v| !v.is_finite())
    {
        return Err(tr!("camera.head_align.fit.not_finite"));
    }
    if let Some(p) = &opts.parallax
        && !(p.camera_height_mm.is_finite()
            && p.feature_height_mm.is_finite()
            && p.feature_height_mm >= 0.0
            && p.camera_height_mm > p.feature_height_mm + 5.0)
    {
        return Err(tr!("camera.head_align.fit.bad_parallax"));
    }
    let machine = pts(obs.iter().map(|o| o.machine_mm));
    let raw = pts(obs.iter().map(|o| o.px));
    let (w, h) = (f64::from(opts.image_size[0]), f64::from(opts.image_size[1]));
    let mut issues = Vec::new();

    // 1. Imagen corregida: la que entró, o la de una lente estimada que mejora el ajuste.
    let mut lens_out = None;
    let mut corrected = raw.clone();
    if opts.lens.is_none() && opts.estimate_lens && n >= lensfit::MIN_POINTS_FOR_LENS {
        let size = (opts.image_size[0], opts.image_size[1]);
        if let Some(g) = lensfit::estimate_lens(&raw, &machine, size) {
            corrected = lensfit::correct_all(&g.lens, &raw);
            issues.push(HeadAlignIssue::LensEstimated {
                model: g.lens.model,
                rms_before_mm: g.rms_before_mm,
                rms_after_mm: g.rms_after_mm,
            });
            lens_out = Some(g.lens);
        }
    }
    let px: Vec<Point> = corrected
        .iter()
        .map(|p| Point::new(p.x + 0.5, p.y + 0.5))
        .collect();
    // Vertical de la cámara en la imagen corregida (origen en el borde del píxel).
    let principal = match (&lens_out, &opts.lens) {
        (Some(l), _) => Point::new(l.cx + 0.5, l.cy + 0.5),
        (None, Some(l)) => {
            let (sx, sy) = (w / f64::from(l.width), h / f64::from(l.height));
            Point::new(sx * (l.cx + 0.5), sy * (l.cy + 0.5))
        }
        (None, None) => Point::new(w / 2.0, h / 2.0),
    };

    // 2. Paralaje: a qué punto del material corresponde cada posición del cabezal.
    let h0 = dlt(&px, &machine)?;
    let nadir = h0.apply(principal);
    let (k, nadir) = match &opts.parallax {
        Some(p) if nadir.x.is_finite() && nadir.y.is_finite() => (
            p.camera_height_mm / (p.camera_height_mm - p.feature_height_mm),
            nadir,
        ),
        _ => (1.0, Point::new(0.0, 0.0)),
    };
    let offset = Point::new(opts.offset_mm[0], opts.offset_mm[1]);
    let target = |m: Point| {
        Point::new(
            nadir.x + k * (m.x - nadir.x) + offset.x,
            nadir.y + k * (m.y - nadir.y) + offset.y,
        )
    };
    let bed: Vec<Point> = machine.iter().map(|&m| target(m)).collect();

    // 3. Ajuste con descarte de atípicos, de uno en uno el peor.
    let mut used = vec![true; n];
    let max_drop = (n * 3 / 10).max(1);
    let mut hom = dlt(&px, &bed)?;
    loop {
        let res: Vec<f64> = (0..n).map(|i| dist(hom.apply(px[i]), bed[i])).collect();
        let live: Vec<f64> = (0..n).filter(|&i| used[i]).map(|i| res[i]).collect();
        let scale = (1.4826 * median(&live)).max(0.05);
        let thr = opts.outlier_mm.max(4.0 * scale);
        let worst = (0..n)
            .filter(|&i| used[i])
            .max_by(|&a, &b| res[a].total_cmp(&res[b]));
        let dropped = used.iter().filter(|u| !**u).count();
        match worst {
            Some(i) if res[i] > thr && live.len() > MIN_POINTS && dropped < max_drop => {
                used[i] = false;
                hom = dlt(&pick(&px, &used), &pick(&bed, &used))?;
            }
            _ => break,
        }
    }
    let count = used.iter().filter(|u| **u).count();
    if count < MIN_POINTS {
        return Err(tr!(
            "camera.head_align.fit.few_points",
            n = count,
            min = MIN_POINTS
        ));
    }

    // 4. Errores finales: ajuste sobre los usados y validación cruzada de cada uno.
    let mut points = Vec::with_capacity(n);
    let (mut s2, mut mx, mut l2, mut lmx) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
    for i in 0..n {
        let residual = dist(hom.apply(px[i]), bed[i]);
        let error = if used[i] {
            let mut without = used.clone();
            without[i] = false;
            let e = dlt(&pick(&px, &without), &pick(&bed, &without))
                .map_or(f64::INFINITY, |hi| dist(hi.apply(px[i]), bed[i]));
            s2 += residual * residual;
            mx = mx.max(residual);
            l2 += e * e;
            lmx = lmx.max(e);
            e
        } else {
            residual
        };
        points.push(PointFit {
            machine_mm: obs[i].machine_mm,
            image_px: [px[i].x, px[i].y],
            bed_mm: [bed[i].x, bed[i].y],
            error_mm: error,
            residual_mm: residual,
            used: used[i],
        });
    }
    let c = count as f64;
    let (rms_mm, loo_rms_mm) = ((s2 / c).sqrt(), (l2 / c).sqrt());
    let dropped = n - count;
    let mut covered_mm = [
        f64::INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NEG_INFINITY,
    ];
    for p in points.iter().filter(|p| p.used) {
        covered_mm = [
            covered_mm[0].min(p.machine_mm[0]),
            covered_mm[1].min(p.machine_mm[1]),
            covered_mm[2].max(p.machine_mm[0]),
            covered_mm[3].max(p.machine_mm[1]),
        ];
    }
    if dropped > 0 {
        issues.push(HeadAlignIssue::Dropped { count: dropped });
    }
    if count < FEW_POINTS {
        issues.push(HeadAlignIssue::FewPoints { used: count });
    }
    if loo_rms_mm > GOOD_LOO_MM {
        issues.push(HeadAlignIssue::HighError { loo_rms_mm });
    }
    let quality = if loo_rms_mm <= GOOD_LOO_MM && lmx <= 2.5 * GOOD_LOO_MM {
        Quality::Good
    } else if loo_rms_mm <= FAIR_LOO_MM {
        Quality::Fair
    } else {
        Quality::Poor
    };
    Ok(HeadAlignFit {
        to_bed: hom.m,
        lens: lens_out,
        points,
        rms_mm,
        max_mm: mx,
        loo_rms_mm,
        loo_max_mm: lmx,
        used: count,
        dropped,
        covered_mm,
        quality,
        offset_mm: opts.offset_mm,
        parallax_scale: k,
        nadir_mm: [nadir.x, nadir.y],
        issues,
    })
}

/// Desfase (mm) que hace que el píxel `ref_px` (origen en el borde del píxel, en la imagen
/// corregida del ajuste) caiga en `ref_bed`: el desfase actual más lo que falta. Sirve con un
/// único punto conocido, p. ej. un disparo de prueba visible que el usuario marca en la foto.
pub fn reference_offset(fit: &HeadAlignFit, ref_px: [f64; 2], ref_bed: [f64; 2]) -> [f64; 2] {
    let at = Homography { m: fit.to_bed }.apply(Point::new(ref_px[0], ref_px[1]));
    [
        fit.offset_mm[0] + ref_bed[0] - at.x,
        fit.offset_mm[1] + ref_bed[1] - at.y,
    ]
}
