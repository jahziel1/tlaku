//! Accesorio rotativo en lugar del eje Y.
//!
//! El documento y el `Toolpath` siempre están en mm de **superficie** de la pieza. La conversión
//! a los mm de Y de la controladora es un factor fijo, `y_scale`:
//!
//! - rodillo: `mm_per_rotation / (π · roller_diameter)`
//! - mandril: `mm_per_rotation / (π · object_diameter)`
//!
//! y se aplica al final, en el G-code y en el encuadre, a la Y **absoluta** (relativa al cero de
//! la controladora, no al inicio del trabajo). Así lo que muestra GRBL como posición Y y lo que
//! el usuario fijó como origen siguen siendo coherentes: no hace falta emitir `G92` ni mover el
//! cero. `current_position` y `user_origin` llegan en coordenadas de la máquina (mm de la
//! controladora) y se pasan a superficie con la inversa antes de colocar el trabajo.
//!
//! La velocidad del bloque se interpreta como velocidad **sobre la superficie** (así la
//! potencia por mm de grabado es la que el usuario pidió); `tlaku-grbl` reescala la F de cada
//! tramo con Y. La aceleración y la velocidad máxima de Y de la controladora se pasan a
//! superficie dividiéndolas por `y_scale`, para que el tiempo estimado y el overscan sean los
//! reales.

use std::f64::consts::PI;
use tlaku_doc::{DeviceProfile, RotaryKind};

/// Factor de mm de la controladora por mm de superficie en Y; 1 si no hay rotativo. Falla si
/// el rotativo está activo y alguno de sus valores no es un número mayor que 0.
pub(crate) fn y_scale(device: &DeviceProfile) -> Result<f64, String> {
    let r = &device.rotary;
    if !r.enabled {
        return Ok(1.0);
    }
    let positive = |name: &str, v: f64| {
        if v.is_finite() && v > 0.0 {
            Ok(())
        } else {
            Err(format!(
                "la máquina '{}': rotativo: {name} debe ser mayor que 0",
                device.name
            ))
        }
    };
    positive("mm por vuelta", r.mm_per_rotation)?;
    positive("el diámetro del rodillo", r.roller_diameter)?;
    positive("el diámetro de la pieza", r.object_diameter)?;
    let diameter = match r.kind {
        RotaryKind::Roller => r.roller_diameter,
        RotaryKind::Chuck => r.object_diameter,
    };
    Ok(r.mm_per_rotation / (PI * diameter))
}

/// Circunferencia de la pieza, en mm de superficie; `None` sin rotativo.
pub(crate) fn circumference_mm(device: &DeviceProfile) -> Option<f64> {
    device
        .rotary
        .enabled
        .then_some(PI * device.rotary.object_diameter)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn device(kind: RotaryKind) -> DeviceProfile {
        let mut d = DeviceProfile::default();
        d.rotary.enabled = true;
        d.rotary.kind = kind;
        d.rotary.mm_per_rotation = 360.0;
        d.rotary.roller_diameter = 20.0;
        d.rotary.object_diameter = 80.0;
        d
    }

    #[test]
    fn formulas_follow_the_kind() {
        let roller = y_scale(&device(RotaryKind::Roller)).unwrap();
        assert!((roller - 360.0 / (PI * 20.0)).abs() < 1e-12);
        let chuck = y_scale(&device(RotaryKind::Chuck)).unwrap();
        assert!((chuck - 360.0 / (PI * 80.0)).abs() < 1e-12);
        assert!((circumference_mm(&device(RotaryKind::Chuck)).unwrap() - PI * 80.0).abs() < 1e-12);
    }

    #[test]
    fn disabled_rotary_is_the_identity() {
        let d = DeviceProfile::default();
        assert_eq!(y_scale(&d).unwrap(), 1.0);
        assert_eq!(circumference_mm(&d), None);
    }

    #[test]
    fn invalid_values_are_rejected_only_when_enabled() {
        for edit in [
            |d: &mut DeviceProfile| d.rotary.mm_per_rotation = 0.0,
            |d: &mut DeviceProfile| d.rotary.roller_diameter = -1.0,
            |d: &mut DeviceProfile| d.rotary.object_diameter = f64::NAN,
        ] {
            let mut d = device(RotaryKind::Roller);
            edit(&mut d);
            let e = y_scale(&d).unwrap_err();
            assert!(e.contains("rotativo") && e.contains("mayor que 0"), "{e}");
            d.rotary.enabled = false;
            assert_eq!(y_scale(&d).unwrap(), 1.0);
        }
    }
}
