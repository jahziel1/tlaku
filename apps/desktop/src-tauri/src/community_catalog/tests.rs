//! Catálogo de la comunidad: descarga, caché y límites contra un servidor local de prueba (sin red real).

use super::*;
use std::io::{Read, Write};
use std::net::{Ipv4Addr, TcpListener};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use tlaku_doc::CatalogOperation;

struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        static COUNT: AtomicUsize = AtomicUsize::new(0);
        let n = COUNT.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!("tlaku-community-{}-{n}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Aporte de ejemplo (inventado para las pruebas; no es un ajuste real).
const SAMPLE: &str = r#"{
    "id": "community.basswood.3.cut.diode_20.7",
    "material": "basswood", "thickness": 3, "role": "cut",
    "machine_class": "diode_20", "laser_watts": 22, "machine": "Máquina de prueba 22 W",
    "speed": 500, "power": 100, "passes": 2, "air": true,
    "notes": "Ejemplo de prueba; no es un ajuste real.",
    "source": "https://github.com/jahziel1/tlaku/issues/7",
    "confidence": "forum", "license": "CC0-1.0", "contributor": "alguien"
}"#;

fn index_text(files: &[(&str, &str)]) -> String {
    let files: Vec<(String, String)> = files
        .iter()
        .map(|(n, t)| (n.to_string(), t.to_string()))
        .collect();
    let index = tlaku_doc::build_index(&files, "2026-10-15").unwrap();
    serde_json::to_string(&index).unwrap()
}

fn one_entry() -> String {
    index_text(&[("7.json", SAMPLE)])
}

/// Respuesta HTTP/1.1 que cierra la conexión al terminar.
fn http(status: &str, headers: &[(&str, &str)], body: &[u8]) -> Vec<u8> {
    let mut out = format!("HTTP/1.1 {status}\r\nConnection: close\r\n");
    for (k, v) in headers {
        out.push_str(&format!("{k}: {v}\r\n"));
    }
    if !headers
        .iter()
        .any(|(k, _)| k.eq_ignore_ascii_case("content-length"))
    {
        out.push_str(&format!("Content-Length: {}\r\n", body.len()));
    }
    out.push_str("\r\n");
    let mut bytes = out.into_bytes();
    bytes.extend_from_slice(body);
    bytes
}

/// Servidor local que atiende todas las conexiones, una a una, y guarda las peticiones.
struct Server {
    url: String,
    requests: Arc<Mutex<Vec<String>>>,
}

impl Server {
    fn start(handler: impl Fn(&str, usize) -> Vec<u8> + Send + 'static) -> Self {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        let requests = Arc::new(Mutex::new(Vec::new()));
        let seen = requests.clone();
        thread::spawn(move || {
            for (n, conn) in listener.incoming().enumerate() {
                let Ok(mut sock) = conn else { break };
                let mut got = Vec::new();
                let mut buf = [0u8; 2048];
                while !got.windows(4).any(|w| w == b"\r\n\r\n") {
                    match sock.read(&mut buf) {
                        Ok(0) | Err(_) => break,
                        Ok(k) => got.extend_from_slice(&buf[..k]),
                    }
                }
                let request = String::from_utf8_lossy(&got).into_owned();
                let reply = handler(&request, n);
                seen.lock().unwrap().push(request);
                let _ = sock.write_all(&reply);
                let _ = sock.flush();
            }
        });
        Self {
            url: format!("http://127.0.0.1:{port}/catalog/v1/index.json"),
            requests,
        }
    }

    fn serving(body: String) -> Self {
        Self::start(move |_, _| {
            http(
                "200 OK",
                &[("Content-Type", "application/json"), ("ETag", "\"v1\"")],
                body.as_bytes(),
            )
        })
    }

    fn source(&self) -> Source {
        Source {
            url: self.url.clone(),
            https_only: false,
        }
    }

    fn requests(&self) -> Vec<String> {
        self.requests.lock().unwrap().clone()
    }
}

/// Un puerto en el que nadie escucha: la conexión se rechaza (como sin red).
fn dead_source() -> Source {
    let port = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    Source {
        url: format!("http://127.0.0.1:{port}/catalog/v1/index.json"),
        https_only: false,
    }
}

#[tokio::test]
async fn descarga_valida_y_guarda_la_cache() {
    let dir = TempDir::new();
    let server = Server::serving(one_entry());
    let state = refresh(&dir.0, &server.source()).await;
    assert_eq!(state.source, CatalogSource::Network);
    assert_eq!(state.error, None);
    let index = state.index.expect("índice");
    assert_eq!(index.entries.len(), 1);
    assert_eq!(index.entries[0].operation, CatalogOperation::Cut);
    let fetched = state.fetched_at.expect("fecha");
    assert!(fetched > 1.7e12, "ms desde 1970: {fetched}");

    // La caché se lee sin red y conserva la fecha de la descarga.
    let cached = cached_state(&dir.0);
    assert_eq!(cached.source, CatalogSource::Cache);
    assert_eq!(cached.fetched_at, Some(fetched));
    assert_eq!(cached.index.unwrap().entries.len(), 1);
}

#[tokio::test]
async fn la_peticion_es_un_get_sin_cookies_ni_referer_ni_identificadores() {
    let dir = TempDir::new();
    let server = Server::serving(one_entry());
    refresh(&dir.0, &server.source()).await;
    let requests = server.requests();
    assert_eq!(requests.len(), 1);
    let req = requests[0].to_lowercase();
    assert!(
        req.starts_with("get /catalog/v1/index.json http/1.1"),
        "{req}"
    );
    for header in ["cookie:", "referer:", "authorization:", "x-"] {
        assert!(!req.contains(header), "no debe mandar {header}: {req}");
    }
    assert!(
        req.contains(&format!("user-agent: tlaku/{}", env!("CARGO_PKG_VERSION"))),
        "{req}"
    );
    // Sin caché previa no hay validador condicional.
    assert!(!req.contains("if-none-match"), "{req}");
}

#[tokio::test]
async fn sin_conexion_se_usa_la_ultima_cache_con_su_fecha() {
    let dir = TempDir::new();
    let online = Server::serving(one_entry());
    let first = refresh(&dir.0, &online.source()).await;
    assert_eq!(first.source, CatalogSource::Network);

    let offline = refresh(&dir.0, &dead_source()).await;
    assert_eq!(offline.source, CatalogSource::Cache);
    assert_eq!(
        offline.fetched_at, first.fetched_at,
        "la fecha es la de la copia"
    );
    assert_eq!(offline.index, first.index);
    assert!(offline.error.is_some(), "debe decir que no pudo actualizar");
}

#[tokio::test]
async fn sin_conexion_y_sin_cache_no_hay_catalogo_pero_hay_motivo() {
    let dir = TempDir::new();
    let state = refresh(&dir.0, &dead_source()).await;
    assert_eq!(state.source, CatalogSource::None);
    assert!(state.index.is_none());
    assert!(state.error.is_some());
    assert!(!cache_path(&dir.0).exists());
    assert_eq!(cached_state(&dir.0).source, CatalogSource::None);
}

#[tokio::test]
async fn un_indice_invalido_se_descarta_y_se_conserva_la_cache_buena() {
    let dir = TempDir::new();
    let good = Server::serving(one_entry());
    let first = refresh(&dir.0, &good.source()).await;

    let bad = SAMPLE.replace("\"power\": 100", "\"power\": 150");
    // Un índice «armado a mano» con potencia imposible (el compilador de CI no lo dejaría pasar).
    let mut doc: serde_json::Value = serde_json::from_str(&one_entry()).unwrap();
    doc["entries"][0]["power"] =
        serde_json::from_str::<serde_json::Value>(&bad).unwrap()["power"].clone();
    let evil = Server::serving(doc.to_string());
    let state = refresh(&dir.0, &evil.source()).await;
    assert_eq!(state.source, CatalogSource::Cache);
    assert_eq!(state.index, first.index);
    assert!(state.error.unwrap().contains("power"));
    // Y la caché en disco sigue siendo la buena.
    assert_eq!(read_cache(&dir.0).unwrap().index, first.index.unwrap());
}

#[tokio::test]
async fn una_version_mayor_desconocida_se_rechaza_con_un_mensaje_claro() {
    let dir = TempDir::new();
    let body = r#"{"schema":"tlaku.community-materials","version":2,"generated_at":"2026-10-15","entries":[]}"#;
    let server = Server::serving(body.to_string());
    let state = refresh(&dir.0, &server.source()).await;
    assert!(state.index.is_none());
    assert!(state.error.unwrap().contains('2'));
}

#[tokio::test]
async fn una_respuesta_que_no_es_el_catalogo_se_descarta() {
    for body in ["<html>404</html>", "", "[]", "null"] {
        let dir = TempDir::new();
        let server = Server::serving(body.to_string());
        let state = refresh(&dir.0, &server.source()).await;
        assert!(state.index.is_none(), "{body}");
        assert!(state.error.is_some(), "{body}");
        assert!(
            !cache_path(&dir.0).exists(),
            "no se guarda lo inválido: {body}"
        );
    }
}

#[tokio::test]
async fn el_tamano_maximo_se_aplica_con_y_sin_content_length() {
    let big = vec![b' '; MAX_INDEX_BYTES + 1];
    // Con Content-Length: se rechaza sin bajar el cuerpo.
    let with_len = Server::start({
        let big = big.clone();
        move |_, _| http("200 OK", &[], &big)
    });
    // Sin Content-Length (cuerpo hasta cerrar): se corta al pasar el tope.
    let without_len = Server::start(move |_, _| {
        let mut out = b"HTTP/1.1 200 OK\r\nConnection: close\r\n\r\n".to_vec();
        out.extend_from_slice(&big);
        out
    });
    for server in [&with_len, &without_len] {
        let dir = TempDir::new();
        let state = refresh(&dir.0, &server.source()).await;
        assert!(state.index.is_none());
        assert!(
            state.error.unwrap().contains(&MAX_INDEX_BYTES.to_string()),
            "debe mencionar el máximo"
        );
    }
}

#[tokio::test]
async fn un_error_del_sitio_deja_la_cache_y_dice_el_codigo() {
    let dir = TempDir::new();
    let ok = Server::serving(one_entry());
    refresh(&dir.0, &ok.source()).await;
    let down = Server::start(|_, _| http("503 Service Unavailable", &[], b"nope"));
    let state = refresh(&dir.0, &down.source()).await;
    assert_eq!(state.source, CatalogSource::Cache);
    assert!(state.error.unwrap().contains("503"));

    let missing = Server::start(|_, _| http("404 Not Found", &[], b"nope"));
    let fresh = TempDir::new();
    let state = refresh(&fresh.0, &missing.source()).await;
    assert!(state.index.is_none());
    assert!(state.error.unwrap().contains("404"));
}

#[tokio::test]
async fn con_etag_la_segunda_vez_pregunta_y_un_304_renueva_la_fecha() {
    let dir = TempDir::new();
    let body = one_entry();
    let server = Server::start(move |req, _| {
        if req.to_lowercase().contains("if-none-match: \"v1\"") {
            http("304 Not Modified", &[("ETag", "\"v1\"")], b"")
        } else {
            http("200 OK", &[("ETag", "\"v1\"")], body.as_bytes())
        }
    });
    let first = refresh(&dir.0, &server.source()).await;
    assert_eq!(first.source, CatalogSource::Network);
    thread::sleep(Duration::from_millis(5));
    let second = refresh(&dir.0, &server.source()).await;
    assert_eq!(second.source, CatalogSource::Network);
    assert_eq!(second.index, first.index);
    assert!(second.fetched_at > first.fetched_at, "la fecha se renueva");
    assert_eq!(second.error, None);
    assert_eq!(server.requests().len(), 2);
    // La caché guardó la fecha nueva y conserva el etag.
    let cached = read_cache(&dir.0).unwrap();
    assert_eq!(Some(cached.fetched_at), second.fetched_at);
    assert_eq!(cached.etag.as_deref(), Some("\"v1\""));
}

#[tokio::test]
async fn una_redireccion_a_otro_sitio_no_se_sigue() {
    let target = Server::serving(one_entry());
    let host_other = target.url.replace("127.0.0.1", "localhost");
    let redirect = Server::start(move |_, _| http("302 Found", &[("Location", &host_other)], b""));
    let dir = TempDir::new();
    let state = refresh(&dir.0, &redirect.source()).await;
    assert!(state.index.is_none());
    assert!(state.error.unwrap().contains("302"));
    assert!(
        target.requests().is_empty(),
        "no debe haber ido al otro sitio"
    );
}

#[tokio::test]
async fn una_redireccion_dentro_del_mismo_sitio_si_se_sigue() {
    let body = one_entry();
    let server = Server::start(move |req, _| {
        if req.starts_with("GET /catalog/v1/index.json") {
            http("301 Moved Permanently", &[("Location", "/moved.json")], b"")
        } else {
            http("200 OK", &[], body.as_bytes())
        }
    });
    let dir = TempDir::new();
    let state = refresh(&dir.0, &server.source()).await;
    assert_eq!(state.source, CatalogSource::Network, "{:?}", state.error);
}

#[tokio::test]
async fn solo_https_en_produccion() {
    let server = Server::serving(one_entry());
    let dir = TempDir::new();
    let strict = Source {
        https_only: true,
        ..server.source()
    };
    let state = refresh(&dir.0, &strict).await;
    assert!(state.index.is_none());
    assert!(state.error.is_some());
    assert!(
        server.requests().is_empty(),
        "http plano no debe ni conectarse"
    );
}

#[test]
fn el_destino_de_produccion_es_fijo_y_es_el_sitio_del_proyecto() {
    let site = Source::site();
    assert!(site.https_only);
    assert_eq!(site.url, format!("{SITE_URL}/{CATALOG_PATH}"));
    let url = reqwest::Url::parse(&site.url).unwrap();
    assert_eq!(url.scheme(), "https");
    assert_eq!(url.host_str(), Some("jahziel1.github.io"));
    assert!(url.query().is_none() && url.fragment().is_none());
}

#[test]
fn una_cache_danada_o_alterada_se_ignora() {
    let dir = TempDir::new();
    let index: serde_json::Value = serde_json::from_str(&one_entry()).unwrap();
    let write = |value: serde_json::Value| {
        fs::write(cache_path(&dir.0), value.to_string()).unwrap();
    };
    write(serde_json::json!({"fetched_at": 1.7e12, "index": index}));
    assert!(read_cache(&dir.0).is_some());

    let mut tampered = index.clone();
    tampered["entries"][0]["power"] = 500.into();
    write(serde_json::json!({"fetched_at": 1.7e12, "index": tampered}));
    assert!(read_cache(&dir.0).is_none(), "valores imposibles");

    write(serde_json::json!({"fetched_at": -1, "index": index}));
    assert!(read_cache(&dir.0).is_none(), "fecha imposible");

    fs::write(cache_path(&dir.0), "{ no es json").unwrap();
    assert!(read_cache(&dir.0).is_none());
    assert_eq!(cached_state(&dir.0).source, CatalogSource::None);

    let huge = vec![b' '; MAX_INDEX_BYTES + CACHE_OVERHEAD + 1];
    fs::write(cache_path(&dir.0), huge).unwrap();
    assert!(read_cache(&dir.0).is_none());
}

/// Comprobación manual contra el sitio real (`cargo test -p tlaku-desktop --lib -- --ignored sitio_real`):
/// el apretón TLS con las raíces del sistema funciona. Sin catálogo publicado el sitio contesta 404, y
/// eso también prueba que se llegó por HTTPS.
#[tokio::test]
#[ignore = "usa la red real"]
async fn sitio_real_responde_por_tls() {
    let dir = TempDir::new();
    let state = refresh(&dir.0, &Source::site()).await;
    match (&state.index, &state.error) {
        (Some(_), None) => assert_eq!(state.source, CatalogSource::Network),
        (None, Some(error)) => assert!(error.contains("404"), "{error}"),
        other => panic!("estado inesperado: {other:?}"),
    }
}
