//! Comandos del área `camera` (fase 3). Pendiente de implementar.

use serde_json::Value;

#[tauri::command]
pub async fn detect_board(args: Option<Value>) -> Result<Value, String> {
    let _ = args;
    Err("todavía no disponible".into())
}

#[tauri::command]
pub async fn calibrate_lens(args: Option<Value>) -> Result<Value, String> {
    let _ = args;
    Err("todavía no disponible".into())
}
