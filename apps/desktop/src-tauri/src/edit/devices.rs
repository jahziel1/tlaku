//! Comandos del área `devices` (fase 4): valores por defecto de cada tipo de controladora y
//! prueba de una vuelta del rotativo.

use tlaku_pipeline::doc::{DeviceKind, DeviceProfile, Document};

/// Perfil de partida de un tipo de controladora (dialecto, `s_max`, modo de potencia y baudios).
/// La interfaz lo ofrece al cambiar el `kind` de un perfil; no toca el resto de los campos.
#[tauri::command]
pub fn device_defaults(kind: DeviceKind) -> DeviceProfile {
    DeviceProfile::for_kind(kind)
}

/// Documento con una línea de exactamente una vuelta de la pieza (prueba de una vuelta del
/// rotativo). `speed` en mm/min y `power` en %. Falla con un mensaje claro si el rotativo está
/// apagado o sus valores no son válidos.
#[tauri::command]
pub async fn rotary_test(
    device: DeviceProfile,
    speed: f64,
    power: f64,
) -> Result<Document, String> {
    tauri::async_runtime::spawn_blocking(move || tlaku_pipeline::rotary_test(&device, speed, power))
        .await
        .map_err(crate::edit::task_interrupted)?
}

#[cfg(test)]
mod tests {
    use super::*;
    use tlaku_pipeline::doc::PowerMode;

    #[test]
    fn defaults_follow_the_kind_table() {
        let marlin = device_defaults(DeviceKind::Marlin);
        assert_eq!(marlin.kind, DeviceKind::Marlin);
        assert_eq!((marlin.s_max, marlin.baud), (255.0, 250_000));
        assert_eq!(marlin.power_mode, PowerMode::Constant);
        assert_eq!(marlin.dialect.constant_on, "M3 S0 I");
        assert!(marlin.dialect.dynamic_on.is_none());
        let smoothie = device_defaults(DeviceKind::Smoothie);
        assert_eq!((smoothie.s_max, smoothie.baud), (1.0, 115_200));
        assert_eq!(smoothie.dialect.end_sync.as_deref(), Some("M400"));
        // GRBL es el perfil por defecto de siempre.
        assert_eq!(device_defaults(DeviceKind::Grbl), DeviceProfile::default());
    }

    #[test]
    fn rotary_test_is_one_turn_and_needs_the_rotary() {
        let mut d = DeviceProfile::default();
        let err = tlaku_pipeline::rotary_test(&d, 1000.0, 5.0).unwrap_err();
        assert!(err.contains("activa el rotativo"), "{err}");
        d.rotary.enabled = true;
        let doc = tlaku_pipeline::rotary_test(&d, 1000.0, 5.0).unwrap();
        assert_eq!(doc.objects.len(), 1);
        assert_eq!(doc.layers.len(), 1);
    }
}
