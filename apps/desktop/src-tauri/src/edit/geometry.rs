//! Comandos del área `geometry` (fase 3). Pendiente de implementar.

use serde_json::Value;

#[tauri::command]
pub async fn geometry_op(args: Option<Value>) -> Result<Value, String> {
    let _ = args;
    Err("todavía no disponible".into())
}
