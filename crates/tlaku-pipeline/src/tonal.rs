//! Vista previa «por tonos» (mejora 8, versión reducida): simula cuánto se oscurece el material
//! con el trabajo ya calculado, sin texturas fotográficas.
//!
//! Se recorre la trayectoria real ([`ComputedJob`]): cada movimiento con láser deposita
//! `potencia / velocidad` (fracción de potencia x segundos por mm) a lo largo de su recorrido
//! sobre un [`DoseMap`]; las pasadas, los rellenos que se solapan y las imágenes con sus puntos
//! tramados se acumulan solos porque son movimientos distintos sobre los mismos píxeles. La
//! dosis de cada píxel (energía / área) pasa por la curva del material ([`ToneCurve`]) y sale un
//! PNG RGBA.
//!
//! - **Ancho del punto** (`spot_mm`): si el punto es más ancho que el píxel, la energía se
//!   reparte en ese ancho; si es más fino, cada píxel muestra el promedio de su área (un trazo
//!   fino se ve más claro de lejos y se afina al acercar la vista con `region_mm`).
//! - **M4 contra M3** (`DeviceProfile::power_mode`): con potencia dinámica (M4) la controladora baja la potencia al frenar y la
//!   dosis es pareja. Con potencia constante (M3) el cabezal pasa más tiempo en cada punto donde
//!   acelera o frena: al empezar y terminar cada recorrido que arranca desde el reposo (después
//!   de un viaje) la dosis sube por `v / v_local` (con tope [`MAX_BOOST`]), con la aceleración
//!   del perfil. No se simulan las esquinas ni el ajuste fino del planificador de la máquina.
//! - **Hoja**: el material se supone del tamaño de lo que se graba más un margen; fuera de la
//!   hoja la imagen es transparente.

use crate::job::ComputedJob;
use tlaku_core::Point;
use tlaku_core::toolpath::{Move, ToolpathBlock};
use tlaku_doc::{DeviceProfile, ToneCurve};
use tlaku_i18n::tr;
use tlaku_raster::{DoseMap, rgba_to_png};

/// Margen de la hoja alrededor de lo grabado (mm).
pub const SHEET_MARGIN_MM: f64 = 3.0;
/// Píxel más fino que se acepta (mm).
pub const MIN_PX_MM: f64 = 0.005;
/// Lados máximos de la imagen (píxeles).
pub const MAX_SIDE_PX: u32 = 4000;
/// Aumento máximo de la dosis por frenar o acelerar con potencia constante.
pub const MAX_BOOST: f64 = 6.0;
/// Trozos en que se parte lo que cae dentro de una rampa de aceleración.
const RAMP_PIECES: usize = 12;

/// Qué se pide a la simulación.
#[derive(Debug, Clone, PartialEq)]
pub struct TonalOptions {
    pub curve: ToneCurve,
    /// Ancho del punto del láser (mm).
    pub spot_mm: f64,
    /// Rectángulo `[x0, y0, x1, y1]` (mm del documento) a dibujar; `None` = toda la hoja.
    pub region_mm: Option<[f64; 4]>,
    /// Lado mayor de la imagen, en píxeles.
    pub max_px: u32,
}

/// La imagen simulada y dónde va.
#[derive(Debug, Clone, PartialEq)]
pub struct TonalImage {
    /// PNG RGBA (transparente fuera de la hoja).
    pub png: Vec<u8>,
    pub size_px: [u32; 2],
    /// Rectángulo que cubre el PNG `[x0, y0, x1, y1]` (mm del documento, Y hacia arriba).
    pub covered_mm: [f64; 4],
    /// Hoja de material supuesta `[x0, y0, x1, y1]` (mm).
    pub sheet_mm: [f64; 4],
    /// Lado de un píxel (mm).
    pub px_mm: f64,
    /// Dosis máxima de la imagen.
    pub max_dose: f64,
}

/// Caja de lo que realmente graba (movimientos con potencia), sin el overscan.
fn burn_bounds(job: &ComputedJob) -> Option<[f64; 4]> {
    let mut b: Option<[f64; 4]> = None;
    let mut grow = |p: Point| {
        let r = b.get_or_insert([p.x, p.y, p.x, p.y]);
        r[0] = r[0].min(p.x);
        r[1] = r[1].min(p.y);
        r[2] = r[2].max(p.x);
        r[3] = r[3].max(p.y);
    };
    for block in &job.toolpath.blocks {
        let mut cur: Option<Point> = None;
        for mv in &block.moves {
            match *mv {
                Move::Travel { to } => cur = Some(to),
                Move::Cut { to, power_pct } => {
                    if power_pct > 0.0 && to.x.is_finite() && to.y.is_finite() {
                        if let Some(from) = cur {
                            grow(from);
                        }
                        grow(to);
                    }
                    cur = Some(to);
                }
                Move::Dwell { to, power_pct, .. } => {
                    if power_pct > 0.0 && to.x.is_finite() && to.y.is_finite() {
                        grow(to);
                    }
                    cur = Some(to);
                }
            }
        }
    }
    b
}

fn dist(a: Point, b: Point) -> f64 {
    (b.x - a.x).hypot(b.y - a.y)
}

fn lerp(a: Point, b: Point, t: f64) -> (f64, f64) {
    (a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t)
}

/// Deposita un bloque (una capa, una imagen o una pasada) sobre el mapa.
fn deposit_block(map: &mut DoseMap, block: &ToolpathBlock, constant_power: bool, accel_mm_s2: f64) {
    let v = block.speed_mm_min / 60.0;
    if !(v.is_finite() && v > 0.0) {
        return;
    }
    let boost = constant_power && accel_mm_s2 > 0.0;
    let d_acc = v * v / (2.0 * accel_mm_s2.max(1e-9));
    let moves = &block.moves;
    let mut cur: Option<Point> = None;
    let mut i = 0;
    while i < moves.len() {
        match moves[i] {
            Move::Travel { to } => {
                cur = Some(to);
                i += 1;
            }
            Move::Dwell {
                to, ms, power_pct, ..
            } => {
                if power_pct > 0.0 && ms > 0.0 {
                    map.add_point(to.x, to.y, power_pct / 100.0 * ms / 1000.0);
                }
                cur = Some(to);
                i += 1;
            }
            Move::Cut { .. } => {
                // Una cadena de cortes seguidos: arranca desde el reposo (viaje, pausa o inicio)
                // y termina en el siguiente viaje o pausa.
                let mut end = i;
                while end < moves.len() && matches!(moves[end], Move::Cut { .. }) {
                    end += 1;
                }
                let start_pt = cur;
                if boost && let Some(first) = start_pt {
                    deposit_chain_m3(map, first, &moves[i..end], v, d_acc);
                } else {
                    let mut at = start_pt;
                    for mv in &moves[i..end] {
                        if let Move::Cut { to, power_pct } = *mv {
                            if let Some(from) = at
                                && power_pct > 0.0
                            {
                                map.add_segment(
                                    (from.x, from.y),
                                    (to.x, to.y),
                                    power_pct / 100.0 / v,
                                );
                            }
                            at = Some(to);
                        }
                    }
                }
                if let Some(Move::Cut { to, .. }) = moves[i..end].last() {
                    cur = Some(*to);
                }
                i = end;
            }
        }
    }
}

/// Cadena de cortes con potencia constante: cerca de sus dos extremos el cabezal va más lento
/// que `v` y la dosis sube.
fn deposit_chain_m3(map: &mut DoseMap, first: Point, chain: &[Move], v: f64, d_acc: f64) {
    // Largo total de la cadena.
    let mut total = 0.0;
    let mut at = first;
    for mv in chain {
        if let Move::Cut { to, .. } = *mv {
            total += dist(at, to);
            at = to;
        }
    }
    if total <= 0.0 {
        return;
    }
    let a = (v * v) / (2.0 * d_acc.max(1e-12));
    // Dosis relativa a la de velocidad plena en la posición `s` (mm desde el inicio).
    let mult = |s: f64| {
        let edge = s.min(total - s).max(0.0);
        let local = (2.0 * a * edge).sqrt().min(v);
        if local <= v / MAX_BOOST {
            MAX_BOOST
        } else {
            v / local
        }
    };
    let mut at = first;
    let mut s = 0.0;
    for mv in chain {
        let Move::Cut { to, power_pct } = *mv else {
            continue;
        };
        let len = dist(at, to);
        if power_pct > 0.0 && len > 0.0 {
            let e = power_pct / 100.0 / v;
            // Lejos de los dos extremos (lo habitual en una imagen): velocidad plena, sin partir.
            if s >= d_acc && s + len <= total - d_acc {
                map.add_segment((at.x, at.y), (to.x, to.y), e);
                s += len;
                at = to;
                continue;
            }
            // Puntos de corte a lo largo de este segmento: donde empieza y termina cada rampa.
            let mut cuts = vec![0.0, len];
            for edge in [d_acc, total - d_acc] {
                if edge > s && edge < s + len {
                    cuts.push(edge - s);
                }
            }
            cuts.sort_by(f64::total_cmp);
            for w in cuts.windows(2) {
                let (l0, l1) = (w[0], w[1]);
                if l1 - l0 <= 0.0 {
                    continue;
                }
                let mid = s + (l0 + l1) / 2.0;
                let in_ramp = mid.min(total - mid) < d_acc;
                let pieces = if in_ramp { RAMP_PIECES } else { 1 };
                for k in 0..pieces {
                    let p0 = l0 + (l1 - l0) * k as f64 / pieces as f64;
                    let p1 = l0 + (l1 - l0) * (k + 1) as f64 / pieces as f64;
                    let factor = if in_ramp {
                        mult(s + (p0 + p1) / 2.0)
                    } else {
                        1.0
                    };
                    map.add_segment(lerp(at, to, p0 / len), lerp(at, to, p1 / len), e * factor);
                }
            }
        }
        s += len;
        at = to;
    }
}

/// Simula el trabajo y devuelve la imagen. Falla si no hay nada que grabar, la curva no es
/// válida o la región cae fuera de la hoja.
pub fn render_tonal(
    job: &ComputedJob,
    device: &DeviceProfile,
    opts: &TonalOptions,
) -> Result<TonalImage, String> {
    opts.curve.validate()?;
    if !(opts.spot_mm.is_finite() && opts.spot_mm >= 0.0) {
        return Err(tr!("pipeline.tonal.bad_spot", spot = opts.spot_mm));
    }
    let burn = burn_bounds(job).ok_or_else(|| tr!("pipeline.tonal.nothing"))?;
    let sheet = [
        burn[0] - SHEET_MARGIN_MM,
        burn[1] - SHEET_MARGIN_MM,
        burn[2] + SHEET_MARGIN_MM,
        burn[3] + SHEET_MARGIN_MM,
    ];
    let region = match opts.region_mm {
        None => sheet,
        Some(r) => {
            let c = [
                r[0].max(sheet[0]),
                r[1].max(sheet[1]),
                r[2].min(sheet[2]),
                r[3].min(sheet[3]),
            ];
            if !(c.iter().all(|v| v.is_finite()) && c[2] > c[0] && c[3] > c[1]) {
                return Err(tr!("pipeline.tonal.outside"));
            }
            c
        }
    };
    let side = f64::from(opts.max_px.clamp(64, MAX_SIDE_PX));
    let px = ((region[2] - region[0]).max(region[3] - region[1]) / side).max(MIN_PX_MM);
    let mut map = DoseMap::new(region, px)?;
    let accel = device.accel[0].min(device.accel[1]);
    // El modo de potencia lo decide el perfil al escribir el G-code (M3 contra M4).
    let constant_power = device.power_mode == tlaku_doc::PowerMode::Constant;
    for block in &job.toolpath.blocks {
        deposit_block(&mut map, block, constant_power, accel);
    }
    map.spread(opts.spot_mm);

    let curve = opts.curve;
    let [bx, by, tx, ty] = map.covered_mm();
    let base = curve.mix(0.0);
    let mut rgba = map.to_rgba(|dose| {
        if dose <= 0.0 {
            [base[0], base[1], base[2], 255]
        } else {
            let c = curve.color(dose);
            [c[0], c[1], c[2], 255]
        }
    });
    // Fuera de la hoja (el último píxel puede pasarse del borde) no hay material.
    let (w, h) = (map.width(), map.height());
    let center = |i: usize, o: f64| o + (i as f64 + 0.5) * px;
    for row in 0..h {
        let y = center(h - 1 - row, by);
        for col in 0..w {
            let x = center(col, bx);
            if x < sheet[0] || x > sheet[2] || y < sheet[1] || y > sheet[3] {
                rgba[(row * w + col) * 4 + 3] = 0;
            }
        }
    }
    let png = rgba_to_png(w as u32, h as u32, &rgba)?;
    Ok(TonalImage {
        png,
        size_px: [w as u32, h as u32],
        covered_mm: [bx, by, tx, ty],
        sheet_mm: sheet,
        px_mm: px,
        max_dose: map.max_dose(),
    })
}
