//! Conversiones de parámetros a las unidades de la tarjeta, y sus inversas para los
//! validadores. **Sin verificar con hardware**: vienen de galvoplotter y MeerK40t `balormk` (MIT),
//! que coinciden en todas menos en el retardo de polígono (ver `codes`).
//!
//! | Parámetro | Unidad de la tarjeta |
//! |---|---|
//! | velocidad | unidades galvo por ms = `mm/s x unidades_por_mm / 1000` |
//! | frecuencia, fibra | periodo `round(20000 / kHz)` (pasos de 50 ns) |
//! | frecuencia, CO2 y UV | periodo `round(10000 / kHz)` |
//! | potencia, fibra | `round(% x 0xFFF / 100)` |
//! | potencia, CO2 y UV | `round(200 x % / kHz)` |
//! | first pulse killer, CO2 y UV | `round(2000 / kHz)` |
//! | espera y dwell | pasos de 10 us |
//! | retardos de láser y de salto | us con signo |
//! | retardo de polígono | pasos de 10 us (MeerK40t), una sola palabra |

use crate::calib::units_per_mm;
use tlaku_doc::LaserSource;

pub const FIBER_BASE: f64 = 20000.0;
pub const CO2_BASE: f64 = 10000.0;
/// Máximo de la potencia de fibra (12 bits).
pub const CURRENT_MAX: u16 = 0xFFF;
/// Máximo de pasos de 10 us en un solo comando de dwell (galvoplotter).
pub const MAX_DWELL_TICKS: u16 = 60000;

/// `true` si la fuente se maneja con el periodo del Q-switch y la corriente (fibra).
pub fn is_fiber(s: LaserSource) -> bool {
    matches!(s, LaserSource::FiberQ | LaserSource::FiberMopa)
}

pub fn base_for(s: LaserSource) -> f64 {
    if is_fiber(s) { FIBER_BASE } else { CO2_BASE }
}

/// Periodo de la tarjeta para una frecuencia en kHz; `None` si no cabe en 16 bits o no es válida.
pub fn period(source: LaserSource, freq_khz: f64) -> Option<u16> {
    if !(freq_khz.is_finite() && freq_khz > 0.0) {
        return None;
    }
    let p = (base_for(source) / freq_khz).round();
    (1.0..=65535.0).contains(&p).then_some(p as u16)
}

/// Frecuencia en kHz que corresponde a un periodo.
pub fn freq_from_period(source: LaserSource, period: u16) -> f64 {
    base_for(source) / f64::from(period.max(1))
}

/// Potencia de fibra (0 a 0xFFF) para un porcentaje.
pub fn fiber_current(pct: f64) -> u16 {
    (pct.clamp(0.0, 100.0) * f64::from(CURRENT_MAX) / 100.0).round() as u16
}

/// Razón de potencia de CO2 y UV.
pub fn power_ratio(pct: f64, freq_khz: f64) -> u16 {
    (200.0 * pct.clamp(0.0, 100.0) / freq_khz)
        .round()
        .clamp(0.0, 65535.0) as u16
}

/// First pulse killer de CO2 y UV.
pub fn fpk(freq_khz: f64) -> u16 {
    (2000.0 / freq_khz).round().clamp(0.0, 65535.0) as u16
}

/// Velocidad de la tarjeta (unidades por ms) para mm/s, sin saturar. `None` si no es válida.
pub fn speed_units_f(mm_s: f64, field_mm: f64) -> Option<f64> {
    (mm_s.is_finite() && mm_s > 0.0).then(|| mm_s * units_per_mm(field_mm) / 1000.0)
}

/// Pasos de 10 us para milisegundos (redondeado hacia arriba, mínimo 0).
pub fn ticks_from_ms(ms: f64) -> u64 {
    if ms.is_finite() && ms > 0.0 {
        (ms * 100.0).ceil() as u64
    } else {
        0
    }
}

/// Milisegundos de una cuenta de pasos de 10 us.
pub fn ms_from_ticks(ticks: u64) -> f64 {
    ticks as f64 / 100.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frequency_periods_follow_the_documented_bases() {
        // 30 kHz en fibra: 20000/30 = 666,7 -> 667 (33,4 us con pasos de 50 ns).
        assert_eq!(period(LaserSource::FiberQ, 30.0), Some(667));
        assert_eq!(period(LaserSource::Co2, 30.0), Some(333));
        assert_eq!(period(LaserSource::Uv, 30.0), Some(333));
        assert_eq!(period(LaserSource::FiberQ, 0.0), None);
        assert_eq!(period(LaserSource::FiberQ, f64::NAN), None);
        assert_eq!(
            period(LaserSource::FiberMopa, 0.2),
            None,
            "no cabe en 16 bits"
        );
        assert!((freq_from_period(LaserSource::FiberQ, 667) - 29.985).abs() < 1e-2);
    }

    #[test]
    fn power_conversions() {
        assert_eq!(fiber_current(0.0), 0);
        assert_eq!(fiber_current(100.0), 0xFFF);
        assert_eq!(fiber_current(50.0), 2048);
        assert_eq!(fiber_current(150.0), 0xFFF, "satura");
        assert_eq!(power_ratio(50.0, 30.0), 333);
        assert_eq!(fpk(30.0), 67);
    }

    #[test]
    fn speed_and_ticks() {
        // 4000 mm/s en 110 mm: 595,78 unidades/mm x 4 = 2383 por ms.
        let v = speed_units_f(4000.0, 110.0).unwrap();
        assert!((v - 2383.1).abs() < 0.1);
        assert!(speed_units_f(0.0, 110.0).is_none());
        assert_eq!(ticks_from_ms(8.0), 800);
        assert_eq!(ticks_from_ms(-1.0), 0);
        assert_eq!(ms_from_ticks(800), 8.0);
    }
}
