//! Autenticación del control remoto: clave aleatoria, sesiones, límite de intentos y lista de
//! quién está conectado.
//!
//! - La **clave** (128 bits al azar) viaja en el QR dentro del fragmento de la URL (`#clave`), que el
//!   navegador nunca manda al servidor. La página la entrega una vez a `POST /api/login` y recibe
//!   una cookie de **sesión** (`HttpOnly`, `SameSite=Strict`) con otro valor aleatorio distinto de la
//!   clave. Un cliente que no quiera cookies puede mandar la clave en cada petición con el
//!   encabezado `Authorization: Bearer <clave>` o `X-Tlaku-Token`.
//! - Toda comparación de secretos es de **tiempo constante** ([`ct_eq`]) y la búsqueda de sesión
//!   recorre todas, sin cortar al primer acierto.
//! - Los intentos fallidos con una clave incorrecta se cuentan por dirección: pasado el límite, esa
//!   dirección queda bloqueada un rato, incluso si luego manda la clave buena.
//! - Regenerar la clave borra todas las sesiones: los clientes conectados quedan expulsados.
//!
//! Nada aquí toca la red ni el reloj: el tiempo se recibe como parámetro para poder probarlo.

use serde::Serialize;
use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// Intentos fallidos permitidos por dirección dentro de [`FAIL_WINDOW`].
pub const MAX_FAILS: u32 = 5;
/// Ventana en la que se cuentan los fallos.
pub const FAIL_WINDOW: Duration = Duration::from_secs(60);
/// Cuánto queda bloqueada una dirección al pasarse del límite.
pub const LOCKOUT: Duration = Duration::from_secs(120);
/// Sesiones simultáneas máximas; al llenarse sale la que lleva más tiempo sin actividad.
pub const MAX_SESSIONS: usize = 8;
/// Una sesión sin actividad ni conexión abierta caduca pasado este tiempo.
pub const SESSION_IDLE: Duration = Duration::from_secs(30 * 60);
/// Un cliente cuenta como "conectado" si habló hace menos de esto. La conexión en vivo manda un
/// latido cada 10 s, así que un celular que perdió la red deja de contar pronto aunque su conexión
/// siga abierta del lado de la PC.
pub const PRESENT_WINDOW: Duration = Duration::from_secs(20);
/// Direcciones distintas que se recuerdan para el límite de intentos.
const MAX_TRACKED_IPS: usize = 256;

/// Bytes al azar del sistema operativo, en hexadecimal.
pub fn random_hex(bytes: usize) -> String {
    let mut buf = vec![0u8; bytes];
    // Si el sistema no puede dar aleatoriedad no hay forma segura de continuar.
    getrandom::fill(&mut buf).expect("el sistema no pudo generar números aleatorios");
    buf.iter().map(|b| format!("{b:02x}")).collect()
}

/// Compara dos secretos sin cortar al primer byte distinto: el tiempo depende solo del largo.
pub fn ct_eq(a: &[u8], b: &[u8]) -> bool {
    let mut diff = u8::from(a.len() != b.len());
    for i in 0..a.len().max(b.len()) {
        diff |= a.get(i).copied().unwrap_or(0) ^ b.get(i).copied().unwrap_or(0);
    }
    std::hint::black_box(diff) == 0
}

/// Por qué no se aceptó una petición.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthError {
    /// Sin credenciales, o sesión desconocida o expulsada.
    Unauthorized,
    /// Clave incorrecta.
    BadToken,
    /// Demasiados fallos desde esa dirección; faltan estos segundos.
    Locked { retry_after_s: u64 },
}

/// Un cliente conectado, tal como lo ve la PC.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ClientView {
    /// Identificador corto (público): sirve para desconectarlo.
    pub id: String,
    pub ip: String,
    /// Aparato y navegador aproximados (`iPhone · Safari`).
    pub agent: String,
    /// Cuándo entró, en milisegundos desde 1970.
    pub since_ms: u64,
    /// Hace cuántos segundos hizo algo.
    pub idle_s: u64,
    /// Habló hace poco (la conexión en vivo manda un latido cada pocos segundos).
    pub connected: bool,
}

struct Session {
    /// Valor de la cookie (secreto).
    secret: String,
    id: String,
    ip: IpAddr,
    agent: String,
    /// Entró con la clave en un encabezado (sin cookie).
    via_header: bool,
    created_ms: u64,
    last_seen: Instant,
    streams: usize,
}

#[derive(Default)]
struct Fails {
    count: u32,
    first: Option<Instant>,
    locked_until: Option<Instant>,
}

struct Inner {
    token: String,
    sessions: Vec<Session>,
    fails: HashMap<IpAddr, Fails>,
}

/// Estado de autenticación del servidor.
pub struct Auth {
    inner: Mutex<Inner>,
}

fn unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

impl Auth {
    /// Con una clave nueva al azar.
    pub fn new() -> Self {
        Self::with_token(random_hex(16))
    }

    pub fn with_token(token: String) -> Self {
        Self {
            inner: Mutex::new(Inner {
                token,
                sessions: Vec::new(),
                fails: HashMap::new(),
            }),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        // Un pánico en otro hilo no debe dejar el servidor sin poder autenticar.
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// La clave vigente (para el QR).
    pub fn token(&self) -> String {
        self.lock().token.clone()
    }

    /// Clave nueva: la anterior deja de valer y todas las sesiones se cierran.
    pub fn regenerate(&self) -> String {
        let mut g = self.lock();
        g.token = random_hex(16);
        g.sessions.clear();
        g.fails.clear();
        g.token.clone()
    }

    fn prune(g: &mut Inner, now: Instant) {
        g.sessions
            .retain(|s| s.streams > 0 || now.saturating_duration_since(s.last_seen) < SESSION_IDLE);
        g.fails.retain(|_, f| {
            f.locked_until.is_some_and(|t| t > now)
                || f.first
                    .is_some_and(|t| now.saturating_duration_since(t) < FAIL_WINDOW)
        });
    }

    fn locked(g: &Inner, ip: IpAddr, now: Instant) -> Option<u64> {
        let until = g.fails.get(&ip)?.locked_until?;
        (until > now).then(|| (until - now).as_secs() + 1)
    }

    fn register_fail(g: &mut Inner, ip: IpAddr, now: Instant) {
        if !g.fails.contains_key(&ip) && g.fails.len() >= MAX_TRACKED_IPS {
            // Tabla llena: sale la dirección con el fallo más viejo.
            if let Some(old) = g.fails.iter().min_by_key(|(_, f)| f.first).map(|(k, _)| *k) {
                g.fails.remove(&old);
            }
        }
        let f = g.fails.entry(ip).or_default();
        match f.first {
            Some(t) if now.saturating_duration_since(t) < FAIL_WINDOW => f.count += 1,
            _ => {
                f.first = Some(now);
                f.count = 1;
            }
        }
        if f.count >= MAX_FAILS {
            f.locked_until = Some(now + LOCKOUT);
        }
    }

    fn check_token(
        g: &mut Inner,
        ip: IpAddr,
        presented: &str,
        now: Instant,
    ) -> Result<(), AuthError> {
        if let Some(retry_after_s) = Self::locked(g, ip, now) {
            return Err(AuthError::Locked { retry_after_s });
        }
        if ct_eq(presented.as_bytes(), g.token.as_bytes()) {
            g.fails.remove(&ip);
            Ok(())
        } else {
            Self::register_fail(g, ip, now);
            Err(AuthError::BadToken)
        }
    }

    fn new_session(
        g: &mut Inner,
        ip: IpAddr,
        agent: &str,
        via_header: bool,
        now: Instant,
    ) -> usize {
        if g.sessions.len() >= MAX_SESSIONS
            && let Some(oldest) = g
                .sessions
                .iter()
                .enumerate()
                .min_by_key(|(_, s)| s.last_seen)
                .map(|(i, _)| i)
        {
            g.sessions.remove(oldest);
        }
        let mut id = random_hex(4);
        while g.sessions.iter().any(|s| s.id == id) {
            id = random_hex(4);
        }
        g.sessions.push(Session {
            secret: random_hex(16),
            id,
            ip,
            agent: friendly_agent(agent),
            via_header,
            created_ms: unix_ms(),
            last_seen: now,
            streams: 0,
        });
        g.sessions.len() - 1
    }

    /// `POST /api/login`: cambia la clave por una sesión. Devuelve `(id de la sesión, cookie)`.
    pub fn login(
        &self,
        ip: IpAddr,
        agent: &str,
        presented: &str,
        now: Instant,
    ) -> Result<(String, String), AuthError> {
        let mut g = self.lock();
        Self::prune(&mut g, now);
        Self::check_token(&mut g, ip, presented, now)?;
        let i = Self::new_session(&mut g, ip, agent, false, now);
        Ok((g.sessions[i].id.clone(), g.sessions[i].secret.clone()))
    }

    /// Autentica una petición: por cookie de sesión o, si no la hay, por la clave en un encabezado.
    /// Devuelve el id de la sesión. Una cookie desconocida (de un cliente expulsado) no cuenta como
    /// intento fallido: es un secreto de 128 bits, no se puede adivinar.
    pub fn authenticate(
        &self,
        ip: IpAddr,
        agent: &str,
        cookie: Option<&str>,
        header_token: Option<&str>,
        now: Instant,
    ) -> Result<String, AuthError> {
        let mut g = self.lock();
        Self::prune(&mut g, now);
        if let Some(c) = cookie {
            // Se recorren todas aunque ya haya acertado una: el tiempo no revela cuál fue.
            let mut found = None;
            for (i, s) in g.sessions.iter().enumerate() {
                if ct_eq(c.as_bytes(), s.secret.as_bytes()) {
                    found = Some(i);
                }
            }
            if let Some(i) = found {
                g.sessions[i].last_seen = now;
                return Ok(g.sessions[i].id.clone());
            }
        }
        let Some(token) = header_token else {
            return Err(AuthError::Unauthorized);
        };
        Self::check_token(&mut g, ip, token, now)?;
        // Un cliente que manda la clave en cada petición comparte sesión por dirección y aparato.
        let agent = friendly_agent(agent);
        if let Some(s) = g
            .sessions
            .iter_mut()
            .find(|s| s.via_header && s.ip == ip && s.agent == agent)
        {
            s.last_seen = now;
            return Ok(s.id.clone());
        }
        let i = Self::new_session(&mut g, ip, &agent, true, now);
        Ok(g.sessions[i].id.clone())
    }

    /// ¿Sigue viva la sesión? (la conexión en vivo lo pregunta para cortarse al expulsarla).
    pub fn is_alive(&self, id: &str) -> bool {
        self.lock().sessions.iter().any(|s| s.id == id)
    }

    /// Anota actividad de una sesión (la conexión en vivo lo hace con cada latido).
    pub fn touch(&self, id: &str, now: Instant) {
        if let Some(s) = self.lock().sessions.iter_mut().find(|s| s.id == id) {
            s.last_seen = now;
        }
    }

    /// Una conexión en vivo se abrió; `false` si la sesión ya no existe.
    pub fn stream_open(&self, id: &str, now: Instant) -> bool {
        match self.lock().sessions.iter_mut().find(|s| s.id == id) {
            Some(s) => {
                s.streams += 1;
                s.last_seen = now;
                true
            }
            None => false,
        }
    }

    pub fn stream_close(&self, id: &str, now: Instant) {
        if let Some(s) = self.lock().sessions.iter_mut().find(|s| s.id == id) {
            s.streams = s.streams.saturating_sub(1);
            s.last_seen = now;
        }
    }

    /// Etiqueta legible de un cliente (`192.168.1.9 · iPhone · Safari`).
    pub fn label(&self, id: &str) -> Option<String> {
        self.lock()
            .sessions
            .iter()
            .find(|s| s.id == id)
            .map(|s| format!("{} · {}", s.ip, s.agent))
    }

    /// Desconecta a un cliente por su id. `true` si existía.
    pub fn kick(&self, id: &str) -> bool {
        let mut g = self.lock();
        let before = g.sessions.len();
        g.sessions.retain(|s| s.id != id);
        g.sessions.len() != before
    }

    /// Quién está en la lista (los más recientes primero).
    pub fn clients(&self, now: Instant) -> Vec<ClientView> {
        let mut g = self.lock();
        Self::prune(&mut g, now);
        let mut list: Vec<_> = g
            .sessions
            .iter()
            .map(|s| {
                let idle = now.saturating_duration_since(s.last_seen);
                ClientView {
                    id: s.id.clone(),
                    ip: s.ip.to_string(),
                    agent: s.agent.clone(),
                    since_ms: s.created_ms,
                    idle_s: idle.as_secs(),
                    connected: idle < PRESENT_WINDOW,
                }
            })
            .collect();
        list.sort_by(|a, b| b.since_ms.cmp(&a.since_ms));
        list
    }

    /// Cuántos clientes están conectados ahora (la cámara solo se manda si hay alguno).
    pub fn connected_count(&self, now: Instant) -> usize {
        self.clients(now).iter().filter(|c| c.connected).count()
    }
}

impl Default for Auth {
    fn default() -> Self {
        Self::new()
    }
}

/// Aparato y navegador a partir del `User-Agent`, para reconocer al cliente en la lista de la PC.
/// Es solo una etiqueta: nada de seguridad depende de ella.
pub fn friendly_agent(ua: &str) -> String {
    let device = if ua.contains("iPhone") {
        "iPhone"
    } else if ua.contains("iPad") {
        "iPad"
    } else if ua.contains("Android") {
        "Android"
    } else if ua.contains("Windows") {
        "Windows"
    } else if ua.contains("Macintosh") {
        "Mac"
    } else if ua.contains("Linux") {
        "Linux"
    } else {
        let plain: String = ua.chars().filter(|c| !c.is_control()).take(40).collect();
        let plain = plain.trim();
        return if plain.is_empty() {
            tlaku_i18n::tr!("desktop.remote.browser")
        } else {
            plain.to_string()
        };
    };
    let browser = if ua.contains("Edg/") || ua.contains("EdgA/") {
        "Edge"
    } else if ua.contains("Firefox/") || ua.contains("FxiOS/") {
        "Firefox"
    } else if ua.contains("CriOS/") || ua.contains("Chrome/") {
        "Chrome"
    } else if ua.contains("Safari/") {
        "Safari"
    } else {
        ""
    };
    if browser.is_empty() {
        device.to_string()
    } else {
        format!("{device} · {browser}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ip(s: &str) -> IpAddr {
        s.parse().unwrap()
    }

    fn auth() -> Auth {
        Auth::with_token("clave-buena".into())
    }

    #[test]
    fn tokens_are_random_and_long_enough() {
        let a = random_hex(16);
        let b = random_hex(16);
        assert_eq!(a.len(), 32);
        assert_ne!(a, b);
        assert!(a.chars().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(Auth::new().token(), Auth::new().token());
    }

    #[test]
    fn comparison_is_exact_and_length_aware() {
        assert!(ct_eq(b"abc", b"abc"));
        assert!(ct_eq(b"", b""));
        assert!(!ct_eq(b"abc", b"abd"));
        assert!(!ct_eq(b"abc", b"abcd"));
        assert!(!ct_eq(b"abcd", b"abc"));
        // Los bytes que faltan se toman como 0, pero el largo distinto ya los delata.
        assert!(!ct_eq(b"abc", b"abc\0"));
    }

    #[test]
    fn login_with_the_right_token_gives_a_session_cookie() {
        let a = auth();
        let now = Instant::now();
        let (id, cookie) = a
            .login(ip("192.168.1.9"), "iPhone Safari/604", "clave-buena", now)
            .unwrap();
        assert_ne!(cookie, "clave-buena", "la cookie no es la clave");
        assert_eq!(
            a.authenticate(ip("192.168.1.9"), "", Some(&cookie), None, now)
                .unwrap(),
            id
        );
        assert!(a.is_alive(&id));
    }

    #[test]
    fn requests_without_credentials_or_with_wrong_ones_are_refused() {
        let a = auth();
        let now = Instant::now();
        let who = ip("192.168.1.9");
        assert_eq!(
            a.authenticate(who, "", None, None, now),
            Err(AuthError::Unauthorized)
        );
        assert_eq!(
            a.authenticate(who, "", Some("cookie-inventada"), None, now),
            Err(AuthError::Unauthorized)
        );
        assert_eq!(
            a.authenticate(who, "", None, Some("clave-mala"), now),
            Err(AuthError::BadToken)
        );
        assert_eq!(
            a.login(who, "", "otra", now).unwrap_err(),
            AuthError::BadToken
        );
        assert!(a.clients(now).is_empty());
    }

    #[test]
    fn the_token_in_a_header_works_on_every_request_and_shares_one_session() {
        let a = auth();
        let now = Instant::now();
        let who = ip("192.168.1.30");
        let first = a
            .authenticate(who, "curl/8", None, Some("clave-buena"), now)
            .unwrap();
        let again = a
            .authenticate(who, "curl/8", None, Some("clave-buena"), now)
            .unwrap();
        assert_eq!(first, again);
        assert_eq!(a.clients(now).len(), 1);
        // Otro aparato es otro cliente.
        let other = a
            .authenticate(ip("192.168.1.31"), "curl/8", None, Some("clave-buena"), now)
            .unwrap();
        assert_ne!(first, other);
        assert_eq!(a.clients(now).len(), 2);
    }

    #[test]
    fn five_wrong_tokens_lock_the_address_even_for_the_right_one() {
        let a = auth();
        let t0 = Instant::now();
        let bad = ip("192.168.1.66");
        for i in 0..MAX_FAILS {
            assert_eq!(
                a.login(bad, "", "x", t0 + Duration::from_secs(u64::from(i))),
                Err(AuthError::BadToken)
            );
        }
        let t1 = t0 + Duration::from_secs(10);
        match a.login(bad, "", "clave-buena", t1) {
            Err(AuthError::Locked { retry_after_s }) => {
                assert!(retry_after_s > 100, "{retry_after_s}")
            }
            other => panic!("debía estar bloqueada: {other:?}"),
        }
        // También por encabezado.
        assert!(matches!(
            a.authenticate(bad, "", None, Some("clave-buena"), t1),
            Err(AuthError::Locked { .. })
        ));
        // Otra dirección no se afecta.
        assert!(a.login(ip("192.168.1.67"), "", "clave-buena", t1).is_ok());
        // Pasado el bloqueo vuelve a funcionar.
        assert!(
            a.login(
                bad,
                "",
                "clave-buena",
                t0 + LOCKOUT + Duration::from_secs(10)
            )
            .is_ok()
        );
    }

    #[test]
    fn failures_outside_the_window_do_not_accumulate_and_success_resets() {
        let a = auth();
        let t0 = Instant::now();
        let who = ip("192.168.1.70");
        for i in 0..MAX_FAILS - 1 {
            let _ = a.login(who, "", "x", t0 + Duration::from_secs(u64::from(i)));
        }
        // Un acierto borra la cuenta.
        assert!(
            a.login(who, "", "clave-buena", t0 + Duration::from_secs(5))
                .is_ok()
        );
        for i in 0..MAX_FAILS - 1 {
            let _ = a.login(who, "", "x", t0 + Duration::from_secs(6 + u64::from(i)));
        }
        assert!(
            a.login(who, "", "clave-buena", t0 + Duration::from_secs(12))
                .is_ok()
        );
        // Fallos separados por más que la ventana no suman.
        let who = ip("192.168.1.71");
        for k in 0..MAX_FAILS * 2 {
            let at = t0 + FAIL_WINDOW * (k + 1) + Duration::from_secs(1);
            assert_eq!(a.login(who, "", "x", at), Err(AuthError::BadToken));
        }
    }

    #[test]
    fn regenerating_invalidates_the_old_token_and_expels_everyone() {
        let a = auth();
        let now = Instant::now();
        let (id, cookie) = a.login(ip("192.168.1.9"), "", "clave-buena", now).unwrap();
        let old = a.token();
        let new = a.regenerate();
        assert_ne!(old, new);
        assert!(!a.is_alive(&id));
        assert_eq!(
            a.authenticate(ip("192.168.1.9"), "", Some(&cookie), None, now),
            Err(AuthError::Unauthorized)
        );
        assert_eq!(
            a.login(ip("192.168.1.9"), "", &old, now),
            Err(AuthError::BadToken)
        );
        assert!(a.login(ip("192.168.1.9"), "", &new, now).is_ok());
    }

    #[test]
    fn kicking_removes_only_that_client() {
        let a = auth();
        let now = Instant::now();
        let (one, c1) = a.login(ip("192.168.1.9"), "", "clave-buena", now).unwrap();
        let (two, c2) = a.login(ip("192.168.1.10"), "", "clave-buena", now).unwrap();
        assert!(a.kick(&one));
        assert!(!a.kick(&one));
        assert!(
            a.authenticate(ip("192.168.1.9"), "", Some(&c1), None, now)
                .is_err()
        );
        assert_eq!(
            a.authenticate(ip("192.168.1.10"), "", Some(&c2), None, now)
                .unwrap(),
            two
        );
    }

    #[test]
    fn presence_follows_streams_and_activity() {
        let a = auth();
        let t0 = Instant::now();
        let (id, _) = a
            .login(
                ip("192.168.1.9"),
                "Mozilla (iPhone) Safari/1",
                "clave-buena",
                t0,
            )
            .unwrap();
        assert_eq!(a.connected_count(t0), 1);
        // Sin actividad deja de contar como conectado, pero sigue en la lista.
        let later = t0 + PRESENT_WINDOW + Duration::from_secs(5);
        assert_eq!(a.connected_count(later), 0);
        assert_eq!(a.clients(later).len(), 1);
        // Un latido lo devuelve a "conectado"; con la conexión abierta la sesión no caduca.
        assert!(a.stream_open(&id, later));
        assert_eq!(a.connected_count(later), 1);
        let much_later = t0 + SESSION_IDLE + Duration::from_secs(300);
        assert_eq!(
            a.clients(much_later).len(),
            1,
            "con la conexión abierta no caduca"
        );
        assert_eq!(
            a.connected_count(much_later),
            0,
            "pero sin latidos ya no cuenta como conectado"
        );
        a.stream_close(&id, much_later);
        assert!(!a.stream_open("no-existe", much_later));
        // Caduca tras mucho tiempo sin actividad.
        assert!(
            a.clients(much_later + SESSION_IDLE + Duration::from_secs(1))
                .is_empty()
        );
    }

    #[test]
    fn the_session_table_is_bounded() {
        let a = auth();
        let t0 = Instant::now();
        let mut first = None;
        for i in 0..=MAX_SESSIONS {
            let (id, _) = a
                .login(
                    ip("192.168.1.9"),
                    "",
                    "clave-buena",
                    t0 + Duration::from_secs(i as u64),
                )
                .unwrap();
            first.get_or_insert(id);
        }
        let now = t0 + Duration::from_secs(MAX_SESSIONS as u64);
        assert_eq!(a.clients(now).len(), MAX_SESSIONS);
        assert!(!a.is_alive(&first.unwrap()), "sale la sesión más vieja");
    }

    #[test]
    fn agents_get_a_readable_label() {
        let iphone = "Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) AppleWebKit/605.1.15 Version/17.0 Mobile/15E148 Safari/604.1";
        assert_eq!(friendly_agent(iphone), "iPhone · Safari");
        let android = "Mozilla/5.0 (Linux; Android 14; Pixel 8) AppleWebKit/537.36 Chrome/120.0 Mobile Safari/537.36";
        assert_eq!(friendly_agent(android), "Android · Chrome");
        assert_eq!(friendly_agent("curl/8.4.0"), "curl/8.4.0");
        assert_eq!(friendly_agent(""), "Navegador");
    }
}
