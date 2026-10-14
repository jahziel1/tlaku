//! Detección de tablero y calibración de lente.

use super::*;

pub(super) fn check_board(board: [u32; 2]) -> Result<(), String> {
    if board[0] < 2 || board[1] < 2 || board[0].max(board[1]) < 3 {
        return Err(tr!(
            "desktop.edit.camera.board_too_small",
            cols = board[0],
            rows = board[1]
        ));
    }
    Ok(())
}

/// Esquinas internas del tablero (fila por fila, en píxeles) o `None` si no se ve completo.
pub fn run_detect_board(image_b64: &str, board: [u32; 2]) -> Result<Option<Vec<[f64; 2]>>, String> {
    check_board(board)?;
    let img = decode_gray(image_b64)?;
    Ok(find_chessboard(&img, board)
        .ok()
        .map(|pts| pts.iter().map(|p| [p.x, p.y]).collect()))
}

/// Calibra la lente con varias vistas del tablero (todas del mismo tamaño).
#[cfg(test)]
pub fn run_calibrate(
    images_b64: &[String],
    board: [u32; 2],
    square_mm: f64,
) -> Result<CalibrationReport, String> {
    run_calibrate_with(images_b64, board, square_mm, LensModelChoice::Auto)
}

/// Igual que [`run_calibrate`] pero con el modelo de lente fijo o automático.
pub fn run_calibrate_with(
    images_b64: &[String],
    board: [u32; 2],
    square_mm: f64,
    model: LensModelChoice,
) -> Result<CalibrationReport, String> {
    check_board(board)?;
    if !square_mm.is_finite() || square_mm <= 0.0 {
        return Err("El tamaño del cuadro debe ser mayor que 0 mm.".into());
    }
    if images_b64.is_empty() {
        return Err("Captura al menos 3 vistas del tablero antes de calcular.".into());
    }
    let images = images_b64
        .iter()
        .enumerate()
        .map(|(i, b)| decode_gray(b).map_err(|e| format!("Vista {}: {e}", i + 1)))
        .collect::<Result<Vec<_>, _>>()?;
    let opts = CalibrationOptions {
        model,
        ..CalibrationOptions::default()
    };
    tlaku_camera::calibrate_lens_with(&images, board, square_mm, &opts)
}

// La interfaz manda los argumentos en snake_case (`image_png_base64`), como el resto del contrato.
#[tauri::command(rename_all = "snake_case")]
pub async fn detect_board(
    image_png_base64: String,
    board: [u32; 2],
) -> Result<Option<Vec<[f64; 2]>>, String> {
    tauri::async_runtime::spawn_blocking(move || run_detect_board(&image_png_base64, board))
        .await
        .map_err(crate::edit::task_interrupted)?
}

#[tauri::command(rename_all = "snake_case")]
pub async fn calibrate_lens(
    images_png_base64: Vec<String>,
    board: [u32; 2],
    square_mm: f64,
    model: Option<LensModelChoice>,
) -> Result<CalibrationReport, String> {
    tauri::async_runtime::spawn_blocking(move || {
        run_calibrate_with(
            &images_png_base64,
            board,
            square_mm,
            model.unwrap_or_default(),
        )
    })
    .await
    .map_err(crate::edit::task_interrupted)?
}
