//! Reconocimiento de seguridad ([`SafetyAck`]) y paso del perfil a la configuración de la sesión.

use super::*;

// ---------------------------------------------------------------------------
// Reconocimiento de seguridad
// ---------------------------------------------------------------------------

/// Lo que el usuario confirma en el diálogo de seguridad antes de abrir una tarjeta real. Las
/// TRES casillas deben estar marcadas; no se recuerda de una conexión a otra.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SafetyAck {
    /// Lleva gafas con la densidad óptica correcta para la longitud de onda de la fuente.
    pub eyewear: bool,
    /// El láser está en un recinto cerrado.
    pub enclosure: bool,
    /// Sabe dónde está el paro de emergencia.
    pub emergency_stop: bool,
}

impl SafetyAck {
    /// El único lugar de la app que crea el [`LaserSafetyAck`] del USB: falla si falta alguna
    /// confirmación.
    pub fn token(&self) -> Result<LaserSafetyAck, String> {
        if self.eyewear && self.enclosure && self.emergency_stop {
            Ok(LaserSafetyAck::i_understand_laser_safety())
        } else {
            Err(ACK_REQUIRED.into())
        }
    }
}

/// Error cuando falta el reconocimiento de seguridad para una tarjeta real.
pub const ACK_REQUIRED: &str = "Para conectar con una tarjeta galvo real hay que confirmar la \
seguridad láser: gafas con la densidad óptica correcta para la longitud de onda, recinto cerrado \
y saber dónde está el paro de emergencia. Una fibra es clase 4 y su haz es invisible. Para \
probar sin máquina usa la máquina virtual (virtual:galvo).";

// ---------------------------------------------------------------------------
// Del perfil a la sesión
// ---------------------------------------------------------------------------

/// Configuración de la sesión a partir del perfil: límites del perfil y el campo centrado en la
/// cama (convención del pipeline).
pub fn session_config(device: &DeviceProfile) -> Result<SessionConfig, String> {
    if device.kind != DeviceKind::Galvo {
        return Err("El perfil de la máquina no es de tipo galvo.".into());
    }
    // La posición del galvo es la del documento: no se aplica ningún otro origen.
    if device.origin != Origin::BottomLeft {
        return Err(
            "El origen del perfil galvo debe ser «abajo a la izquierda»: el campo de la \
                    lente se coloca con las coordenadas del documento."
                .into(),
        );
    }
    let mut cfg = SessionConfig::new(device.galvo.clone())
        .map_err(|e| format!("El perfil del galvo no es válido: {e}"))?;
    cfg.origin = FieldOrigin::CenterAt(Point::new(device.bed[0] / 2.0, device.bed[1] / 2.0));
    Ok(cfg)
}

/// La parte del perfil que usa el driver. Lo que el pipeline calcula y la sesión NO usa (marcado
/// dividido y repetido, cilindro y cono) se descarta: se ignora al comprobar que el perfil no
/// cambió desde que se conectó.
pub(super) fn driver_part(g: &tlaku_doc::GalvoDevice) -> tlaku_doc::GalvoDevice {
    let mut g = g.clone();
    g.cylinder = tlaku_doc::CylinderCorrection::default();
    g.taper = tlaku_doc::TaperWarp::default();
    g.split = tlaku_doc::SplitMarking::default();
    g.repeat = tlaku_doc::RepeatMarking::default();
    g
}

/// Rectángulo del campo (cuadrado de `field_mm` centrado en la cama), como `tlaku_pipeline::field_rect`.
pub(super) fn field_bounds(bed: [f64; 2], field_mm: f64) -> tlaku_core::Bounds {
    tlaku_pipeline::field_rect(&DeviceProfile {
        bed,
        galvo: tlaku_doc::GalvoDevice {
            field_mm,
            ..tlaku_doc::GalvoDevice::default()
        },
        ..DeviceProfile::default()
    })
}
