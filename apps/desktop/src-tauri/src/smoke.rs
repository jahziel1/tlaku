//! Prueba de humo de la aplicación real: arranca Tauri (runtime Wry) con una ventana oculta,
//! manda mensajes `invoke` por la misma vía que usa la interfaz y revisa la respuesta de los
//! comandos de solo lectura. Detecta lo que el modo simulado no ve: comandos mal registrados,
//! argumentos con el nombre equivocado (camelCase / `rename_all`) y estado sin registrar.
//!
//! Se lanza con `apps/desktop/scripts/smoke-tauri.sh` (aísla `HOME` para no tocar los datos
//! reales) y necesita un servidor de ventanas: el de macOS, o `xvfb-run` en Linux.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{Value, json};
use tauri::ipc::{CallbackFn, InvokeBody, InvokeResponse};
use tauri::webview::InvokeRequest;
use tauri::{RunEvent, WebviewUrl, WebviewWindowBuilder};

/// Qué se espera de la respuesta de una llamada.
#[derive(Clone, Copy, PartialEq)]
enum Expect {
    /// `Ok(..)`.
    Ok,
    /// Cualquier error de negocio (por ejemplo «no hay máquina conectada»), pero que los
    /// argumentos hayan llegado bien.
    ArgsAccepted,
    /// Error de argumentos: Tauri no encontró las claves (se usa para probar que la prueba ve el fallo).
    ArgsRejected,
}

struct Call {
    cmd: &'static str,
    args: Value,
    expect: Expect,
}

fn calls() -> Vec<Call> {
    use Expect::*;
    let c = |cmd, args, expect| Call { cmd, args, expect };
    vec![
        // Solo lectura: no escriben nada ni tocan la máquina.
        c("app_version", json!({}), Ok),
        c("list_ports", json!({}), Ok),
        c("list_devices", json!({}), Ok),
        c("device_defaults", json!({ "kind": "grbl" }), Ok),
        c("list_fonts", json!({}), Ok),
        c("load_preferences", json!({}), Ok),
        c("load_factory_materials", json!({}), Ok),
        c("load_material_library", json!({}), Ok),
        c("load_camera_setup", json!({}), Ok),
        c("art_list_user", json!({}), Ok),
        c("autosave_list", json!({}), Ok),
        c("list_templates", json!({}), Ok),
        c("remote_status", json!({}), Ok),
        c("paths_exist", json!({ "paths": ["/no/existe"] }), Ok),
        c(
            "camera_marks",
            json!({ "bed": [300.0, 200.0], "count": 4 }),
            Ok,
        ),
        // Sin máquina conectada devuelven un error de negocio, pero deben aceptar los argumentos.
        c("ruida_info", json!({}), ArgsAccepted),
        c("lhy_state", json!({}), ArgsAccepted),
        c("galvo_state", json!({}), ArgsAccepted),
        // El caso que falló en la app real: `fire_test` espera snake_case...
        c(
            "fire_test",
            json!({ "kind": "grbl", "power_pct": 1.0, "s_max": 1000.0, "duration_ms": 10 }),
            ArgsAccepted,
        ),
        // ...y con camelCase Tauri lo rechaza (comprueba que esta prueba sí ve el error).
        c(
            "fire_test",
            json!({ "kind": "grbl", "powerPct": 1.0, "sMax": 1000.0, "durationMs": 10 }),
            ArgsRejected,
        ),
    ]
}

fn is_args_error(msg: &str) -> bool {
    msg.contains("invalid args") || msg.contains("missing required key")
}

/// Ejecuta la prueba de humo y devuelve el código de salida (0 = todo bien).
pub fn run() -> i32 {
    let mut context = crate::app_context();
    // Sin ventanas de la configuración: se crea una oculta a mano.
    context.config_mut().app.windows.clear();
    let app = crate::build_app(context);

    let started = Arc::new(Mutex::new(false));
    // `run_return` devuelve el código que se le dé a `exit`; `run` terminaría el proceso por su cuenta.
    app.run_return(move |handle, event| match event {
        RunEvent::Ready => {
            if std::mem::replace(&mut *started.lock().unwrap(), true) {
                return;
            }
            let handle = handle.clone();
            // La ventana y las llamadas van en otro hilo: el principal sigue con el bucle de eventos.
            std::thread::spawn(move || {
                let failures = run_calls(&handle);
                for f in &failures {
                    eprintln!("smoke-tauri FALLO: {f}");
                }
                if failures.is_empty() {
                    println!("smoke-tauri: {} llamadas correctas", calls().len());
                }
                handle.exit(i32::from(!failures.is_empty()));
            });
        }
        RunEvent::Exit => {
            use tauri::Manager;
            handle.state::<crate::commands::AppState>().shutdown();
        }
        _ => {}
    })
}

fn run_calls(handle: &tauri::AppHandle) -> Vec<String> {
    let window =
        match WebviewWindowBuilder::new(handle, "main", WebviewUrl::App("index.html".into()))
            .visible(false)
            .build()
        {
            Ok(w) => w,
            Err(e) => return vec![format!("no se pudo crear la ventana oculta: {e}")],
        };
    let url = if cfg!(any(windows, target_os = "android")) {
        "http://tauri.localhost"
    } else {
        "tauri://localhost"
    };
    let mut failures = Vec::new();
    for call in calls() {
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        let request = InvokeRequest {
            cmd: call.cmd.into(),
            callback: CallbackFn(0),
            error: CallbackFn(1),
            url: url.parse().unwrap(),
            body: InvokeBody::Json(call.args.clone()),
            headers: Default::default(),
            invoke_key: handle.invoke_key().to_string(),
        };
        window.as_ref().clone().on_message(
            request,
            Box::new(move |_w, _c, response, _cb, _err| {
                let _ = tx.send(response);
            }),
        );
        let label = format!("{} {}", call.cmd, call.args);
        match rx.recv_timeout(Duration::from_secs(20)) {
            Err(_) => failures.push(format!("{label}: sin respuesta en 20 s")),
            Ok(InvokeResponse::Ok(_)) => {
                if call.expect != Expect::Ok && call.expect != Expect::ArgsAccepted {
                    failures.push(format!(
                        "{label}: debía rechazar los argumentos y respondió Ok"
                    ));
                }
            }
            Ok(InvokeResponse::Err(e)) => {
                let msg = e.0.to_string();
                let rejected = is_args_error(&msg);
                match call.expect {
                    Expect::Ok => failures.push(format!("{label}: error inesperado: {msg}")),
                    Expect::ArgsAccepted if rejected => {
                        failures.push(format!("{label}: Tauri rechazó los argumentos: {msg}"));
                    }
                    Expect::ArgsRejected if !rejected => {
                        failures.push(format!(
                            "{label}: debía rechazar los argumentos y dijo: {msg}"
                        ));
                    }
                    _ => {}
                }
            }
        }
    }
    failures
}
