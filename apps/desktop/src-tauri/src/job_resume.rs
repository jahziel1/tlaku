//! Comandos de Tauri de «continuar un trabajo interrumpido». La lógica (punto de control, punto de
//! reinicio, preámbulo y condiciones de seguridad) vive en `machine::resume`; aquí solo se
//! adaptan los argumentos y se llega a la carpeta de datos de la app.

use crate::commands::AppState;
use crate::machine::resume::{self, plan::PlanOptions, store::CheckpointStore};
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{AppHandle, Manager, State};
use tlaku_doc::{
    DeviceProfile, InterruptedJob, JobOptions, JobState, ResumeReadiness, ResumeStart, ResumeView,
};
use tlaku_i18n::tr;

/// Carpeta del punto de control, dentro de la carpeta de datos de la app.
pub fn dir(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_data_dir()
        .map(|d| d.join(resume::DIR_NAME))
        .map_err(|e| tr!("desktop.resume.data_dir", err = e))
}

pub fn store(app: &AppHandle) -> Result<Arc<CheckpointStore>, String> {
    Ok(Arc::new(CheckpointStore::new(dir(app)?)))
}

async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    crate::commands::blocking(f).await
}

/// Escribe el marcador de un trabajo de Ruida, K40 o galvo antes de enviarlo. Devuelve la carpeta
/// para quitarlo si el envío falla.
pub fn begin_marker(
    app: &AppHandle,
    device: &DeviceProfile,
    options: &JobOptions,
    document: Option<String>,
) -> Option<Arc<CheckpointStore>> {
    let store = store(app).ok()?;
    resume::write_marker(&store, device, options, document, "").ok()?;
    Some(store)
}

/// Si el envío falló, no hay trabajo que avisar.
pub fn end_marker(store: Option<Arc<CheckpointStore>>, started: bool) {
    if let Some(s) = store
        && !started
    {
        s.clear();
    }
}

/// Un evento final de trabajo: los marcadores de las máquinas que no se reanudan se borran.
pub fn on_job_event(app: &AppHandle, state: JobState) {
    if matches!(
        state,
        JobState::Finished | JobState::Cancelled | JobState::Error
    ) && let Ok(s) = store(app)
    {
        resume::clear_marker_on_end(&s);
    }
}

/// Se conectó otra máquina: el punto de control de la anterior ya no se puede reanudar con esta
/// y se borra.
pub fn forget_if_other_machine(app: &AppHandle, device_id: &str) {
    if let Ok(s) = store(app) {
        resume::forget_if_other_machine(&s, device_id);
    }
}

fn load(app: &AppHandle) -> Result<(Arc<CheckpointStore>, resume::store::Loaded), String> {
    let s = store(app)?;
    let l = s
        .load()
        .ok_or_else(|| tr!("desktop.resume.nothing_to_resume"))?;
    Ok((s, l))
}

/// El trabajo interrumpido que queda, si lo hay. Un punto de control ilegible cuenta como «no hay».
#[tauri::command]
pub async fn interrupted_job(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Option<InterruptedJob>, String> {
    // Con un trabajo transmitiéndose ahora, el punto de control es el suyo, no uno interrumpido.
    if state.handle().is_ok_and(|h| h.is_running()) {
        return Ok(None);
    }
    let s = store(&app)?;
    blocking(move || Ok(s.load().map(|l| resume::summarize(&l)))).await
}

/// El recorrido guardado y el punto de reinicio que se propone. `back_lines` y `back_mm` son lo
/// que se retrocede (los dos a la vez: manda el que queda más atrás).
#[tauri::command(rename_all = "snake_case")]
pub async fn interrupted_job_view(
    app: AppHandle,
    back_lines: Option<u32>,
    back_mm: Option<f64>,
) -> Result<ResumeView, String> {
    let (s, l) = load(&app)?;
    let defaults = PlanOptions::default();
    let opts = PlanOptions {
        back_lines: back_lines.map_or(defaults.back_lines, |n| n.min(10_000) as usize),
        back_mm: back_mm
            .filter(|v| v.is_finite() && *v >= 0.0)
            .map_or(defaults.back_mm, |v| v.min(10_000.0)),
    };
    blocking(move || resume::view(&s, &l, &opts)).await
}

/// Descarta el punto de control (el usuario no quiere continuar).
#[tauri::command]
pub async fn discard_interrupted_job(app: AppHandle) -> Result<(), String> {
    let s = store(&app)?;
    blocking(move || {
        s.clear();
        Ok(())
    })
    .await
}

/// Qué le falta a la máquina conectada para reanudar: si tiene homing y si ya se hizo.
#[tauri::command]
pub async fn continue_check(state: State<'_, AppState>) -> Result<ResumeReadiness, String> {
    let h = state.handle()?;
    blocking(move || Ok(resume::start::readiness(&h))).await
}

/// Encuadre del trabajo completo (el rectángulo del G-code guardado) con el láser apagado.
#[tauri::command]
pub async fn continue_frame(
    app: AppHandle,
    state: State<'_, AppState>,
    speed: f64,
) -> Result<(), String> {
    let h = state.handle()?;
    let (s, l) = load(&app)?;
    blocking(move || resume::start::frame(&h, &s, &l, speed)).await
}

/// Continúa el trabajo desde `args.line`. Falla, sin tocar la máquina, si falta algo de lo que
/// exige la reanudación (ver `machine::resume::start`) o el preflight de siempre.
#[tauri::command]
pub async fn continue_job(
    app: AppHandle,
    state: State<'_, AppState>,
    args: ResumeStart,
) -> Result<(), String> {
    let h = state.handle()?;
    let (s, l) = load(&app)?;
    blocking(move || resume::start::resume_now(&h, s, &l, &args)).await
}
