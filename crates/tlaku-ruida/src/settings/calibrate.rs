//! Calibración de un eje a partir de una distancia medida.
//!
//! El ajuste de paso ("Axis Precision" en las fuentes) es la **longitud que recorre el eje por
//! paso**. Se manda mover el eje una distancia conocida, se mide lo que se movió de verdad y el
//! paso nuevo es `paso actual x medido / pedido`: si el eje recorre menos de lo pedido, cada
//! paso real es más corto de lo que la controladora cree. La dirección de esta corrección
//! depende de que el valor sea una longitud de paso y no pasos por unidad; las fuentes lo
//! muestran como longitud (`Prec:...mm`), pero **no está verificado con una controladora real**:
//! tras escribirlo hay que repetir la medición y, si el error empeora, restaurar el respaldo.

use super::SettingsError;

/// Una corrección razonable no pasa de ±25 %: más es un error de medida casi seguro.
const MIN_RATIO: f64 = 0.8;
const MAX_RATIO: f64 = 1.25;
/// Distancia máxima de una medida aceptable, mm.
const MAX_MM: f64 = 10_000.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Calibration {
    /// Valor actual del paso (la unidad del ajuste; ver `SettingKind::Step`).
    pub current: i64,
    /// Valor propuesto.
    pub proposed: i64,
    /// `medido / pedido`.
    pub ratio: f64,
    /// Pasos por mm antes y después, **si** el valor está en µm (lo que indican las fuentes):
    /// `1000 / valor`.
    pub steps_per_mm_before: f64,
    pub steps_per_mm_after: f64,
}

/// Calcula el paso nuevo. `commanded_mm` es lo que se pidió mover y `measured_mm` lo que se midió
/// con una regla. Falla con medidas no válidas, con una corrección mayor que ±25 % y cuando el
/// cambio queda por debajo de 1 unidad del ajuste (no se puede corregir con esa resolución).
pub fn calibrate_step(
    current: i64,
    commanded_mm: f64,
    measured_mm: f64,
) -> Result<Calibration, SettingsError> {
    let ok = |v: f64| v.is_finite() && v > 0.0 && v <= MAX_MM;
    if !ok(commanded_mm) {
        return Err(SettingsError::BadMeasure("commanded"));
    }
    if !ok(measured_mm) {
        return Err(SettingsError::BadMeasure("measured"));
    }
    if current <= 0 {
        return Err(SettingsError::BadMeasure("current"));
    }
    let ratio = measured_mm / commanded_mm;
    if !(MIN_RATIO..=MAX_RATIO).contains(&ratio) {
        return Err(SettingsError::ImplausibleRatio { ratio });
    }
    let proposed = (current as f64 * ratio).round() as i64;
    if proposed == current {
        return Err(SettingsError::BelowResolution { current, ratio });
    }
    Ok(Calibration {
        current,
        proposed,
        ratio,
        steps_per_mm_before: 1000.0 / current as f64,
        steps_per_mm_after: 1000.0 / proposed as f64,
    })
}
