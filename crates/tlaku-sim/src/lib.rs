//! Simulación de movimiento: estima el tiempo de un `Toolpath` con aceleración
//! por eje y desviación de unión, y calcula el overscan físicamente.
//!
//! Modelo (mm, s): cada segmento tiene una velocidad y una aceleración máximas
//! proyectadas sobre su dirección según los límites por eje. Las uniones se
//! toman a la velocidad que permite la desviación de unión (esquina circular
//! inscrita, como describe la documentación pública de GRBL). Se planifica con
//! una pasada hacia atrás y otra hacia adelante, y cada segmento recibe un
//! perfil trapezoidal o triangular. La máquina se detiene al cambiar entre
//! `Travel` y `Cut` (según el perfil) y entre bloques, y arranca en reposo desde el origen
//! (o desde el punto que se indique en [`estimate_from`]).
//!
//! Una pausa (`Move::Dwell`) detiene la máquina y suma su tiempo tal cual.
//!
//! Frente al emulador (`tlaku-emu`, ver `crates/tlaku-emu/tests/estimate.rs`): en los casos con
//! cinemática calculable a mano (recta, diagonal, tramos que van y vuelven, giros suaves,
//! ventana del planificador) ambos coinciden dentro de 2 %, porque el emulador usa la misma
//! desviación de unión. En GRBL con modo láser `G0` y `G1` seguidos no detienen la máquina
//! (`MachineProfile::stop_on_cut_change` en `false`); en los demás firmwares el estimado sí se
//! detiene en cada cambio entre `Travel` y `Cut`, por prudencia. En `lbrnts-repro04` y
//! `wmc-outline-of-texas-detailed` el estimado queda a -0,6 % y +1,0 % del emulador.
//!
//! No modela: tiempo de comunicación, latencia del búfer serie ni
//! el cambio de velocidad por `PowerMode`. Tampoco el eje Z: el `z_mm` de un bloque (cambio
//! de foco o pasada profunda) no suma tiempo, porque un movimiento de Z entre bloques es
//! corto frente al trabajo y no hay una velocidad de Z en `MachineProfile`.

mod plan;
mod profile;
mod sample;

pub use profile::MachineProfile;
pub use sample::Sampler;

use tlaku_core::Point;
use tlaku_core::toolpath::Toolpath;

/// Tiempo y distancias de un bloque del trabajo.
#[derive(Debug, Clone, PartialEq)]
pub struct BlockEstimate {
    pub label: String,
    pub seconds: f64,
    /// Recorrido con `Move::Cut` (incluye overscan a potencia 0).
    pub cut_mm: f64,
    /// Recorrido con `Move::Travel`.
    pub travel_mm: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Estimate {
    pub total_s: f64,
    /// Un elemento por bloque del `Toolpath`, en el mismo orden.
    pub blocks: Vec<BlockEstimate>,
}

/// Estima el tiempo del trabajo con el cabezal en reposo en el origen (0, 0).
pub fn estimate(tp: &Toolpath, profile: &MachineProfile) -> Estimate {
    estimate_from(Point::new(0.0, 0.0), tp, profile)
}

/// Como [`estimate`], con el cabezal inicialmente en `start`.
pub fn estimate_from(start: Point, tp: &Toolpath, profile: &MachineProfile) -> Estimate {
    estimate_with_times(start, tp, profile, false).0
}

/// Segundos acumulados al terminar cada movimiento del trabajo (todos los bloques en orden,
/// un valor por `Move`), con el cabezal inicialmente en `start`. Un movimiento de longitud
/// cero repite el instante anterior; el último valor coincide con `Estimate::total_s`.
///
/// Si también hace falta el [`Estimate`], [`estimate_with_times`] planifica una sola vez.
pub fn move_times(start: Point, tp: &Toolpath, profile: &MachineProfile) -> Vec<f64> {
    estimate_with_times(start, tp, profile, true).1
}

/// El [`Estimate`] y, con `times`, los segundos acumulados de cada movimiento (ver
/// [`move_times`]; vacío si `times` es `false`), con una sola planificación. Con millones de
/// movimientos esto evita planificar dos veces y guarda solo 56 bytes por movimiento mientras
/// planifica.
pub fn estimate_with_times(
    start: Point,
    tp: &Toolpath,
    profile: &MachineProfile,
    times: bool,
) -> (Estimate, Vec<f64>) {
    let count: usize = tp.blocks.iter().map(|b| b.moves.len()).sum();
    let plan = plan::plan_lean(tp, start, profile);
    let mut blocks: Vec<BlockEstimate> = tp
        .blocks
        .iter()
        .map(|b| BlockEstimate {
            label: b.label.clone(),
            seconds: 0.0,
            cut_mm: 0.0,
            travel_mm: 0.0,
        })
        .collect();
    let mut cumulative = Vec::with_capacity(if times { count } else { 0 });
    let mut t = 0.0;
    for (k, s) in plan.segs.iter().enumerate() {
        let seconds = plan.timing(k).seconds();
        let b = &mut blocks[s.block as usize];
        b.seconds += seconds;
        if s.is_cut {
            b.cut_mm += s.len;
        } else {
            b.travel_mm += s.len;
        }
        if times {
            // Los movimientos sin segmento (longitud cero) repiten el instante anterior.
            cumulative.resize(s.mv as usize, t);
            t += seconds;
            cumulative.push(t);
        }
    }
    if times {
        cumulative.resize(count, t);
    }
    (
        Estimate {
            total_s: blocks.iter().map(|b| b.seconds).sum(),
            blocks,
        },
        cumulative,
    )
}

/// Cuántas líneas por segundo de G-code exige un trabajo (una línea por segmento planificado).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LineRate {
    /// Máximo de líneas por segundo en cualquier ventana de `window_s` segundos del trabajo.
    pub peak_per_s: f64,
    /// Líneas entre tiempo total.
    pub mean_per_s: f64,
    /// Segmentos (líneas) contados.
    pub lines: usize,
}

/// Líneas por segundo que pide el trabajo a la controladora, en promedio y en el tramo más denso
/// (ventana deslizante de `window_s` segundos; con una ventana mayor que el trabajo, el pico se
/// divide igualmente entre `window_s`). Cuenta cada movimiento con recorrido y cada pausa; los
/// de longitud cero no generan línea. El tiempo es el del planificador ([`estimate_from`]), no
/// el de enviar las líneas.
pub fn line_rate(start: Point, tp: &Toolpath, profile: &MachineProfile, window_s: f64) -> LineRate {
    let window = if window_s.is_finite() && window_s > 0.0 {
        window_s
    } else {
        1.0
    };
    let plan = plan::plan_lean(tp, start, profile);
    let mut recent: std::collections::VecDeque<f64> = std::collections::VecDeque::new();
    let (mut t, mut peak) = (0.0, 0usize);
    for k in 0..plan.segs.len() {
        t += plan.timing(k).seconds();
        recent.push_back(t);
        while recent.front().is_some_and(|&e| e <= t - window) {
            recent.pop_front();
        }
        peak = peak.max(recent.len());
    }
    let lines = plan.segs.len();
    LineRate {
        peak_per_s: peak as f64 / window,
        mean_per_s: if t > 0.0 { lines as f64 / t } else { 0.0 },
        lines,
    }
}

/// Distancia (mm) que necesita el cabezal para alcanzar `speed_mm_min` en la
/// dirección `direction_rad`: v²/(2a). La velocidad se recorta al límite de
/// los ejes en esa dirección.
pub fn overscan_mm(speed_mm_min: f64, direction_rad: f64, profile: &MachineProfile) -> f64 {
    if !(speed_mm_min.is_finite() && speed_mm_min > 0.0) {
        return 0.0;
    }
    let p = profile.sanitized();
    let v = (speed_mm_min / 60.0).min(p.max_speed_along(direction_rad));
    v * v / (2.0 * p.accel_along(direction_rad))
}

/// Muestras `(t, posición, láser encendido)` cada `dt` segundos, más una última
/// en el instante final. Con `dt` no positivo o un trabajo sin movimiento no
/// produce muestras. El láser cuenta como encendido en `Cut` con potencia > 0.
pub fn sample(
    tp: &Toolpath,
    profile: &MachineProfile,
    dt: f64,
) -> impl Iterator<Item = (f64, Point, bool)> {
    sample_from(Point::new(0.0, 0.0), tp, profile, dt)
}

/// Como [`sample`], con el cabezal inicialmente en `start`.
pub fn sample_from(
    start: Point,
    tp: &Toolpath,
    profile: &MachineProfile,
    dt: f64,
) -> impl Iterator<Item = (f64, Point, bool)> {
    Sampler::new(plan::plan(tp, start, profile), start, dt)
}

#[cfg(test)]
mod tests;
