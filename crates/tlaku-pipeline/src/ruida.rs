//! Del trabajo calculado al flujo `.rd` de una controladora Ruida.
//!
//! El flujo se genera en claro (sin swizzle); `tlaku-ruida` lo parte en paquetes y lo ofusca al
//! enviarlo. Lo comparten el CLI (`tlaku ruida`) y la app.
//!
//! El origen del perfil (`Origin`) es la esquina donde la máquina tiene su X0 Y0 y las
//! coordenadas crecen hacia dentro de la cama; en Ruida esa esquina es el home (ruida.md 12.10),
//! así que se traduce uno a uno a [`HomeCorner`].

use crate::job::{ComputedJob, LayerReport};
use tlaku_core::toolpath::{Move, Toolpath};
use tlaku_doc::{DeviceProfile, Origin};
use tlaku_ruida::rd::{self, HomeCorner, LayerOptions, MAX_LAYERS, RuidaJobOptions};
use tlaku_ruida::safety::{MAX_SPEED_MM_MIN, SafetyLimits};

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

/// Límites de seguridad con la cama del perfil y el máximo del protocolo Ruida (36 000
/// mm/min). El `max_rate` del perfil no recorta: la controladora tiene su propio máximo. Las
/// capas más rápidas que el tope se recortan al generar el `.rd` (ver [`export_rd_for_device`]).
pub fn limits_for_device(device: &DeviceProfile) -> SafetyLimits {
    SafetyLimits {
        bed_mm: device.bed,
        max_speed_mm_min: MAX_SPEED_MM_MIN,
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

fn job_options(layers: &[LayerReport], home: HomeCorner, limits: SafetyLimits) -> RuidaJobOptions {
    RuidaJobOptions {
        limits,
        layers: layers
            .iter()
            .map(|l| LayerOptions {
                color: [l.color.0, l.color.1, l.color.2],
                min_power_pct: None,
            })
            .collect(),
        home,
        ..RuidaJobOptions::default()
    }
}

/// Genera el `.rd`: recorta la velocidad de las capas al máximo de `limits` y, si hay más de 128
/// capas, fusiona las vecinas de ajustes idénticos (los avisos salen en [`layer_warnings`]).
fn rd_with_limits(
    built: &ComputedJob,
    home: HomeCorner,
    limits: SafetyLimits,
) -> Result<Vec<u8>, String> {
    let opts = job_options(&built.layers, home, limits);
    let clamped = rd::clamp_speeds(&built.toolpath, limits.max_speed_mm_min);
    let toolpath = clamped.as_ref().unwrap_or(&built.toolpath);
    rd::build_rd(toolpath, &opts).map_err(|e| e.to_string())
}

/// Bloques del trabajo que llevan al menos un corte (los que son una capa del `.rd`).
fn cutting_blocks(tp: &Toolpath) -> usize {
    tp.blocks
        .iter()
        .filter(|b| b.moves.iter().any(|m| matches!(m, Move::Cut { .. })))
        .count()
}

/// Avisos del cálculo para una máquina Ruida: capas que el `.rd` fusionará o que no caben en las
/// 128 que admite Ruida. Los de velocidad salen de [`crate::layers::speed_warnings`].
pub(crate) fn layer_warnings(
    toolpath: &Toolpath,
    layers: &[LayerReport],
    device: &DeviceProfile,
) -> Vec<String> {
    // Caso común: no hay nada que decir y no se planifica el trabajo entero.
    if cutting_blocks(toolpath) <= MAX_LAYERS {
        return Vec::new();
    }
    let opts = job_options(
        layers,
        home_for_origin(device.origin),
        limits_for_device(device),
    );
    match rd::plan_summary(toolpath, &opts) {
        Ok(s) if s.fits() => vec![format!(
            "Ruida admite {MAX_LAYERS} capas y el trabajo tiene {}: al exportar se fusionarán {} \
             capas vecinas de ajustes idénticos (velocidad, potencia, aire) y quedarán {}; el \
             orden de corte no cambia",
            s.blocks, s.merged, s.layers
        )],
        Ok(s) => vec![format!(
            "Ruida admite {MAX_LAYERS} capas y el trabajo tiene {}: aun fusionando {} capas \
             vecinas de ajustes idénticos quedan {}, así que no se podrá exportar ni enviar. \
             Unifica colores o ajustes (velocidad, potencia, aire) de capas seguidas",
            s.blocks, s.merged, s.layers
        )],
        Err(e) => vec![format!("Ruida: no se podrá exportar el trabajo: {e}")],
    }
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
