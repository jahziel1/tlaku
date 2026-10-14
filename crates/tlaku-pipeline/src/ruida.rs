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
use tlaku_i18n::tr;
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
        false,
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
///
/// Las capas del láser 2 (`LayerSettings::laser2`) salen con `CA 01 11` solo si el perfil dice
/// que la máquina tiene segundo láser (`device.laser2`); si no, salen con el láser 1 y
/// [`layer_warnings`] lo avisa. **Láser 2 sin probar con una máquina de dos láseres (M32).**
pub fn export_rd_for_device(
    built: &ComputedJob,
    device: &DeviceProfile,
) -> Result<Vec<u8>, String> {
    rd_with_limits(
        built,
        home_for_origin(device.origin),
        limits_for_device(device),
        device.laser2,
    )
}

fn job_options(
    layers: &[LayerReport],
    home: HomeCorner,
    limits: SafetyLimits,
    laser2: bool,
) -> RuidaJobOptions {
    RuidaJobOptions {
        limits,
        layers: layers
            .iter()
            .map(|l| LayerOptions {
                color: [l.color.0, l.color.1, l.color.2],
                min_power_pct: None,
                laser2: laser2 && l.laser2,
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
    laser2: bool,
) -> Result<Vec<u8>, String> {
    let opts = job_options(&built.layers, home, limits, laser2);
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
    let mut out = Vec::new();
    if !device.laser2 && layers.iter().any(|l| l.laser2) {
        out.push(tr!("pipeline.ruida.no_laser2", machine = device.name));
    }
    // Caso común: no hay nada más que decir y no se planifica el trabajo entero.
    if cutting_blocks(toolpath) <= MAX_LAYERS {
        return out;
    }
    let opts = job_options(
        layers,
        home_for_origin(device.origin),
        limits_for_device(device),
        device.laser2,
    );
    out.extend(match rd::plan_summary(toolpath, &opts) {
        Ok(s) if s.fits() => vec![tr!(
            "pipeline.ruida.layers_merged",
            max = MAX_LAYERS,
            blocks = s.blocks,
            merged = s.merged,
            layers = s.layers
        )],
        Ok(s) => vec![tr!(
            "pipeline.ruida.layers_too_many",
            max = MAX_LAYERS,
            blocks = s.blocks,
            merged = s.merged,
            layers = s.layers
        )],
        Err(e) => vec![tr!("pipeline.ruida.cannot_export", error = e)],
    });
    out
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
