//! Buscador de centro: un círculo en una foto.

use super::*;

// ---------------------------------------------------------------------------

/// Busca un círculo en una foto (px de la imagen: origen en el borde). `hint` acota la búsqueda a
/// un objeto cuando hay varios.
pub fn run_detect_circle(image_b64: &str, hint: Option<CircleHint>) -> Result<CircleFit, String> {
    let img = decode_gray(image_b64)?;
    detect_circle(&img, hint)
}

#[tauri::command(rename_all = "snake_case")]
pub async fn detect_circle_in_image(
    image_png_base64: String,
    hint: Option<CircleHint>,
) -> Result<CircleFit, String> {
    tauri::async_runtime::spawn_blocking(move || run_detect_circle(&image_png_base64, hint))
        .await
        .map_err(crate::edit::task_interrupted)?
}
