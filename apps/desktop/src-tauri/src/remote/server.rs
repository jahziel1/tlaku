//! Servidor HTTP del control remoto: la página del celular y su API.
//!
//! # Seguridad (resumen; el detalle está en `docs/app-api.md`)
//!
//! - Escucha en **una** dirección privada de la red local, nunca en `0.0.0.0` (ver `net`).
//! - Cada petición se rechaza si el par que la manda no es una dirección local, y si el encabezado
//!   `Host` no es una IP (defensa contra el "DNS rebinding").
//! - Todo lo que no sea la página estática exige autenticación en **cada** petición: cookie de
//!   sesión (tras `POST /api/login` con la clave del QR) o la clave en un encabezado. Ver `auth`.
//! - Las acciones (`POST`) exigen además `X-Requested-With` y, si el navegador manda `Origin`, que
//!   sea el mismo servidor. No se responde ningún encabezado CORS: ningún otro sitio web puede
//!   llamar a la API desde el navegador del usuario.
//! - Respuestas con `Cache-Control: no-store`, `nosniff`, sin `Referer` y con una política de
//!   contenido que solo deja ejecutar el código de la propia página.

use super::auth::{AuthError, MAX_SESSIONS};
use super::hub::{ActionError, Checklist, Core, START_TIMEOUT, StartOutcome};
use super::net::is_local_peer;
use axum::body::{Body, Bytes};
use axum::extract::{ConnectInfo, DefaultBodyLimit, Request, State};
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::sse::{Event, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Extension, Router};
use std::convert::Infallible;
use std::io;
use std::net::{IpAddr, SocketAddr, TcpListener};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};
use tlaku_i18n::tr;
use tokio::sync::oneshot;

const PAGE: &str = include_str!("page.html");
/// Nombre de la cookie de sesión.
pub const COOKIE: &str = "tlaku_session";
/// Cada cuánto se manda el estado aunque no haya cambios (también mantiene viva la sesión).
const HEARTBEAT: Duration = Duration::from_secs(10);
/// Se juntan los cambios que llegan casi a la vez en un solo evento.
const COALESCE: Duration = Duration::from_millis(150);
/// Conexiones en vivo abiertas a la vez, entre todos los clientes.
const MAX_STREAMS: usize = 2 * MAX_SESSIONS;
/// Tamaño máximo del cuerpo de una petición: son JSON de unas pocas decenas de bytes.
const MAX_BODY: usize = 1024;

/// Quién hizo la petición, ya autenticado.
#[derive(Clone)]
struct Authed {
    id: String,
    label: String,
}

type Shared = Arc<Shared_>;

struct Shared_ {
    core: Arc<Core>,
    streams: std::sync::atomic::AtomicUsize,
}

// ---------------------------------------------------------------------------
// Respuestas
// ---------------------------------------------------------------------------

fn json_response(status: StatusCode, value: serde_json::Value) -> Response {
    let mut r = Response::new(Body::from(value.to_string()));
    *r.status_mut() = status;
    r.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json; charset=utf-8"),
    );
    r
}

fn fail(status: StatusCode, code: &str, message: &str) -> Response {
    json_response(
        status,
        serde_json::json!({ "ok": false, "code": code, "message": message }),
    )
}

fn action_error(e: &ActionError) -> Response {
    fail(StatusCode::CONFLICT, e.code, &e.message)
}

fn auth_error(e: &AuthError) -> Response {
    match e {
        AuthError::Unauthorized => fail(
            StatusCode::UNAUTHORIZED,
            "unauthorized",
            &tr!("desktop.remote.login_required"),
        ),
        AuthError::BadToken => fail(
            StatusCode::UNAUTHORIZED,
            "bad_token",
            &tr!("desktop.remote.bad_key"),
        ),
        AuthError::Locked { retry_after_s } => {
            let mut r = fail(
                StatusCode::TOO_MANY_REQUESTS,
                "locked",
                &tr!("desktop.remote.locked"),
            );
            if let Ok(v) = HeaderValue::from_str(&retry_after_s.to_string()) {
                r.headers_mut().insert(header::RETRY_AFTER, v);
            }
            r
        }
    }
}

// ---------------------------------------------------------------------------
// Filtros
// ---------------------------------------------------------------------------

fn header_str(h: &HeaderMap, name: header::HeaderName) -> Option<&str> {
    h.get(name).and_then(|v| v.to_str().ok())
}

/// Valor de la cookie de sesión, si viene.
fn session_cookie(h: &HeaderMap) -> Option<String> {
    let raw = header_str(h, header::COOKIE)?;
    raw.split(';').find_map(|part| {
        let (name, value) = part.trim().split_once('=')?;
        (name == COOKIE).then(|| value.trim().to_string())
    })
}

/// Clave en un encabezado: `Authorization: Bearer <clave>` o `X-Tlaku-Token`.
fn header_token(h: &HeaderMap) -> Option<String> {
    if let Some(v) = header_str(h, header::AUTHORIZATION)
        && let Some(t) = v.strip_prefix("Bearer ")
    {
        return Some(t.trim().to_string());
    }
    h.get("x-tlaku-token")
        .and_then(|v| v.to_str().ok())
        .map(|t| t.trim().to_string())
}

/// `Host` debe ser una IP (con o sin puerto): así un sitio que apunte su nombre a esta PC no puede
/// hablar con el servidor desde el navegador.
fn host_is_ip(h: &HeaderMap) -> bool {
    let Some(host) = header_str(h, header::HOST) else {
        return false;
    };
    let name = if let Some(rest) = host.strip_prefix('[') {
        rest.split(']').next().unwrap_or("")
    } else {
        host.rsplit_once(':').map_or(host, |(n, port)| {
            if port.chars().all(|c| c.is_ascii_digit()) {
                n
            } else {
                host
            }
        })
    };
    name.parse::<IpAddr>().is_ok()
}

/// Si el navegador manda `Origin`, debe ser este mismo servidor.
fn origin_matches(h: &HeaderMap) -> bool {
    let Some(origin) = header_str(h, header::ORIGIN) else {
        return true;
    };
    let Some(host) = header_str(h, header::HOST) else {
        return false;
    };
    origin.strip_prefix("http://").is_some_and(|o| o == host)
}

fn user_agent(h: &HeaderMap) -> String {
    header_str(h, header::USER_AGENT)
        .unwrap_or("")
        .chars()
        .take(300)
        .collect()
}

/// Primer filtro: par local, `Host` válido y, en las acciones, origen y marca de la página.
async fn gate(ConnectInfo(peer): ConnectInfo<SocketAddr>, req: Request, next: Next) -> Response {
    if !is_local_peer(peer.ip()) {
        return fail(
            StatusCode::FORBIDDEN,
            "not_local",
            &tr!("desktop.remote.only_local"),
        );
    }
    let h = req.headers();
    if !host_is_ip(h) {
        return fail(
            StatusCode::BAD_REQUEST,
            "bad_host",
            &tr!("desktop.remote.use_ip"),
        );
    }
    if req.method() != Method::GET && req.method() != Method::HEAD {
        let marked = h.contains_key("x-requested-with") || header_token(h).is_some();
        if !origin_matches(h) || !marked {
            return fail(
                StatusCode::FORBIDDEN,
                "bad_origin",
                &tr!("desktop.remote.request_not_allowed"),
            );
        }
    }
    next.run(req).await
}

/// Autenticación de la API: sin ella no pasa nada.
async fn require_auth(
    State(sh): State<Shared>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    mut req: Request,
    next: Next,
) -> Response {
    let h = req.headers();
    let cookie = session_cookie(h);
    let token = header_token(h);
    let agent = user_agent(h);
    let id = match sh.core.auth.authenticate(
        peer.ip(),
        &agent,
        cookie.as_deref(),
        token.as_deref(),
        Instant::now(),
    ) {
        Ok(id) => id,
        Err(e) => return auth_error(&e),
    };
    let label = sh
        .core
        .auth
        .label(&id)
        .unwrap_or_else(|| peer.ip().to_string());
    sh.core.refresh_clients(Instant::now());
    req.extensions_mut().insert(Authed { id, label });
    next.run(req).await
}

/// Encabezados de seguridad en todas las respuestas.
async fn harden(req: Request, next: Next) -> Response {
    let mut r = next.run(req).await;
    let h = r.headers_mut();
    h.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    h.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    h.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );
    h.insert(header::X_FRAME_OPTIONS, HeaderValue::from_static("DENY"));
    h.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(
            "default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; \
             img-src 'self'; connect-src 'self'; base-uri 'none'; form-action 'none'; \
             frame-ancestors 'none'",
        ),
    );
    r
}

// ---------------------------------------------------------------------------
// Manejadores
// ---------------------------------------------------------------------------

async fn page() -> Response {
    let mut r = Response::new(Body::from(PAGE));
    r.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/html; charset=utf-8"),
    );
    r
}

async fn nothing() -> StatusCode {
    StatusCode::NO_CONTENT
}

async fn not_found() -> Response {
    fail(
        StatusCode::NOT_FOUND,
        "not_found",
        &tr!("desktop.remote.not_found"),
    )
}

#[derive(serde::Deserialize)]
struct LoginBody {
    token: String,
}

/// `POST /api/login`: cambia la clave del QR por una cookie de sesión.
async fn login(
    State(sh): State<Shared>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let Ok(parsed) = serde_json::from_slice::<LoginBody>(&body) else {
        return fail(
            StatusCode::BAD_REQUEST,
            "bad_request",
            &tr!("desktop.remote.missing_key"),
        );
    };
    let result = sh.core.auth.login(
        peer.ip(),
        &user_agent(&headers),
        parsed.token.trim(),
        Instant::now(),
    );
    match result {
        Ok((_, cookie)) => {
            sh.core.refresh_clients(Instant::now());
            let mut r = json_response(StatusCode::OK, serde_json::json!({ "ok": true }));
            let value =
                format!("{COOKIE}={cookie}; Path=/; HttpOnly; SameSite=Strict; Max-Age=86400");
            if let Ok(v) = HeaderValue::from_str(&value) {
                r.headers_mut().insert(header::SET_COOKIE, v);
            }
            r
        }
        Err(e) => auth_error(&e),
    }
}

async fn state(State(sh): State<Shared>) -> Response {
    json_response(StatusCode::OK, sh.core.view(Instant::now()))
}

async fn route(State(sh): State<Shared>) -> Response {
    json_response(StatusCode::OK, sh.core.route_json())
}

async fn camera(State(sh): State<Shared>) -> Response {
    match sh.core.camera_jpeg(Instant::now()) {
        Some(bytes) => {
            let mut r = Response::new(Body::from(bytes));
            r.headers_mut()
                .insert(header::CONTENT_TYPE, HeaderValue::from_static("image/jpeg"));
            r
        }
        None => fail(
            StatusCode::NOT_FOUND,
            "no_camera",
            &tr!("desktop.remote.camera_off"),
        ),
    }
}

/// Ejecuta una acción que bloquea hasta que la máquina contesta.
async fn blocking_action(
    sh: Shared,
    me: Authed,
    run: impl FnOnce(&Core, &str) -> Result<(), ActionError> + Send + 'static,
) -> Response {
    let outcome = tokio::task::spawn_blocking(move || run(&sh.core, &me.label)).await;
    match outcome {
        Ok(Ok(())) => json_response(StatusCode::OK, serde_json::json!({ "ok": true })),
        Ok(Err(e)) => action_error(&e),
        Err(_) => fail(
            StatusCode::INTERNAL_SERVER_ERROR,
            "failed",
            &tr!("desktop.remote.action_interrupted"),
        ),
    }
}

async fn pause(State(sh): State<Shared>, Extension(me): Extension<Authed>) -> Response {
    blocking_action(sh, me, |c, who| c.pause(who)).await
}

async fn resume(State(sh): State<Shared>, Extension(me): Extension<Authed>) -> Response {
    blocking_action(sh, me, |c, who| c.resume(who)).await
}

async fn stop(State(sh): State<Shared>, Extension(me): Extension<Authed>) -> Response {
    blocking_action(sh, me, |c, who| c.stop(who)).await
}

#[derive(serde::Deserialize)]
struct StartBody {
    #[serde(default)]
    checklist: Checklist,
}

/// `POST /api/start`: valida el checklist y las reglas de seguridad, y pide a la interfaz de la PC
/// que inicie con su ruta de siempre. Espera su respuesta para contarle al celular qué pasó.
async fn start(
    State(sh): State<Shared>,
    Extension(me): Extension<Authed>,
    body: Bytes,
) -> Response {
    let Ok(parsed) = serde_json::from_slice::<StartBody>(&body) else {
        return fail(
            StatusCode::BAD_REQUEST,
            "bad_request",
            &tr!("desktop.remote.bad_request"),
        );
    };
    let (id, rx): (u64, oneshot::Receiver<StartOutcome>) =
        match sh.core.begin_start(parsed.checklist, Instant::now()) {
            Ok(r) => r,
            Err(e) => return action_error(&e),
        };
    sh.core.ask_start(id, &me.label);
    match tokio::time::timeout(START_TIMEOUT, rx).await {
        Ok(Ok(o)) if o.ok => json_response(StatusCode::OK, serde_json::json!({ "ok": true })),
        Ok(Ok(o)) => {
            let code = if o.code.is_empty() {
                "failed".to_string()
            } else {
                o.code
            };
            let mut r = fail(StatusCode::CONFLICT, &code, &o.message);
            if let Ok(v) = HeaderValue::from_str("close") {
                r.headers_mut().insert(header::CONNECTION, v);
            }
            r
        }
        _ => {
            sh.core.abandon_start(id);
            fail(
                StatusCode::GATEWAY_TIMEOUT,
                "timeout",
                &super::hub::start_reason_text("timeout"),
            )
        }
    }
}

/// Cierra la cuenta de conexiones en vivo al soltarse, pase lo que pase.
struct StreamGuard {
    sh: Shared,
    id: String,
}

impl Drop for StreamGuard {
    fn drop(&mut self) {
        self.sh
            .streams
            .fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
        let now = Instant::now();
        self.sh.core.auth.stream_close(&self.id, now);
        self.sh.core.refresh_clients(now);
    }
}

/// `GET /api/events`: el estado en vivo (SSE). Un evento `state` al conectar, en cada cambio y
/// cada [`HEARTBEAT`]. Se corta sola si el cliente es expulsado.
async fn events(State(sh): State<Shared>, Extension(me): Extension<Authed>) -> Response {
    use std::sync::atomic::Ordering;
    if sh.streams.fetch_add(1, Ordering::SeqCst) >= MAX_STREAMS {
        sh.streams.fetch_sub(1, Ordering::SeqCst);
        return fail(
            StatusCode::SERVICE_UNAVAILABLE,
            "too_many",
            &tr!("desktop.remote.too_many_streams"),
        );
    }
    if !sh.core.auth.stream_open(&me.id, Instant::now()) {
        sh.streams.fetch_sub(1, Ordering::SeqCst);
        return auth_error(&AuthError::Unauthorized);
    }
    let guard = StreamGuard {
        sh: sh.clone(),
        id: me.id,
    };
    sh.core.refresh_clients(Instant::now());
    let rx = sh.core.subscribe();
    let stream =
        futures_util::stream::unfold((guard, rx, true), |(guard, mut rx, first)| async move {
            if !first {
                tokio::select! {
                    changed = rx.changed() => if changed.is_err() { return None; },
                    () = tokio::time::sleep(HEARTBEAT) => {}
                }
                tokio::time::sleep(COALESCE).await;
            }
            rx.borrow_and_update();
            let core = &guard.sh.core;
            if !core.auth.is_alive(&guard.id) {
                return None;
            }
            let now = Instant::now();
            core.auth.touch(&guard.id, now);
            let data = core.view(now).to_string();
            let event = Event::default().event("state").data(data);
            Some((Ok::<Event, Infallible>(event), (guard, rx, false)))
        });
    Sse::new(stream).into_response()
}

fn router(sh: Shared) -> Router {
    let api = Router::new()
        .route("/state", get(state))
        .route("/events", get(events))
        .route("/route", get(route))
        .route("/camera.jpg", get(camera))
        .route("/pause", post(pause))
        .route("/resume", post(resume))
        .route("/stop", post(stop))
        .route("/start", post(start))
        .layer(middleware::from_fn_with_state(sh.clone(), require_auth));
    Router::new()
        .route("/", get(page))
        .route("/favicon.ico", get(nothing))
        .route("/api/login", post(login))
        .nest("/api", api)
        .fallback(not_found)
        .layer(DefaultBodyLimit::max(MAX_BODY))
        .layer(middleware::from_fn(gate))
        .layer(middleware::from_fn(harden))
        .with_state(sh)
}

// ---------------------------------------------------------------------------
// Servidor
// ---------------------------------------------------------------------------

/// Servidor en marcha. Soltarlo lo detiene, corta las conexiones y libera el puerto.
pub struct Server {
    addr: SocketAddr,
    stop: Option<oneshot::Sender<()>>,
    thread: Option<JoinHandle<()>>,
}

impl Server {
    /// Escucha en `addr` (puerto 0 = uno libre, para las pruebas). El error de abrir el puerto
    /// (por ejemplo "ya en uso") se devuelve aquí, no en el hilo.
    pub fn start(core: Arc<Core>, addr: SocketAddr) -> io::Result<Self> {
        let listener = TcpListener::bind(addr)?;
        listener.set_nonblocking(true)?;
        let addr = listener.local_addr()?;
        let (stop, stopped) = oneshot::channel::<()>();
        let shared: Shared = Arc::new(Shared_ {
            core,
            streams: std::sync::atomic::AtomicUsize::new(0),
        });
        let thread = thread::Builder::new()
            .name("tlaku-remote".into())
            .spawn(move || {
                let Ok(rt) = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                else {
                    return;
                };
                rt.block_on(async move {
                    let Ok(listener) = tokio::net::TcpListener::from_std(listener) else {
                        return;
                    };
                    let app =
                        router(shared.clone()).into_make_service_with_connect_info::<SocketAddr>();
                    // Mientras el servidor vive, revisa cada pocos segundos si cambió la lista de
                    // clientes (caducidades) para avisar a la PC.
                    let ticker = {
                        let shared = shared.clone();
                        async move {
                            loop {
                                tokio::time::sleep(Duration::from_secs(4)).await;
                                shared.core.refresh_clients(Instant::now());
                            }
                        }
                    };
                    tokio::select! {
                        _ = axum::serve(listener, app) => {}
                        () = ticker => {}
                        _ = stopped => {}
                    }
                });
                // Al soltar el motor se cancelan las conexiones que sigan abiertas (SSE).
                drop(rt);
            })?;
        Ok(Self {
            addr,
            stop: Some(stop),
            thread: Some(thread),
        })
    }

    pub fn addr(&self) -> SocketAddr {
        self.addr
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        if let Some(s) = self.stop.take() {
            let _ = s.send(());
        }
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

#[cfg(test)]
mod unit {
    use super::*;

    fn headers(pairs: &[(&str, &str)]) -> HeaderMap {
        let mut h = HeaderMap::new();
        for (k, v) in pairs {
            h.insert(
                header::HeaderName::from_bytes(k.as_bytes()).unwrap(),
                HeaderValue::from_str(v).unwrap(),
            );
        }
        h
    }

    #[test]
    fn host_must_be_an_ip() {
        for ok in ["192.168.1.20:47802", "127.0.0.1", "[::1]:80", "10.0.0.1:1"] {
            assert!(host_is_ip(&headers(&[("host", ok)])), "{ok}");
        }
        for bad in [
            "evil.example:47802",
            "localhost:80",
            "192.168.1.20.evil.com",
            "pc.local",
        ] {
            assert!(!host_is_ip(&headers(&[("host", bad)])), "{bad}");
        }
        assert!(!host_is_ip(&HeaderMap::new()));
    }

    #[test]
    fn origin_must_be_this_server() {
        let ok = headers(&[
            ("host", "192.168.1.20:47802"),
            ("origin", "http://192.168.1.20:47802"),
        ]);
        assert!(origin_matches(&ok));
        // Sin Origin (curl, scripts) pasa este filtro; la autenticación sigue exigiéndose.
        assert!(origin_matches(&headers(&[("host", "192.168.1.20:47802")])));
        for evil in [
            "http://evil.example",
            "https://192.168.1.20:47802",
            "null",
            "http://192.168.1.20:1",
        ] {
            let h = headers(&[("host", "192.168.1.20:47802"), ("origin", evil)]);
            assert!(!origin_matches(&h), "{evil}");
        }
    }

    #[test]
    fn cookie_and_token_parsing() {
        let h = headers(&[("cookie", "a=1; tlaku_session=abc123; b=2")]);
        assert_eq!(session_cookie(&h).as_deref(), Some("abc123"));
        assert_eq!(session_cookie(&headers(&[("cookie", "otra=1")])), None);
        assert_eq!(
            header_token(&headers(&[("authorization", "Bearer k1")])).as_deref(),
            Some("k1")
        );
        assert_eq!(
            header_token(&headers(&[("x-tlaku-token", " k2 ")])).as_deref(),
            Some("k2")
        );
        assert_eq!(
            header_token(&headers(&[("authorization", "Basic zzz")])),
            None
        );
    }
}
