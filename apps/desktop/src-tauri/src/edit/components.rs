//! Componentes guardados (issue #22): selecciones con sus capas y ajustes, en
//! `<configuración>/components/<nombre>.tlakucomp`, junto a la biblioteca de arte (`art`).
//!
//! El motor no interpreta el contenido: el componente es un JSON de la interfaz
//! (`{ format: "tlaku-component", version, objects, layers, bounds, thumb }`) que solo se valida en
//! su forma básica. El nombre del componente es el del archivo.

use super::art::clean_art_name;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};
use tlaku_i18n::tr;

/// Marca del formato (la interfaz la escribe y la lee).
const FORMAT: &str = "tlaku-component";
/// Extensión de los archivos de la biblioteca.
const EXT: &str = "tlakucomp";
/// Tamaño máximo de un componente: una hoja de 60 x 60 cm con miles de piezas pesa varios MB.
const MAX_BYTES: u64 = 64 * 1024 * 1024;

/// Un componente de la biblioteca del usuario.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UserComponent {
    /// Nombre del archivo sin la extensión; es su identificador.
    pub name: String,
    pub data: Value,
}

/// Lo que dejó guardar: los que entraron (con el nombre final) y el motivo de los que no.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SaveComponentsResult {
    pub added: Vec<UserComponent>,
    pub errors: Vec<String>,
}

fn components_dir(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_config_dir()
        .map(|d| d.join("components"))
        .map_err(crate::edit::config_dir_missing)
}

fn is_component(path: &Path) -> bool {
    path.extension()
        .is_some_and(|e| e.eq_ignore_ascii_case(EXT))
}

/// Forma básica: un objeto con la marca del formato, objetos y capas.
fn valid_shape(data: &Value) -> bool {
    data.get("format").and_then(Value::as_str) == Some(FORMAT)
        && data
            .get("objects")
            .and_then(Value::as_array)
            .is_some_and(|o| !o.is_empty())
        && data.get("layers").is_some_and(Value::is_array)
}

fn read_component(path: &Path) -> Result<UserComponent, String> {
    let name = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let meta = std::fs::metadata(path).map_err(|e| format!("{}: {e}", path.display()))?;
    if meta.len() > MAX_BYTES {
        return Err(too_big(&name, meta.len()));
    }
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let data: Value = serde_json::from_str(&text)
        .map_err(|_| tr!("desktop.edit.component.invalid", name = name))?;
    if !valid_shape(&data) {
        return Err(tr!("desktop.edit.component.invalid", name = name));
    }
    Ok(UserComponent { name, data })
}

fn too_big(name: &str, bytes: u64) -> String {
    tr!(
        "desktop.edit.component.too_big",
        name = name,
        mb = bytes / (1024 * 1024),
        max = MAX_BYTES / (1024 * 1024)
    )
}

/// Los componentes de `dir`, por orden alfabético. Una carpeta que no existe es una biblioteca
/// vacía; un archivo ilegible o dañado se salta.
pub fn list_components(dir: &Path) -> Result<Vec<UserComponent>, String> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Ok(Vec::new());
    };
    let mut out: Vec<UserComponent> = entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.is_file() && is_component(p))
        .filter_map(|p| read_component(&p).ok())
        .collect();
    out.sort_by_key(|c| c.name.to_lowercase());
    Ok(out)
}

/// Un nombre libre en `dir`: `base`, `base-2`, `base-3`...
fn unique_name(dir: &Path, base: &str) -> String {
    let taken = |n: &str| dir.join(format!("{n}.{EXT}")).exists();
    if !taken(base) {
        return base.to_string();
    }
    (2..)
        .map(|k| format!("{base}-{k}"))
        .find(|n| !taken(n))
        .expect("siempre hay un número libre")
}

/// Escritura atómica: a un temporal y renombrar, para no dejar un componente a medias.
fn write_atomic(dst: &Path, text: &str) -> Result<(), String> {
    let tmp = dst.with_extension(format!("{EXT}.tmp"));
    std::fs::write(&tmp, text).map_err(|e| format!("{}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, dst).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        format!("{}: {e}", dst.display())
    })
}

/// Guarda componentes que llegan con su contenido. Nunca pisa uno existente: si el nombre está
/// ocupado entra con otro (`nombre-2`).
pub fn save_components(
    dir: &Path,
    items: &[UserComponent],
) -> Result<SaveComponentsResult, String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let mut result = SaveComponentsResult {
        added: Vec::new(),
        errors: Vec::new(),
    };
    for item in items {
        let Some(base) = clean_art_name(&item.name) else {
            result
                .errors
                .push(tr!("desktop.edit.component.bad_name", name = item.name));
            continue;
        };
        if !valid_shape(&item.data) {
            result
                .errors
                .push(tr!("desktop.edit.component.invalid", name = item.name));
            continue;
        }
        let Ok(text) = serde_json::to_string(&item.data) else {
            result
                .errors
                .push(tr!("desktop.edit.component.invalid", name = item.name));
            continue;
        };
        if text.len() as u64 > MAX_BYTES {
            result.errors.push(too_big(&item.name, text.len() as u64));
            continue;
        }
        let name = unique_name(dir, &base);
        let dst = dir.join(format!("{name}.{EXT}"));
        if let Err(e) = write_atomic(&dst, &text) {
            result.errors.push(e);
            continue;
        }
        result.added.push(UserComponent {
            name,
            data: item.data.clone(),
        });
    }
    Ok(result)
}

/// Nombre que es solo un nombre, nunca una ruta.
fn plain_name(name: &str) -> Result<(), String> {
    if name.is_empty() || name.contains(['/', '\\']) || name.contains("..") {
        return Err(tr!("desktop.edit.component.bad_name", name = name));
    }
    Ok(())
}

/// Cambia el nombre de un componente y devuelve el nombre final (con otro número si el nuevo ya
/// existía). Renombrar a su mismo nombre no hace nada.
pub fn rename_component(dir: &Path, old: &str, new: &str) -> Result<String, String> {
    plain_name(old)?;
    let Some(base) = clean_art_name(new) else {
        return Err(tr!("desktop.edit.component.bad_name", name = new));
    };
    let src = dir.join(format!("{old}.{EXT}"));
    if !src.is_file() {
        return Err(tr!("desktop.edit.component.not_found", name = old));
    }
    if base == old {
        return Ok(base);
    }
    let name = unique_name(dir, &base);
    let dst = dir.join(format!("{name}.{EXT}"));
    std::fs::rename(&src, &dst).map_err(|e| format!("{}: {e}", src.display()))?;
    Ok(name)
}

/// Borra un componente. `name` es solo el nombre, nunca una ruta.
pub fn remove_component(dir: &Path, name: &str) -> Result<(), String> {
    plain_name(name)?;
    let path = dir.join(format!("{name}.{EXT}"));
    std::fs::remove_file(&path).map_err(|e| format!("{}: {e}", path.display()))
}

#[tauri::command(rename_all = "snake_case")]
pub async fn component_list_user(app: AppHandle) -> Result<Vec<UserComponent>, String> {
    let dir = components_dir(&app)?;
    tauri::async_runtime::spawn_blocking(move || list_components(&dir))
        .await
        .map_err(crate::edit::task_interrupted)?
}

#[tauri::command(rename_all = "snake_case")]
pub async fn component_save_user(
    app: AppHandle,
    items: Vec<UserComponent>,
) -> Result<SaveComponentsResult, String> {
    let dir = components_dir(&app)?;
    tauri::async_runtime::spawn_blocking(move || save_components(&dir, &items))
        .await
        .map_err(crate::edit::task_interrupted)?
}

#[tauri::command(rename_all = "snake_case")]
pub async fn component_rename_user(
    app: AppHandle,
    name: String,
    new_name: String,
) -> Result<String, String> {
    let dir = components_dir(&app)?;
    tauri::async_runtime::spawn_blocking(move || rename_component(&dir, &name, &new_name))
        .await
        .map_err(crate::edit::task_interrupted)?
}

#[tauri::command(rename_all = "snake_case")]
pub async fn component_remove_user(app: AppHandle, name: String) -> Result<(), String> {
    let dir = components_dir(&app)?;
    tauri::async_runtime::spawn_blocking(move || remove_component(&dir, &name))
        .await
        .map_err(crate::edit::task_interrupted)?
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("tlaku-comp-test-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn comp(name: &str) -> UserComponent {
        UserComponent {
            name: name.into(),
            data: json!({
                "format": FORMAT,
                "version": 1,
                "objects": [{ "id": "a" }],
                "layers": [{ "color": "#ff0000" }],
                "bounds": [[0.0, 0.0], [600.0, 600.0]],
                "thumb": "<svg/>",
            }),
        }
    }

    #[test]
    fn guarda_lista_renombra_y_borra_componentes() {
        let work = scratch("ciclo");
        let lib = work.join("components");
        // Sin carpeta: biblioteca vacía.
        assert!(list_components(&lib).unwrap().is_empty());

        let bad = UserComponent {
            name: "roto".into(),
            data: json!({ "format": "otra-cosa" }),
        };
        let r = save_components(
            &lib,
            &[
                comp("hoja 60x60"),
                comp("hoja 60x60"),
                comp("../fuera"),
                bad,
                comp("   "),
            ],
        )
        .unwrap();
        let names: Vec<_> = r.added.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["hoja 60x60", "hoja 60x60-2", "-fuera"]);
        assert_eq!(r.errors.len(), 2, "{:?}", r.errors);
        // Todo quedó dentro de la biblioteca.
        assert!(!work.join("fuera.tlakucomp").exists());

        // El contenido vuelve intacto.
        let listed = list_components(&lib).unwrap();
        assert_eq!(listed.len(), 3);
        let hoja = listed.iter().find(|c| c.name == "hoja 60x60").unwrap();
        assert_eq!(hoja.data["bounds"][1][0], 600.0);

        // Renombrar: nombre libre, mismo nombre y nombre ocupado.
        assert_eq!(
            rename_component(&lib, "hoja 60x60", "panel").unwrap(),
            "panel"
        );
        assert_eq!(rename_component(&lib, "panel", "panel").unwrap(), "panel");
        assert_eq!(
            rename_component(&lib, "panel", "hoja 60x60-2").unwrap(),
            "hoja 60x60-2-2"
        );
        assert!(rename_component(&lib, "no-existe", "x").is_err());
        assert!(rename_component(&lib, "../a", "x").is_err());
        assert!(rename_component(&lib, "hoja 60x60-2", "  ").is_err());

        // Borrar.
        assert!(remove_component(&lib, "../fuera").is_err());
        remove_component(&lib, "hoja 60x60-2-2").unwrap();
        assert_eq!(list_components(&lib).unwrap().len(), 2);
        assert!(remove_component(&lib, "hoja 60x60-2-2").is_err());
        let _ = std::fs::remove_dir_all(work);
    }

    #[test]
    fn un_archivo_danado_se_salta_al_listar() {
        let work = scratch("danado");
        let lib = work.join("components");
        save_components(&lib, &[comp("bueno")]).unwrap();
        std::fs::write(lib.join("malo.tlakucomp"), "{ no es json").unwrap();
        std::fs::write(lib.join("otro.txt"), "hola").unwrap();
        let names: Vec<_> = list_components(&lib)
            .unwrap()
            .into_iter()
            .map(|c| c.name)
            .collect();
        assert_eq!(names, ["bueno"]);
        let _ = std::fs::remove_dir_all(work);
    }
}
