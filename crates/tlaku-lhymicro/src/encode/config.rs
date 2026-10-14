//! Esquina de home y configuración del codificador.

use crate::speed::{Board, MM_PER_MIL};
use tlaku_core::Point;

/// Esquina de la cama donde está el home (los finales de carrera) en el sistema de Tlaku
/// (origen abajo a la izquierda, Y hacia arriba). El K40 de serie lo tiene arriba a la izquierda.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HomeCorner {
    #[default]
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

impl HomeCorner {
    pub fn parse(s: &str) -> Option<HomeCorner> {
        Some(match s.trim().to_ascii_lowercase().as_str() {
            "top-left" => HomeCorner::TopLeft,
            "top-right" => HomeCorner::TopRight,
            "bottom-left" => HomeCorner::BottomLeft,
            "bottom-right" => HomeCorner::BottomRight,
            _ => return None,
        })
    }

    pub fn name(self) -> &'static str {
        match self {
            HomeCorner::TopLeft => "top-left",
            HomeCorner::TopRight => "top-right",
            HomeCorner::BottomLeft => "bottom-left",
            HomeCorner::BottomRight => "bottom-right",
        }
    }

    fn right(self) -> bool {
        matches!(self, HomeCorner::TopRight | HomeCorner::BottomRight)
    }

    fn top(self) -> bool {
        matches!(self, HomeCorner::TopLeft | HomeCorner::TopRight)
    }

    /// Punto de Tlaku (mm) a distancia desde el home en cada eje de la placa (mm). Los ejes de
    /// la placa crecen alejándose del home.
    pub fn to_machine_mm(self, p: Point, bed_mm: [f64; 2]) -> (f64, f64) {
        let x = if self.right() { bed_mm[0] - p.x } else { p.x };
        let y = if self.top() { bed_mm[1] - p.y } else { p.y };
        (x, y)
    }

    /// Inverso de [`HomeCorner::to_machine_mm`].
    pub fn from_machine_mm(self, m: (f64, f64), bed_mm: [f64; 2]) -> Point {
        let x = if self.right() { bed_mm[0] - m.0 } else { m.0 };
        let y = if self.top() { bed_mm[1] - m.1 } else { m.1 };
        Point::new(x, y)
    }

    /// Desplazamiento de Tlaku (mm) a desplazamiento de la placa (mm).
    pub fn delta_to_machine(self, dx: f64, dy: f64) -> (f64, f64) {
        (
            if self.right() { -dx } else { dx },
            if self.top() { -dy } else { dy },
        )
    }
}

/// Mils a milímetros.
pub fn mil_to_mm(mil: i64) -> f64 {
    mil as f64 * MM_PER_MIL
}

/// Milímetros al mil más cercano.
pub fn mm_to_mil(mm: f64) -> i64 {
    (mm / MM_PER_MIL).round() as i64
}

#[derive(Debug, Clone)]
pub struct EncodeConfig {
    pub board: Board,
    /// Ancho y alto de la cama (mm).
    pub bed_mm: [f64; 2],
    pub home: HomeCorner,
    /// Dónde está el cabezal al empezar (mils, ejes de la placa). `(0, 0)` justo después del home.
    pub start_mil: (i64, i64),
    /// Dejar el riel bloqueado tras cada movimiento (lo normal). Con `false` se añade `IS2P`.
    pub autolock: bool,
    /// Velocidad máxima de corte (mm/s). Lo que pase de aquí es un error, no se recorta.
    pub max_speed_mm_s: f64,
    /// Un viaje más corto que esto (mils) se hace dentro del programa con el láser apagado; uno
    /// más largo sale del modo compacto y usa un salto rápido. 256 mil son 6,5 mm.
    pub min_jog_mil: i64,
    /// Forzar el modo sin rampa de aceleración (sufijo `C`). Por defecto se usa el engranaje que
    /// escoge el software chino para cada velocidad y `C` solo por debajo de 7 mm/s.
    pub constant_speed: bool,
    /// Usar la pendiente medida por MeerK40t en vez de la del software chino.
    pub fix_speeds: bool,
    pub d_ratio: Option<f64>,
}

impl Default for EncodeConfig {
    fn default() -> Self {
        EncodeConfig {
            board: Board::M2,
            // Cama habitual del K40 (área útil ~ 300 x 200).
            bed_mm: [300.0, 200.0],
            home: HomeCorner::TopLeft,
            start_mil: (0, 0),
            autolock: true,
            max_speed_mm_s: 140.0,
            min_jog_mil: 256,
            constant_speed: false,
            fix_speeds: false,
            d_ratio: None,
        }
    }
}
