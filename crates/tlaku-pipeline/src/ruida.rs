//! Del trabajo calculado al flujo `.rd` de una controladora Ruida.
//!
//! El flujo se genera en claro (sin swizzle); `tlaku-ruida` lo parte en paquetes y lo ofusca al
//! enviarlo. Lo comparten el CLI (`tlaku ruida`) y la app.
//!
//! El origen del perfil (`Origin`) es la esquina donde la máquina tiene su X0 Y0 y las
//! coordenadas crecen hacia dentro de la cama; en Ruida esa esquina es el home (ruida.md 12.10),
//! así que se traduce uno a uno a [`HomeCorner`].

use crate::job::ComputedJob;
use tlaku_doc::{DeviceProfile, Origin};
use tlaku_ruida::rd::{self, HomeCorner, LayerOptions, RuidaJobOptions};
use tlaku_ruida::safety::SafetyLimits;

/// Velocidad máxima que se admite en el archivo cuando el perfil no pide más (600 mm/s).
const MIN_SPEED_LIMIT_MM_MIN: f64 = 36_000.0;

/// Esquina del home de Ruida que corresponde al origen del perfil.
pub fn home_for_origin(origin: Origin) -> HomeCorner {
    match origin {
        Origin::BottomLeft => HomeCorner::BottomLeft,
        Origin::TopLeft => HomeCorner::TopLeft,
        Origin::BottomRight => HomeCorner::BottomRight,
        Origin::TopRight => HomeCorner::TopRight,
    }
}

/// Genera los bytes del `.rd` (en claro) de un trabajo ya calculado. Usa el color de cada capa y
/// la cama indicada; los demás ajustes son los de [`RuidaJobOptions`] por defecto.
pub fn export_rd(
    built: &ComputedJob,
    bed: (f64, f64),
    home: HomeCorner,
) -> Result<Vec<u8>, String> {
    rd_with_limits(
        built,
        home,
        SafetyLimits {
            bed_mm: [bed.0, bed.1],
            ..SafetyLimits::default()
        },
    )
}

/// Límites de seguridad con la cama y la velocidad máxima del perfil de la máquina.
pub fn limits_for_device(device: &DeviceProfile) -> SafetyLimits {
    let max_rate = device.max_rate[0].max(device.max_rate[1]);
    SafetyLimits {
        bed_mm: device.bed,
        max_speed_mm_min: max_rate.max(MIN_SPEED_LIMIT_MM_MIN),
        ..SafetyLimits::default()
    }
}

/// Como [`export_rd`] con la cama, el home y el tope de velocidad del perfil de la máquina.
pub fn export_rd_for_device(
    built: &ComputedJob,
    device: &DeviceProfile,
) -> Result<Vec<u8>, String> {
    rd_with_limits(
        built,
        home_for_origin(device.origin),
        limits_for_device(device),
    )
}

fn rd_with_limits(
    built: &ComputedJob,
    home: HomeCorner,
    limits: SafetyLimits,
) -> Result<Vec<u8>, String> {
    let opts = RuidaJobOptions {
        limits,
        layers: built
            .layers
            .iter()
            .map(|l| LayerOptions {
                color: [l.color.0, l.color.1, l.color.2],
                min_power_pct: None,
            })
            .collect(),
        home,
        ..RuidaJobOptions::default()
    };
    rd::build_rd(&built.toolpath, &opts).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn origins_map_to_the_same_corner() {
        assert_eq!(home_for_origin(Origin::TopLeft), HomeCorner::TopLeft);
        assert_eq!(
            home_for_origin(Origin::BottomRight),
            HomeCorner::BottomRight
        );
        assert_eq!(home_for_origin(Origin::BottomLeft), HomeCorner::BottomLeft);
        assert_eq!(home_for_origin(Origin::TopRight), HomeCorner::TopRight);
    }
}
