//! Prueba con hardware real a través de la app (Tauri real, ventana oculta): los mismos `invoke`
//! que manda la interfaz. Herramienta de desarrollo para validar una máquina conectada
//! (`docs/validacion-hardware.md`); **nunca** enciende el láser. Uso:
//!
//! `cargo run -p tlaku-desktop --example hwtest -- <puerto> <paso>...`
//!
//! Pasos: `ports`, `connect`, `disconnect`, `status`, `settings`, `home`, `jog:<dx>,<dy>,<vel>`,
//! `frame:<x>,<y>,<lado>,<vel>` (encuadre con el láser apagado), `job0:<x>,<y>,<lado>,<vel>` y
//! `fill0:...` (trabajo con potencia 0: el G-code se revisa antes de enviarlo), `pause`, `resume`,
//! `cancel`, `interrupted`, `view`, `readiness`, `discard`, `wait:<ms>` y `cmd:<línea>` (solo de
//! lectura o movimiento; se rechaza `M3`, `M4` y cualquier `S` distinto de 0).

use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{Value, json};
use tauri::{Listener, RunEvent, WebviewUrl, WebviewWindowBuilder};

use crate::smoke::invoke_json;

/// Líneas que nunca se mandan desde aquí: todo lo que pueda encender el láser.
fn forbidden(line: &str) -> bool {
    let l = line.to_ascii_uppercase();
    l.contains("M3")
        || l.contains("M4")
        || l.contains(" S")
        || l.starts_with('S')
        || l.contains("$J=") && l.contains('S')
}

pub fn run(port: String, steps: Vec<String>) -> i32 {
    let mut context = crate::app_context();
    context.config_mut().app.windows.clear();
    let app = crate::build_app(context);
    let started = Arc::new(Mutex::new(false));
    app.run_return(move |handle, event| match event {
        RunEvent::Ready => {
            if std::mem::replace(&mut *started.lock().unwrap(), true) {
                return;
            }
            let handle = handle.clone();
            let (port, steps) = (port.clone(), steps.clone());
            std::thread::spawn(move || {
                let code = steps_on(&handle, &port, &steps);
                handle.exit(code);
            });
        }
        RunEvent::Exit => {
            use tauri::Manager;
            handle.state::<crate::commands::AppState>().shutdown();
        }
        _ => {}
    })
}

fn steps_on(handle: &tauri::AppHandle, port: &str, steps: &[String]) -> i32 {
    let window =
        match WebviewWindowBuilder::new(handle, "main", WebviewUrl::App("about:blank".into()))
            .visible(false)
            .build()
        {
            Ok(w) => w,
            Err(e) => {
                eprintln!("hwtest: sin ventana: {e}");
                return 2;
            }
        };
    // Todo lo que el motor emite: registro de la consola, estado, conexión.
    let t0 = std::time::Instant::now();
    for ev in [
        "machine://log",
        "machine://status",
        "machine://connection",
        "machine://alarm",
        "machine://error",
    ] {
        let name = ev.to_string();
        handle.listen(ev, move |e| {
            println!(
                "  {:>6} ms [{name}] {}",
                t0.elapsed().as_millis(),
                e.payload()
            )
        });
    }
    let wv = window.as_ref();
    let mut code = 0;
    for step in steps {
        println!("== {:>6} ms {step}", t0.elapsed().as_millis());
        let r = match step.as_str() {
            "ports" => invoke_json(handle, wv, "list_ports", json!({})),
            "connect" => invoke_json(
                handle,
                wv,
                "connect",
                json!({ "port": port, "baud": 115200, "device": falcon2(), "galvoAck": Value::Null }),
            ),
            "disconnect" => invoke_json(handle, wv, "disconnect", json!({})),
            "status" => invoke_json(handle, wv, "machine_command", json!({ "line": "?" })),
            "settings" => invoke_json(handle, wv, "machine_command", json!({ "line": "$$" })),
            "home" => invoke_json(handle, wv, "machine_command", json!({ "line": "$H" })),
            "cancel" => invoke_json(handle, wv, "cancel_job", json!({})),
            "pause" => invoke_json(handle, wv, "pause_job", json!({})),
            "interrupted" => invoke_json(handle, wv, "interrupted_job", json!({})),
            "view" => invoke_json(
                handle,
                wv,
                "interrupted_job_view",
                json!({ "back_lines": Value::Null, "back_mm": Value::Null }),
            ),
            "readiness" => invoke_json(handle, wv, "continue_check", json!({})),
            "discard" => invoke_json(handle, wv, "discard_interrupted_job", json!({})),
            "resume" => invoke_json(handle, wv, "resume_job", json!({})),
            s if s.starts_with("job0:") || s.starts_with("fill0:") => {
                // job0:<x>,<y>,<lado>,<vel> (contorno) o fill0:... (relleno a 0,5 mm): potencia 0 %.
                let fill = s.starts_with("fill0:");
                let v: Vec<f64> = s[s.find(':').unwrap() + 1..]
                    .split(',')
                    .filter_map(|x| x.parse().ok())
                    .collect();
                if v.len() != 4 {
                    Err("job0:<x>,<y>,<lado>,<vel>".into())
                } else {
                    let mut doc = square_doc(v[0], v[1], v[2]);
                    for l in &mut doc.layers {
                        l.speed = v[3];
                        l.power = 0.0;
                        l.min_power = 0.0;
                        l.passes = 1;
                        l.air = false;
                        if fill {
                            l.mode = tlaku_doc::LayerMode::Fill;
                            l.interval = 0.5;
                        }
                    }
                    if let Ok(job) = tlaku_pipeline::compute_at(
                        &doc,
                        &falcon2(),
                        &tlaku_doc::JobOptions::default(),
                        None,
                    ) {
                        println!("  ESTIMADO {:.2} s", job.preview.total_s);
                    }
                    let (device, options) = (falcon2(), tlaku_doc::JobOptions::default());
                    match crate::engine::job_gcode(&doc, &device, &options, None) {
                        Err(e) => Err(e),
                        Ok(g) => match zero_power_only(&g.lines) {
                            Err(bad) => Err(format!(
                                "ABORTADO: el G-code tiene potencia distinta de 0: «{bad}»"
                            )),
                            Ok(()) => {
                                println!(
                                    "  G-code revisado: {} líneas, todas con S0 y sin M3",
                                    g.lines.len()
                                );
                                for l in &g.lines {
                                    println!("  | {l}");
                                }
                                invoke_json(
                                    handle,
                                    wv,
                                    "start_job",
                                    json!({
                                        "doc": doc, "device": device, "options": options, "document": Value::Null,
                                    }),
                                )
                            }
                        },
                    }
                }
            }
            s if s.starts_with("wait:") => {
                std::thread::sleep(Duration::from_millis(s[5..].parse().unwrap_or(1000)));
                Ok(Value::Null)
            }
            s if s.starts_with("frame:") => {
                // frame:<x>,<y>,<lado>,<vel>: encuadre de un cuadrado con el láser APAGADO.
                let v: Vec<f64> = s[6..].split(',').filter_map(|x| x.parse().ok()).collect();
                if v.len() == 4 {
                    invoke_json(
                        handle,
                        wv,
                        "frame_job",
                        json!({
                            "doc": square_doc(v[0], v[1], v[2]),
                            "device": falcon2(),
                            "options": tlaku_doc::JobOptions::default(),
                            "speed": v[3],
                            "power_pct": Value::Null,
                        }),
                    )
                } else {
                    Err("frame:<x>,<y>,<lado>,<vel>".into())
                }
            }
            s if s.starts_with("jog:") => {
                let v: Vec<f64> = s[4..].split(',').filter_map(|x| x.parse().ok()).collect();
                if v.len() == 3 {
                    invoke_json(
                        handle,
                        wv,
                        "jog",
                        json!({ "dx": v[0], "dy": v[1], "speed": v[2] }),
                    )
                } else {
                    Err("jog:<dx>,<dy>,<vel>".into())
                }
            }
            s if s.starts_with("cmd:") => {
                let line = &s[4..];
                if forbidden(line) {
                    Err(format!("rechazado: «{line}» podría encender el láser"))
                } else {
                    invoke_json(handle, wv, "machine_command", json!({ "line": line }))
                }
            }
            other => Err(format!("paso desconocido: {other}")),
        };
        match r {
            Ok(v) => println!("  OK {v}"),
            Err(e) => {
                println!("  ERROR {e}");
                code = 1;
            }
        }
    }
    let _ = invoke_json(handle, wv, "disconnect", json!({}));
    code
}

/// Perfil de la Falcon2 22 W (cama 400 × 415, origen abajo a la izquierda, sin Z).
fn falcon2() -> tlaku_doc::DeviceProfile {
    tlaku_doc::DeviceProfile {
        kind: tlaku_doc::DeviceKind::Grbl,
        bed: [400.0, 415.0],
        origin: tlaku_doc::Origin::BottomLeft,
        max_rate: [25000.0, 8000.0],
        ..tlaku_doc::DeviceProfile::default()
    }
}

/// Un cuadrado de `side` mm con la esquina inferior izquierda en (`x`, `y`).
fn square_doc(x: f64, y: f64, side: f64) -> tlaku_doc::Document {
    use tlaku_core::{Point, Polyline, Rgb};
    let red = Rgb(255, 0, 0);
    let path = tlaku_core::bezpath::BezPath::from(&Polyline {
        points: vec![
            Point::new(x, y),
            Point::new(x + side, y),
            Point::new(x + side, y + side),
            Point::new(x, y + side),
        ],
        closed: true,
    });
    tlaku_doc::Document {
        objects: vec![tlaku_doc::DesignObject {
            id: "hw".into(),
            layer: red,
            transform: tlaku_doc::affine::IDENTITY,
            kind: tlaku_doc::ObjectKind::Vector(tlaku_doc::VectorObject::new(vec![path])),
            locked: false,
            hidden: false,
            group: None,
            priority: None,
            power_scale: None,
            shape: None,
            start: None,
            manual_tabs: Vec::new(),
            array: None,
        }],
        layers: vec![tlaku_doc::LayerSettings::for_color(red)],
        ..tlaku_doc::Document::default()
    }
}

/// Revisa que ninguna línea pueda encender el láser: sin `M3` y todas las `S` en 0.
fn zero_power_only(lines: &[String]) -> Result<(), String> {
    for line in lines {
        let code = line.split(';').next().unwrap_or("").to_ascii_uppercase();
        if code.contains("M3") {
            return Err(line.clone());
        }
        let bytes = code.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            if bytes[i] == b'S' {
                let rest: String = code[i + 1..]
                    .chars()
                    .take_while(|c| c.is_ascii_digit() || *c == '.')
                    .collect();
                if !rest.parse::<f64>().is_ok_and(|v| v == 0.0) {
                    return Err(line.clone());
                }
            }
            i += 1;
        }
    }
    Ok(())
}
