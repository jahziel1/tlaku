//! Tipos de comando Ruida: opciones, `Command` y errores de decodificación.

use crate::num::NumError;
use std::fmt;

/// Mínima o máxima de una potencia.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Limit {
    Min,
    Max,
}

/// Subcódigos de `D8`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessOp {
    /// `D8 00` iniciar proceso.
    Start,
    /// `D8 01` detener.
    Stop,
    /// `D8 02` pausar.
    Pause,
    /// `D8 03` reanudar.
    Resume,
    /// `D8 10` coordenadas respecto al cero de la máquina (`REF_POINT_MACHINE` en ruida-pa,
    /// verificado en una RDC6442S). El ejemplo 1 empieza con `D8 10` + `F0`.
    RefPoint,
    /// `D8 11` coordenadas respecto al origen del usuario (ancla).
    RefOrigin,
    /// `D8 12` coordenadas respecto a la posición actual (la usan los `.rd` de RDWorks).
    RefCurrent,
    /// `D8 2A` ir al home XY (ruida-pa y MeerK40t, verificado en RDC6442S).
    HomeXY,
    /// `D8 2C` home Z.
    HomeZ,
    /// `D8 2D` home U.
    HomeU,
    /// `D8 2E` enfocar Z.
    FocusZ,
    /// Cualquier otro subcódigo (sin documentar).
    Other(u8),
}

impl ProcessOp {
    pub fn sub(self) -> u8 {
        match self {
            ProcessOp::Start => 0x00,
            ProcessOp::Stop => 0x01,
            ProcessOp::Pause => 0x02,
            ProcessOp::Resume => 0x03,
            ProcessOp::RefPoint => 0x10,
            ProcessOp::RefOrigin => 0x11,
            ProcessOp::RefCurrent => 0x12,
            ProcessOp::HomeXY => 0x2A,
            ProcessOp::HomeZ => 0x2C,
            ProcessOp::HomeU => 0x2D,
            ProcessOp::FocusZ => 0x2E,
            ProcessOp::Other(s) => s,
        }
    }

    pub fn from_sub(s: u8) -> Self {
        match s {
            0x00 => ProcessOp::Start,
            0x01 => ProcessOp::Stop,
            0x02 => ProcessOp::Pause,
            0x03 => ProcessOp::Resume,
            0x10 => ProcessOp::RefPoint,
            0x11 => ProcessOp::RefOrigin,
            0x12 => ProcessOp::RefCurrent,
            0x2A => ProcessOp::HomeXY,
            0x2C => ProcessOp::HomeZ,
            0x2D => ProcessOp::HomeU,
            0x2E => ProcessOp::FocusZ,
            other => ProcessOp::Other(other),
        }
    }
}

/// Respecto a qué se interpreta la coordenada de un salto `D9` (byte de opciones). El bit 0 del
/// byte (`1`/`3`) enciende el "haz de luz" del láser y **no se genera nunca**: un `D9` con ese
/// bit se decodifica como [`Command::Unknown`] y la comprobación de seguridad lo rechaza.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RapidRef {
    /// Opciones `00`: coordenada absoluta desde el cero de la máquina.
    Machine,
    /// Opciones `02`: desplazamiento desde la posición actual.
    Current,
}

impl RapidRef {
    pub fn byte(self) -> u8 {
        match self {
            RapidRef::Machine => 0x00,
            RapidRef::Current => 0x02,
        }
    }

    pub fn from_byte(b: u8) -> Option<Self> {
        match b {
            0x00 => Some(RapidRef::Machine),
            0x02 => Some(RapidRef::Current),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    // ----- Movimiento y corte (coordenadas en µm) -----
    /// `88 X(5) Y(5)`: mover rápido, láser apagado.
    MoveAbs { x: i64, y: i64 },
    /// `89 dX(2) dY(2)`.
    MoveRel { dx: i32, dy: i32 },
    /// `8A dX(2)`.
    MoveRelX(i32),
    /// `8B dY(2)`.
    MoveRelY(i32),
    /// `A8 X(5) Y(5)`: cortar con el láser encendido.
    CutAbs { x: i64, y: i64 },
    /// `A9 dX(2) dY(2)`.
    CutRel { dx: i32, dy: i32 },
    /// `AA dX(2)`.
    CutRelX(i32),
    /// `AB dY(2)`.
    CutRelY(i32),

    // ----- Potencia y velocidad -----
    /// `C6 01/02` (láser 1), `C6 21/22` (2), `C6 05/06` (3), `C6 07/08` (4): `P(2)` (14 bits).
    LaserPower { laser: u8, limit: Limit, raw: u16 },
    /// `C6 31/32` (láser 1), `41/42` (2), `35/36` (3), `37/38` (4): `capa P(2)`.
    LayerPower {
        laser: u8,
        limit: Limit,
        layer: u8,
        raw: u16,
    },
    /// `C7 P(2)`: potencia inmediata del láser 1 (`IMD_POWER_1`). Fija la potencia de los
    /// cortes siguientes; así se escribe la potencia por píxel de una imagen (ruida-pa la
    /// verifica en una RDC6442S y la usa en su relleno ráster).
    ImmPower(u16),
    /// `C8 P(2)`: potencia final del láser 1 (`END_POWER_1`). Sin semántica verificada: solo
    /// se decodifica, nunca se genera.
    EndPower(u16),
    /// `C9 02 V(5)`: velocidad en µm/s.
    Speed(u64),
    /// `C9 04 capa V(5)`.
    LayerSpeed { layer: u8, um_s: u64 },
    /// `C6 10..13` (intervalo, retardo añadido, retardo de encendido y apagado) y `C6 15/16`
    /// (segundo par de retardos, `LASER_ON/OFF_DELAY2` en ruida-pa): `T(5)`.
    LaserTime { sub: u8, micros: u64 },
    /// `C6 60 láser capa F(5)`: frecuencia en Hz.
    LayerFrequency { laser: u8, layer: u8, hz: u64 },

    // ----- Capas y aire -----
    /// `CA 01 00`: fin de capa.
    EndLayer,
    /// `CA 01 12`: aire apagado.
    AirOff,
    /// `CA 01 13`: aire encendido.
    AirOn,
    /// Otro `CA 01 xx` (modos de trabajo, dispositivo `10`/`11`, `30`/`31`...).
    LayerFlag(u8),
    /// `CA 02 capa`: capa actual.
    LayerSelect(u8),
    /// `CA 05 color(5)`.
    Color(u64),
    /// `CA 06 capa color(5)`: color BBGGRR.
    LayerColor { layer: u8, color: u64 },
    /// `CA 22 capa`: número máximo de capa.
    MaxLayer(u8),
    /// `CA 41 capa atributos` (`LAYER_ATTRIBUTES` en ruida-pa): 0 en todos los archivos vistos.
    LayerMode { layer: u8, mode: u8 },

    // ----- Archivo y proceso -----
    /// `D8 xx`.
    Process(ProcessOp),
    /// `E7 00`: fin de bloque.
    EndBlock,
    /// `D7`: fin de archivo.
    EndOfFile,
    /// `E6 01`: modo absoluto.
    Absolute,
    /// `F0`: fijar punto de referencia.
    SetReference,
    /// `E5 05 suma(5)` (`END_JOB`/`SET_FILE_SUM`): suma de verificación del archivo. Ver
    /// [`crate::rd::FileChecksum`] para qué bytes se suman.
    FileChecksum(u64),
    /// `E7 01 nombre\0`.
    FileName(String),
    /// `DA 00 dirección(2)`: consulta de memoria (solo lectura).
    MemRead { addr: u16 },
    /// `DA 01 dirección(2) valor(5)...`: escritura de memoria o respuesta de una lectura (un
    /// valor). En una escritura lleva dos valores (el pie de los `.rd` de RDWorks escribe la
    /// longitud de corte en `0x320`).
    MemWrite { addr: u16, values: Vec<u64> },

    // ----- Estructura de la cabecera (significado sin documentar) -----
    /// `op sub X(5) Y(5)`: `E7 03/06/07/13/17/23/50/51`, `F1 03`, `F2 03/04/06`.
    Point2 { op: u8, sub: u8, x: i64, y: i64 },
    /// `E7 52/53/61/62 parte X(5) Y(5)`.
    PartPoint { sub: u8, part: u8, x: i64, y: i64 },
    /// `E7 54/55 n valor(5)`: offset de pluma / de capa.
    PartAbs { sub: u8, n: u8, v: i64 },
    /// Matriz de copias `op sub columnas(2) filas(2) paso_x(5) paso_y(5)`: `E7 04` (trabajo),
    /// `E7 08` (matriz) y `F2 05` (elemento). Los nombres salen de ruida-pa (`JOB_COPIES`,
    /// `ARRAY_COPIES`, `ELEMENT_COPIES`); los siete valores "sin sentido" del ejemplo de
    /// EduTech (`1,1,-1,-710,-7808,4,7597`) son en realidad 1 columna, 1 fila y pasos de
    /// -90,813 mm y 73,133 mm (el tamaño del dibujo más 3 µm). Con 1x1 los pasos no se usan.
    Copies {
        op: u8,
        sub: u8,
        cols: u16,
        rows: u16,
        step_x: i64,
        step_y: i64,
    },
    /// `op sub n`: `E7 05/24/60`, `F1 00/01/02`, `F2 00/01/07`, `CA 03`, `CA 10`.
    Small { op: u8, sub: u8, n: u8 },
    /// `EB` (sin parámetros): fin de matriz (`ARRAY_END`, verificado en RDC6442S).
    Marker(u8),
    /// `EA n`: inicio de matriz (`ARRAY_START`).
    ShortArg { op: u8, n: u8 },
    /// `F2 02` + 10 bytes de 7 bits: nombre del elemento, 8 caracteres ASCII empaquetados de
    /// 4 en 4 en dos valores de 35 bits (ver [`crate::rd::encode_element_name`]).
    ElementName([u8; 10]),

    // ----- Saltos rápidos con el láser apagado (ruida-pa y MeerK40t, verificado en RDC6442S) -----
    /// `D9 00 opciones X(5)`: salto solo en X.
    RapidX { rel: RapidRef, v: i64 },
    /// `D9 01 opciones Y(5)`: salto solo en Y.
    RapidY { rel: RapidRef, v: i64 },
    /// `D9 10 opciones X(5) Y(5)`: salto en XY.
    RapidXY { rel: RapidRef, x: i64, y: i64 },

    /// Opcode o subcódigo no reconocido: se conservan los bytes tal cual.
    Unknown { op: u8, data: Vec<u8> },
}

#[derive(Debug, Clone, PartialEq)]
pub enum DecodeError {
    /// El flujo no empieza con un byte de opcode (bit 7 en 1).
    NotAnOpcode { offset: usize, byte: u8 },
    /// El comando conocido tiene un número de bytes de datos distinto del esperado.
    BadLength {
        offset: usize,
        op: u8,
        expected: usize,
        found: usize,
    },
    /// Un número no se pudo decodificar.
    Num { offset: usize, source: NumError },
    /// El valor no es válido para ese comando.
    Invalid { offset: usize, what: &'static str },
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DecodeError::NotAnOpcode { offset, byte } => write!(
                f,
                "byte {offset}: 0x{byte:02X} no es un opcode (bit 7 en 0); el flujo debe empezar en un comando"
            ),
            DecodeError::BadLength {
                offset,
                op,
                expected,
                found,
            } => write!(
                f,
                "byte {offset}: el comando 0x{op:02X} debería llevar {expected} bytes de datos y lleva {found}"
            ),
            DecodeError::Num { offset, source } => write!(f, "byte {offset}: {source}"),
            DecodeError::Invalid { offset, what } => write!(f, "byte {offset}: {what}"),
        }
    }
}

impl std::error::Error for DecodeError {}
