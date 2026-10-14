//! Prueba de humo de la aplicación real: arranca Tauri (runtime Wry) con una ventana oculta,
//! manda mensajes `invoke` por la misma vía que usa la interfaz y revisa la respuesta de los
//! comandos de solo lectura. Detecta lo que el modo simulado no ve: comandos mal registrados,
//! argumentos con el nombre equivocado (camelCase / `rename_all`) y estado sin registrar.
//!
//! Además prueba «Nueva ventana» (D19) con una segunda ventana real: que `new_window` la cree y
//! exista, que responda por `invoke` (la capacidad `doc-*`), que las preferencias que guarda una
//! ventana las lea la otra, que la recuperación no mezcle los documentos de cada ventana ni
//! ofrezca a una la copia viva de la otra, y que al cerrarla sus copias queden huérfanas. Una
//! última parte abre dos ventanas del mismo origen sobre una página local y comprueba que el
//! `localStorage` que escribe una llega a la otra como evento `storage` (en lo que se apoya
//! `startPrefsWindowSync`).
//!
//! Se lanza con `apps/desktop/scripts/smoke-tauri.sh` (aísla `HOME` para no tocar los datos
//! reales) y necesita un servidor de ventanas: el de macOS, o `xvfb-run` en Linux.

use std::io::{Read, Write};
use std::net::TcpListener;
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
                    println!(
                        "smoke-tauri: {} llamadas correctas, segunda ventana y evento storage",
                        calls().len()
                    );
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
    let mut failures = Vec::new();
    for call in calls() {
        let label = format!("{} {}", call.cmd, call.args);
        match invoke(handle, window.as_ref(), call.cmd, call.args.clone()) {
            None => failures.push(format!("{label}: sin respuesta en 20 s")),
            Some(InvokeResponse::Ok(_)) => {
                if call.expect != Expect::Ok && call.expect != Expect::ArgsAccepted {
                    failures.push(format!(
                        "{label}: debía rechazar los argumentos y respondió Ok"
                    ));
                }
            }
            Some(InvokeResponse::Err(e)) => {
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
    let first: &tauri::Webview = window.as_ref();
    failures.extend(second_window(handle, first));
    failures.extend(storage_between_windows(handle));
    failures
}

/// Manda un `invoke` por la ventana `webview`, como lo haría su interfaz. `None` si no responde.
fn invoke(
    handle: &tauri::AppHandle,
    webview: &tauri::Webview,
    cmd: &str,
    args: Value,
) -> Option<InvokeResponse> {
    let url = if cfg!(any(windows, target_os = "android")) {
        "http://tauri.localhost"
    } else {
        "tauri://localhost"
    };
    let (tx, rx) = std::sync::mpsc::sync_channel(1);
    let request = InvokeRequest {
        cmd: cmd.into(),
        callback: CallbackFn(0),
        error: CallbackFn(1),
        url: url.parse().unwrap(),
        body: InvokeBody::Json(args),
        headers: Default::default(),
        invoke_key: handle.invoke_key().to_string(),
    };
    webview.clone().on_message(
        request,
        Box::new(move |_w, _c, response, _cb, _err| {
            let _ = tx.send(response);
        }),
    );
    rx.recv_timeout(Duration::from_secs(20)).ok()
}

/// Respuesta correcta como JSON, o el motivo del fallo.
fn invoke_json(
    handle: &tauri::AppHandle,
    webview: &tauri::Webview,
    cmd: &str,
    args: Value,
) -> Result<Value, String> {
    match invoke(handle, webview, cmd, args) {
        None => Err(format!("{cmd}: sin respuesta en 20 s")),
        Some(InvokeResponse::Err(e)) => Err(format!("{cmd}: {}", e.0)),
        Some(InvokeResponse::Ok(body)) => body
            .deserialize::<Value>()
            .map_err(|e| format!("{cmd}: respuesta ilegible: {e}")),
    }
}

/// Espera hasta 5 s a que `cond` sea cierta (los eventos de ventana llegan por el bucle principal).
fn wait_until(mut cond: impl FnMut() -> bool) -> bool {
    for _ in 0..50 {
        if cond() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    cond()
}

/// Un documento mínimo y reconocible por sus notas.
fn marked_doc(notes: &str) -> Value {
    let doc = tlaku_doc::Document {
        notes: notes.to_string(),
        ..Default::default()
    };
    serde_json::to_value(doc).unwrap()
}

fn copy_ids(list: &Value) -> Vec<String> {
    let mut ids: Vec<String> = list
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|i| i["id"].as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    ids.sort();
    ids
}

/// Anota un fallo si `ok` es falso.
fn check(fails: &mut Vec<String>, ok: bool, msg: &str) {
    if !ok {
        fails.push(msg.to_string());
    }
}

/// D19: abre una segunda ventana por el comando `new_window` y comprueba lo que se promete de
/// ella. Devuelve los fallos (vacío si todo está bien) y deja las dos copias de prueba borradas.
fn second_window(handle: &tauri::AppHandle, first: &tauri::Webview) -> Vec<String> {
    use tauri::Manager;
    let mut fails = Vec::new();

    // Se abre por el mismo comando que usa el menú Archivo > Nueva ventana.
    let label = match invoke_json(handle, first, "new_window", json!({})) {
        Ok(v) => v.as_str().unwrap_or_default().to_string(),
        Err(e) => return vec![format!("nueva ventana: {e}")],
    };
    check(
        &mut fails,
        label.starts_with("doc-"),
        "la etiqueta de la ventana nueva no empieza por doc-",
    );
    let Some(second) = handle.get_webview_window(&label) else {
        return vec![format!("la ventana «{label}» no existe tras new_window")];
    };
    check(
        &mut fails,
        handle.webview_windows().len() == 2,
        "debía haber exactamente dos ventanas abiertas",
    );
    // Cada llamada a `new_window` crea una etiqueta distinta.
    let other = invoke_json(handle, first, "new_window", json!({}));
    match other {
        Ok(v) => {
            let l2 = v.as_str().unwrap_or_default().to_string();
            check(
                &mut fails,
                l2 != label,
                "dos ventanas nuevas recibieron la misma etiqueta",
            );
            if let Some(w) = handle.get_webview_window(&l2) {
                let _ = w.destroy();
            }
        }
        Err(e) => fails.push(format!("segunda llamada a new_window: {e}")),
    }
    let _ = second.hide(); // la prueba no necesita verla
    let second_view: &tauri::Webview = second.as_ref();

    // La capacidad `doc-*` deja a la ventana nueva usar el motor.
    if let Err(e) = invoke_json(handle, second_view, "app_version", json!({})) {
        fails.push(format!("la ventana nueva no puede llamar al motor: {e}"));
    }

    // Preferencias: las dos ventanas leen y escriben el mismo archivo.
    let prefs = json!({ "version": 1, "prefs": { "smoke": "desde-la-primera" }, "shortcuts": {} });
    match invoke_json(handle, first, "save_preferences", json!({ "data": prefs })) {
        Err(e) => fails.push(e),
        Ok(_) => match invoke_json(handle, second_view, "load_preferences", json!({})) {
            Ok(read) => check(
                &mut fails,
                read == prefs,
                "la ventana nueva no ve las preferencias que guardó la primera",
            ),
            Err(e) => fails.push(e),
        },
    }
    let prefs2 = json!({ "version": 1, "prefs": { "smoke": "desde-la-segunda" }, "shortcuts": {} });
    match invoke_json(
        handle,
        second_view,
        "save_preferences",
        json!({ "data": prefs2 }),
    ) {
        Err(e) => fails.push(e),
        Ok(_) => match invoke_json(handle, first, "load_preferences", json!({})) {
            Ok(read) => check(
                &mut fails,
                read == prefs2,
                "la primera ventana no ve las preferencias que guardó la nueva",
            ),
            Err(e) => fails.push(e),
        },
    }

    // Recuperación: cada ventana escribe la copia de su documento.
    let write = |view: &tauri::Webview, id: &str, notes: &str| {
        invoke_json(
            handle,
            view,
            "autosave_write",
            json!({ "id": id, "name": notes, "path": null, "doc": marked_doc(notes) }),
        )
    };
    for (view, id, notes) in [
        (first, "smoke-primera", "documento de la primera"),
        (second_view, "smoke-segunda", "documento de la segunda"),
    ] {
        if let Err(e) = write(view, id, notes) {
            fails.push(e);
        }
    }
    let list = |view: &tauri::Webview| {
        invoke_json(handle, view, "autosave_list", json!({})).map(|v| copy_ids(&v))
    };
    // Cada ventana solo ve la copia propia: la de la otra sigue viva.
    match list(first) {
        Ok(ids) => check(
            &mut fails,
            ids == ["smoke-primera"],
            &format!("la primera ventana debía ver solo su copia y vio {ids:?}"),
        ),
        Err(e) => fails.push(e),
    }
    match list(second_view) {
        Ok(ids) => check(
            &mut fails,
            ids == ["smoke-segunda"],
            &format!("la ventana nueva debía ver solo su copia y vio {ids:?}"),
        ),
        Err(e) => fails.push(e),
    }
    // Cada copia conserva su propio documento.
    for (id, notes) in [
        ("smoke-primera", "documento de la primera"),
        ("smoke-segunda", "documento de la segunda"),
    ] {
        match invoke_json(handle, first, "autosave_read", json!({ "id": id })) {
            Ok(doc) => check(
                &mut fails,
                doc["notes"] == notes,
                &format!("la copia {id} trae otro documento: {}", doc["notes"]),
            ),
            Err(e) => fails.push(e),
        }
    }

    // Al cerrar la ventana nueva, su copia queda huérfana y la primera ya puede recuperarla.
    if let Err(e) = second.destroy() {
        fails.push(format!("no se pudo cerrar la ventana nueva: {e}"));
    }
    check(
        &mut fails,
        wait_until(|| handle.get_webview_window(&label).is_none()),
        "la ventana nueva sigue existiendo tras cerrarla",
    );
    check(
        &mut fails,
        handle.webview_windows().len() == 1,
        "tras cerrar debía quedar una sola ventana",
    );
    let mut offered = false;
    wait_until(|| {
        offered = matches!(list(first), Ok(ids) if ids == ["smoke-primera", "smoke-segunda"]);
        offered
    });
    check(
        &mut fails,
        offered,
        "al cerrar la ventana nueva, su copia no se ofreció a la primera",
    );

    for id in ["smoke-primera", "smoke-segunda"] {
        let _ = invoke_json(handle, first, "autosave_discard", json!({ "id": id }));
    }
    fails
}

/// Página que avisa por HTTP cuando está lista y cuando otra ventana cambia el `localStorage`.
const PROBE_PAGE: &str = "<!doctype html><meta charset=utf-8><title>probe</title><script>\
addEventListener('storage', e => fetch('/got?key=' + encodeURIComponent(e.key) + '&value=' + \
encodeURIComponent(e.newValue)));\
fetch('/ready');</script>";

/// Atiende la página de prueba y apunta las rutas que piden las ventanas (`/ready`, `/got?...`).
fn serve_probe(listener: TcpListener, seen: Arc<Mutex<Vec<String>>>) {
    for stream in listener.incoming() {
        let Ok(mut stream) = stream else { break };
        let mut buf = [0u8; 2048];
        let n = stream.read(&mut buf).unwrap_or(0);
        let head = String::from_utf8_lossy(&buf[..n]);
        let path = head
            .lines()
            .next()
            .and_then(|l| l.split_whitespace().nth(1))
            .unwrap_or("/")
            .to_string();
        if path == "/stop" {
            break;
        }
        let body = if path == "/" { PROBE_PAGE } else { "ok" };
        if path != "/" {
            seen.lock().unwrap().push(path);
        }
        let _ = write!(
            stream,
            "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
    }
}

/// D19: las preferencias se avisan entre ventanas por el evento `storage` del `localStorage`
/// (`startPrefsWindowSync`). Aquí se comprueba que el motor web de verdad lo entrega: dos
/// ventanas del mismo origen (una página local mínima, porque en esta prueba la interfaz no se
/// sirve) y lo que una escribe lo recibe la otra.
fn storage_between_windows(handle: &tauri::AppHandle) -> Vec<String> {
    use tauri::Manager;
    let Ok(listener) = TcpListener::bind("127.0.0.1:0") else {
        return vec!["no se pudo abrir el puerto local de la prueba de storage".into()];
    };
    let port = listener.local_addr().unwrap().port();
    let seen = Arc::new(Mutex::new(Vec::<String>::new()));
    let server = {
        let seen = seen.clone();
        std::thread::spawn(move || serve_probe(listener, seen))
    };
    let url: tauri::Url = format!("http://127.0.0.1:{port}/").parse().unwrap();
    let open = |label: &str| {
        WebviewWindowBuilder::new(handle, label, WebviewUrl::External(url.clone()))
            .visible(false)
            .build()
    };
    let mut fails = Vec::new();
    let (a, b) = (open("probe-a"), open("probe-b"));
    match (&a, &b) {
        (Ok(a), Ok(_)) => {
            let ready = |n: usize| {
                wait_until(|| {
                    seen.lock()
                        .unwrap()
                        .iter()
                        .filter(|p| *p == "/ready")
                        .count()
                        >= n
                })
            };
            if !ready(2) {
                fails.push("las ventanas de la prueba de storage no cargaron".into());
            } else {
                let _ = a.eval("localStorage.setItem('tlaku-smoke', 'cambio')");
                let got = wait_until(|| {
                    seen.lock()
                        .unwrap()
                        .iter()
                        .any(|p| p.starts_with("/got?key=tlaku-smoke&value=cambio"))
                });
                if !got {
                    fails.push(
                        "el cambio de localStorage de una ventana no llegó a la otra (evento storage)"
                            .into(),
                    );
                }
            }
        }
        _ => fails.push("no se pudieron abrir las ventanas de la prueba de storage".into()),
    }
    for label in ["probe-a", "probe-b"] {
        if let Some(w) = handle.get_webview_window(label) {
            let _ = w.destroy();
        }
    }
    if let Ok(mut s) = std::net::TcpStream::connect(("127.0.0.1", port)) {
        let _ = s.write_all(b"GET /stop HTTP/1.1\r\n\r\n");
    }
    let _ = server.join();
    fails
}
