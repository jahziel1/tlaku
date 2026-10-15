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
            // ---- Pasos con láser: solo con TLAKU_HW_LASER=1 y con topes fijos. ----
            s if s.starts_with("fire:") => {
                // fire:<pct>,<ms>: disparo de prueba (tope 5 % y 500 ms).
                let v = nums(&s[5..]);
                match laser_allowed() {
                    Err(e) => Err(e),
                    Ok(())
                        if v.len() != 2
                            || v[0] <= 0.0
                            || v[0] > 5.0
                            || v[1] <= 0.0
                            || v[1] > 500.0 =>
                    {
                        Err("fire:<pct ≤ 5>,<ms ≤ 500>".into())
                    }
                    Ok(()) => invoke_json(
                        handle,
                        wv,
                        "fire_test",
                        json!({ "kind": "grbl", "power_pct": v[0], "s_max": 1000.0, "duration_ms": v[1] as u32 }),
                    ),
                }
            }
            s if s.starts_with("framelaser:") => {
                // framelaser:<x>,<y>,<lado>,<vel>,<pct>: encuadre con el láser a baja potencia (tope 5 %).
                let v = nums(&s[11..]);
                match laser_allowed() {
                    Err(e) => Err(e),
                    Ok(()) if v.len() != 5 || v[4] <= 0.0 || v[4] > 5.0 => {
                        Err("framelaser:<x>,<y>,<lado>,<vel>,<pct ≤ 5>".into())
                    }
                    Ok(()) => invoke_json(
                        handle,
                        wv,
                        "frame_job",
                        json!({
                            "doc": square_doc(v[0], v[1], v[2]),
                            "device": falcon2(),
                            "options": tlaku_doc::JobOptions::default(),
                            "speed": v[3],
                            "power_pct": v[4],
                        }),
                    ),
                }
            }
            s if s.starts_with("job:")
                || s.starts_with("fill:")
                || s.starts_with("fillair:")
                || s.starts_with("grid:")
                || s.starts_with("image:") =>
            {
                // job:<x>,<y>,<lado>,<vel>,<pot>  (contorno)
                // fill:<x>,<y>,<lado>,<vel>,<pot>,<intervalo>  (relleno)
                // grid:<x>,<y>,<celda>,<n>,<pot0>,<pot1>,<vel0>,<vel1>  (prueba de materiales n×n, relleno 0,1 mm)
                // Topes: potencia ≤ 40 %, piezas ≤ 40 mm, n ≤ 6.
                let kind = &s[..s.find(':').unwrap()];
                let v = nums(&s[kind.len() + 1..]);
                match laser_allowed().and_then(|()| laser_doc(kind, &v)) {
                    Err(e) => Err(e),
                    Ok(doc) => {
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
                            Ok(g) => match power_at_most(&g.lines, MAX_JOB_PCT * 10.0) {
                                Err(bad) => {
                                    Err(format!("ABORTADO: potencia por encima del tope: «{bad}»"))
                                }
                                Ok(max) => {
                                    println!(
                                        "  G-code revisado: {} líneas, S máxima {max}",
                                        g.lines.len()
                                    );
                                    invoke_json(
                                        handle,
                                        wv,
                                        "start_job",
                                        json!({ "doc": doc, "device": device, "options": options, "document": Value::Null }),
                                    )
                                }
                            },
                        }
                    }
                }
            }
            s if s.starts_with("contframe:") => {
                // contframe:<vel>: encuadre del trabajo interrumpido con el láser apagado.
                let speed: f64 = s[10..].parse().unwrap_or(3000.0);
                invoke_json(handle, wv, "continue_frame", json!({ "speed": speed }))
            }
            "contjob" => {
                // Continúa el trabajo interrumpido desde el punto sugerido. Sin TLAKU_HW_LASER=1 solo
                // si el G-code guardado no puede encender el láser.
                let gcode = crate::job_resume::dir(handle)
                    .map(|d| d.join("job.gcode"))
                    .and_then(|p| std::fs::read_to_string(&p).map_err(|e| e.to_string()));
                let lines: Vec<String> = gcode
                    .unwrap_or_default()
                    .lines()
                    .map(str::to_string)
                    .collect();
                let safe = laser_allowed().is_ok()
                    || (!lines.is_empty() && zero_power_only(&lines).is_ok());
                if !safe {
                    Err(
                        "contjob: el trabajo guardado enciende el láser y falta TLAKU_HW_LASER=1"
                            .into(),
                    )
                } else {
                    let job = invoke_json(handle, wv, "interrupted_job", json!({}));
                    let view = invoke_json(
                        handle,
                        wv,
                        "interrupted_job_view",
                        json!({ "back_lines": Value::Null, "back_mm": Value::Null }),
                    );
                    match (job, view) {
                        (Ok(j), Ok(v)) => {
                            let id = j.get("id").cloned().unwrap_or(Value::Null);
                            let sug = v.get("suggested").cloned().unwrap_or(Value::Null);
                            println!("  CONTINUAR desde {sug}");
                            invoke_json(
                                handle,
                                wv,
                                "continue_job",
                                json!({ "args": {
                                    "id": id,
                                    "line": sug.get("line").cloned().unwrap_or(json!(0)),
                                    "at": sug.get("at").cloned().unwrap_or(Value::Null),
                                    "confirm_unmoved": false,
                                    "framed_ok": true,
                                    "checklist_ok": true,
                                }}),
                            )
                        }
                        (Err(e), _) | (_, Err(e)) => Err(e),
                    }
                }
            }
            s if s.starts_with("snap:") => {
                // snap:<archivo>: foto con el programa de TLAKU_HW_SNAP (cámara de la máquina).
                match std::env::var("TLAKU_HW_SNAP") {
                    Err(_) => Err("snap: falta TLAKU_HW_SNAP".into()),
                    Ok(bin) => std::process::Command::new(bin)
                        .arg(
                            std::env::var("TLAKU_HW_CAMERA")
                                .unwrap_or_else(|_| "Creality Falcon Camera".into()),
                        )
                        .arg(&s[5..])
                        .output()
                        .map_err(|e| e.to_string())
                        .and_then(|o| {
                            if o.status.success() {
                                Ok(Value::Null)
                            } else {
                                Err(String::from_utf8_lossy(&o.stdout).into_owned())
                            }
                        }),
                }
            }
            s if s.starts_with("goto:") => {
                // goto:<x>,<y>,<vel>: lleva el cabezal a una posición de máquina (láser apagado).
                let v = nums(&s[5..]);
                if v.len() == 3 {
                    invoke_json(
                        handle,
                        wv,
                        "jog_to",
                        json!({ "x": v[0], "y": v[1], "speed": v[2] }),
                    )
                } else {
                    Err("goto:<x>,<y>,<vel>".into())
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

/// Tope de potencia de los trabajos con láser de esta herramienta (porcentaje de `s_max`).
const MAX_JOB_PCT: f64 = 40.0;

fn nums(s: &str) -> Vec<f64> {
    s.split(',').filter_map(|x| x.trim().parse().ok()).collect()
}

/// Los pasos con láser solo corren si quien lanza la herramienta lo pide con `TLAKU_HW_LASER=1`.
fn laser_allowed() -> Result<(), String> {
    if std::env::var("TLAKU_HW_LASER").as_deref() == Ok("1") {
        Ok(())
    } else {
        Err("paso con láser bloqueado: hace falta TLAKU_HW_LASER=1".into())
    }
}

/// Documento de un paso con láser, validado contra los topes.
fn laser_doc(kind: &str, v: &[f64]) -> Result<tlaku_doc::Document, String> {
    use tlaku_core::Rgb;
    let pct_ok = |p: f64| p > 0.0 && p <= MAX_JOB_PCT;
    match kind {
        "job" | "fill" | "fillair" => {
            let need = if kind == "job" { 5 } else { 6 };
            if v.len() != need || v[2] <= 0.0 || v[2] > 40.0 || v[3] <= 0.0 || !pct_ok(v[4]) {
                return Err(format!(
                    "{kind}: argumentos fuera de rango (lado ≤ 40 mm, potencia ≤ {MAX_JOB_PCT} %)"
                ));
            }
            let mut doc = square_doc(v[0], v[1], v[2]);
            for l in &mut doc.layers {
                l.speed = v[3];
                l.power = v[4];
                l.min_power = 0.0;
                l.passes = 1;
                l.air = kind == "fillair";
                if kind != "job" {
                    l.mode = tlaku_doc::LayerMode::Fill;
                    l.interval = v[5].clamp(0.05, 1.0);
                }
            }
            Ok(doc)
        }
        "grid" => {
            if v.len() != 8
                || v[2] <= 0.0
                || v[3] < 2.0
                || v[3] > 6.0
                || !pct_ok(v[4])
                || !pct_ok(v[5])
                || v[6] <= 0.0
                || v[7] <= 0.0
            {
                return Err(
                    "grid:<x>,<y>,<celda>,<n 2..6>,<pot0>,<pot1>,<vel0>,<vel1> (potencia ≤ 40 %)"
                        .into(),
                );
            }
            let (x0, y0, cell, n) = (v[0], v[1], v[2], v[3] as usize);
            let gap = 2.0;
            if (cell + gap) * n as f64 > 40.0 {
                return Err("grid: la cuadrícula pasa de 40 mm".into());
            }
            let mut doc = tlaku_doc::Document::default();
            for row in 0..n {
                for col in 0..n {
                    let k = row * n + col;
                    let color = Rgb((10 + k * 9) as u8, (200 - k * 5) as u8, (40 + k * 3) as u8);
                    let mut sq = square_doc(
                        x0 + col as f64 * (cell + gap),
                        y0 + row as f64 * (cell + gap),
                        cell,
                    );
                    let mut obj = sq.objects.remove(0);
                    obj.id = format!("g{k}");
                    obj.layer = color;
                    doc.objects.push(obj);
                    let mut l = tlaku_doc::LayerSettings::for_color(color);
                    let t = |a: f64, b: f64, i: usize| a + (b - a) * i as f64 / (n - 1) as f64;
                    l.power = t(v[4], v[5], col);
                    l.speed = t(v[6], v[7], row);
                    l.min_power = 0.0;
                    l.passes = 1;
                    l.air = false;
                    l.mode = tlaku_doc::LayerMode::Fill;
                    l.interval = 0.1;
                    doc.layers.push(l);
                }
            }
            Ok(doc)
        }
        "image" => {
            // image:<x>,<y>,<lado>,<vel>,<pot>; el archivo sale de TLAKU_HW_IMAGE (PNG en escala de grises).
            if v.len() != 5 || v[2] <= 0.0 || v[2] > 40.0 || v[3] <= 0.0 || !pct_ok(v[4]) {
                return Err(format!(
                    "image: argumentos fuera de rango (lado ≤ 40 mm, potencia ≤ {MAX_JOB_PCT} %)"
                ));
            }
            let path = std::env::var("TLAKU_HW_IMAGE")
                .map_err(|_| "image: falta TLAKU_HW_IMAGE".to_string())?;
            let (w, h) = image::image_dimensions(&path).map_err(|e| format!("image: {e}"))?;
            let mut layer = tlaku_doc::LayerSettings::for_images();
            layer.speed = v[3];
            layer.power = v[4];
            layer.min_power = 0.0;
            layer.passes = 1;
            layer.air = false;
            layer.interval = 0.1;
            let obj = tlaku_doc::DesignObject {
                id: "img".into(),
                layer: tlaku_doc::LayerSettings::IMAGE_COLOR,
                transform: tlaku_doc::affine::translate(v[0], v[1]),
                kind: tlaku_doc::ObjectKind::Image(tlaku_doc::ImageObject {
                    source_path: path,
                    width_px: w,
                    height_px: h,
                    size_mm: [v[2], v[2] * h as f64 / w as f64],
                    mask: Vec::new(),
                    variable: None,
                }),
                locked: false,
                hidden: false,
                group: None,
                priority: None,
                power_scale: None,
                shape: None,
                start: None,
                manual_tabs: Vec::new(),
                array: None,
            };
            Ok(tlaku_doc::Document {
                objects: vec![obj],
                layers: vec![layer],
                ..tlaku_doc::Document::default()
            })
        }
        _ => Err(format!("paso desconocido: {kind}")),
    }
}

/// Revisa que ninguna `S` pase de `max` y que no haya `M3`; devuelve la S más alta.
fn power_at_most(lines: &[String], max: f64) -> Result<f64, String> {
    let mut top: f64 = 0.0;
    for line in lines {
        let code = line.split(';').next().unwrap_or("").to_ascii_uppercase();
        if code.contains("M3") {
            return Err(line.clone());
        }
        let b = code.as_bytes();
        for i in 0..b.len() {
            if b[i] == b'S' {
                let rest: String = code[i + 1..]
                    .chars()
                    .take_while(|c| c.is_ascii_digit() || *c == '.')
                    .collect();
                match rest.parse::<f64>() {
                    Ok(v) if v <= max => top = top.max(v),
                    _ => return Err(line.clone()),
                }
            }
        }
    }
    Ok(top)
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
