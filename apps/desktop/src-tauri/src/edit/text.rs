//! Comandos del área `text` (fase 3). Pendiente de implementar.

use serde_json::Value;

#[tauri::command]
pub async fn list_fonts(args: Option<Value>) -> Result<Value, String> {
    let _ = args;
    Err("todavía no disponible".into())
}

#[tauri::command]
pub async fn text_outline(args: Option<Value>) -> Result<Value, String> {
    let _ = args;
    Err("todavía no disponible".into())
}
