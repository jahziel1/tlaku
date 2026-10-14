//! Reconocimiento de seguridad ([`SafetyAck`]) y paso del perfil a la configuración de la sesión.

use super::*;
use tlaku_i18n::tr;

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
            Err(ack_required())
        }
    }
}

/// Error cuando falta el reconocimiento de seguridad para una tarjeta real.
pub fn ack_required() -> String {
    tr!("desktop.galvo.ack_required")
}

// ---------------------------------------------------------------------------
// Del perfil a la sesión
// ---------------------------------------------------------------------------

/// Configuración de la sesión a partir del perfil: límites del perfil y el campo centrado en la
/// cama (convención del pipeline).
pub fn session_config(device: &DeviceProfile) -> Result<SessionConfig, String> {
    if device.kind != DeviceKind::Galvo {
        return Err(tr!("desktop.galvo.profile_not_galvo"));
    }
    // La posición del galvo es la del documento: no se aplica ningún otro origen.
    if device.origin != Origin::BottomLeft {
        return Err(tr!("desktop.galvo.origin_bottom_left"));
    }
    let mut cfg = SessionConfig::new(device.galvo.clone())
        .map_err(|e| tr!("desktop.galvo.profile_invalid", err = e))?;
    cfg.origin = FieldOrigin::CenterAt(Point::new(device.bed[0] / 2.0, device.bed[1] / 2.0));
    // El desfase del puntero (el mismo del perfil de las demás máquinas) lo compensan el encuadre
    // y el recorrido en rojo; una marca real nunca lo aplica.
    cfg.pointer_offset = device.pointer_offset;
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
