//! Opciones del trabajo `.rd`, esquina de home y errores.

use crate::num::NumError;
use crate::safety::{SafetyError, SafetyLimits};
use std::fmt;
use tlaku_core::Point;

/// Capas posibles (int7).
pub const MAX_LAYERS: usize = 128;

/// Nombre de elemento que escribe RDWorks en todos los archivos vistos (8 caracteres).
pub const ELEMENT_NAME: &str = "UNNAMED ";

/// Empaqueta 8 caracteres ASCII para `F2 02`: cuatro caracteres por cada valor de 35 bits
/// (32 bits útiles, big endian) y cada valor en 5 bytes de 7 bits. `UNNAMED ` da
/// `05 2A 39 1C 41 04 6A 15 08 20`, los bytes de todos los `.rd` de RDWorks vistos.
pub fn encode_element_name(name: &str) -> Option<[u8; 10]> {
    let b = name.as_bytes();
    if b.len() != 8 || !b.is_ascii() {
        return None;
    }
    let mut out = [0u8; 10];
    for half in 0..2 {
        let v = u32::from_be_bytes([
            b[4 * half],
            b[4 * half + 1],
            b[4 * half + 2],
            b[4 * half + 3],
        ]);
        for k in 0..5 {
            out[5 * half + k] = ((v >> (7 * (4 - k))) & 0x7F) as u8;
        }
    }
    Some(out)
}

/// Inversa de [`encode_element_name`]; los bytes no imprimibles salen como `?`.
pub fn decode_element_name(bytes: &[u8; 10]) -> String {
    let mut s = String::with_capacity(8);
    for half in 0..2 {
        let mut v = 0u64;
        for &b in &bytes[5 * half..5 * half + 5] {
            v = (v << 7) | u64::from(b & 0x7F);
        }
        for c in (v as u32).to_be_bytes() {
            s.push(if (0x20..0x7F).contains(&c) {
                c as char
            } else {
                '?'
            });
        }
    }
    s
}

/// Esquina de la cama donde está el home de la máquina. Las coordenadas de un `.rd` con
/// `D8 10` (cero de la máquina) son distancias desde el home hacia dentro de la cama
/// (ruida-pa nombra las cajas como "top right" y MeerK40t necesita "Flip X" en una máquina con
/// el home a la derecha). Tlaku usa origen abajo a la izquierda con Y hacia arriba, así que el
/// perfil debe decir dónde está el home. Sin hardware no hay un valor universal: el defecto es
/// arriba a la izquierda (lo más común) y la prueba 2 de ruida.md 10.6 lo confirma.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HomeCorner {
    #[default]
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

impl HomeCorner {
    fn flips(self) -> (bool, bool) {
        match self {
            HomeCorner::TopLeft => (false, true),
            HomeCorner::TopRight => (true, true),
            HomeCorner::BottomLeft => (false, false),
            HomeCorner::BottomRight => (true, false),
        }
    }

    /// Punto de Tlaku (mm, abajo-izquierda, Y arriba) a coordenadas de la máquina (mm).
    pub fn to_machine(self, p: Point, bed_mm: [f64; 2]) -> (f64, f64) {
        let (fx, fy) = self.flips();
        (
            if fx { bed_mm[0] - p.x } else { p.x },
            if fy { bed_mm[1] - p.y } else { p.y },
        )
    }

    /// Inversa de [`HomeCorner::to_machine`].
    pub fn from_machine(self, x: f64, y: f64, bed_mm: [f64; 2]) -> Point {
        let (fx, fy) = self.flips();
        Point::new(
            if fx { bed_mm[0] - x } else { x },
            if fy { bed_mm[1] - y } else { y },
        )
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().replace('_', "-").as_str() {
            "top-left" | "tl" => Some(HomeCorner::TopLeft),
            "top-right" | "tr" => Some(HomeCorner::TopRight),
            "bottom-left" | "bl" => Some(HomeCorner::BottomLeft),
            "bottom-right" | "br" => Some(HomeCorner::BottomRight),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            HomeCorner::TopLeft => "top-left",
            HomeCorner::TopRight => "top-right",
            HomeCorner::BottomLeft => "bottom-left",
            HomeCorner::BottomRight => "bottom-right",
        }
    }
}

/// Qué hacer con un corte a potencia 0 dentro de un bloque de potencia variable (el overscan).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ZeroPower {
    /// Salto con el láser apagado (`88/89`): lo que hacen MeerK40t y ruida-pa.
    #[default]
    Travel,
    /// `C7 0` y corte: el cabezal recorre el tramo a la velocidad de la capa con potencia 0.
    /// Sin verificar en hardware.
    Cut,
}

/// Ajustes de una capa que el `Toolpath` no trae.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct LayerOptions {
    /// Color RGB de la capa (en el archivo va como BBGGRR).
    pub color: [u8; 3],
    /// Potencia mínima en %. `None` = igual a la máxima (potencia constante). Con valores
    /// distintos la controladora escala con la velocidad (comportamiento de foros, ruida.md 4.2;
    /// ruida-pa lo modela igual: a poca velocidad sube hacia el máximo). Se ignora en los
    /// bloques de potencia variable.
    pub min_power_pct: Option<f64>,
}

/// Qué suma `E5 05` lleva al final del trabajo.
///
/// MeerK40t (MIT, probado en una RDC6442S) escribe la suma de **todos** los bytes de los
/// comandos del trabajo más `0xD7`; ruida-pa (MIT) la describe igual (excluye lecturas `DA 00`
/// y teclas `A7`, y el propio `E5 05`). Con capturas de LightBurn el resultado difiere de forma
/// constante en 220: falta algún byte por identificar. Los `.rd` de RDWorks no llevan `E5 05`
/// y funcionan en todas las máquinas, así que el defecto es no emitirlo; no se sabe si el
/// firmware rechaza una suma errónea (ruida-pa dice que "puede mostrar un mensaje").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FileChecksum {
    /// No emitir `E5 05` (como RDWorks).
    #[default]
    None,
    /// Suma de los bytes de todos los comandos anteriores más `0xD7` (fórmula de MeerK40t).
    SumAll,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RuidaJobOptions {
    /// Límites de seguridad; se comprueban antes y después de generar.
    pub limits: SafetyLimits,
    /// Ajustes por bloque del `Toolpath` (mismo índice); los que falten usan los de por defecto.
    pub layers: Vec<LayerOptions>,
    /// Esquina del home de la máquina (define el origen y el signo de X e Y del archivo).
    pub home: HomeCorner,
    /// Desplazamiento en mm que se suma a todas las coordenadas (ya en el sistema de la máquina).
    pub offset_mm: [f64; 2],
    /// Usar movimientos relativos cuando el salto cabe en 14 bits con signo (+-8,191 mm).
    pub use_relative: bool,
    /// Usar `8A/8B/AA/AB` cuando un solo eje cambia. Fuentes menos firmes que `89/A9`: apagado.
    pub use_axis_shortcuts: bool,
    pub file_checksum: FileChecksum,
    /// Emitir `DA 01 06 20 L L` en el pie, con L = longitud de corte en mm (inferencia propia a
    /// partir del ejemplo: 327 mm de perímetro). Es una **escritura de memoria** que MeerK40t
    /// no emite: apagado.
    pub footer_stats: bool,
    /// Potencia de los láseres 2 a 4 en %. 0 por seguridad; el ejemplo usa ~30 %.
    pub secondary_laser_power_pct: f64,
    /// `CA 03 n`: tubos activos al empezar. MeerK40t escribe 1 (solo el láser 1, verificado en
    /// una RDC6442S); RDWorks escribe 0x0D o 0x0F según la configuración de la máquina.
    pub laser_tube_start: u8,
    /// Cortes a potencia 0 de los bloques de potencia variable.
    pub zero_power: ZeroPower,
}

impl Default for RuidaJobOptions {
    fn default() -> Self {
        RuidaJobOptions {
            limits: SafetyLimits::default(),
            layers: Vec::new(),
            home: HomeCorner::default(),
            offset_mm: [0.0, 0.0],
            use_relative: true,
            use_axis_shortcuts: false,
            file_checksum: FileChecksum::None,
            footer_stats: false,
            secondary_laser_power_pct: 0.0,
            laser_tube_start: 1,
            zero_power: ZeroPower::Travel,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum RdError {
    Safety(SafetyError),
    Num(NumError),
    /// No hay ningún bloque con cortes.
    EmptyJob,
    /// Más bloques con cortes que capas posibles (128).
    TooManyLayers {
        layers: usize,
    },
    /// La potencia mínima configurada es mayor que la máxima del bloque.
    MinPowerAboveMax {
        block: usize,
    },
    BadOption(&'static str),
}

impl fmt::Display for RdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RdError::Safety(e) => write!(f, "{e}"),
            RdError::Num(e) => write!(f, "{e}"),
            RdError::EmptyJob => write!(f, "el trabajo no tiene ningún corte"),
            RdError::TooManyLayers { layers } => write!(
                f,
                "el trabajo tiene {layers} capas con cortes y Ruida admite {MAX_LAYERS}"
            ),
            RdError::MinPowerAboveMax { block } => write!(
                f,
                "bloque {block}: la potencia mínima configurada supera la potencia del bloque"
            ),
            RdError::BadOption(w) => write!(f, "opción inválida: {w}"),
        }
    }
}

impl std::error::Error for RdError {}

impl From<SafetyError> for RdError {
    fn from(e: SafetyError) -> Self {
        RdError::Safety(e)
    }
}

impl From<NumError> for RdError {
    fn from(e: NumError) -> Self {
        RdError::Num(e)
    }
}
