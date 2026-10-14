//! Ayudas sueltas: perfil de simulación, repetición de pasadas e imagen reflejada.

use crate::galvo;
use crate::rotary;
use tlaku_core::toolpath::{Move, ToolpathBlock};
use tlaku_doc::{DeviceKind, DeviceProfile};
use tlaku_raster::GrayF32;
use tlaku_sim::MachineProfile;

pub(super) fn machine_profile(device: &DeviceProfile) -> Result<MachineProfile, String> {
    let positive = |name: &str, values: &[f64]| {
        if values.iter().all(|v| v.is_finite() && *v > 0.0) {
            Ok(())
        } else {
            Err(format!(
                "la máquina '{}': {name} debe ser mayor que 0",
                device.name
            ))
        }
    };
    positive("el área de trabajo", &device.bed)?;
    // El galvo no tiene aceleración de ejes: usa su propio perfil de tiempo.
    if device.kind == DeviceKind::Galvo {
        return Ok(galvo::sim_profile(&device.galvo));
    }
    positive("la aceleración", &device.accel)?;
    positive("la velocidad máxima", &device.max_rate)?;
    positive("la velocidad de movimientos rápidos", &[device.rapid_rate])?;
    // Con rotativo el trabajo está en mm de superficie: los límites de Y de la controladora se
    // pasan a superficie para que el tiempo y el overscan sean los reales (ver `rotary`).
    let k = rotary::y_scale(device)?;
    Ok(MachineProfile {
        max_rate_mm_min: [device.max_rate[0], device.max_rate[1] / k],
        accel_mm_s2: [device.accel[0], device.accel[1] / k],
        rapid_rate_mm_min: device.rapid_rate,
        stop_on_cut_change: !crate::gcode::plans_laser_moves_together(device),
        ..MachineProfile::default()
    })
}

pub(super) fn travel_count(block: &ToolpathBlock) -> usize {
    block
        .moves
        .iter()
        .filter(|m| matches!(m, Move::Travel { .. }))
        .count()
}

/// Repite los movimientos del bloque una vez por pasada.
pub(super) fn repeat_moves(block: &mut ToolpathBlock, passes: u32) {
    if passes > 1 {
        let once = block.moves.clone();
        for _ in 1..passes {
            block.moves.extend_from_slice(&once);
        }
    }
}

/// Como [`repeat_moves`] y anota las pasadas en la etiqueta.
pub(super) fn repeat_passes(block: &mut ToolpathBlock, passes: u32) {
    if passes > 1 {
        repeat_moves(block, passes);
        block.label.push_str(&format!(" pasadas={passes}"));
    }
}

pub(super) fn flipped(gray: GrayF32, mirror_x: bool, mirror_y: bool) -> GrayF32 {
    if !mirror_x && !mirror_y {
        return gray;
    }
    GrayF32::from_fn(gray.width, gray.height, |x, y| {
        let sx = if mirror_x { gray.width - 1 - x } else { x };
        let sy = if mirror_y { gray.height - 1 - y } else { y };
        gray.get(sx, sy)
    })
}
