//! Simulación de movimiento: estima el tiempo de un `Toolpath` con aceleración
//! por eje y desviación de unión, y calcula el overscan físicamente.
//!
//! Modelo (mm, s): cada segmento tiene una velocidad y una aceleración máximas
//! proyectadas sobre su dirección según los límites por eje. Las uniones se
//! toman a la velocidad que permite la desviación de unión (esquina circular
//! inscrita, como describe la documentación pública de GRBL). Se planifica con
//! una pasada hacia atrás y otra hacia adelante, y cada segmento recibe un
//! perfil trapezoidal o triangular. La máquina se detiene al cambiar entre
//! `Travel` y `Cut` y entre bloques, y arranca en reposo desde el origen
//! (o desde el punto que se indique en [`estimate_from`]).
//!
//! No modela: tiempo de comunicación, latencia del búfer serie, pausas ni
//! el cambio de velocidad por `PowerMode`.

mod plan;
mod profile;
mod sample;

pub use profile::MachineProfile;
pub use sample::Sampler;

use laser_core::Point;
use laser_core::toolpath::Toolpath;

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
    let segs = plan::plan(tp, start, profile);
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
    for s in &segs {
        let b = &mut blocks[s.block];
        b.seconds += s.seconds();
        if s.is_cut {
            b.cut_mm += s.len;
        } else {
            b.travel_mm += s.len;
        }
    }
    Estimate {
        total_s: blocks.iter().map(|b| b.seconds).sum(),
        blocks,
    }
}

/// Segundos acumulados al terminar cada movimiento del trabajo (todos los bloques en orden,
/// un valor por `Move`), con el cabezal inicialmente en `start`. Un movimiento de longitud
/// cero repite el instante anterior; el último valor coincide con `Estimate::total_s`.
pub fn move_times(start: Point, tp: &Toolpath, profile: &MachineProfile) -> Vec<f64> {
    let count: usize = tp.blocks.iter().map(|b| b.moves.len()).sum();
    let segs = plan::plan(tp, start, profile);
    let mut out = Vec::with_capacity(count);
    let mut t = 0.0;
    let mut k = 0;
    for i in 0..count {
        while k < segs.len() && segs[k].mv == i {
            t += segs[k].seconds();
            k += 1;
        }
        out.push(t);
    }
    out
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
