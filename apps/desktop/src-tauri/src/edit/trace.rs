//! Comandos del área `trace` (fase 3). Pendiente de implementar.

use serde_json::Value;

#[tauri::command]
pub async fn trace_image(args: Option<Value>) -> Result<Value, String> {
    let _ = args;
    Err("todavía no disponible".into())
}
