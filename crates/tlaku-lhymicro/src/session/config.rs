//! Configuración de la sesión.

use crate::encode::{EncodeConfig, HomeCorner};
use crate::speed::Board;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct SessionConfig {
    pub board: Board,
    /// Ancho y alto de la cama (mm).
    pub bed_mm: [f64; 2],
    pub home: HomeCorner,
    pub autolock: bool,
    /// Velocidad máxima de corte (mm/s).
    pub max_speed_mm_s: f64,
    /// Forzar el modo sin rampa de aceleración (ver [`EncodeConfig::constant_speed`]).
    pub constant_speed: bool,
    pub fix_speeds: bool,
    /// Reenvíos de un paquete rechazado por CRC antes de rendirse.
    pub crc_retries: u32,
    /// Espera máxima a que la placa esté lista o confirme un paquete. Un paquete puede encerrar
    /// un movimiento largo y lento, así que es generosa.
    pub busy_timeout: Duration,
    /// Espera máxima de las órdenes en tiempo real (pausa, reanudar, detener).
    pub realtime_timeout: Duration,
    /// Sondeos seguidos sin un estado utilizable (0, truncado o desconocido) antes de abortar.
    pub anomaly_polls: u32,
    /// Pausa máxima entre sondeos cuando la placa tarda (tras los primeros diez).
    pub poll_backoff_max: Duration,
    /// Esperar el estado 236 en cada `-` del flujo (como MeerK40t). Si se desactiva, solo
    /// manda la espera por "lista" de cada paquete.
    pub strict_finish: bool,
    /// Espera máxima del home.
    pub home_timeout: Duration,
    /// Margen (s) sobre el tiempo estimado de un salto antes de darlo por terminado.
    pub motion_margin_s: f64,
}

impl Default for SessionConfig {
    fn default() -> Self {
        let e = EncodeConfig::default();
        SessionConfig {
            board: e.board,
            bed_mm: e.bed_mm,
            home: e.home,
            autolock: e.autolock,
            max_speed_mm_s: e.max_speed_mm_s,
            constant_speed: false,
            fix_speeds: false,
            crc_retries: 3,
            busy_timeout: Duration::from_secs(600),
            realtime_timeout: Duration::from_secs(5),
            anomaly_polls: 200,
            poll_backoff_max: Duration::from_millis(50),
            strict_finish: true,
            home_timeout: Duration::from_secs(60),
            motion_margin_s: 3.0,
        }
    }
}
