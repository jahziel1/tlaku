//! Comandos del área `devices` (fase 4): valores por defecto de cada tipo de controladora,
//! prueba de una vuelta del rotativo y lectura de perfiles `.lbdev` de LightBurn.

use std::fs;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};
use tlaku_i18n::tr;
use tlaku_pipeline::doc::{DeviceKind, DeviceProfile, Document, LbdevImport, parse_lbdev};

/// Tope del tamaño de un `.lbdev` (los reales pesan unos pocos KB).
const LBDEV_MAX_BYTES: u64 = 2 * 1024 * 1024;

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

/// Lee un `.lbdev` y fabrica los perfiles **sin guardar nada**: la interfaz muestra el resumen
/// y los avisos y solo guarda (con `save_device`) lo que el usuario confirma. Los ids y nombres
/// nuevos no chocan con los perfiles guardados. Nunca toca la máquina.
pub fn read_lbdev_file(config_dir: &Path, path: &Path) -> Result<LbdevImport, String> {
    let io = |e: std::io::Error| format!("{}: {e}", path.display());
    let meta = fs::metadata(path).map_err(io)?;
    if meta.len() > LBDEV_MAX_BYTES {
        return Err(tr!("desktop.devices.lbdev_too_big", path = path.display()));
    }
    let bytes = fs::read(path).map_err(io)?;
    let text = String::from_utf8_lossy(&bytes);
    let existing: Vec<(String, String)> = crate::engine::list_devices(config_dir)?
        .into_iter()
        .map(|d| (d.id, d.name))
        .collect();
    parse_lbdev(&text, &existing)
}

fn config_dir(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_config_dir()
        .map_err(crate::edit::config_dir_missing)
}

/// Importar perfil de LightBurn (`.lbdev`): lee el archivo y devuelve los perfiles propuestos y
/// los avisos (claves sin equivalente, controladores sin soporte...). No guarda nada.
#[tauri::command]
pub async fn read_lbdev(app: AppHandle, path: String) -> Result<LbdevImport, String> {
    let dir = config_dir(&app)?;
    tauri::async_runtime::spawn_blocking(move || read_lbdev_file(&dir, Path::new(&path)))
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

    #[test]
    fn lbdev_file_is_read_without_saving_and_ids_avoid_saved_profiles() {
        let dir = std::env::temp_dir().join(format!("tlaku-lbdev-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        // Sin devices.json, el perfil de fábrica ya ocupa su id y su nombre.
        let taken = DeviceProfile::default();
        let file = dir.join("p.lbdev");
        let json = format!(
            r#"{{"DeviceList":[{{"DisplayName":"{}","Name":"GRBL","Width":300,"Height":200}}]}}"#,
            taken.name
        );
        fs::write(&file, json).unwrap();
        let out = read_lbdev_file(&dir, &file).unwrap();
        assert_eq!(out.devices.len(), 1);
        assert_ne!(out.devices[0].id, taken.id);
        assert_eq!(out.devices[0].name, format!("{} (2)", taken.name));
        assert!(
            !dir.join("devices.json").exists(),
            "importar no guarda nada"
        );
        // Archivo ausente y JSON roto: errores con la ruta o el motivo.
        assert!(read_lbdev_file(&dir, &dir.join("no.lbdev")).is_err());
        fs::write(&file, "{ roto").unwrap();
        assert!(read_lbdev_file(&dir, &file).unwrap_err().contains("JSON"));
        fs::remove_dir_all(&dir).unwrap();
    }
}
