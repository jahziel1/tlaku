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
//! También prueba la ventana aparte de la vista en vivo (`live`): que solo pueda escuchar eventos
//! y llamar a `live_window_hello` / `live_window_close`, y que CUALQUIER otro comando de la app
//! (máquina, trabajo, archivos, preferencias) lo rechace el control de acceso de Tauri.
//!
//! Para las pestañas de documentos comprueba además que `new_window` pasa el parámetro `handoff` a la dirección de
//! la ventana nueva (y rechaza uno raro) y que en macOS el menú de la app no trae «Cerrar ventana» (Cmd+W es de la pestaña).
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
        c("list_generators", json!({}), Ok),
        c(
            "generate_part",
            json!({ "generator": "box", "params": {}, "max_width_mm": 300.0, "gap_mm": 5.0 }),
            Ok,
        ),
        c("load_preferences", json!({}), Ok),
        c("load_factory_materials", json!({}), Ok),
        c("load_material_library", json!({}), Ok),
        c("load_remnants", json!({}), Ok),
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
                        "smoke-tauri: {} llamadas correctas, segunda ventana, ventana aparte de solo lectura y evento storage",
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
    failures.extend(live_window(handle, first));
    failures.extend(storage_between_windows(handle));
    failures.extend(menu_without_close_window(handle));
    failures
}

/// Pestañas: en macOS el menú de la app no debe traer «Cerrar ventana» (su Cmd+W es de la pestaña).
/// En el resto de sistemas no hay nada que comprobar.
fn menu_without_close_window(handle: &tauri::AppHandle) -> Vec<String> {
    #[cfg(target_os = "macos")]
    {
        use tauri::menu::MenuItemKind;
        let Some(menu) = handle.menu() else {
            return vec!["la app no tiene menú".to_string()];
        };
        let mut fails = Vec::new();
        let items = menu.items().unwrap_or_default();
        if items.is_empty() {
            fails.push("el menú de la app está vacío".to_string());
        }
        for kind in items {
            let MenuItemKind::Submenu(sub) = kind else {
                continue;
            };
            for item in sub.items().unwrap_or_default() {
                if let MenuItemKind::Predefined(p) = &item
                    && p.text().unwrap_or_default() == "Close Window"
                {
                    fails.push("el menú aún trae «Close Window» (Cmd+W cerraría la ventana, no la pestaña)".to_string());
                }
            }
        }
        fails
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = handle;
        Vec::new()
    }
}

/// Manda un `invoke` por la ventana `webview`, como lo haría su interfaz. `None` si no responde.
pub(crate) fn invoke(
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
pub(crate) fn invoke_json(
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
    // «Mover a ventana nueva» (pestañas): `handoff` llega a la dirección de la ventana, y uno raro se rechaza.
    let handoff = "%7B%22id%22%3A%22handoff-smoke%22%7D";
    match invoke_json(handle, first, "new_window", json!({ "handoff": handoff })) {
        Ok(v) => {
            let l3 = v.as_str().unwrap_or_default().to_string();
            match handle.get_webview_window(&l3).map(|w| (w.url(), w)) {
                Some((Ok(url), w)) => {
                    check(
                        &mut fails,
                        url.query() == Some(&format!("handoff={handoff}")),
                        "la ventana movida no recibió el parámetro handoff en su dirección",
                    );
                    let _ = w.destroy();
                }
                _ => {
                    fails.push("la ventana con handoff no existe o no tiene dirección".to_string())
                }
            }
        }
        Err(e) => fails.push(format!("new_window con handoff: {e}")),
    }
    check(
        &mut fails,
        invoke_json(handle, first, "new_window", json!({ "handoff": "a&b=1" })).is_err(),
        "new_window aceptó un handoff con caracteres que no son de una dirección",
    );
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

/// La ventana aparte de la vista en vivo es de solo lectura: su capacidad (`capabilities/live.json`)
/// no deja llamar a los comandos de máquina ni a ninguno más que los dos suyos. Ninguna de estas
/// llamadas llega a ejecutarse (la rechaza el control de acceso antes), así que no se toca nada.
fn live_window(handle: &tauri::AppHandle, first: &tauri::Webview) -> Vec<String> {
    use tauri::Manager;
    let mut fails = Vec::new();
    if let Err(e) = invoke_json(handle, first, "live_window_open", json!({})) {
        return vec![format!("abrir la ventana aparte: {e}")];
    }
    let Some(live) = handle.get_webview_window("live") else {
        return vec!["la ventana «live» no existe tras live_window_open".into()];
    };
    let _ = live.hide(); // la prueba no necesita verla
    let view: &tauri::Webview = live.as_ref();

    // Una sola ventana `live`: abrir otra vez la reutiliza.
    if let Err(e) = invoke_json(handle, first, "live_window_open", json!({})) {
        fails.push(format!("abrir la ventana aparte por segunda vez: {e}"));
    }
    check(
        &mut fails,
        handle
            .webview_windows()
            .keys()
            .filter(|l| l.as_str() == "live")
            .count()
            == 1,
        "debía haber una sola ventana «live»",
    );

    // Todo comando de la app que no sea suyo lo rechaza el ACL (mensaje «not allowed»).
    let denied: [(&str, Value); 14] = [
        ("app_version", json!({})),
        ("list_ports", json!({})),
        ("connect", json!({ "port": "", "baud": 0 })),
        ("disconnect", json!({})),
        ("machine_command", json!({ "line": "$H" })),
        ("machine_realtime", json!({ "cmd": "status" })),
        ("jog", json!({ "dx": 0.0, "dy": 0.0, "speed": 1.0 })),
        ("start_job", json!({})),
        ("pause_job", json!({})),
        ("cancel_job", json!({})),
        (
            "fire_test",
            json!({ "kind": "grbl", "power_pct": 1.0, "s_max": 1000.0, "duration_ms": 10 }),
        ),
        (
            "save_preferences",
            json!({ "data": { "version": 1, "prefs": {}, "shortcuts": {} } }),
        ),
        ("live_window_send", json!({ "kind": "state", "data": "{}" })),
        ("new_window", json!({})),
    ];
    for (cmd, args) in denied {
        match invoke(handle, view, cmd, args) {
            Some(InvokeResponse::Err(e)) => {
                let msg = e.0.to_string();
                check(
                    &mut fails,
                    msg.contains("not allowed") || msg.contains("ACL"),
                    &format!("{cmd} desde la ventana «live» falló, pero no por el ACL: {msg}"),
                );
            }
            Some(InvokeResponse::Ok(_)) => fails.push(format!(
                "la ventana «live» pudo llamar a {cmd}: su capacidad debe rechazarlo"
            )),
            None => fails.push(format!("{cmd} desde «live»: sin respuesta en 20 s")),
        }
    }

    // Lo suyo sí: pedir el estado (solo ella) y mandarlo desde la principal.
    if let Err(e) = invoke_json(handle, view, "live_window_hello", json!({})) {
        fails.push(format!("live_window_hello desde la ventana «live»: {e}"));
    }
    check(
        &mut fails,
        invoke_json(handle, first, "live_window_hello", json!({})).is_err(),
        "live_window_hello debe rechazar a una ventana que no es «live»",
    );
    match invoke_json(
        handle,
        first,
        "live_window_send",
        json!({ "kind": "state", "data": "{}" }),
    ) {
        Ok(v) => check(
            &mut fails,
            v == json!(true),
            "live_window_send debía devolver true con la ventana abierta",
        ),
        Err(e) => fails.push(e),
    }
    check(
        &mut fails,
        invoke_json(
            handle,
            first,
            "live_window_send",
            json!({ "kind": "machine_command", "data": "x" }),
        )
        .is_err(),
        "live_window_send debe rechazar un canal desconocido",
    );

    // Cerrarse sí puede.
    if let Err(e) = invoke_json(handle, view, "live_window_close", json!({})) {
        fails.push(format!("live_window_close desde la ventana «live»: {e}"));
    }
    check(
        &mut fails,
        wait_until(|| handle.get_webview_window("live").is_none()),
        "la ventana «live» sigue existiendo tras live_window_close",
    );
    match invoke_json(
        handle,
        first,
        "live_window_send",
        json!({ "kind": "state", "data": "{}" }),
    ) {
        Ok(v) => check(
            &mut fails,
            v == json!(false),
            "sin ventana, live_window_send debía devolver false",
        ),
        Err(e) => fails.push(e),
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
