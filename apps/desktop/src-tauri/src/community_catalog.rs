//! Catálogo de ajustes de material de la comunidad: descarga a pedido y caché local (mejora 5 del
//! plan de usuarios).
//!
//! Sin servidor propio: el sitio estático del proyecto (GitHub Pages) publica
//! `catalog/v1/index.json`, que compila CI desde `community/materials/` (ver `tlaku_doc::community_catalog`).
//!
//! **Red, lo único que sale de la app:**
//! - Un solo destino fijo ([`CATALOG_URL`]); los comandos no reciben ninguna dirección, así que la
//!   interfaz no puede apuntarlos a otro sitio. La interfaz sigue sin poder hacer `fetch` (CSP
//!   `connect-src 'self' ipc:`): la red es de Rust.
//! - Solo `GET`, solo HTTPS, sin cookies, sin referer, con un `User-Agent` fijo (`Tlaku/<versión>`).
//!   No se envía ningún identificador, ni el contenido del documento, ni las máquinas del usuario:
//!   el sitio solo ve la IP, como con cualquier página web.
//! - Solo cuando la persona abre el navegador de la comunidad o pulsa «Actualizar»
//!   (`community_catalog_refresh`). Nunca al arrancar ni en segundo plano.
//! - Redirecciones solo dentro del mismo sitio y de a lo más 3; plazos de conexión (10 s) y total (20 s).
//! - Tope de tamaño ([`MAX_INDEX_BYTES`]) aplicado mientras llega, no después: una respuesta
//!   enorme se corta.
//! - La respuesta se valida con `tlaku_doc::validate_index` (esquema, versión, número de entradas y
//!   rangos de cada valor) antes de guardarla o mostrarla. Un índice inválido se descarta y se
//!   conserva la última copia buena.
//!
//! **Caché:** `community-catalog.json` en la carpeta de caché de la app: `{ fetched_at, etag, index }`.
//! Se revalida al leerla (un archivo dañado o alterado se ignora), se escribe de forma atómica y
//! permite trabajar sin conexión con la última copia y su fecha. Se manda `If-None-Match` con el
//! `ETag` guardado para no bajar de nuevo lo que no cambió.

use crate::engine::write_atomic;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Manager};
use tlaku_doc::{
    CATALOG_PATH, CatalogIndex, CatalogSource, CatalogState, MAX_INDEX_BYTES, validate_index,
};
use tlaku_i18n::tr;

/// Sitio del proyecto (GitHub Pages). Debe coincidir con `SITE_URL` de `features/help/links.ts`.
pub const SITE_URL: &str = "https://jahziel1.github.io/tlaku";
/// El único destino de red de este módulo.
pub const CATALOG_URL: &str = "https://jahziel1.github.io/tlaku/catalog/v1/index.json";
/// Archivo de la caché, dentro de la carpeta de caché de la app.
pub const CACHE_FILE: &str = "community-catalog.json";

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const TOTAL_TIMEOUT: Duration = Duration::from_secs(20);
const MAX_REDIRECTS: usize = 3;
/// Margen para el sobre de la caché (fecha, etag) sobre el índice.
const CACHE_OVERHEAD: usize = 4096;

/// Cómo hablar con el sitio. Producción: [`Source::site`]. Las pruebas usan un servidor local en http.
#[derive(Debug, Clone)]
pub struct Source {
    pub url: String,
    pub https_only: bool,
}

impl Source {
    pub fn site() -> Self {
        debug_assert!(CATALOG_URL.starts_with(SITE_URL) && CATALOG_URL.ends_with(CATALOG_PATH));
        Self {
            url: CATALOG_URL.to_string(),
            https_only: true,
        }
    }
}

// ---------------------------------------------------------------------------
// Caché
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, Deserialize)]
struct CacheFile {
    /// ms desde 1970 de la última vez que el sitio confirmó este índice.
    fetched_at: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    etag: Option<String>,
    index: serde_json::Value,
}

/// Lo que se recuperó de la caché.
#[derive(Debug)]
pub struct Cached {
    pub index: CatalogIndex,
    pub fetched_at: f64,
    pub etag: Option<String>,
}

fn cache_path(dir: &Path) -> PathBuf {
    dir.join(CACHE_FILE)
}

/// La última copia guardada, revalidada. Archivo ausente, dañado, demasiado grande o inválido: `None`.
pub fn read_cache(dir: &Path) -> Option<Cached> {
    let path = cache_path(dir);
    let size = fs::metadata(&path).ok()?.len();
    if size > (MAX_INDEX_BYTES + CACHE_OVERHEAD) as u64 {
        return None;
    }
    let file: CacheFile = serde_json::from_slice(&fs::read(&path).ok()?).ok()?;
    if !file.fetched_at.is_finite() || file.fetched_at < 0.0 {
        return None;
    }
    let index = validate_index(&file.index.to_string()).ok()?;
    Some(Cached {
        index,
        fetched_at: file.fetched_at,
        etag: file.etag.filter(|e| is_etag(e)),
    })
}

fn write_cache(
    dir: &Path,
    index: &CatalogIndex,
    fetched_at: f64,
    etag: Option<&str>,
) -> Result<(), String> {
    let file = CacheFile {
        fetched_at,
        etag: etag.map(str::to_string),
        index: serde_json::to_value(index).map_err(|e| e.to_string())?,
    };
    let text = serde_json::to_string(&file).map_err(|e| e.to_string())?;
    fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    write_atomic(&cache_path(dir), &text)
}

fn is_etag(text: &str) -> bool {
    !text.is_empty() && text.len() <= 200 && text.bytes().all(|b| (0x20..0x7f).contains(&b))
}

fn now_ms() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as f64)
        .unwrap_or(0.0)
}

/// El estado a partir de la caché, sin tocar la red.
pub fn cached_state(dir: &Path) -> CatalogState {
    match read_cache(dir) {
        Some(c) => CatalogState {
            index: Some(c.index),
            source: CatalogSource::Cache,
            fetched_at: Some(c.fetched_at),
            error: None,
        },
        None => CatalogState {
            index: None,
            source: CatalogSource::None,
            fetched_at: None,
            error: None,
        },
    }
}

// ---------------------------------------------------------------------------
// Red
// ---------------------------------------------------------------------------

enum Fetched {
    /// 304: el sitio confirma que la copia guardada sigue vigente.
    NotModified,
    Body {
        text: String,
        etag: Option<String>,
    },
}

/// Instala el proveedor criptográfico de rustls una sola vez (el único compilado es `ring`).
fn ensure_crypto_provider() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        // Si otro componente ya instaló uno, `install_default` devuelve error y no pasa nada.
        let _ = rustls::crypto::ring::default_provider().install_default();
    });
}

fn client(source: &Source) -> Result<reqwest::Client, String> {
    ensure_crypto_provider();
    let host = reqwest::Url::parse(&source.url)
        .ok()
        .and_then(|u| u.host_str().map(str::to_string))
        .unwrap_or_default();
    let https_only = source.https_only;
    let policy = reqwest::redirect::Policy::custom(move |attempt| {
        let same_site = attempt.url().host_str() == Some(host.as_str())
            && (!https_only || attempt.url().scheme() == "https");
        if attempt.previous().len() >= MAX_REDIRECTS {
            attempt.error("demasiadas redirecciones")
        } else if same_site {
            attempt.follow()
        } else {
            // Se corta: la respuesta 3xx llega como está y se trata como error.
            attempt.stop()
        }
    });
    reqwest::Client::builder()
        .user_agent(format!("Tlaku/{} (catalog)", env!("CARGO_PKG_VERSION")))
        .https_only(https_only)
        .redirect(policy)
        .referer(false)
        .connect_timeout(CONNECT_TIMEOUT)
        .timeout(TOTAL_TIMEOUT)
        .build()
        .map_err(|e| tr!("desktop.community.client", err = e))
}

async fn fetch(source: &Source, etag: Option<&str>) -> Result<Fetched, String> {
    let client = client(source)?;
    let mut request = client
        .get(&source.url)
        .header(reqwest::header::ACCEPT, "application/json");
    if let Some(etag) = etag {
        request = request.header(reqwest::header::IF_NONE_MATCH, etag);
    }
    let mut response = request
        .send()
        .await
        .map_err(|e| tr!("desktop.community.connect", err = describe(&e)))?;
    let status = response.status();
    if status == reqwest::StatusCode::NOT_MODIFIED && etag.is_some() {
        return Ok(Fetched::NotModified);
    }
    if !status.is_success() {
        return Err(tr!("desktop.community.status", status = status.as_u16()));
    }
    let too_big = || tr!("desktop.community.too_big", max = MAX_INDEX_BYTES);
    if response
        .content_length()
        .is_some_and(|n| n > MAX_INDEX_BYTES as u64)
    {
        return Err(too_big());
    }
    let new_etag = response
        .headers()
        .get(reqwest::header::ETAG)
        .and_then(|v| v.to_str().ok())
        .filter(|e| is_etag(e))
        .map(str::to_string);
    // El tope se aplica trozo a trozo: una respuesta sin `Content-Length` no puede llenar la memoria.
    let mut body = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|e| tr!("desktop.community.connect", err = describe(&e)))?
    {
        if body.len() + chunk.len() > MAX_INDEX_BYTES {
            return Err(too_big());
        }
        body.extend_from_slice(&chunk);
    }
    let text = String::from_utf8(body).map_err(|_| tr!("desktop.community.not_utf8"))?;
    Ok(Fetched::Body {
        text,
        etag: new_etag,
    })
}

/// Texto corto de un error de red (sin la dirección completa, que es siempre la misma).
fn describe(e: &reqwest::Error) -> String {
    if e.is_timeout() {
        return tr!("desktop.community.timeout");
    }
    // `reqwest::Error` anida la causa real (DNS, conexión rechazada, certificado...).
    let mut cause: &dyn std::error::Error = e;
    while let Some(next) = cause.source() {
        cause = next;
    }
    cause.to_string()
}

/// Baja el índice, lo valida y lo guarda en la caché. Nunca falla por culpa de la red o del
/// contenido: devuelve la última copia buena (si la hay) con el motivo en `error`.
pub async fn refresh(dir: &Path, source: &Source) -> CatalogState {
    let cached = read_cache(dir);
    let etag = cached.as_ref().and_then(|c| c.etag.clone());
    let fallback = |error: String| match &cached {
        Some(c) => CatalogState {
            index: Some(c.index.clone()),
            source: CatalogSource::Cache,
            fetched_at: Some(c.fetched_at),
            error: Some(error),
        },
        None => CatalogState {
            index: None,
            source: CatalogSource::None,
            fetched_at: None,
            error: Some(error),
        },
    };
    match fetch(source, etag.as_deref()).await {
        Err(e) => fallback(e),
        Ok(Fetched::NotModified) => {
            // Solo `cached` puede dar 304 (si no, no se mandó `If-None-Match`).
            let Some(c) = &cached else {
                return fallback(tr!("desktop.community.status", status = 304));
            };
            let now = now_ms();
            let error = write_cache(dir, &c.index, now, c.etag.as_deref())
                .err()
                .map(|e| tr!("desktop.community.write_cache", err = e));
            CatalogState {
                index: Some(c.index.clone()),
                source: CatalogSource::Network,
                fetched_at: Some(now),
                error,
            }
        }
        Ok(Fetched::Body { text, etag }) => match validate_index(&text) {
            Err(e) => fallback(tr!("desktop.community.invalid", err = e)),
            Ok(index) => {
                let now = now_ms();
                let error = write_cache(dir, &index, now, etag.as_deref())
                    .err()
                    .map(|e| tr!("desktop.community.write_cache", err = e));
                CatalogState {
                    index: Some(index),
                    source: CatalogSource::Network,
                    fetched_at: Some(now),
                    error,
                }
            }
        },
    }
}

// ---------------------------------------------------------------------------
// Comandos
// ---------------------------------------------------------------------------

fn cache_dir(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_cache_dir()
        .map_err(|e| tr!("desktop.community.cache_dir", err = e))
}

/// La última copia guardada del catálogo. **No usa la red.**
#[tauri::command]
pub async fn community_catalog_cached(app: AppHandle) -> Result<CatalogState, String> {
    let dir = cache_dir(&app)?;
    crate::commands::blocking(move || Ok(cached_state(&dir))).await
}

/// Baja el catálogo del sitio del proyecto (destino fijo), lo valida y lo guarda en la caché. La
/// interfaz solo lo llama cuando la persona abre el navegador de la comunidad o pulsa «Actualizar».
#[tauri::command]
pub async fn community_catalog_refresh(app: AppHandle) -> Result<CatalogState, String> {
    let dir = cache_dir(&app)?;
    Ok(refresh(&dir, &Source::site()).await)
}

#[cfg(test)]
mod tests;
