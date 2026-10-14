//! Eje Z por capa: foco (`z_offset`) y bajada por pasada (`z_step`).
//!
//! Convención interna: la Z de un bloque es la **profundidad del foco respecto a la altura del
//! cabezal al empezar el trabajo** (0 = donde el usuario enfocó en la superficie). Positivo =
//! más hacia dentro del material (el cabezal baja o la cama sube); negativo = hacia fuera. Con
//! `DeviceProfile.z_axis`, cada bloque lleva `z_mm = z_offset + z_step * (pasada - 1)`.
//! `DeviceProfile.z_invert` solo cambia el signo de las palabras Z del G-code (ver `tlaku-grbl`):
//! sin él, profundidad positiva sale como Z negativa (en GRBL +Z aleja el cabezal).
//!
//! El G-code es **relativo** (`G91` / `G0 Z..` / `G90`), no absoluto: Tlaku no conoce el cero de
//! Z de la máquina (no usa G54 ni G10), así que una Z absoluta con "0 = foco" chocaba el
//! cabezal con la pieza. Cada bloque vuelve a la altura inicial al terminar, antes de cualquier
//! desplazamiento o del regreso al origen.
//!
//! Mientras `z_step` sea 0 todas las pasadas comparten Z y la capa
//! sigue siendo un solo bloque con las pasadas repetidas. Con `z_step != 0` y más de una
//! pasada, cada pasada necesita su propia Z y un bloque solo guarda la Z de su inicio, así que
//! **la capa se divide en un bloque por pasada**, y cada bloque tiene su propio `LayerReport`
//! (y, por tanto, su propia fila `PreviewLayer`): se conserva la correspondencia 1:1 entre
//! bloques, informes y filas del resumen, y la interfaz solo ve más filas ("pasada 2/3").
//!
//! Sin eje Z, los valores distintos de 0 se ignoran con una advertencia y no se emite Z.
//! El simulador no suma tiempo por mover Z (ver `tlaku-sim`).

use tlaku_doc::{DeviceProfile, LayerSettings};
use tlaku_i18n::tr;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct LayerZ {
    /// La máquina tiene eje Z: solo entonces se emite `z_mm`.
    pub axis: bool,
    pub offset: f64,
    pub step: f64,
}

impl LayerZ {
    /// Z (mm) al empezar la pasada `pass` (desde 0); `None` si la máquina no tiene Z.
    pub fn z_at(&self, pass: u32) -> Option<f64> {
        self.axis.then(|| self.offset + self.step * f64::from(pass))
    }

    /// Cada pasada va en su propio bloque.
    pub fn splits(&self, passes: u32) -> bool {
        self.axis && self.step != 0.0 && passes > 1
    }
}

/// Lee y valida `z_offset` y `z_step` de la capa; avisa si se ignoran por no haber eje Z.
pub(crate) fn resolve(
    device: &DeviceProfile,
    l: &LayerSettings,
    warnings: &mut Vec<String>,
) -> Result<LayerZ, String> {
    for (name, v) in [("z_offset", l.z_offset), ("z_step", l.z_step)] {
        if !v.is_finite() {
            return Err(tr!(
                "pipeline.zaxis.bad_value",
                layer = l.color,
                name = name,
                value = v
            ));
        }
    }
    if !device.z_axis && (l.z_offset != 0.0 || l.z_step != 0.0) {
        warnings.push(tr!(
            "pipeline.zaxis.ignored_no_axis",
            layer = l.color,
            machine = device.name
        ));
    }
    Ok(LayerZ {
        axis: device.z_axis,
        offset: l.z_offset,
        step: l.z_step,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tlaku_core::Rgb;

    fn layer(offset: f64, step: f64) -> LayerSettings {
        LayerSettings {
            z_offset: offset,
            z_step: step,
            ..LayerSettings::for_color(Rgb(255, 0, 0))
        }
    }

    #[test]
    fn z_follows_offset_plus_step_per_pass() {
        let z = LayerZ {
            axis: true,
            offset: 2.0,
            step: 0.5,
        };
        assert_eq!(z.z_at(0), Some(2.0));
        assert_eq!(z.z_at(3), Some(3.5));
        assert!(z.splits(2) && !z.splits(1));
        let flat = LayerZ { step: 0.0, ..z };
        assert!(!flat.splits(5));
        let none = LayerZ { axis: false, ..z };
        assert_eq!(none.z_at(0), None);
        assert!(!none.splits(5));
    }

    #[test]
    fn warns_only_when_ignored_and_rejects_non_finite() {
        let mut dev = DeviceProfile::default();
        let mut w = Vec::new();
        resolve(&dev, &layer(0.0, 0.0), &mut w).unwrap();
        assert!(w.is_empty());
        resolve(&dev, &layer(1.0, 0.0), &mut w).unwrap();
        assert_eq!(w.len(), 1);
        assert!(w[0].contains("#ff0000") && w[0].contains("eje Z"), "{w:?}");
        dev.z_axis = true;
        w.clear();
        resolve(&dev, &layer(1.0, 0.2), &mut w).unwrap();
        assert!(w.is_empty());
        let e = resolve(&dev, &layer(f64::NAN, 0.0), &mut w).unwrap_err();
        assert!(e.contains("z_offset") && e.contains("#ff0000"), "{e}");
    }
}
