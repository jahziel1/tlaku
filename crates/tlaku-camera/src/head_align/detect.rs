//! Localiza el cabezal en cada foto.
//!
//! Dos métodos, según lo que se vea:
//!
//! - **LED verde** (`HeadMethod::Led`): muchos módulos láser llevan un LED indicador. Se busca el
//!   punto donde el canal verde supera al rojo, que en una imagen rojiza (ventana de la tapa) es
//!   inconfundible (el núcleo del LED puede saturar a blanco: se sigue su halo verde), sin necesidad de comparar fotos. Un punto verde que no se mueve
//!   entre fotos (un LED de la cámara) se descarta. El centro sale ponderado por el exceso de
//!   verde: precisión de subpíxel y es un punto físico del cabezal (no cambia con la perspectiva).
//! - **Diferencia de imágenes** (`HeadMethod::Diff`): no hace falta saber cómo se ve el cabezal.
//!   Con N fotos donde se mueve y todo lo demás está quieto, la mediana por píxel es la cama sin
//!   cabezal (el cabezal tapa un píxel en pocas fotos). Lo que cambia entre una foto y esa
//!   mediana es el cabezal. El pórtico (el riel del eje X) se mueve con la posición Y: las fotos
//!   se agrupan por fila (misma Y) y cada grupo tiene su propia mediana, así el pórtico queda en
//!   el fondo. Antes de restar se iguala el brillo de la foto al del fondo (ganancia y desfase
//!   robustos): la exposición automática de las cámaras baratas cambia de una foto a otra.
//!
//! `HeadMethod::Auto` usa el LED si se ve en alguna foto y la diferencia si no.

use crate::imgproc::{FloatImage, gaussian_blur};
use image::{GrayImage, RgbImage};
use serde::{Deserialize, Serialize};
use tlaku_i18n::tr;

/// Fotos mínimas por grupo para que la mediana sea un fondo fiable.
pub const MIN_FRAMES: usize = 3;

/// Cómo se localiza el cabezal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HeadMethod {
    /// LED verde si aparece en alguna foto; si no, diferencia de imágenes.
    #[default]
    Auto,
    Led,
    Diff,
}

/// Qué punto de la silueta se toma como posición del cabezal (método `Diff`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HeadFeature {
    /// Centro de la silueta: el más estable ante el ruido y los cambios de perspectiva.
    #[default]
    Centroid,
    /// Extremo inferior de la silueta (mayor `y` de la imagen): útil si el cabezal se ve con un
    /// brazo o cable arriba que tapa la forma, y la boquilla apunta hacia abajo en la imagen.
    BottomTip,
}

/// Opciones del detector.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct HeadDetectOptions {
    pub method: HeadMethod,
    pub feature: HeadFeature,
    /// Cambio mínimo de brillo (niveles de gris, ya suavizado) para contar como cabezal.
    pub min_contrast: f32,
    /// Fracción del cambio máximo que debe tener un píxel: deja fuera sombras y reflejos tenues.
    pub relative_threshold: f32,
    /// Área mínima y máxima de la silueta, como fracción de la imagen.
    pub min_area_frac: f64,
    pub max_area_frac: f64,
    /// Una silueta a menos de estos píxeles del borde se considera cortada (cabezal fuera de vista).
    pub border_px: usize,
    /// Suavizado (desviación, px) de la diferencia antes de umbralar.
    pub blur_sigma: f32,
    /// Exceso mínimo de verde sobre rojo (niveles) para aceptar un LED.
    pub led_min_score: f32,
    /// Área máxima del LED como fracción de la imagen.
    pub led_max_area_frac: f64,
}

impl Default for HeadDetectOptions {
    fn default() -> Self {
        Self {
            method: HeadMethod::Auto,
            feature: HeadFeature::Centroid,
            min_contrast: 14.0,
            relative_threshold: 0.25,
            min_area_frac: 0.0003,
            max_area_frac: 0.30,
            border_px: 2,
            blur_sigma: 1.5,
            led_min_score: 35.0,
            led_max_area_frac: 0.002,
        }
    }
}

/// El cabezal encontrado en una foto.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HeadDetection {
    /// Punto elegido, en px (centro del píxel `(i, j)` = `(i, j)`): el LED, o la silueta según
    /// `HeadDetectOptions::feature`.
    pub px: [f64; 2],
    /// Centro y extremo inferior de la silueta (con `Led` valen lo mismo que `px`).
    pub centroid_px: [f64; 2],
    pub tip_px: [f64; 2],
    /// Píxeles de la silueta (con huecos rellenos) o del LED.
    pub area_px: f64,
    /// Caja `[x0, y0, x1, y1]`.
    pub bbox: [f64; 4],
    /// Cambio medio de brillo (silueta) o exceso de verde (LED), en niveles.
    pub contrast: f32,
    /// Fiabilidad 0..1.
    pub confidence: f64,
}

/// Por qué no se encontró el cabezal en una foto.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DetectFailure {
    /// Nada cambió lo bastante (o no hay LED a la vista): el cabezal está fuera de vista o hay
    /// poca luz.
    NoChange,
    /// Lo que cambió es demasiado pequeño para ser el cabezal.
    TooSmall,
    /// Cambió media imagen: la luz o la cámara se movieron.
    TooLarge,
    /// La silueta toca el borde: el cabezal está a medias fuera de la foto.
    TouchesBorder,
    /// Hay dos siluetas parecidas: no se sabe cuál es el cabezal.
    Ambiguous,
    /// Pocas fotos de la misma fila para formar el fondo.
    NoBackground,
}

/// Resultado de [`detect_heads`].
#[derive(Debug, Clone, PartialEq)]
pub struct DetectOutcome {
    /// Método usado (`Led` o `Diff`, nunca `Auto`).
    pub method: HeadMethod,
    /// Una entrada por foto, en el mismo orden.
    pub results: Vec<Result<HeadDetection, DetectFailure>>,
}

/// Busca el cabezal en cada foto. `groups[i]` es la fila (misma Y de la máquina) de la foto `i`:
/// las fotos de un grupo comparten el fondo del método `Diff`; vacío = todas en un grupo. Falla
/// si las fotos no miden lo mismo o si `groups` no coincide con `frames`.
pub fn detect_heads(
    frames: &[RgbImage],
    groups: &[usize],
    opts: &HeadDetectOptions,
) -> Result<DetectOutcome, String> {
    let Some(first) = frames.first() else {
        return Err(tr!(
            "camera.head_align.detect.few_frames",
            n = 0,
            min = MIN_FRAMES
        ));
    };
    let (w, h) = (first.width(), first.height());
    if w < 16 || h < 16 {
        return Err(tr!("camera.head_align.detect.tiny"));
    }
    if let Some(bad) = frames.iter().find(|f| (f.width(), f.height()) != (w, h)) {
        return Err(tr!(
            "camera.head_align.detect.size_mismatch",
            w = bad.width(),
            h = bad.height(),
            fw = w,
            fh = h
        ));
    }
    if !groups.is_empty() && groups.len() != frames.len() {
        return Err(tr!("camera.head_align.detect.groups_mismatch"));
    }
    let leds = find_leds(frames, opts);
    let led_frames = leds.iter().filter(|l| l.is_some()).count();
    let method = match opts.method {
        HeadMethod::Led => HeadMethod::Led,
        HeadMethod::Diff => HeadMethod::Diff,
        HeadMethod::Auto if led_frames > 0 => HeadMethod::Led,
        HeadMethod::Auto => HeadMethod::Diff,
    };
    let results = match method {
        HeadMethod::Led => leds
            .into_iter()
            .map(|l| l.ok_or(DetectFailure::NoChange))
            .collect(),
        _ => detect_by_diff(frames, groups, opts),
    };
    Ok(DetectOutcome { method, results })
}

// ---------------------------------------------------------------------------
// LED verde
// ---------------------------------------------------------------------------

/// El LED de cada foto (o `None`). Los que no se mueven entre fotos se descartan.
fn find_leds(frames: &[RgbImage], opts: &HeadDetectOptions) -> Vec<Option<HeadDetection>> {
    let mut found: Vec<Option<HeadDetection>> = frames.iter().map(|f| find_led(f, opts)).collect();
    // Un punto verde en el mismo lugar en la mitad de las fotos (o más) es de la cámara, no del cabezal.
    if frames.len() >= MIN_FRAMES {
        let pos: Vec<Option<[f64; 2]>> = found.iter().map(|d| d.as_ref().map(|d| d.px)).collect();
        for (i, p) in pos.iter().enumerate() {
            let Some(p) = p else { continue };
            let same = pos
                .iter()
                .flatten()
                .filter(|q| (q[0] - p[0]).hypot(q[1] - p[1]) < 3.0)
                .count();
            if same * 2 > frames.len() {
                found[i] = None;
            }
        }
    }
    found
}

fn find_led(img: &RgbImage, opts: &HeadDetectOptions) -> Option<HeadDetection> {
    let (w, h) = (img.width() as usize, img.height() as usize);
    let score = FloatImage {
        w,
        h,
        data: img
            .pixels()
            .map(|p| {
                // Verde sobre rojo: el núcleo del LED suele saturar a blanco, pero su halo es verde.
                let [r, g, _] = p.0;
                if g < 110 {
                    0.0
                } else {
                    (f32::from(g) - f32::from(r)).max(0.0)
                }
            })
            .collect(),
    };
    let score = gaussian_blur(&score, 1.0);
    let max = score.data.iter().copied().fold(0.0f32, f32::max);
    if max < opts.led_min_score {
        return None;
    }
    let thr = (0.5 * max).max(opts.led_min_score * 0.6);
    let mask: Vec<bool> = score.data.iter().map(|&v| v > thr).collect();
    let comps = components(&mask, w, h);
    let total = (w * h) as f64;
    // La más fuerte (suma del exceso de verde), no la más grande.
    let best = comps
        .iter()
        .filter(|c| c.area >= 3 && (c.area as f64) <= opts.led_max_area_frac * total)
        .max_by(|a, b| {
            let s = |c: &Component| c.pixels.iter().map(|&i| score.data[i]).sum::<f32>();
            s(a).total_cmp(&s(b))
        })?;
    let bd = opts.border_px.max(2);
    if best.x0 <= bd || best.y0 <= bd || best.x1 + bd >= w - 1 || best.y1 + bd >= h - 1 {
        return None;
    }
    let (mut sw, mut sx, mut sy) = (0.0f64, 0.0f64, 0.0f64);
    for &i in &best.pixels {
        let wgt = f64::from(score.data[i]);
        sw += wgt;
        sx += wgt * (i % w) as f64;
        sy += wgt * (i / w) as f64;
    }
    let c = [sx / sw, sy / sw];
    Some(HeadDetection {
        px: c,
        centroid_px: c,
        tip_px: c,
        area_px: best.area as f64,
        bbox: [
            best.x0 as f64,
            best.y0 as f64,
            best.x1 as f64,
            best.y1 as f64,
        ],
        contrast: max,
        confidence: f64::from(max / (3.0 * opts.led_min_score)).clamp(0.2, 1.0),
    })
}

// ---------------------------------------------------------------------------
// Diferencia de imágenes
// ---------------------------------------------------------------------------

fn luma(img: &RgbImage) -> GrayImage {
    GrayImage::from_raw(
        img.width(),
        img.height(),
        img.pixels()
            .map(|p| {
                let [r, g, b] = p.0;
                ((299 * u32::from(r) + 587 * u32::from(g) + 114 * u32::from(b)) / 1000) as u8
            })
            .collect(),
    )
    .expect("tamaño")
}

fn detect_by_diff(
    frames: &[RgbImage],
    groups: &[usize],
    opts: &HeadDetectOptions,
) -> Vec<Result<HeadDetection, DetectFailure>> {
    let gray: Vec<GrayImage> = frames.iter().map(luma).collect();
    let ids: Vec<usize> = if groups.is_empty() {
        vec![0; frames.len()]
    } else {
        groups.to_vec()
    };
    let mut out: Vec<Result<HeadDetection, DetectFailure>> =
        vec![Err(DetectFailure::NoBackground); frames.len()];
    let mut seen: Vec<usize> = Vec::new();
    for &g in &ids {
        if seen.contains(&g) {
            continue;
        }
        seen.push(g);
        let members: Vec<usize> = (0..frames.len()).filter(|&i| ids[i] == g).collect();
        if members.len() < MIN_FRAMES {
            continue;
        }
        let refs: Vec<&GrayImage> = members.iter().map(|&i| &gray[i]).collect();
        let background = median_background(&refs);
        for &i in &members {
            out[i] = detect_one(&gray[i], &background, opts);
        }
    }
    out
}

/// Mediana por píxel (con un número par de fotos, el promedio de las dos centrales).
fn median_background(frames: &[&GrayImage]) -> FloatImage {
    let (w, h) = (frames[0].width() as usize, frames[0].height() as usize);
    let n = frames.len();
    let raws: Vec<&[u8]> = frames.iter().map(|f| f.as_raw().as_slice()).collect();
    let mut data = vec![0.0f32; w * h];
    let mut buf = vec![0u8; n];
    for (i, out) in data.iter_mut().enumerate() {
        for (b, r) in buf.iter_mut().zip(&raws) {
            *b = r[i];
        }
        buf.sort_unstable();
        *out = if n % 2 == 1 {
            f32::from(buf[n / 2])
        } else {
            (f32::from(buf[n / 2 - 1]) + f32::from(buf[n / 2])) / 2.0
        };
    }
    FloatImage { w, h, data }
}

/// Ganancia y desfase `f ≈ a·bg + b` robustos (mínimos cuadrados con pasadas que descartan los
/// píxeles que cambian de verdad, como el cabezal).
fn gain_offset(frame: &[u8], bg: &[f32]) -> (f32, f32) {
    let sample: Vec<(f32, f32)> = bg
        .iter()
        .zip(frame)
        .step_by(5)
        .map(|(&b, &f)| (b, f32::from(f)))
        .collect();
    let solve = |pts: &mut dyn Iterator<Item = (f32, f32)>| -> Option<(f32, f32)> {
        let (mut n, mut sx, mut sy, mut sxx, mut sxy) = (0.0f64, 0.0, 0.0, 0.0, 0.0);
        for (x, y) in pts {
            let (x, y) = (f64::from(x), f64::from(y));
            n += 1.0;
            sx += x;
            sy += y;
            sxx += x * x;
            sxy += x * y;
        }
        let den = n * sxx - sx * sx;
        if n < 10.0 || den.abs() < 1e-6 {
            return None;
        }
        let a = (n * sxy - sx * sy) / den;
        Some((a as f32, ((sy - a * sx) / n) as f32))
    };
    let Some((mut a, mut b)) = solve(&mut sample.iter().copied()) else {
        return (1.0, 0.0);
    };
    for _ in 0..2 {
        let mut res: Vec<f32> = sample
            .iter()
            .map(|&(x, y)| (y - (a * x + b)).abs())
            .collect();
        res.sort_unstable_by(f32::total_cmp);
        let cut = (res[res.len() / 2] * 3.0).max(4.0);
        let mut keep = sample
            .iter()
            .copied()
            .filter(|&(x, y)| (y - (a * x + b)).abs() <= cut);
        match solve(&mut keep) {
            Some(v) => (a, b) = v,
            None => break,
        }
    }
    // Una ganancia absurda es un mal ajuste (imagen casi uniforme): se deja solo el desfase.
    if (0.5..=2.0).contains(&a) {
        (a, b)
    } else {
        (1.0, b)
    }
}

fn percentile(sorted: &[f32], p: f64) -> f32 {
    let i = ((sorted.len() - 1) as f64 * p).round() as usize;
    sorted[i.min(sorted.len() - 1)]
}

fn detect_one(
    frame: &GrayImage,
    bg: &FloatImage,
    opts: &HeadDetectOptions,
) -> Result<HeadDetection, DetectFailure> {
    let (w, h) = (bg.w, bg.h);
    let (gain, offset) = gain_offset(frame.as_raw(), &bg.data);
    let diff = FloatImage {
        w,
        h,
        data: frame
            .as_raw()
            .iter()
            .zip(&bg.data)
            .map(|(&f, &g)| (f32::from(f) - (gain * g + offset)).abs())
            .collect(),
    };
    let diff = gaussian_blur(&diff, opts.blur_sigma);

    // Ruido de fondo y cambio máximo.
    let mut sorted: Vec<f32> = diff.data.iter().step_by(3).copied().collect();
    sorted.sort_unstable_by(f32::total_cmp);
    let med = percentile(&sorted, 0.5);
    let mut dev: Vec<f32> = sorted.iter().map(|v| (v - med).abs()).collect();
    dev.sort_unstable_by(f32::total_cmp);
    let noise = 1.4826 * percentile(&dev, 0.5);
    let peak = percentile(&sorted, 0.999);
    if peak < opts.min_contrast * 1.2 {
        return Err(DetectFailure::NoChange);
    }
    let thr = opts
        .min_contrast
        .max(med + 6.0 * noise)
        .max(opts.relative_threshold * peak);
    let mut mask: Vec<bool> = diff.data.iter().map(|&v| v > thr).collect();

    // Cierre morfológico: une las partes de la silueta que el fondo "disimula".
    let r = ((w.max(h) as f64) * 0.004).round().clamp(2.0, 6.0) as usize;
    mask = erode(&dilate(&mask, w, h, r), w, h, r);

    let comps = components(&mask, w, h);
    let total = (w * h) as f64;
    let Some(best) = comps.first() else {
        return Err(DetectFailure::NoChange);
    };
    if (best.area as f64) < opts.min_area_frac * total {
        return Err(DetectFailure::TooSmall);
    }
    if best.area as f64 > opts.max_area_frac * total {
        return Err(DetectFailure::TooLarge);
    }
    let second = comps.get(1).map_or(0, |c| c.area);
    if second as f64 >= 0.6 * best.area as f64 && second as f64 >= opts.min_area_frac * total {
        return Err(DetectFailure::Ambiguous);
    }
    // La erosión del cierre come `r` píxeles contra el borde: la silueta cortada queda a esa distancia.
    let bd = opts.border_px + r;
    if best.x0 <= bd || best.y0 <= bd || best.x1 + bd >= w - 1 || best.y1 + bd >= h - 1 {
        return Err(DetectFailure::TouchesBorder);
    }

    // Silueta de la componente, con los huecos rellenos.
    let filled = fill_holes(&best.pixels, best, w);
    let (bw, bh) = (best.x1 - best.x0 + 1, best.y1 - best.y0 + 1);
    let (mut n, mut sx, mut sy) = (0.0f64, 0.0f64, 0.0f64);
    let mut contrast = 0.0f64;
    for (j, row) in filled.chunks(bw).enumerate() {
        for (i, &on) in row.iter().enumerate() {
            if on {
                n += 1.0;
                sx += (best.x0 + i) as f64;
                sy += (best.y0 + j) as f64;
                contrast += f64::from(diff.get(best.x0 + i, best.y0 + j));
            }
        }
    }
    let centroid = [sx / n, sy / n];
    // Extremo inferior: promedio en x de las últimas filas de la silueta.
    let tip_rows = 3.min(bh);
    let (mut tn, mut tx) = (0.0f64, 0.0f64);
    for j in (bh - tip_rows)..bh {
        for i in 0..bw {
            if filled[j * bw + i] {
                tn += 1.0;
                tx += (best.x0 + i) as f64;
            }
        }
    }
    let tip = if tn > 0.0 {
        [tx / tn, best.y1 as f64]
    } else {
        centroid
    };
    let contrast = (contrast / n) as f32;
    let px = match opts.feature {
        HeadFeature::Centroid => centroid,
        HeadFeature::BottomTip => tip,
    };
    let rival = (second as f64 / best.area as f64).min(1.0);
    Ok(HeadDetection {
        px,
        centroid_px: centroid,
        tip_px: tip,
        area_px: n,
        bbox: [
            best.x0 as f64,
            best.y0 as f64,
            best.x1 as f64,
            best.y1 as f64,
        ],
        contrast,
        confidence: (f64::from(contrast / (2.0 * thr)).clamp(0.0, 1.0) * (1.0 - 0.7 * rival))
            .clamp(0.0, 1.0),
    })
}

/// Dilatación cuadrada de radio `r` (separable).
fn dilate(m: &[bool], w: usize, h: usize, r: usize) -> Vec<bool> {
    window(m, w, h, r, true)
}

/// Erosión cuadrada de radio `r` (separable); fuera de la imagen cuenta como apagado.
fn erode(m: &[bool], w: usize, h: usize, r: usize) -> Vec<bool> {
    window(m, w, h, r, false)
}

fn window(m: &[bool], w: usize, h: usize, r: usize, any: bool) -> Vec<bool> {
    // Con `any` basta un píxel encendido en la ventana; sin él, deben estarlo todos.
    let mut tmp = vec![false; w * h];
    for y in 0..h {
        let row = &m[y * w..(y + 1) * w];
        for x in 0..w {
            let (lo, hi) = (x.saturating_sub(r), (x + r).min(w - 1));
            let full = hi - lo == 2 * r;
            tmp[y * w + x] = if any {
                row[lo..=hi].iter().any(|&v| v)
            } else {
                full && row[lo..=hi].iter().all(|&v| v)
            };
        }
    }
    let mut out = vec![false; w * h];
    for y in 0..h {
        let (lo, hi) = (y.saturating_sub(r), (y + r).min(h - 1));
        let full = hi - lo == 2 * r;
        if !any && !full {
            continue;
        }
        let dst = &mut out[y * w..(y + 1) * w];
        for (x, d) in dst.iter_mut().enumerate() {
            *d = if any {
                (lo..=hi).any(|yy| tmp[yy * w + x])
            } else {
                (lo..=hi).all(|yy| tmp[yy * w + x])
            };
        }
    }
    out
}

/// Componente conexa (8 vecinos) de píxeles encendidos.
struct Component {
    /// Índices `y * w + x`.
    pixels: Vec<usize>,
    area: usize,
    x0: usize,
    y0: usize,
    x1: usize,
    y1: usize,
}

/// Componentes ordenadas de mayor a menor área.
fn components(mask: &[bool], w: usize, h: usize) -> Vec<Component> {
    let mut seen = vec![false; w * h];
    let mut out = Vec::new();
    let mut stack = Vec::new();
    for start in 0..w * h {
        if !mask[start] || seen[start] {
            continue;
        }
        seen[start] = true;
        stack.push(start);
        let mut c = Component {
            pixels: Vec::new(),
            area: 0,
            x0: usize::MAX,
            y0: usize::MAX,
            x1: 0,
            y1: 0,
        };
        while let Some(i) = stack.pop() {
            let (x, y) = (i % w, i / w);
            c.pixels.push(i);
            c.x0 = c.x0.min(x);
            c.x1 = c.x1.max(x);
            c.y0 = c.y0.min(y);
            c.y1 = c.y1.max(y);
            for yy in y.saturating_sub(1)..=(y + 1).min(h - 1) {
                for xx in x.saturating_sub(1)..=(x + 1).min(w - 1) {
                    let j = yy * w + xx;
                    if mask[j] && !seen[j] {
                        seen[j] = true;
                        stack.push(j);
                    }
                }
            }
        }
        c.area = c.pixels.len();
        out.push(c);
    }
    out.sort_by_key(|c| std::cmp::Reverse(c.area));
    out
}

/// Máscara (de la caja de la componente) con los huecos rellenos: lo que no se alcanza desde
/// afuera pasando por píxeles apagados es silueta.
fn fill_holes(pixels: &[usize], c: &Component, w: usize) -> Vec<bool> {
    let (bw, bh) = (c.x1 - c.x0 + 1, c.y1 - c.y0 + 1);
    // Caja con un píxel de margen para que el relleno pueda rodear la silueta.
    let (pw, ph) = (bw + 2, bh + 2);
    let mut on = vec![false; pw * ph];
    for &i in pixels {
        on[(i / w - c.y0 + 1) * pw + (i % w - c.x0 + 1)] = true;
    }
    let mut outside = vec![false; pw * ph];
    let mut stack = vec![0usize];
    outside[0] = true;
    while let Some(i) = stack.pop() {
        let (x, y) = (i % pw, i / pw);
        let mut visit = |xx: usize, yy: usize| {
            let j = yy * pw + xx;
            if !on[j] && !outside[j] {
                outside[j] = true;
                stack.push(j);
            }
        };
        if x > 0 {
            visit(x - 1, y);
        }
        if x + 1 < pw {
            visit(x + 1, y);
        }
        if y > 0 {
            visit(x, y - 1);
        }
        if y + 1 < ph {
            visit(x, y + 1);
        }
    }
    let mut filled = vec![false; bw * bh];
    for j in 0..bh {
        for i in 0..bw {
            filled[j * bw + i] = !outside[(j + 1) * pw + i + 1];
        }
    }
    filled
}
