//! Pruebas del servidor remoto de punta a punta: HTTP real sobre `127.0.0.1`, con el doble del
//! núcleo para las reglas y con la máquina virtual (`virtual:falcon2`) para pausar y detener de verdad.

use super::auth::{MAX_FAILS, PRESENT_WINDOW};
use super::hub::testing::Fake;
use super::hub::{
    Control, Core, JobPhase, MachineInfo, MachineKind, Snapshot, StartOutcome, StartRequest,
};
use super::net::LocalAddr;
use super::server::Server;
use super::{RemoteConfig, RemoteState, qr_svg};
use crate::machine::active::Machine;
use crate::machine::{Connection, Emit, Sink, Timing};
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tlaku_doc::MachineState;

// ---------------------------------------------------------------------------
// Cliente HTTP mínimo
// ---------------------------------------------------------------------------

struct Resp {
    status: u16,
    head: String,
    body: String,
}

impl Resp {
    fn header(&self, name: &str) -> Option<String> {
        self.head.lines().skip(1).find_map(|l| {
            let (k, v) = l.split_once(':')?;
            k.trim()
                .eq_ignore_ascii_case(name)
                .then(|| v.trim().to_string())
        })
    }

    fn json(&self) -> serde_json::Value {
        serde_json::from_str(&self.body)
            .unwrap_or_else(|e| panic!("no es JSON ({e}): {}", self.body))
    }
}

fn send(addr: SocketAddr, method: &str, path: &str, headers: &[(&str, &str)], body: &str) -> Resp {
    let mut s = TcpStream::connect(addr).unwrap();
    s.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
    let mut req = format!("{method} {path} HTTP/1.1\r\nConnection: close\r\n");
    if !headers.iter().any(|(k, _)| k.eq_ignore_ascii_case("host")) {
        req += &format!("Host: {addr}\r\n");
    }
    for (k, v) in headers {
        req += &format!("{k}: {v}\r\n");
    }
    if method == "POST" {
        req += &format!("Content-Length: {}\r\n", body.len());
    }
    req += "\r\n";
    req += body;
    s.write_all(req.as_bytes()).unwrap();
    let mut raw = Vec::new();
    let _ = s.read_to_end(&mut raw);
    let text = String::from_utf8_lossy(&raw).into_owned();
    let (head, body) = text.split_once("\r\n\r\n").unwrap_or((&text, ""));
    let status = head
        .split_whitespace()
        .nth(1)
        .and_then(|c| c.parse().ok())
        .unwrap_or(0);
    Resp {
        status,
        head: head.to_string(),
        body: body.to_string(),
    }
}

/// Como lo hace la página: con cookie y la marca de las acciones.
fn call(addr: SocketAddr, cookie: &str, method: &str, path: &str, body: &str) -> Resp {
    let origin = format!("http://{addr}");
    let cookie = format!("tlaku_session={cookie}");
    send(
        addr,
        method,
        path,
        &[
            ("Cookie", &cookie),
            ("X-Requested-With", "tlaku"),
            ("Origin", &origin),
            ("Content-Type", "application/json"),
        ],
        body,
    )
}

fn login(addr: SocketAddr, token: &str) -> Resp {
    let origin = format!("http://{addr}");
    send(
        addr,
        "POST",
        "/api/login",
        &[
            ("X-Requested-With", "tlaku"),
            ("Origin", &origin),
            ("User-Agent", "Mozilla/5.0 (iPhone) Safari/604.1"),
        ],
        &format!("{{\"token\":\"{token}\"}}"),
    )
}

/// Entra con la clave y devuelve el valor de la cookie de sesión.
fn session(addr: SocketAddr, token: &str) -> String {
    let r = login(addr, token);
    assert_eq!(r.status, 200, "{}", r.body);
    let set = r.header("set-cookie").expect("Set-Cookie");
    set.strip_prefix("tlaku_session=")
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_string()
}

fn wait_for(what: &str, secs: u64, mut f: impl FnMut() -> bool) {
    let end = Instant::now() + Duration::from_secs(secs);
    while !f() {
        assert!(Instant::now() < end, "se agotó la espera de: {what}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

// ---------------------------------------------------------------------------
// Apoyos
// ---------------------------------------------------------------------------

fn loopback() -> SocketAddr {
    SocketAddr::from(([127, 0, 0, 1], 0))
}

struct Rig {
    core: Arc<Core>,
    server: Server,
    addr: SocketAddr,
}

fn rig(control: Arc<dyn Control>) -> Rig {
    let core = Arc::new(Core::new(control));
    let server = Server::start(core.clone(), loopback()).unwrap();
    let addr = server.addr();
    Rig { core, server, addr }
}

fn ready_snapshot() -> Snapshot {
    Snapshot {
        doc_name: "placa.tlaku".into(),
        device_name: "Falcon2".into(),
        bed: Some([400.0, 400.0]),
        has_output: true,
        framed: true,
        ..Snapshot::default()
    }
}

// ---------------------------------------------------------------------------
// Página y encabezados
// ---------------------------------------------------------------------------

#[test]
fn the_page_is_public_static_and_has_no_cors() {
    let r = rig(Fake::with(MachineKind::Grbl, MachineState::Idle));
    let page = send(r.addr, "GET", "/", &[], "");
    assert_eq!(page.status, 200);
    assert!(page.body.contains("<title>Tlaku</title>"));
    // Sin dependencias externas: ninguna URL absoluta en la página.
    // (el único "http://" permitido es el espacio de nombres del SVG, que no es una descarga)
    let external = page.body.replace("http://www.w3.org/2000/svg", "");
    assert!(
        !external.contains("http://")
            && !external.contains("https://")
            && !external.contains("//cdn"),
        "la página no debe cargar nada de fuera"
    );
    assert!(
        !external.contains("@import") && !external.contains("<link"),
        "sin hojas de estilo externas"
    );
    assert_eq!(
        page.header("content-type").unwrap(),
        "text/html; charset=utf-8"
    );
    assert!(
        page.header("content-security-policy")
            .unwrap()
            .contains("default-src 'none'")
    );
    assert_eq!(page.header("x-content-type-options").unwrap(), "nosniff");
    assert_eq!(page.header("cache-control").unwrap(), "no-store");
    // Nunca se emiten encabezados CORS, ni siquiera ante una pre-petición.
    let pre = send(
        r.addr,
        "OPTIONS",
        "/api/stop",
        &[
            ("Origin", "http://evil.example"),
            ("Access-Control-Request-Method", "POST"),
        ],
        "",
    );
    assert!(pre.header("access-control-allow-origin").is_none());
    assert!(pre.status >= 400, "{}", pre.status);
    assert!(page.header("access-control-allow-origin").is_none());
}

#[test]
fn the_page_keeps_pause_and_stop_in_a_fixed_bar_in_both_languages() {
    let r = rig(Fake::with(MachineKind::Grbl, MachineState::Idle));
    let page = send(r.addr, "GET", "/", &[], "").body;
    // La barra con Pausar/Detener va fija abajo, con la zona segura del iPhone, y fuera del contenido.
    assert!(
        page.contains(".dock {\n  position: fixed;")
            && page.contains("env(safe-area-inset-bottom)")
    );
    let bar = page.find("id=\"bar\"").unwrap();
    let main_end = page.find("</main>").unwrap();
    assert!(
        bar > main_end,
        "la barra no debe estar dentro del contenido que se desplaza"
    );
    for id in ["btn-pause", "btn-stop", "stop-hint"] {
        assert!(page[bar..].contains(&format!("id=\"{id}\"")), "{id}");
    }
    // El aviso del segundo toque existe en español y en inglés.
    assert!(
        page.contains("stopHint: 'Toca Detener otra vez")
            && page.contains("stopHint: 'Tap Stop again")
    );
}

#[test]
fn the_page_declares_language_zoom_and_landmarks() {
    let r = rig(Fake::with(MachineKind::Grbl, MachineState::Idle));
    let page = send(r.addr, "GET", "/", &[], "").body;
    assert!(page.contains("<html lang=\"es\">"));
    // El zoom del usuario nunca se bloquea.
    let viewport = page.find("name=\"viewport\"").unwrap();
    let tag = &page[viewport..viewport + page[viewport..].find('>').unwrap()];
    assert!(
        !tag.contains("user-scalable") && !tag.contains("maximum-scale"),
        "{tag}"
    );
    // Un solo h1, un <main> y la barra de abajo como región con nombre (en los dos idiomas).
    assert_eq!(page.matches("<h1").count(), 1);
    assert!(page.contains("<main id=\"main\"") && page.contains("<header>"));
    assert!(
        page.contains("controls: 'Controles del trabajo'")
            && page.contains("controls: 'Job controls'")
    );
}

#[test]
fn the_page_announces_status_progress_and_errors_to_screen_readers() {
    let r = rig(Fake::with(MachineKind::Grbl, MachineState::Idle));
    let page = send(r.addr, "GET", "/", &[], "").body;
    // Estado del trabajo en una región en vivo cortés; el avance como barra con valor, sin región en vivo.
    assert!(page.contains("class=\"state\" role=\"status\" aria-live=\"polite\""));
    assert!(page.contains("id=\"progress\" role=\"progressbar\""));
    assert!(page.contains("aria-valuemin=\"0\"") && page.contains("aria-valuemax=\"100\""));
    assert!(page.contains("setAttribute('aria-valuenow'"));
    let bar = page.find("id=\"progress\"").unwrap();
    assert!(
        !page[bar..bar + 200].contains("aria-live"),
        "el avance no debe anunciarse en cada tick"
    );
    // Errores y avisos urgentes en role=alert, incluido el segundo toque de Detener.
    for id in ["offline", "stop-hint", "sheet-err"] {
        let at = page.find(&format!("id=\"{id}\"")).unwrap();
        let tag = &page[at..at + page[at..].find('>').unwrap()];
        assert!(tag.contains("role=\"alert\""), "{id}: {tag}");
    }
    // Detener tiene nombre accesible (texto) en los dos idiomas y la barra sigue fija con el mismo id.
    assert!(page.contains("stop: 'Detener'") && page.contains("stop: 'Stop'"));
    assert!(page.contains("<button type=\"button\" id=\"btn-stop\" class=\"danger\">"));
    assert!(page.contains("role=\"alertdialog\"") && page.contains("aria-modal=\"true\""));
}

#[test]
fn the_page_respects_reduced_motion_and_shows_keyboard_focus() {
    let r = rig(Fake::with(MachineKind::Grbl, MachineState::Idle));
    let page = send(r.addr, "GET", "/", &[], "").body;
    let rm = page
        .find("@media (prefers-reduced-motion: reduce)")
        .expect("falta la regla de movimiento reducido");
    let block = &page[rm..rm + 260];
    assert!(block.contains("animation: none") && block.contains("transition: none"));
    assert!(page.contains(":focus-visible { outline: 3px solid var(--focus)"));
    // Objetivo táctil de Detener de al menos 44 px (la barra usa 62).
    assert!(
        page.contains("min-width: 44px") && page.contains(".dock button { min-height: 62px; }")
    );
}

#[test]
fn the_host_header_must_be_an_ip_and_the_origin_this_server() {
    let r = rig(Fake::with(MachineKind::Grbl, MachineState::Idle));
    let evil_host = send(r.addr, "GET", "/", &[("Host", "evil.example:80")], "");
    assert_eq!(evil_host.status, 400);
    let cookie = session(r.addr, &r.core.auth.token());
    // Una acción desde otro origen se rechaza aunque lleve una cookie válida.
    let cross = send(
        r.addr,
        "POST",
        "/api/stop",
        &[
            ("Cookie", &format!("tlaku_session={cookie}")),
            ("Origin", "http://evil.example"),
            ("X-Requested-With", "tlaku"),
        ],
        "{}",
    );
    assert_eq!(cross.status, 403);
    // Sin la marca de la página (un formulario de otro sitio) tampoco.
    let unmarked = send(
        r.addr,
        "POST",
        "/api/stop",
        &[("Cookie", &format!("tlaku_session={cookie}"))],
        "{}",
    );
    assert_eq!(unmarked.status, 403);
    assert!(r.core.control().machine().is_some());
}

// ---------------------------------------------------------------------------
// Autenticación
// ---------------------------------------------------------------------------

#[test]
fn every_api_route_refuses_requests_without_a_valid_token() {
    let fake = Fake::with(MachineKind::Grbl, MachineState::Run);
    let r = rig(fake.clone());
    for (m, p) in [
        ("GET", "/api/state"),
        ("GET", "/api/events"),
        ("GET", "/api/route"),
        ("GET", "/api/live"),
        ("GET", "/api/camera.jpg"),
        ("POST", "/api/pause"),
        ("POST", "/api/resume"),
        ("POST", "/api/stop"),
        ("POST", "/api/start"),
    ] {
        let none = call(r.addr, "", m, p, "{}");
        assert_eq!(none.status, 401, "{m} {p} sin sesión");
        let fake_cookie = call(r.addr, "cookie-inventada", m, p, "{}");
        assert_eq!(fake_cookie.status, 401, "{m} {p} con cookie falsa");
    }
    // Con clave mala por encabezado (pocas veces: pasado el límite la dirección queda bloqueada).
    for (m, p) in [
        ("GET", "/api/state"),
        ("POST", "/api/stop"),
        ("POST", "/api/start"),
    ] {
        let bad = send(
            r.addr,
            m,
            p,
            &[
                ("X-Tlaku-Token", "clave-mala"),
                ("X-Requested-With", "tlaku"),
            ],
            "{}",
        );
        assert_eq!(bad.status, 401, "{m} {p} con clave mala");
    }
    // Nada de eso llegó a la máquina.
    assert!(fake.calls().is_empty(), "{:?}", fake.calls());
}

#[test]
fn login_gives_a_session_and_the_token_header_also_works() {
    let fake = Fake::with(MachineKind::Grbl, MachineState::Idle);
    let r = rig(fake);
    let token = r.core.auth.token();
    assert_eq!(login(r.addr, "mala").status, 401);
    let ok = login(r.addr, &token);
    assert_eq!(ok.status, 200);
    let set = ok.header("set-cookie").unwrap();
    assert!(
        set.contains("HttpOnly") && set.contains("SameSite=Strict"),
        "{set}"
    );
    assert!(!set.contains(&token), "la cookie no repite la clave");
    let cookie = session(r.addr, &token);
    let st = call(r.addr, &cookie, "GET", "/api/state", "");
    assert_eq!(st.status, 200);
    assert_eq!(st.json()["v"], 1);
    // La clave en un encabezado vale en cada petición, sin cookie.
    let hdr = send(
        r.addr,
        "GET",
        "/api/state",
        &[("Authorization", &format!("Bearer {token}"))],
        "",
    );
    assert_eq!(hdr.status, 200);
    let hdr = send(
        r.addr,
        "GET",
        "/api/state",
        &[("X-Tlaku-Token", &token)],
        "",
    );
    assert_eq!(hdr.status, 200);
}

#[test]
fn repeated_wrong_tokens_lock_the_client_out_even_with_the_right_one() {
    let r = rig(Fake::with(MachineKind::Grbl, MachineState::Idle));
    let token = r.core.auth.token();
    for _ in 0..MAX_FAILS {
        assert_eq!(login(r.addr, "adivinando").status, 401);
    }
    let locked = login(r.addr, &token);
    assert_eq!(locked.status, 429);
    assert!(
        locked
            .header("retry-after")
            .unwrap()
            .parse::<u64>()
            .unwrap()
            > 0
    );
    // El bloqueo alcanza también a la clave en encabezado.
    let hdr = send(
        r.addr,
        "GET",
        "/api/state",
        &[("X-Tlaku-Token", &token)],
        "",
    );
    assert_eq!(hdr.status, 429);
}

#[test]
fn regenerating_the_token_expels_connected_clients() {
    let r = rig(Fake::with(MachineKind::Grbl, MachineState::Idle));
    let old = r.core.auth.token();
    let cookie = session(r.addr, &old);
    assert_eq!(call(r.addr, &cookie, "GET", "/api/state", "").status, 200);
    let new = r.core.auth.regenerate();
    assert_ne!(old, new);
    assert_eq!(
        call(r.addr, &cookie, "GET", "/api/state", "").status,
        401,
        "la sesión vieja ya no vale"
    );
    assert_eq!(login(r.addr, &old).status, 401, "la clave vieja ya no vale");
    assert_eq!(login(r.addr, &new).status, 200);
}

#[test]
fn kicking_a_client_ends_its_session_and_its_live_stream() {
    let r = rig(Fake::with(MachineKind::Grbl, MachineState::Idle));
    let cookie = session(r.addr, &r.core.auth.token());
    let victim = r.core.auth.clients(Instant::now())[0].id.clone();
    let other = session(r.addr, &r.core.auth.token());
    // Conexión en vivo abierta con la primera sesión.
    let mut s = TcpStream::connect(r.addr).unwrap();
    s.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
    write!(
        s,
        "GET /api/events HTTP/1.1\r\nHost: {}\r\nCookie: tlaku_session={cookie}\r\nAccept: text/event-stream\r\nConnection: close\r\n\r\n",
        r.addr
    )
    .unwrap();
    let mut seen = String::new();
    let mut buf = [0u8; 4096];
    while !seen.contains("event: state") || !seen.contains("\n\n") {
        let n = s.read(&mut buf).unwrap();
        assert!(
            n > 0,
            "la conexión se cerró antes del primer evento: {seen}"
        );
        seen += &String::from_utf8_lossy(&buf[..n]);
    }
    assert!(seen.contains("text/event-stream"), "{seen}");
    assert!(seen.contains("\"v\":1"));
    assert_eq!(r.core.auth.clients(Instant::now()).len(), 2);

    // La PC desconecta solo al primero: su conexión en vivo se cierra y su cookie deja de valer.
    assert!(r.core.auth.kick(&victim));
    r.core.bump();
    let mut rest = Vec::new();
    let _ = s.read_to_end(&mut rest);
    assert_eq!(call(r.addr, &cookie, "GET", "/api/state", "").status, 401);
    // El otro cliente sigue entrando.
    assert_eq!(call(r.addr, &other, "GET", "/api/state", "").status, 200);
    assert_eq!(r.core.auth.clients(Instant::now()).len(), 1);
}

#[test]
fn clients_are_listed_with_their_address_and_device() {
    let fake = Fake::with(MachineKind::Grbl, MachineState::Idle);
    let r = rig(fake.clone());
    let _ = session(r.addr, &r.core.auth.token());
    let list = r.core.auth.clients(Instant::now());
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].ip, "127.0.0.1");
    assert_eq!(list[0].agent, "iPhone · Safari");
    assert!(list[0].connected);
    // La PC se entera del cambio.
    assert!(
        fake.calls().iter().any(|c| c == "clients 1"),
        "{:?}",
        fake.calls()
    );
    let later = Instant::now() + PRESENT_WINDOW + Duration::from_secs(5);
    assert!(!r.core.auth.clients(later)[0].connected);
}

// ---------------------------------------------------------------------------
// Iniciar
// ---------------------------------------------------------------------------

fn start_body(nearby: bool, clear: bool, air: bool) -> String {
    format!("{{\"checklist\":{{\"nearby\":{nearby},\"clear\":{clear},\"air\":{air}}}}}")
}

#[test]
fn the_phone_sees_the_queue_and_confirms_a_step_only_with_the_material_item() {
    use super::hub::{QueueInfo, QueueItemInfo};
    let fake = Fake::with(MachineKind::Grbl, MachineState::Idle);
    let r = rig(fake.clone());
    let cookie = session(r.addr, &r.core.auth.token());
    let go = |body: &str| call(r.addr, &cookie, "POST", "/api/start", body);
    let item = QueueItemInfo {
        name: "caja.tlaku".to_string(),
        copies: 3,
        done: 1,
        status: "pending".to_string(),
    };
    r.core.update(
        Snapshot {
            queue: Some(QueueInfo {
                state: "awaiting".to_string(),
                done: 1,
                total: 3,
                copy: 2,
                current: Some(item.clone()),
                items: vec![item],
                ..QueueInfo::default()
            }),
            ..ready_snapshot()
        },
        Instant::now(),
    );
    // La cola se ve en el estado (solo lectura).
    let state = call(r.addr, &cookie, "GET", "/api/state", "");
    assert_eq!(state.json()["queue"]["state"], "awaiting");
    assert_eq!(state.json()["queue"]["current"]["name"], "caja.tlaku");
    assert_eq!(state.json()["queue"]["copy"], 2);
    // Con las tres casillas de siempre no se confirma el siguiente trabajo: falta «cambié el material».
    let denied = go(&start_body(true, true, true));
    assert_eq!(denied.status, 409);
    assert_eq!(denied.json()["code"], "checklist_material");
    assert!(
        !fake.calls().iter().any(|c| c.starts_with("ask_start")),
        "{:?}",
        fake.calls()
    );
    // Con la cuarta, el pedido llega a la interfaz (que vuelve a comprobar todo).
    let core = r.core.clone();
    let answerer = std::thread::spawn(move || {
        wait_for("pedido de inicio", 5, || {
            fake.calls().iter().any(|c| c.starts_with("ask_start"))
        });
        let id: u64 = fake
            .calls()
            .iter()
            .find_map(|c| c.strip_prefix("ask_start ")?.parse().ok())
            .unwrap();
        core.finish_start(
            id,
            StartOutcome {
                ok: true,
                code: String::new(),
                message: String::new(),
            },
        );
    });
    let ok = go("{\"checklist\":{\"nearby\":true,\"clear\":true,\"air\":true,\"material\":true}}");
    answerer.join().unwrap();
    assert_eq!(ok.status, 200, "{}", ok.body);
}

#[test]
fn starting_needs_the_phone_checklist_and_a_prior_frame() {
    let fake = Fake::with(MachineKind::Grbl, MachineState::Idle);
    let r = rig(fake.clone());
    let cookie = session(r.addr, &r.core.auth.token());
    let go = |body: &str| call(r.addr, &cookie, "POST", "/api/start", body);

    // La PC todavía no informó nada.
    assert_eq!(
        go(&start_body(true, true, true)).json()["code"],
        "pc_offline"
    );

    // Sin encuadre previo.
    r.core.update(
        Snapshot {
            framed: false,
            ..ready_snapshot()
        },
        Instant::now(),
    );
    let denied = go(&start_body(true, true, true));
    assert_eq!(denied.status, 409);
    assert_eq!(denied.json()["code"], "not_framed");

    // Encuadrado, pero con el checklist incompleto o sin él.
    r.core.update(ready_snapshot(), Instant::now());
    for body in [
        start_body(true, true, false),
        start_body(false, true, true),
        start_body(true, false, true),
        "{}".to_string(),
        "{\"checklist\":{}}".to_string(),
    ] {
        let r2 = go(&body);
        assert_eq!(r2.status, 409, "{body}");
        assert_eq!(r2.json()["code"], "checklist", "{body}");
    }
    assert!(
        !fake.calls().iter().any(|c| c.starts_with("ask_start")),
        "{:?}",
        fake.calls()
    );

    // Máquina ocupada.
    fake.set(Some((MachineKind::Grbl, MachineState::Hold)));
    assert_eq!(go(&start_body(true, true, true)).json()["code"], "not_idle");
    fake.set(Some((MachineKind::Grbl, MachineState::Idle)));

    // Todo en regla: el pedido llega a la interfaz y su respuesta vuelve al celular.
    let core = r.core.clone();
    let answerer = std::thread::spawn(move || {
        wait_for("pedido de inicio", 5, || {
            fake.calls().iter().any(|c| c.starts_with("ask_start"))
        });
        let id: u64 = fake
            .calls()
            .iter()
            .find_map(|c| c.strip_prefix("ask_start ")?.parse().ok())
            .unwrap();
        core.finish_start(
            id,
            StartOutcome {
                ok: true,
                code: String::new(),
                message: String::new(),
            },
        );
    });
    let ok = go(&start_body(true, true, true));
    answerer.join().unwrap();
    assert_eq!(ok.status, 200, "{}", ok.body);
    assert_eq!(ok.json()["ok"], true);
}

#[test]
fn the_interface_can_refuse_a_start_and_the_phone_hears_why() {
    let fake = Fake::with(MachineKind::Grbl, MachineState::Idle);
    let r = rig(fake.clone());
    let cookie = session(r.addr, &r.core.auth.token());
    r.core.update(ready_snapshot(), Instant::now());
    let core = r.core.clone();
    let f2 = fake.clone();
    let t = std::thread::spawn(move || {
        wait_for("pedido", 5, || {
            f2.calls().iter().any(|c| c.starts_with("ask_start"))
        });
        core.finish_start(
            1,
            StartOutcome {
                ok: false,
                code: "not_framed".into(),
                message: "Cambió el diseño; encuadra otra vez.".into(),
            },
        );
    });
    let denied = call(
        r.addr,
        &cookie,
        "POST",
        "/api/start",
        &start_body(true, true, true),
    );
    t.join().unwrap();
    assert_eq!(denied.status, 409);
    assert_eq!(denied.json()["code"], "not_framed");
    assert!(
        denied.json()["message"]
            .as_str()
            .unwrap()
            .contains("encuadra")
    );
    // Tras la negativa se puede volver a intentar.
    assert_eq!(r.core.start_block(Instant::now()), None);
}

#[test]
fn the_state_explains_why_start_is_disabled() {
    let fake = Fake::with(MachineKind::Grbl, MachineState::Idle);
    let r = rig(fake);
    let cookie = session(r.addr, &r.core.auth.token());
    r.core.update(
        Snapshot {
            framed: false,
            ..ready_snapshot()
        },
        Instant::now(),
    );
    let v = call(r.addr, &cookie, "GET", "/api/state", "").json();
    assert_eq!(v["start"]["allowed"], false);
    assert_eq!(v["start"]["reason"], "not_framed");
    assert!(v["start"]["message"].as_str().unwrap().contains("encuadra"));
    r.core.update(ready_snapshot(), Instant::now());
    let v = call(r.addr, &cookie, "GET", "/api/state", "").json();
    assert_eq!(v["start"]["allowed"], true);
    assert_eq!(v["doc"], "placa.tlaku");
}

// ---------------------------------------------------------------------------
// Detener, pausar y reanudar con la máquina virtual
// ---------------------------------------------------------------------------

struct Quiet;

impl Sink for Quiet {
    fn emit(&self, _event: Emit) {}
}

/// Control sobre una máquina virtual de verdad (la misma conexión que usa la app).
struct Virtual {
    conn: Mutex<Option<Connection>>,
    asked: Mutex<Vec<u64>>,
}

impl Virtual {
    fn new(port: &str) -> Arc<Self> {
        let (conn, _) =
            Connection::start_virtual(port, Arc::new(Quiet), Timing::default()).expect("conectar");
        let me = Arc::new(Self {
            conn: Mutex::new(Some(conn)),
            asked: Mutex::default(),
        });
        wait_for("estado inicial", 5, || me.state().is_some());
        me
    }

    fn handle(&self) -> Option<crate::machine::Handle> {
        self.conn.lock().unwrap().as_ref().map(Connection::handle)
    }

    fn state(&self) -> Option<MachineState> {
        self.handle()?.status_report().map(|s| s.state)
    }

    fn machine(&self) -> Option<Machine> {
        self.handle().map(Machine::Grbl)
    }

    /// Un trabajo largo (unos 24 s a ×1, ~1,2 s a ×20).
    fn start_long_job(&self) {
        let lines = ["G90", "G21", "M4", "G1 X200 Y0 S500 F500", "M5"]
            .map(String::from)
            .to_vec();
        self.handle().unwrap().start_job(lines, 127).unwrap();
        wait_for("trabajo en marcha", 5, || {
            self.state() == Some(MachineState::Run)
        });
    }

    /// Esperar a que la máquina deje de trabajar: tras detener, GRBL queda en reposo o, con el
    /// reinicio, en alarma de "falta referenciar" (es lo mismo que pasa con el botón de la PC).
    fn wait_stopped(&self) {
        wait_for("detenida", 10, || {
            matches!(self.state(), Some(MachineState::Idle | MachineState::Alarm))
        });
    }

    fn disconnect(&self) {
        if let Some(c) = self.conn.lock().unwrap().take() {
            c.close();
        }
    }
}

impl Control for Virtual {
    fn machine(&self) -> Option<MachineInfo> {
        let h = self.handle()?;
        Some(MachineInfo {
            kind: MachineKind::Grbl,
            is_virtual: h.is_virtual(),
            status: h.status_report(),
        })
    }
    fn pause(&self) -> Result<(), String> {
        self.machine().ok_or("sin máquina")?.pause()
    }
    fn resume(&self) -> Result<(), String> {
        self.machine().ok_or("sin máquina")?.resume()
    }
    fn stop(&self) -> Result<(), String> {
        self.machine().ok_or("sin máquina")?.cancel()
    }
    fn ask_start(&self, request: StartRequest) {
        self.asked.lock().unwrap().push(request.id);
    }
    fn notify_action(&self, _action: &str, _client: &str) {}
    fn clients_changed(&self, _clients: &[super::auth::ClientView]) {}
}

#[test]
fn pause_resume_and_stop_drive_the_virtual_machine() {
    let v = Virtual::new("virtual:falcon2@20");
    let r = rig(v.clone());
    let cookie = session(r.addr, &r.core.auth.token());
    v.start_long_job();

    let pause = call(r.addr, &cookie, "POST", "/api/pause", "");
    assert_eq!(pause.status, 200, "{}", pause.body);
    wait_for("Hold", 5, || v.state() == Some(MachineState::Hold));

    let resume = call(r.addr, &cookie, "POST", "/api/resume", "");
    assert_eq!(resume.status, 200, "{}", resume.body);
    wait_for("de nuevo en marcha", 5, || {
        v.state() == Some(MachineState::Run)
    });

    let stop = call(r.addr, &cookie, "POST", "/api/stop", "");
    assert_eq!(stop.status, 200, "{}", stop.body);
    v.wait_stopped();
    v.disconnect();
}

#[test]
fn stop_works_even_when_nothing_else_is_allowed() {
    // La PC no informa nada, sin encuadre, sin checklist, la ventana "congelada": detener va directo al motor.
    let v = Virtual::new("virtual:falcon2@20");
    let r = rig(v.clone());
    let cookie = session(r.addr, &r.core.auth.token());
    assert_eq!(
        call(r.addr, &cookie, "GET", "/api/state", "").json()["start"]["allowed"],
        false
    );
    v.start_long_job();
    // Mientras tanto, iniciar de nuevo está vedado...
    assert_eq!(
        call(
            r.addr,
            &cookie,
            "POST",
            "/api/start",
            &start_body(true, true, true)
        )
        .status,
        409
    );
    // ...pero detener se atiende.
    assert_eq!(call(r.addr, &cookie, "POST", "/api/stop", "").status, 200);
    v.wait_stopped();
    // Y detener cuando no hay nada en marcha tampoco falla ni deja la máquina rara.
    assert_eq!(call(r.addr, &cookie, "POST", "/api/stop", "").status, 200);
    v.disconnect();
    // Sin máquina conectada, el motivo es claro.
    let none = call(r.addr, &cookie, "POST", "/api/stop", "");
    assert_eq!(none.status, 409);
    assert_eq!(none.json()["code"], "not_connected");
}

#[test]
fn the_galvo_has_no_pause_but_can_always_be_stopped() {
    let fake = Fake::with(MachineKind::Galvo, MachineState::Run);
    let r = rig(fake.clone());
    let cookie = session(r.addr, &r.core.auth.token());
    let pause = call(r.addr, &cookie, "POST", "/api/pause", "");
    assert_eq!(
        (pause.status, pause.json()["code"].as_str().unwrap()),
        (409, "no_pause")
    );
    assert_eq!(
        call(r.addr, &cookie, "POST", "/api/resume", "").json()["code"],
        "no_pause"
    );
    assert_eq!(call(r.addr, &cookie, "POST", "/api/stop", "").status, 200);
    assert_eq!(fake.calls().iter().filter(|c| *c == "stop").count(), 1);
    let v = call(r.addr, &cookie, "GET", "/api/state", "").json();
    assert_eq!(v["caps"]["pause"], false);
    assert_eq!(v["caps"]["stop"], true);
    // Y un galvo no se inicia desde el celular.
    r.core.update(ready_snapshot(), Instant::now());
    fake.set(Some((MachineKind::Galvo, MachineState::Idle)));
    assert_eq!(
        call(
            r.addr,
            &cookie,
            "POST",
            "/api/start",
            &start_body(true, true, true)
        )
        .json()["code"],
        "galvo"
    );
}

// ---------------------------------------------------------------------------
// Datos que informa la interfaz
// ---------------------------------------------------------------------------

#[test]
fn route_and_camera_are_served_to_authenticated_clients_only() {
    let r = rig(Fake::with(MachineKind::Grbl, MachineState::Idle));
    let cookie = session(r.addr, &r.core.auth.token());
    assert_eq!(
        call(r.addr, &cookie, "GET", "/api/camera.jpg", "").status,
        404
    );
    r.core
        .set_route(Some(super::hub::RouteIn {
            bed: [400.0, 300.0],
            d: "M0 0L10 10".into(),
        }))
        .unwrap();
    let route = call(r.addr, &cookie, "GET", "/api/route", "").json();
    assert_eq!(
        (route["version"].as_u64(), route["d"].as_str()),
        (Some(1), Some("M0 0L10 10"))
    );
    let jpeg = vec![0xff, 0xd8, 0xff, 0xe0, 0, 1, 2, 3];
    assert_eq!(r.core.set_camera(jpeg.clone(), Instant::now()), Ok(true));
    let img = call(r.addr, &cookie, "GET", "/api/camera.jpg", "");
    assert_eq!(img.status, 200);
    assert_eq!(img.header("content-type").unwrap(), "image/jpeg");
    assert_eq!(call(r.addr, "", "GET", "/api/camera.jpg", "").status, 401);
    let st = call(r.addr, &cookie, "GET", "/api/state", "").json();
    assert_eq!(st["camera_v"], 1);
    assert_eq!(st["route_v"], 1);
}

fn live_run(t0: f64) -> super::live::LiveRun {
    super::live::LiveRun {
        k: 0,
        l: 0,
        w: 0.3,
        p: vec![10.0, 10.0, t0, 50.0, 10.0, t0 + 4.0, 50.0, 40.0, t0 + 7.0],
    }
}

fn live_route(runs: Vec<super::live::LiveRun>) -> super::live::LiveRouteIn {
    super::live::LiveRouteIn {
        bed: [400.0, 300.0],
        total: 20.0,
        layers: vec!["#ff0000".into()],
        runs,
        decimated: false,
    }
}

#[test]
fn live_view_is_read_only_and_served_to_authenticated_clients_only() {
    let fake = Fake::with(MachineKind::Grbl, MachineState::Run);
    let r = rig(fake.clone());
    let cookie = session(r.addr, &r.core.auth.token());
    // Sin ruta: versión 0 y nada que dibujar.
    let none = call(r.addr, &cookie, "GET", "/api/live", "").json();
    assert_eq!(
        (
            none["version"].as_u64(),
            none["runs"].as_array().map(Vec::len)
        ),
        (Some(0), Some(0))
    );
    let st = call(r.addr, &cookie, "GET", "/api/state", "").json();
    assert_eq!(st["live_v"], 0);
    assert!(st["live"].is_null());

    r.core
        .set_live(Some(live_route(vec![live_run(0.0), live_run(10.0)])))
        .unwrap();
    let live = call(r.addr, &cookie, "GET", "/api/live", "").json();
    assert_eq!(live["version"], 1);
    assert_eq!(live["runs"].as_array().unwrap().len(), 2);
    assert_eq!(live["layers"][0], "#ff0000");
    // Sin sesión no se sirve.
    assert_eq!(call(r.addr, "", "GET", "/api/live", "").status, 401);
    // No hay ninguna acción en esa ruta: solo se puede leer.
    for m in ["POST", "PUT", "DELETE"] {
        let resp = call(r.addr, &cookie, m, "/api/live", "{}");
        assert!(
            resp.status == 405 || resp.status == 404,
            "{m}: {}",
            resp.status
        );
    }
    // Lo único que pasó con la máquina fue el aviso de que entró un cliente.
    assert!(
        fake.calls().iter().all(|c| c.starts_with("clients")),
        "{:?}",
        fake.calls()
    );

    // La ubicación viaja en el estado y se acota.
    let mut snap = ready_snapshot();
    snap.job_state = JobPhase::Running;
    snap.live = Some(super::live::LiveReport {
        t: 6.5,
        running: true,
        retreat_seq: 2,
        src: "position".into(),
    });
    r.core.update(snap, Instant::now());
    let st = call(r.addr, &cookie, "GET", "/api/state", "").json();
    assert_eq!(st["live_v"], 1);
    assert_eq!(st["live"]["t"], 6.5);
    assert_eq!(st["live"]["running"], true);
    assert_eq!(st["live"]["retreat_seq"], 2);

    // Una ruta nueva sube la versión; una inválida se rechaza y no pisa la buena.
    r.core
        .set_live(Some(live_route(vec![live_run(0.0)])))
        .unwrap();
    assert!(
        r.core
            .set_live(Some(live_route(vec![live_run(0.0), live_run(0.0)])))
            .is_err()
    );
    assert_eq!(
        call(r.addr, &cookie, "GET", "/api/live", "").json()["version"],
        2
    );
    r.core.set_live(None).unwrap();
    assert_eq!(
        call(r.addr, &cookie, "GET", "/api/live", "").json()["version"],
        0
    );
}

#[test]
fn the_phone_sees_progress_and_the_end_of_the_job() {
    let r = rig(Fake::with(MachineKind::Grbl, MachineState::Run));
    let cookie = session(r.addr, &r.core.auth.token());
    let running = Snapshot {
        job_state: JobPhase::Running,
        progress: 0.4,
        elapsed_s: 30.0,
        remaining_s: Some(45.0),
        head: Some([12.0, 34.0]),
        layer: Some(super::hub::LayerInfo {
            index: 2,
            count: 3,
            color: "#ff0000".into(),
            mode: "line".into(),
        }),
        ..ready_snapshot()
    };
    r.core.update(running.clone(), Instant::now());
    let v = call(r.addr, &cookie, "GET", "/api/state", "").json();
    assert_eq!(v["job"]["phase"], "running");
    assert_eq!(v["job"]["progress"], 0.4);
    assert_eq!(v["job"]["layer"]["index"], 2);
    assert_eq!(v["head"], serde_json::json!([12.0, 34.0]));
    r.core.update(
        Snapshot {
            job_state: JobPhase::Finished,
            progress: 1.0,
            ..running
        },
        Instant::now(),
    );
    let v = call(r.addr, &cookie, "GET", "/api/state", "").json();
    assert_eq!(v["job"]["finished_seq"], 1);
}

// ---------------------------------------------------------------------------
// Ciclo de vida
// ---------------------------------------------------------------------------

#[test]
fn dropping_the_server_frees_the_port_and_closes_connections() {
    let r = rig(Fake::with(MachineKind::Grbl, MachineState::Idle));
    let addr = r.addr;
    let cookie = session(addr, &r.core.auth.token());
    // Una conexión en vivo abierta no impide apagar.
    let mut s = TcpStream::connect(addr).unwrap();
    s.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
    write!(
        s,
        "GET /api/events HTTP/1.1\r\nHost: {addr}\r\nCookie: tlaku_session={cookie}\r\n\r\n"
    )
    .unwrap();
    let mut buf = [0u8; 256];
    let _ = s.read(&mut buf).unwrap();
    drop(r.server);
    assert!(
        TcpListener::bind(addr).is_ok(),
        "el puerto debe quedar libre"
    );
    assert!(TcpStream::connect(addr).is_err());
}

fn lo_addr() -> Vec<LocalAddr> {
    vec![LocalAddr {
        ip: "127.0.0.1".into(),
        name: "lo".into(),
        recommended: true,
    }]
}

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

#[test]
fn configuration_starts_stops_regenerates_and_reports_errors() {
    let state = RemoteState::default();
    let fake: Arc<dyn Control> = Fake::with(MachineKind::Grbl, MachineState::Idle);
    let cfg = |enabled, port, ip: Option<&str>| RemoteConfig {
        enabled,
        port,
        ip: ip.map(String::from),
    };

    // Apagado de fábrica: no escucha.
    let s = state.configure(&cfg(false, 47802, None), fake.clone(), lo_addr());
    assert!(!s.running && s.error.is_none() && s.full_url.is_none());
    // Puertos con privilegios.
    let s = state.configure(&cfg(true, 80, None), fake.clone(), lo_addr());
    assert!(!s.running && s.error.unwrap().contains("1024"));
    // Sin red local.
    let s = state.configure(&cfg(true, 47802, None), fake.clone(), Vec::new());
    assert!(!s.running && s.error.unwrap().contains("red local"));
    // Puerto ocupado por otro programa.
    let taken = TcpListener::bind("127.0.0.1:0").unwrap();
    let busy = taken.local_addr().unwrap().port();
    let s = state.configure(&cfg(true, busy, None), fake.clone(), lo_addr());
    assert!(!s.running && s.error.unwrap().contains("ya lo usa otro programa"));
    drop(taken);

    // Se enciende: el QR lleva la clave en el fragmento.
    let port = free_port();
    let s = state.configure(&cfg(true, port, None), fake.clone(), lo_addr());
    assert!(s.running, "{:?}", s.error);
    assert_eq!(
        s.url.as_deref(),
        Some(format!("http://127.0.0.1:{port}/").as_str())
    );
    let full = s.full_url.clone().unwrap();
    assert!(full.starts_with(&format!("http://127.0.0.1:{port}/#")));
    let token = full.split_once('#').unwrap().1.to_string();
    assert_eq!(token.len(), 32);
    assert!(s.qr_svg.unwrap().starts_with("<svg"));
    let addr: SocketAddr = format!("127.0.0.1:{port}").parse().unwrap();
    assert_eq!(login(addr, &token).status, 200);

    // Reaplicar lo mismo no reinicia ni cambia la clave.
    let again = state.configure(&cfg(true, port, None), fake.clone(), lo_addr());
    assert_eq!(again.full_url.as_deref(), Some(full.as_str()));
    assert_eq!(again.clients.len(), 1);

    // Regenerar: clave nueva y los clientes quedan afuera.
    let cookie = session(addr, &token);
    let regen = state.regenerate(lo_addr());
    let new_token = regen
        .full_url
        .unwrap()
        .split_once('#')
        .unwrap()
        .1
        .to_string();
    assert_ne!(new_token, token);
    assert!(regen.clients.is_empty());
    assert_eq!(call(addr, &cookie, "GET", "/api/state", "").status, 401);
    assert_eq!(login(addr, &token).status, 401);
    assert_eq!(login(addr, &new_token).status, 200);

    // Desconectar a un cliente por su id.
    let id = state.status(lo_addr()).clients[0].id.clone();
    assert!(state.kick(&id));
    assert!(!state.kick(&id));

    // Apagar libera el puerto.
    let off = state.configure(&cfg(false, port, None), fake, lo_addr());
    assert!(!off.running);
    TcpListener::bind(addr).unwrap();
}

#[test]
fn the_server_listens_on_the_chosen_address_only() {
    let state = RemoteState::default();
    let fake: Arc<dyn Control> = Fake::with(MachineKind::Grbl, MachineState::Idle);
    let two = vec![
        LocalAddr {
            ip: "127.0.0.1".into(),
            name: "lo".into(),
            recommended: true,
        },
        LocalAddr {
            ip: "127.0.0.2".into(),
            name: "lo2".into(),
            recommended: false,
        },
    ];
    let port = free_port();
    let s = state.configure(
        &RemoteConfig {
            enabled: true,
            port,
            ip: Some("127.0.0.2".into()),
        },
        fake,
        two,
    );
    // Si el sistema no tiene 127.0.0.2 (macOS sin alias), la prueba solo comprueba el error claro.
    if s.running {
        assert_eq!(s.ip.as_deref(), Some("127.0.0.2"));
        assert!(TcpStream::connect(("127.0.0.2", port)).is_ok());
        assert!(
            TcpStream::connect(("127.0.0.1", port)).is_err(),
            "no debe escuchar en las demás"
        );
    } else {
        assert!(s.error.unwrap().contains("127.0.0.2"));
    }
}

// ---------------------------------------------------------------------------
// QR
// ---------------------------------------------------------------------------

#[test]
fn the_qr_is_a_self_contained_svg_with_a_quiet_zone() {
    let svg = qr_svg("http://192.168.1.20:47802/#0123456789abcdef0123456789abcdef").unwrap();
    assert!(svg.starts_with("<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 "));
    assert!(svg.contains("fill=\"#fff\"") && svg.contains("fill-rule=\"evenodd\""));
    // Se voltea la Y para dibujarlo (el motor de códigos entrega Y hacia arriba).
    assert!(svg.contains("scale(1 -1)"));
    assert!(!svg.contains("script"));
    // Dos QR distintos son dibujos distintos.
    assert_ne!(
        svg,
        qr_svg("http://192.168.1.20:47802/#ffffffffffffffffffffffffffffffff").unwrap()
    );
}

/// Cajas (xmin, ymin, xmax, ymax) de cada subtrayecto de las formas del QR.
fn subpath_boxes(paths: &[tlaku_doc::BezPath]) -> Vec<(f64, f64, f64, f64)> {
    use tlaku_doc::PathSeg;
    let mut boxes = Vec::new();
    for p in paths {
        let mut cur: Option<(f64, f64, f64, f64)> = None;
        for seg in &p.0 {
            let pt = match seg {
                PathSeg::MoveTo { to } => {
                    if let Some(b) = cur.take() {
                        boxes.push(b);
                    }
                    Some(*to)
                }
                PathSeg::LineTo { to }
                | PathSeg::QuadTo { to, .. }
                | PathSeg::CubicTo { to, .. } => Some(*to),
                PathSeg::Close => None,
            };
            if let Some(p) = pt {
                cur = Some(match cur {
                    None => (p.x, p.y, p.x, p.y),
                    Some((a, b, c, d)) => (a.min(p.x), b.min(p.y), c.max(p.x), d.max(p.y)),
                });
            }
        }
        if let Some(b) = cur {
            boxes.push(b);
        }
    }
    boxes
}

#[test]
fn the_qr_keeps_its_orientation_when_flipped_for_drawing() {
    // Un QR tiene los tres cuadros de posición arriba a la izquierda, arriba a la derecha y abajo a
    // la izquierda. `tlaku-codes` los entrega con Y hacia arriba (arriba = Y grande); la página los
    // dibuja con `scale(1 -1)`, que los deja en su sitio. Si alguien quita el volteo, el QR sale
    // espejado y el celular no lo lee.
    let url = "http://192.168.1.20:47802/#0123456789abcdef0123456789abcdef";
    let spec = tlaku_codes::CodeSpec {
        data: url.into(),
        size_mm: 100.0,
        margin_mm: 0.0,
        ..Default::default()
    };
    let out = tlaku_codes::generate(&tlaku_text::FontLibrary::new(), &spec).unwrap();
    let module = out.width_mm / out.modules as f64;
    let squares: Vec<_> = subpath_boxes(&out.paths)
        .into_iter()
        .filter(|(x0, y0, x1, y1)| {
            ((x1 - x0) - 7.0 * module).abs() < 0.01 && ((y1 - y0) - 7.0 * module).abs() < 0.01
        })
        .collect();
    let at = |x: f64, y: f64| {
        squares
            .iter()
            .any(|(x0, y0, _, _)| (x0 - x).abs() < 0.01 && (y0 - y).abs() < 0.01)
    };
    let far = out.width_mm - 7.0 * module;
    assert!(
        at(0.0, far),
        "falta el cuadro de arriba a la izquierda: {squares:?}"
    );
    assert!(at(far, far), "falta el cuadro de arriba a la derecha");
    assert!(at(0.0, 0.0), "falta el cuadro de abajo a la izquierda");
    assert!(
        !at(far, 0.0),
        "no debe haber un cuadro de posición abajo a la derecha"
    );
    // Y la imagen SVG voltea exactamente esa orientación.
    let svg = qr_svg(url).unwrap();
    assert!(svg.contains("scale(1 -1)"));
}

// ---------------------------------------------------------------------------
// Herramienta manual
// ---------------------------------------------------------------------------

/// JPEG de prueba (un degradado) para simular la cámara de la PC.
fn demo_jpeg() -> Vec<u8> {
    let img = image::RgbImage::from_fn(160, 90, |x, y| {
        image::Rgb([(x * 255 / 160) as u8, (y * 255 / 90) as u8, 120])
    });
    let mut out = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 70)
        .encode_image(&img)
        .unwrap();
    out
}

/// Servidor de demostración para probar la página con un navegador o un celular **sin Tauri**:
///
/// `TLAKU_DEMO_SECS=600 cargo test -p tlaku-desktop demo_server -- --ignored --nocapture`
///
/// Imprime el enlace con la clave. Hace de "ventana de la PC": informa el estado de una máquina
/// virtual (`virtual:falcon2`), un recorrido de ejemplo y una cámara falsa, y atiende los pedidos de
/// inicio (que arrancan un trabajo largo en la máquina virtual). Escucha en `TLAKU_DEMO_IP`
/// (127.0.0.1 por omisión) y el puerto `TLAKU_DEMO_PORT` (47802).
#[test]
#[ignore = "herramienta manual: deja un servidor abierto"]
fn demo_server() {
    use std::sync::atomic::{AtomicBool, Ordering};
    let secs: u64 = std::env::var("TLAKU_DEMO_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(600);
    let ip = std::env::var("TLAKU_DEMO_IP").unwrap_or_else(|_| "127.0.0.1".into());
    let port: u16 = std::env::var("TLAKU_DEMO_PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(47802);
    let v = Virtual::new("virtual:falcon2@4");
    let core = Arc::new(Core::new(v.clone()));
    let server = Server::start(core.clone(), format!("{ip}:{port}").parse().unwrap()).unwrap();
    println!(
        "\nEnlace para el celular: http://{}/#{}\n",
        server.addr(),
        core.auth.token()
    );
    let framed = Arc::new(AtomicBool::new(true));
    // Un recorrido de ejemplo: un cuadrado con una diagonal.
    core.set_route(Some(super::hub::RouteIn {
        bed: [400.0, 400.0],
        d: "M100 100L300 100L300 300L100 300L100 100L300 300".into(),
    }))
    .unwrap();
    let started = Instant::now();
    let mut asked_seen = 0;
    let mut was_running = false;
    let mut began: Option<Instant> = None;
    while started.elapsed() < Duration::from_secs(secs) {
        // Pedidos de inicio: se atienden como la ventana de verdad.
        let asked = v.asked.lock().unwrap().clone();
        for id in asked.iter().skip(asked_seen) {
            if v.state() == Some(MachineState::Idle) && framed.load(Ordering::SeqCst) {
                v.start_long_job();
                core.finish_start(
                    *id,
                    StartOutcome {
                        ok: true,
                        code: String::new(),
                        message: String::new(),
                    },
                );
            } else {
                core.finish_start(
                    *id,
                    StartOutcome {
                        ok: false,
                        code: "not_idle".into(),
                        message: "La máquina no está en reposo.".into(),
                    },
                );
            }
        }
        asked_seen = asked.len();
        let state = v.state();
        let running = matches!(state, Some(MachineState::Run | MachineState::Hold));
        if running && !was_running {
            began = Some(Instant::now());
        }
        let elapsed = began.map_or(0.0, |b| b.elapsed().as_secs_f64());
        let job_state = match (running, was_running, state) {
            (true, _, Some(MachineState::Hold)) => JobPhase::Paused,
            (true, _, _) => JobPhase::Running,
            (false, true, _) => JobPhase::Finished,
            _ => JobPhase::Idle,
        };
        was_running = running;
        let progress = (elapsed / 24.0 * 4.0).min(1.0);
        let head = v
            .handle()
            .and_then(|h| h.status_report())
            .map(|s| s.work_pos);
        core.update(
            Snapshot {
                doc_name: "demo.tlaku".into(),
                device_name: "Falcon2 (virtual)".into(),
                bed: Some([400.0, 400.0]),
                job_state,
                progress: if job_state == JobPhase::Finished {
                    1.0
                } else {
                    progress
                },
                elapsed_s: elapsed,
                remaining_s: Some((24.0 - elapsed * 4.0).max(0.0)),
                total_s: Some(24.0),
                layer: Some(super::hub::LayerInfo {
                    index: 1,
                    count: 2,
                    color: "#ff0000".into(),
                    mode: "line".into(),
                }),
                head: head.map(|p| [100.0 + p[0], 100.0 + p[1]]),
                has_output: true,
                framed: framed.load(Ordering::SeqCst),
                ..Snapshot::default()
            },
            Instant::now(),
        );
        let _ = core.set_camera(demo_jpeg(), Instant::now());
        std::thread::sleep(Duration::from_millis(300));
    }
    v.disconnect();
}
