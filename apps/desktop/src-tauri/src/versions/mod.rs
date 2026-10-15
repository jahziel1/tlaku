//! Historial de versiones del proyecto (mejora 10 del plan de usuarios).
//!
//! Cada vez que se guarda un `.tlaku`, la interfaz pide conservar antes lo que tenía el archivo
//! (`versions_capture`); además hace instantáneas automáticas mientras se edita y manuales con
//! nombre (`versions_snapshot`). Todo es local: nada sale del equipo.
//!
//! - `store.rs`: una carpeta de versiones (gzip, escritura atómica, duplicados).
//! - `retention.rs`: política de retención (función pura).
//! - `config.rs`: `versions.json` (dónde se guarda, cada cuánto, qué se conserva).
//! - `project.rs`: de ruta de proyecto a carpeta; captura, lista, lectura, borrado.
//! - `commands.rs`: comandos de Tauri (asíncronos).
//!
//! El guardado principal NO depende de esto: la interfaz llama a `versions_capture` por separado y,
//! si falla (disco lleno, permisos), avisa y sigue guardando el proyecto. Los proyectos sin
//! guardar no tienen historial: los cubre la recuperación (`autosave.rs`).

pub mod commands;
pub mod config;
pub mod project;
pub mod retention;
pub mod store;

#[cfg(test)]
mod tests;
