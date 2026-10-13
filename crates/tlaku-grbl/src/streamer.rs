//! Envío de trabajos a GRBL 1.1 con conteo de caracteres, comandos en tiempo
//! real y parser del reporte de estado; con un [`LinkProfile`] de Marlin o Smoothieware
//! envía en ping-pong (una línea por `ok`).
//!
//! El conteo de caracteres lleva los bytes que cada línea ocupa en el búfer de
//! recepción de la controladora y solo envía otra mientras quepa; cada `ok` o
//! `error:N` libera la línea más antigua. Así el búfer se mantiene lleno sin
//! desbordarse (a diferencia del modo "una línea por vez", que se entrecorta).
//!
//! Qué cambia entre familias (control de flujo, consulta de estado, pausa, cancelación, fin
//! del trabajo, saludo de reinicio) lo decide el [`LinkProfile`]; `Streamer::new` usa el de
//! GRBL y se comporta como siempre. Para otro firmware: [`Streamer::with_link`] o
//! [`Streamer::for_device`].

use std::collections::VecDeque;
use std::fmt;
use std::io;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use tlaku_doc::DeviceProfile;

use crate::link::{
    CancelStyle, ControlAction, FinishDetection, FlowControl, LinkFamily, LinkProfile,
    numbered_line,
};
use crate::serial::{LineReader, Transport};

/// Espera máxima de cada lectura; acota la latencia de pausa, cancelación y sondeo.
const TICK: Duration = Duration::from_millis(5);

/// Comandos en tiempo real de GRBL 1.1: un solo byte, sin `\n` y fuera del conteo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Realtime {
    StatusQuery,
    FeedHold,
    CycleStart,
    SoftReset,
    SafetyDoor,
    JogCancel,
    FeedReset,
    FeedPlus10,
    FeedMinus10,
    FeedPlus1,
    FeedMinus1,
    RapidFull,
    RapidHalf,
    RapidQuarter,
    SpindleReset,
    SpindlePlus10,
    SpindleMinus10,
    SpindlePlus1,
    SpindleMinus1,
    SpindleStop,
    ToggleFlood,
    ToggleMist,
}

impl Realtime {
    pub const fn byte(self) -> u8 {
        match self {
            Self::StatusQuery => b'?',
            Self::FeedHold => b'!',
            Self::CycleStart => b'~',
            Self::SoftReset => 0x18,
            Self::SafetyDoor => 0x84,
            Self::JogCancel => 0x85,
            Self::FeedReset => 0x90,
            Self::FeedPlus10 => 0x91,
            Self::FeedMinus10 => 0x92,
            Self::FeedPlus1 => 0x93,
            Self::FeedMinus1 => 0x94,
            Self::RapidFull => 0x95,
            Self::RapidHalf => 0x96,
            Self::RapidQuarter => 0x97,
            Self::SpindleReset => 0x99,
            Self::SpindlePlus10 => 0x9A,
            Self::SpindleMinus10 => 0x9B,
            Self::SpindlePlus1 => 0x9C,
            Self::SpindleMinus1 => 0x9D,
            Self::SpindleStop => 0x9E,
            Self::ToggleFlood => 0xA0,
            Self::ToggleMist => 0xA1,
        }
    }
}

// ---------------------------------------------------------------------------
// Reporte de estado
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MachineState {
    Idle,
    Run,
    /// Subcódigo: 0 = detención completa, 1 = deteniéndose.
    Hold(Option<u8>),
    Jog,
    Alarm,
    /// Subcódigo: 0 = puerta cerrada y lista para reanudar; 1 = máquina detenida con la puerta
    /// aún entreabierta (no se puede reanudar); 2 = puerta abierta, con hold o retracción en
    /// curso (un reset lanzaría alarma); 3 = puerta cerrada y reanudando (un reset lanzaría
    /// alarma).
    Door(Option<u8>),
    Check,
    Home,
    Sleep,
    /// Estado que Tlaku no conoce (p. ej. `Tool` de grblHAL). El reporte no se descarta: el
    /// nombre original queda en [`StatusReport::other_state`].
    Other,
}

impl MachineState {
    /// `true` si, con este estado, la máquina no se está moviendo y un soft reset conserva la
    /// posición. `Hold` sin subcódigo, `Hold:1`, `Door:2`/`Door:3`, `Run`, `Jog`, `Home` y los
    /// estados desconocidos cuentan como "puede estar en movimiento".
    pub fn is_stationary(self) -> bool {
        matches!(
            self,
            Self::Idle
                | Self::Alarm
                | Self::Check
                | Self::Sleep
                | Self::Hold(Some(0))
                | Self::Door(Some(0 | 1))
        )
    }
}

/// Coordenadas de la máquina. `z` vale 0 si el reporte solo trae dos ejes;
/// los ejes adicionales (A, B, ...) van en `extra`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Position {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub extra: Vec<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BufferState {
    /// Bloques libres en el planificador de movimiento.
    pub planner_blocks: u32,
    /// Bytes libres en el búfer de recepción serie.
    pub rx_bytes: u32,
}

/// Porcentajes de override activos.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Overrides {
    pub feed: u32,
    pub rapid: u32,
    pub spindle: u32,
}

/// Reporte de estado (`<Idle|MPos:...|...>`). Los campos ausentes quedan en `None`.
#[derive(Debug, Clone, PartialEq)]
pub struct StatusReport {
    pub state: MachineState,
    /// Nombre original del estado cuando es [`MachineState::Other`].
    pub other_state: Option<String>,
    pub mpos: Option<Position>,
    pub wpos: Option<Position>,
    pub wco: Option<Position>,
    pub buffer: Option<BufferState>,
    pub line_number: Option<u32>,
    /// Velocidad de avance actual (de `FS:` o `F:`).
    pub feed: Option<f64>,
    /// Velocidad/potencia del husillo (solo en `FS:`).
    pub spindle: Option<f64>,
    pub overrides: Option<Overrides>,
    /// Pines activos (`Pn:`), p. ej. `XYZPDHRS`.
    pub pins: Option<String>,
    /// Accesorios activos (`A:`), p. ej. `SFM`.
    pub accessories: Option<String>,
}

/// Número finito. `nan`, `-nan` e `inf` se leen bien con `parse::<f64>()`, pero no son una
/// posición ni un valor válido: el firmware de Creality llega a imprimir `-nan` en G54 y
/// `$#`. Se tratan como desconocidos.
fn finite(s: &str) -> Option<f64> {
    s.trim().parse::<f64>().ok().filter(|v| v.is_finite())
}

/// Lista de números finitos separados por comas; falla entera si alguno no lo es.
fn finite_list(s: &str) -> Option<Vec<f64>> {
    s.split(',').map(finite).collect()
}

fn parse_position(s: &str) -> Option<Position> {
    let v = finite_list(s)?;
    if v.len() < 2 {
        return None;
    }
    Some(Position {
        x: v[0],
        y: v[1],
        z: v.get(2).copied().unwrap_or(0.0),
        extra: v.iter().skip(3).copied().collect(),
    })
}

/// Estado y nombre original si es desconocido. Un subcódigo que no sea un número se ignora
/// en lugar de descartar el reporte.
fn parse_state(s: &str, has_fields: bool) -> Option<(MachineState, Option<String>)> {
    let (name, sub) = match s.split_once(':') {
        Some((n, c)) => (n.trim(), c.trim().parse::<u8>().ok()),
        None => (s.trim(), None),
    };
    let known = match name {
        "Idle" => MachineState::Idle,
        "Run" => MachineState::Run,
        "Hold" => MachineState::Hold(sub),
        "Jog" => MachineState::Jog,
        "Alarm" => MachineState::Alarm,
        "Door" => MachineState::Door(sub),
        "Check" => MachineState::Check,
        "Home" => MachineState::Home,
        "Sleep" => MachineState::Sleep,
        // Un nombre desconocido pero con forma de estado y seguido de campos (p. ej.
        // `<Tool|MPos:...>`) no tira el reporte; la basura sí (vacío, símbolos, demasiado
        // largo, o un `<Nope>` sin campos).
        other
            if has_fields
                && (1..=16).contains(&other.len())
                && other.bytes().all(|b| b.is_ascii_alphabetic()) =>
        {
            return Some((MachineState::Other, Some(other.to_string())));
        }
        _ => return None,
    };
    Some((known, None))
}

impl StatusReport {
    /// Interpreta una línea `<...>`. Devuelve `None` si no es un reporte o no trae un estado
    /// con forma válida; los campos desconocidos o mal formados se ignoran, y los valores
    /// `nan`/`inf` cuentan como mal formados (campo desconocido, nunca una posición válida).
    pub fn parse(line: &str) -> Option<Self> {
        let inner = line.trim().strip_prefix('<')?.strip_suffix('>')?;
        let mut fields = inner.split('|');
        let (state, other_state) = parse_state(fields.next()?, inner.contains('|'))?;
        let mut report = StatusReport {
            state,
            other_state,
            mpos: None,
            wpos: None,
            wco: None,
            buffer: None,
            line_number: None,
            feed: None,
            spindle: None,
            overrides: None,
            pins: None,
            accessories: None,
        };
        for field in fields {
            let Some((key, value)) = field.split_once(':') else {
                continue;
            };
            match key {
                "MPos" => report.mpos = parse_position(value),
                "WPos" => report.wpos = parse_position(value),
                "WCO" => report.wco = parse_position(value),
                "Bf" => {
                    if let Some(&[a, b]) = finite_list(value).as_deref() {
                        report.buffer = Some(BufferState {
                            planner_blocks: a as u32,
                            rx_bytes: b as u32,
                        });
                    }
                }
                "Ln" => report.line_number = value.trim().parse().ok(),
                // grblHAL manda tres valores (`FS:avance,S programada,S real`); aquí
                // interesan los dos primeros.
                "FS" => {
                    if let Some(&[f, s] | &[f, s, _]) = finite_list(value).as_deref() {
                        report.feed = Some(f);
                        report.spindle = Some(s);
                    }
                }
                "F" => report.feed = finite(value),
                "Ov" => {
                    if let Some(&[f, r, s]) = finite_list(value).as_deref() {
                        report.overrides = Some(Overrides {
                            feed: f as u32,
                            rapid: r as u32,
                            spindle: s as u32,
                        });
                    }
                }
                "Pn" => report.pins = Some(value.trim().to_string()),
                "A" => report.accessories = Some(value.trim().to_string()),
                _ => {}
            }
        }
        Some(report)
    }
}

impl StatusReport {
    /// Interpreta el informe de estado con comas de GRBL 0.9 y de Smoothieware:
    /// `<Idle,MPos:0.0000,0.0000,0.0000,WPos:0.0000,0.0000,0.0000>`. Solo trae estado y
    /// posiciones. El formato de `?` de Smoothie no está confirmado por su documentación
    /// (`dialectos-gcode.md`, 3.2, y 7 para GRBL 0.9): contrástalo con una máquina real.
    pub fn parse_legacy(line: &str) -> Option<Self> {
        let inner = line.trim().strip_prefix('<')?.strip_suffix('>')?;
        let mut tokens = inner.split(',');
        let (state, other_state) = parse_state(tokens.next()?, inner.contains(','))?;
        let mut report = StatusReport {
            state,
            other_state,
            mpos: None,
            wpos: None,
            wco: None,
            buffer: None,
            line_number: None,
            feed: None,
            spindle: None,
            overrides: None,
            pins: None,
            accessories: None,
        };
        // Los valores sin clave pertenecen al campo `clave:` anterior.
        let mut fields: Vec<(&str, Vec<&str>)> = Vec::new();
        for t in tokens {
            match t.split_once(':') {
                Some((k, v)) => fields.push((k, vec![v])),
                None => {
                    if let Some(f) = fields.last_mut() {
                        f.1.push(t);
                    }
                }
            }
        }
        for (key, vals) in fields {
            let pos = parse_position(&vals.join(","));
            match key {
                "MPos" => report.mpos = pos,
                "WPos" => report.wpos = pos,
                _ => {}
            }
        }
        Some(report)
    }
}

/// Línea de `$#` (`[G54:x,y,z]`, `[TLO:z]`, `[PRB:x,y,z:1]`...). Devuelve el nombre y los
/// valores, o `None` en los valores si alguno es `nan`/`inf`/ilegible: el firmware de la
/// Falcon2 llega a imprimir `[G54:-nan,1.579,1.619]`, que no es una posición válida.
pub fn parse_parameter_line(line: &str) -> Option<(String, Option<Vec<f64>>)> {
    let inner = line.trim().strip_prefix('[')?.strip_suffix(']')?;
    let (name, rest) = inner.split_once(':')?;
    if name.is_empty() || !name.bytes().all(|b| b.is_ascii_alphanumeric()) {
        return None;
    }
    // `PRB` añade `:éxito` tras las coordenadas.
    let values = rest.split(':').next().unwrap_or("");
    Some((name.to_string(), finite_list(values)))
}

// ---------------------------------------------------------------------------
// Mensajes de error y alarma
// ---------------------------------------------------------------------------

/// Familia del firmware. Los números de `error:N` y `ALARM:N` por encima de los de GRBL 1.1
/// significan cosas distintas según la familia (p. ej. `ALARM:10` es autoescuadre en GRBL 1.1
/// y paro de emergencia en grblHAL), así que el texto depende de ella.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Firmware {
    /// Aún no identificado.
    #[default]
    Unknown,
    /// GRBL 1.1 y derivados que dicen `[VER:...]` sin más (Arduino, Falcon2, GRBL-LPC...).
    Grbl,
    GrblHal,
    FluidNc,
}

impl Firmware {
    /// Familia a partir de las líneas de `$I` (`[VER:...]`, `[OPT:...]`, `[NEWOPT:...]`).
    /// grblHAL se reconoce por su nombre o por `[NEWOPT:` ([I]: inferido de la documentación
    /// para remitentes); FluidNC por su nombre en `[VER:...]`.
    pub fn detect<S: AsRef<str>>(lines: &[S]) -> Self {
        let mut family = Self::Unknown;
        for l in lines {
            let l = l.as_ref().trim();
            let low = l.to_ascii_lowercase();
            if low.contains("fluidnc") {
                return Self::FluidNc;
            }
            if low.contains("grblhal") || low.starts_with("[newopt:") {
                return Self::GrblHal;
            }
            if low.starts_with("[ver:") {
                family = Self::Grbl;
            }
        }
        family
    }

    /// Familia a partir de una línea de saludo (`GrblHAL ...`, `Grbl 3.4 [FluidNC v3.4.3 ...`).
    /// `None` si el saludo no la delata (un `Grbl 1.1h` no distingue nada).
    pub fn from_banner(line: &str) -> Option<Self> {
        let low = line.to_ascii_lowercase();
        if low.contains("fluidnc") {
            Some(Self::FluidNc)
        } else if low.starts_with("grblhal") {
            Some(Self::GrblHal)
        } else {
            None
        }
    }
}

/// Descripción de un `error:N` de GRBL 1.1 (1 a 17 y 20 a 38), válida en todas las familias.
fn classic_error(code: u32) -> Option<&'static str> {
    Some(match code {
        1 => "falta la letra del comando G-code",
        2 => "formato de número incorrecto",
        3 => "comando '$' no válido",
        4 => "valor negativo no permitido",
        5 => "ajuste desactivado",
        6 => "pulso de paso demasiado corto",
        7 => "fallo al leer la memoria no volátil",
        8 => "la máquina no está en reposo",
        9 => "G-code bloqueado por alarma o jog",
        10 => "límite blando excedido",
        11 => "línea demasiado larga para el búfer de la controladora",
        12 => "velocidad de pasos máxima excedida",
        13 => "puerta abierta",
        14 => "longitud de línea excedida",
        15 => "recorrido del jog excedido",
        16 => "comando de jog no válido",
        17 => "el modo láser requiere salida PWM",
        20 => "comando G-code no soportado o no válido",
        21 => "varios comandos del mismo grupo modal en una línea",
        22 => "velocidad de avance sin definir",
        23 => "el comando requiere un valor entero",
        24 => "dos comandos usan palabras de eje a la vez",
        25 => "palabra G-code repetida",
        26 => "el comando requiere palabras de eje",
        27 => "número de línea no válido",
        28 => "falta un valor requerido",
        29 => "G59.x no soportado",
        30 => "G53 solo es válido con G0 y G1",
        31 => "hay palabras de eje que ningún comando usa",
        32 => "G2/G3 requieren al menos un eje del plano",
        33 => "destino del movimiento no válido",
        34 => "radio de arco no válido",
        35 => "G2/G3 requieren al menos un desplazamiento del plano",
        36 => "hay palabras con valor sin usar",
        37 => "compensación de herramienta G43.1 no asignada al eje configurado",
        38 => "número de herramienta mayor al máximo",
        _ => return None,
    })
}

/// `error:N` de grblHAL por encima de GRBL 1.1 (nombres de su API pública; la documentación
/// consultada no trae descripciones, así que los textos son una traducción fiel del nombre).
fn hal_error(code: u32) -> Option<&'static str> {
    Some(match code {
        18 => "la entrada de reset debe estar desactivada",
        19 => "valor no positivo",
        39 => "valor de G-code fuera de rango",
        40 => "cambio de herramienta pendiente",
        41 => "el husillo no está en marcha",
        42 => "plano no válido",
        43 => "avance máximo excedido",
        44 => "velocidad del husillo fuera de rango",
        45 => "límites activados",
        46 => "hace falta homing ($H)",
        47 => "error de herramienta",
        48 => "conflicto entre palabras de valor",
        49 => "autodiagnóstico fallido",
        50 => "parada de emergencia activa",
        51 => "fallo de motor",
        52 => "valor del ajuste fuera de rango",
        53 => "ajuste desactivado",
        54 => "posición de retracción no válida",
        55 => "configuración de homing no válida",
        56 => "sistema de coordenadas bloqueado",
        57 => "delimitador inesperado",
        58 => "puerto auxiliar no disponible",
        60..=66 => "error de la tarjeta SD o del sistema de ficheros",
        70 => "fallo al iniciar Bluetooth",
        71..=76 => "error en una expresión del G-code",
        77..=79 => "acceso denegado o autenticación requerida",
        80..=83 => "error de flujo de control (macro, sintaxis, pila o memoria)",
        84..=89 => "error de ficheros o de herramienta en el husillo",
        90..=92 => "error de compensación de cortador",
        _ => return None,
    })
}

/// `error:N` de FluidNC por encima de GRBL 1.1 ([T]: lista de terceros, sin confirmar con su
/// documentación oficial). FluidNC ofrece `$E=N` para consultar el texto en la propia máquina.
fn fluidnc_error(code: u32) -> Option<&'static str> {
    Some(match code {
        19 => "homing de un solo eje no permitido",
        39 => "el parámetro P es demasiado grande (va en segundos, no en milisegundos)",
        60..=69 => "error de la tarjeta SD",
        70 | 71 => "fallo al iniciar Bluetooth o WiFi",
        80 | 81 => "ajuste fuera de rango o valor no válido",
        90 => "fallo al enviar un mensaje",
        100 | 101 => "fallo al guardar o leer un ajuste",
        110 => "falló la autenticación",
        120 => "otra interfaz está ocupada",
        152 => "configuración no válida",
        _ => return None,
    })
}

/// Texto de un `error:N` extendido cuyo significado depende del firmware.
const EXTENDED_ERROR: &str = "error extendido del firmware (grblHAL o FluidNC): su significado depende de la familia; \
     en FluidNC consulta $E=N";

/// Descripción breve de un `error:N` sin conocer el firmware. Los códigos de GRBL 1.1 tienen
/// su texto; los extendidos (18, 19 y desde el 39) devuelven un texto genérico, porque cambian
/// de significado según la familia. Ver [`error_message_for`].
pub fn error_message(code: u32) -> &'static str {
    error_message_for(Firmware::Unknown, code)
}

/// Descripción breve de un `error:N` según la familia del firmware.
pub fn error_message_for(fw: Firmware, code: u32) -> &'static str {
    if let Some(m) = classic_error(code) {
        return m;
    }
    match fw {
        Firmware::GrblHal => hal_error(code),
        Firmware::FluidNc => fluidnc_error(code),
        Firmware::Grbl | Firmware::Unknown => None,
    }
    .unwrap_or(if code >= 18 && fw != Firmware::Grbl {
        EXTENDED_ERROR
    } else {
        "error desconocido"
    })
}

/// Descripción de un `ALARM:N` sin conocer el firmware. El 10 es ambiguo (autoescuadre en
/// GRBL 1.1, paro de emergencia en grblHAL); ver [`alarm_message_for`].
pub fn alarm_message(code: u32) -> &'static str {
    alarm_message_for(Firmware::Unknown, code)
}

/// Descripción breve de un `ALARM:N` según la familia del firmware.
pub fn alarm_message_for(fw: Firmware, code: u32) -> &'static str {
    match code {
        1 => "límite físico activado",
        2 => "el movimiento excede el recorrido de la máquina",
        3 => "reinicio durante un movimiento: se perdió la posición",
        4 => "fallo de sonda: estado inicial inesperado",
        5 => "fallo de sonda: sin contacto",
        6 => "fallo de homing: reinicio durante el ciclo",
        7 => "fallo de homing: puerta abierta durante el ciclo",
        8 => "fallo de homing: no se pudo liberar el final de carrera",
        9 => "fallo de homing: no se encontró el final de carrera",
        10 => match fw {
            Firmware::Grbl => {
                "fallo de homing: no se encontró el segundo final de carrera (autoescuadre)"
            }
            Firmware::GrblHal => {
                "parada de emergencia activa: suelta el botón antes de desbloquear"
            }
            Firmware::FluidNc => "fallo del control del husillo (típicamente RS485)",
            Firmware::Unknown => {
                "depende del firmware: parada de emergencia (grblHAL), fallo del control del \
                 husillo (FluidNC) o autoescuadre sin segundo final de carrera (GRBL 1.1)"
            }
        },
        // Solo grblHAL define estos números (nombres de su API pública, sin descripciones).
        11 => "(grblHAL) hace falta homing ($H)",
        12 => "(grblHAL) límites activados",
        13 => "(grblHAL) protección de sonda",
        14 => "(grblHAL) fallo del husillo: no alcanzó la velocidad o falló su control",
        15 => "(grblHAL) autoescuadre: no se encontró el segundo final de carrera al acercarse",
        16 => "(grblHAL) autodiagnóstico fallido",
        17 => "(grblHAL) fallo de motor (lazo cerrado)",
        18 => "(grblHAL) fallo de homing",
        19 => "(grblHAL) excepción de Modbus",
        20 => "(grblHAL) excepción del expansor de E/S",
        21 => "(grblHAL) fallo de la memoria no volátil",
        22 => "(grblHAL) desbordamiento de búfer",
        _ => "alarma desconocida",
    }
}

// ---------------------------------------------------------------------------
// Fallas propias de Creality (`ERROR:NN`)
// ---------------------------------------------------------------------------

/// `ERROR:NN` de Creality (Falcon2): mayúsculas y dos dígitos, **no** es el `error:N` de GRBL
/// y no responde a ninguna línea enviada. Tolera variantes como `ERROR:06.` (con punto) o
/// `ERROR:06. texto en inglés`. Devuelve el código; `error:5` de GRBL no lo activa.
pub fn parse_creality_error(line: &str) -> Option<u32> {
    let rest = line.trim().strip_prefix("ERROR:")?.trim_start();
    let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
    if digits == 0 || digits > 3 {
        return None;
    }
    // Lo que siga debe ser un separador (`.`, `,`, espacio...), no más texto pegado.
    let after = rest[digits..].chars().next();
    if after.is_some_and(|c| c.is_alphanumeric()) {
        return None;
    }
    rest[..digits].parse().ok()
}

/// Significado de un `ERROR:NN` de Creality según su tabla oficial (resumida).
pub fn creality_error_message(code: u32) -> &'static str {
    match code {
        1 => "ángulo anormal: el sensor de inclinación o vibración se activó",
        2 => "no hay archivo G-code en la raíz de la tarjeta TF",
        4 => "sin flujo de aire: revisa la bomba, la perilla de caudal y el tubo",
        5 => "llama detectada (alarma de llama, $154)",
        6 => "lente contaminada (alarma de lente, $155): límpiala",
        7 => "módulo láser demasiado caliente",
        8..=12 => {
            "fallo de un sensor del módulo (presión de aire, llama, lente, temperatura o \
             cortocircuito del diodo láser)"
        }
        13 => "temperatura alta de la CPU del módulo láser",
        14 => "temperatura alta de la CPU del anfitrión",
        24 => "máquina no sellada: cierra la tapa y el cajón",
        25 => "bomba de aire anormal",
        _ => "falla propia de la máquina (código de Creality sin descripción conocida)",
    }
}

/// Qué hacer ante un `ERROR:NN` de Creality. Tlaku no envía `$X` ni nada por su cuenta.
pub const CREALITY_RECOVERY: &str = "Corrige la causa y reinicia la máquina con su botón Reset \
     (y Start en el panel si lo pide); Tlaku no la desbloquea por su cuenta.";

// ---------------------------------------------------------------------------
// Preparación de líneas
// ---------------------------------------------------------------------------

/// Quita comentarios (`;...` y `(...)`) y espacios. Devuelve cadena vacía si no
/// queda nada. Falla con el carácter ofensor si queda uno que GRBL interpretaría
/// como comando en tiempo real (`?`, `!`, `~`, no ASCII) o de control.
pub fn prepare_line(raw: &str) -> Result<String, char> {
    prepare_line_with(raw, true, true)
}

/// Como [`prepare_line`] para cualquier familia: `strip_spaces = false` conserva un espacio
/// entre palabras (Marlin y Smoothie necesitan `M3 S0 I`, no `M3S0I`) y `reject_realtime =
/// false` admite `?`, `!` y `~` (en Marlin no son comandos en tiempo real). Los caracteres no
/// ASCII y de control siempre se rechazan.
pub fn prepare_line_with(
    raw: &str,
    strip_spaces: bool,
    reject_realtime: bool,
) -> Result<String, char> {
    let mut out = String::with_capacity(raw.len());
    let mut in_paren = false;
    for c in raw.chars() {
        match c {
            ';' if !in_paren => break,
            '(' => in_paren = true,
            ')' => in_paren = false,
            _ if in_paren => {}
            _ if c.is_whitespace() => {
                if !strip_spaces && !out.is_empty() && !out.ends_with(' ') {
                    out.push(' ');
                }
            }
            '?' | '!' | '~' if reject_realtime => return Err(c),
            _ if !c.is_ascii() || c.is_control() => return Err(c),
            _ => out.push(c),
        }
    }
    if out.ends_with(' ') {
        out.pop();
    }
    Ok(out)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StreamError {
    /// La línea (con su `\n`) no cabe en el búfer de recepción.
    LineTooLong {
        source_line: usize,
        len: usize,
        max: usize,
    },
    InvalidChar {
        source_line: usize,
        ch: char,
    },
}

impl fmt::Display for StreamError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LineTooLong {
                source_line,
                len,
                max,
            } => write!(
                f,
                "la línea {source_line} ocupa {len} bytes y el búfer de la controladora es de {max}"
            ),
            Self::InvalidChar { source_line, ch } => write!(
                f,
                "la línea {source_line} contiene {ch:?}, que GRBL interpretaría como comando en tiempo real"
            ),
        }
    }
}

impl std::error::Error for StreamError {}

struct Prepared {
    /// Texto sin el `\n` final.
    text: String,
    /// Número de línea (desde 1) en la entrada original.
    source_line: usize,
    /// Escribe en la EEPROM: se envía sola y se espera su `ok` antes de seguir (ver
    /// [`writes_eeprom`]).
    sync: bool,
    /// Línea del propio streamer (`M110 N0` de las líneas numeradas): no cuenta en el avance.
    silent: bool,
}

/// Marca de las líneas pendientes que no son del trabajo (`M0`, `M108`, `M400` de cierre...):
/// su `ok` libera el búfer pero no cuenta como avance.
const EXTRA: usize = usize::MAX;

/// Nota cuando se detuvo con una parada rápida (Marlin, Smoothie): no hay reinicio en caliente
/// ni desaceleración controlada.
pub const QUICK_STOP_NOTE: &str = "Se detuvo con una parada rápida (M410 en Marlin, M112 en \
     Smoothie): los motores frenan sin desaceleración y la posición puede haberse perdido. Vuelve \
     a fijar el origen o haz homing (G28) antes del siguiente trabajo.";

/// ¿La línea (ya sin espacios ni comentarios) escribe en la EEPROM de la controladora? Son
/// los ajustes `$x=valor` (incluidos `$Nx=`, `$I=` y `$RST=`) y los G-codes `G10`, `G28.1` y
/// `G30.1`. Durante la escritura GRBL deshabilita la interrupción de recepción serie (hasta
/// unos 20 ms) y puede perder bytes, así que estas líneas no se envían con conteo de
/// caracteres: se mandan cuando no hay nada pendiente y no se envía nada más hasta su `ok`.
pub fn writes_eeprom(text: &str) -> bool {
    let t = text.trim();
    if t.starts_with('$') {
        // `$J=` es un jog, no un ajuste.
        return t.contains('=') && !t.get(..3).is_some_and(|p| p.eq_ignore_ascii_case("$J="));
    }
    let b = t.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i].eq_ignore_ascii_case(&b'G') {
            let start = i + 1;
            let mut end = start;
            while end < b.len() && (b[end].is_ascii_digit() || b[end] == b'.') {
                end += 1;
            }
            if let Ok(v) = t[start..end].parse::<f64>()
                && (v == 10.0 || v == 28.1 || v == 30.1)
            {
                return true;
            }
            i = end.max(i + 1);
        } else {
            i += 1;
        }
    }
    false
}

/// ¿Es un comando de homing (`$H`, `$HX`...)? Puede tardar un minuto o más sin contestar `?`.
pub fn is_homing_line(text: &str) -> bool {
    text.trim()
        .get(..2)
        .is_some_and(|p| p.eq_ignore_ascii_case("$H"))
}

// ---------------------------------------------------------------------------
// Configuración, eventos y control
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorPolicy {
    /// Un `error:N` detiene el trabajo: feed hold, espera a que frene, soft reset y fin.
    /// GRBL sigue ejecutando lo que ya tiene en el búfer cuando una línea falla (y la línea
    /// fallida no aplicó su S, F o M3/M4), así que no basta con dejar de enviar.
    Stop,
    /// Se registra el error y el trabajo sigue con la línea siguiente.
    Continue,
}

#[derive(Debug, Clone)]
pub struct StreamerConfig {
    /// Bytes que se pueden tener pendientes en la controladora. GRBL declara 128,
    /// pero su búfer circular deja libre un byte, así que se usa 127 por defecto;
    /// grblHAL admite bastante más.
    pub rx_buffer: usize,
    pub error_policy: ErrorPolicy,
    /// Cada cuánto se consulta `?`; `None` desactiva el sondeo (y con él la
    /// espera de reposo y el aviso por silencio).
    pub status_interval: Option<Duration>,
    /// Sin recibir ningún byte durante este tiempo (con sondeo activo) se da
    /// la conexión por perdida.
    ///
    /// Regla del silencio: `?` puede no contestarse durante el homing. Si hay una línea de
    /// homing (`$H`...) sin responder, el plazo es [`home_timeout`](Self::home_timeout) en
    /// lugar de este. Las alarmas (`ALARM:N`) terminan el trabajo al instante, así que no
    /// hay silencio posterior que vigilar aquí.
    pub silence_timeout: Duration,
    /// Plazo de silencio mientras una línea `$H` espera su `ok`.
    pub home_timeout: Duration,
    /// Terminar solo cuando la máquina informe `Idle` tras el último `ok`
    /// (los `ok` llegan al interpretar la línea, no al ejecutarla).
    pub wait_idle: bool,
    /// Margen tras el soft reset: tras ver el saludo (`Grbl ...`) se sigue leyendo este
    /// tiempo para recoger un `ALARM:3` que lo acompañe; luego se descarta lo que quede.
    pub cancel_delay: Duration,
    /// Máximo que se espera, tras el feed hold, a que la máquina termine de frenar
    /// (`Hold:0`) antes de resetear de todos modos. Un reset en `Hold:1` (todavía frenando)
    /// provoca `ALARM:3` y pierde la posición. Con el sondeo desactivado
    /// (`status_interval: None`) no hay forma de preguntar y solo se espera `cancel_delay`.
    pub hold_timeout: Duration,
    /// Cada cuánto se consulta `?` mientras se espera el fin del feed hold.
    pub hold_poll_interval: Duration,
    /// Máximo que se espera el saludo tras enviar el soft reset.
    pub reset_wait: Duration,
}

impl StreamerConfig {
    /// Configuración para un enlace: el búfer RX de GRBL y el sondeo `?` solo con GRBL; en
    /// Marlin y Smoothie no se sondea durante el trabajo.
    pub fn for_link(link: &LinkProfile) -> Self {
        let mut c = Self::default();
        if let FlowControl::CharCounting { rx_buffer } = link.flow {
            c.rx_buffer = rx_buffer;
        }
        if !link.polls_during_job() {
            c.status_interval = None;
        }
        c
    }

    /// Configuración para un perfil de máquina (ver [`LinkProfile::for_device`]).
    pub fn for_device(device: &DeviceProfile) -> Self {
        Self::for_link(&LinkProfile::for_device(device))
    }
}

impl Default for StreamerConfig {
    fn default() -> Self {
        Self {
            rx_buffer: 127,
            error_policy: ErrorPolicy::Stop,
            status_interval: Some(Duration::from_millis(200)),
            silence_timeout: Duration::from_secs(5),
            home_timeout: Duration::from_secs(120),
            wait_idle: true,
            cancel_delay: Duration::from_millis(50),
            hold_timeout: Duration::from_secs(2),
            hold_poll_interval: Duration::from_millis(100),
            reset_wait: Duration::from_millis(500),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    /// Una línea fue respondida. `sent` incluye las aún sin confirmar y
    /// `buffer_used` los bytes que se cree ocupados en la controladora.
    Progress {
        acked: usize,
        sent: usize,
        total: usize,
        buffer_used: usize,
    },
    Status(Box<StatusReport>),
    /// `error:N` de GRBL. `source_line` y `text` son `None` si no hay línea pendiente a la que atribuirlo.
    Error {
        code: u32,
        source_line: Option<usize>,
        text: Option<String>,
    },
    Alarm(u32),
    /// Mensaje `[...]` u otra línea no reconocida.
    Message(String),
    /// La controladora se reinició (saludo `Grbl ...`).
    Reset(String),
    Finished(Outcome),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Outcome {
    /// Todas las líneas se respondieron; `errors` cuenta los `error:N` tolerados.
    Completed {
        errors: usize,
    },
    /// Cancelado: feed hold, espera a que frene y soft reset (ver [`Streamer::last_stop`]).
    Cancelled,
    /// `error:N` con la política `Stop`. Antes de terminar se detuvo la máquina (feed hold,
    /// espera y soft reset).
    LineError {
        code: u32,
        source_line: usize,
    },
    /// Falla propia de Creality (`ERROR:NN`, p. ej. llama o inclinación). No es una línea
    /// rechazada: la máquina se detuvo por seguridad. El trabajo se detuvo igual que con
    /// `LineError`. El significado está en [`creality_error_message`].
    MachineFault(u32),
    Alarm(u32),
    /// La controladora se reinició a mitad del trabajo.
    Reset(String),
    /// Fallo de E/S o silencio prolongado.
    Disconnected(String),
}

#[derive(Debug)]
enum Command {
    Pause,
    Resume,
    Cancel,
    Realtime(Realtime),
}

/// Mando remoto de un trabajo en curso; se puede clonar y usar desde otro hilo.
/// Los envíos a un `Streamer` ya destruido se ignoran.
#[derive(Clone)]
pub struct Control {
    tx: Sender<Command>,
}

impl Control {
    /// Feed hold y dejar de enviar líneas nuevas.
    pub fn pause(&self) {
        let _ = self.tx.send(Command::Pause);
    }

    /// Reanuda (`~`) y vuelve a enviar. No hace nada si no estaba en pausa.
    pub fn resume(&self) {
        let _ = self.tx.send(Command::Resume);
    }

    /// Feed hold, soft reset y fin del trabajo.
    pub fn cancel(&self) {
        let _ = self.tx.send(Command::Cancel);
    }

    /// Byte en tiempo real (overrides, jog cancel...). `SoftReset` también
    /// termina el trabajo como cancelado.
    pub fn realtime(&self, cmd: Realtime) {
        let _ = self.tx.send(Command::Realtime(cmd));
    }
}

/// Trabajo lanzado en su propio hilo con [`Streamer::spawn`].
pub struct Job<T: Transport> {
    pub control: Control,
    pub events: Receiver<Event>,
    handle: JoinHandle<(Streamer<T>, Outcome)>,
}

impl<T: Transport> Job<T> {
    /// Espera el fin del trabajo y devuelve el streamer (con su transporte) y el resultado.
    pub fn join(self) -> thread::Result<(Streamer<T>, Outcome)> {
        self.handle.join()
    }
}

// ---------------------------------------------------------------------------
// Streamer
// ---------------------------------------------------------------------------

pub struct Streamer<T: Transport> {
    transport: T,
    config: StreamerConfig,
    link: LinkProfile,
    cmd_tx: Sender<Command>,
    cmd_rx: Receiver<Command>,
    reader: LineReader,
    stop: Option<StopInfo>,
}

/// Qué pasó al detener la máquina (cancelación, `error:N` con la política `Stop`, falla de
/// Creality o reset pedido por el usuario).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StopInfo {
    /// Se vio la máquina quieta (`Hold:0`, `Idle`...) antes del soft reset.
    pub settled: bool,
    /// El reset llegó con la máquina posiblemente en movimiento (no se llegó a ver quieta) o
    /// la controladora respondió `ALARM:3` tras él: la posición puede haberse perdido.
    pub position_maybe_lost: bool,
    /// `ALARM:N` recibido durante la parada o justo después del reset.
    pub alarm: Option<u32>,
}

/// Aviso para el usuario cuando [`StopInfo::position_maybe_lost`].
pub const POSITION_LOST_NOTE: &str = "El reset llegó con la máquina en movimiento: la posición \
     puede haberse perdido. Si tu máquina tiene finales de carrera, haz homing ($H) antes del \
     siguiente trabajo; si no, vuelve a fijar el origen a mano.";

struct RunState {
    next: usize,
    acked: usize,
    errors: usize,
    used: usize,
    /// (índice en el trabajo, bytes) de las líneas enviadas sin responder.
    pending: VecDeque<(usize, usize)>,
    paused: bool,
    idle_seen: bool,
    last_poll: Option<Instant>,
    last_rx: Instant,
    /// Último estado informado por la máquina.
    last_state: Option<MachineState>,
    /// Líneas propias del streamer al inicio del trabajo (no cuentan en el avance).
    hidden: usize,
    /// Ya se envió la orden de cierre (`M400`) y se espera su `ok`.
    sync_sent: bool,
    /// Tras un `Resend:` no se envía hasta el `ok` que lo acompaña (o hasta este instante).
    resend_until: Option<Instant>,
    /// Reenvíos seguidos sin ningún `ok` entre medias.
    resends: u32,
}

impl<T: Transport> Streamer<T> {
    /// Streamer GRBL (conteo de caracteres con `config.rx_buffer`, `?`, `!`, `~`, `0x18`).
    pub fn new(transport: T, config: StreamerConfig) -> Self {
        let link = LinkProfile::grbl_with_buffer(config.rx_buffer);
        Self::with_link(transport, config, link)
    }

    /// Streamer para el enlace dado (GRBL, Marlin o Smoothieware). El búfer sale de
    /// `link.flow`; `config.rx_buffer` se ignora. Para Marlin y Smoothie conviene
    /// [`StreamerConfig::for_link`].
    pub fn with_link(transport: T, config: StreamerConfig, link: LinkProfile) -> Self {
        let (cmd_tx, cmd_rx) = mpsc::channel();
        Self {
            transport,
            config,
            link,
            cmd_tx,
            cmd_rx,
            reader: LineReader::default(),
            stop: None,
        }
    }

    /// Streamer con el enlace y la configuración que corresponden al perfil de máquina
    /// (`kind`, `dialect` y `rx_buffer`).
    pub fn for_device(transport: T, device: &DeviceProfile) -> Self {
        Self::with_link(
            transport,
            StreamerConfig::for_device(device),
            LinkProfile::for_device(device),
        )
    }

    /// Enlace en uso.
    pub fn link(&self) -> &LinkProfile {
        &self.link
    }

    /// Cómo se detuvo la máquina en el último trabajo, si hubo parada (cancelación, error con
    /// política `Stop`, falla de Creality o reset).
    pub fn last_stop(&self) -> Option<&StopInfo> {
        self.stop.as_ref()
    }

    pub fn control(&self) -> Control {
        Control {
            tx: self.cmd_tx.clone(),
        }
    }

    pub fn into_transport(self) -> T {
        self.transport
    }

    /// Envía un comando en tiempo real fuera de un trabajo.
    pub fn send_realtime(&mut self, cmd: Realtime) -> io::Result<()> {
        self.transport.write_all(&[cmd.byte()])
    }

    fn prepare<S: AsRef<str>>(&self, lines: &[S]) -> Result<Vec<Prepared>, StreamError> {
        let max = self.link.flow.max_line_len();
        let mut job = Vec::new();
        // Con líneas numeradas, la primera fija el contador (`N0 M110 N0*cs`): así `N<n>` es el
        // índice de la línea en `job` y un `Resend: n` apunta a ella sin cuentas.
        if self.link.line_numbering.is_some() {
            job.push(Prepared {
                text: "M110 N0".into(),
                source_line: 0,
                sync: false,
                silent: true,
            });
        }
        for (i, raw) in lines.iter().enumerate() {
            let source_line = i + 1;
            let text = prepare_line_with(
                raw.as_ref(),
                self.link.strip_spaces,
                self.link.reject_realtime_chars,
            )
            .map_err(|ch| StreamError::InvalidChar { source_line, ch })?;
            if text.is_empty() {
                continue;
            }
            let len = self.wire_text_for(job.len(), &text).len() + 1;
            if len > max {
                return Err(StreamError::LineTooLong {
                    source_line,
                    len,
                    max,
                });
            }
            let sync = self.link.eeprom_sync && writes_eeprom(&text);
            job.push(Prepared {
                text,
                source_line,
                sync,
                silent: false,
            });
        }
        Ok(job)
    }

    /// Lo que viaja por el cable (sin `\n`) para la línea `index` del trabajo.
    fn wire_text_for(&self, index: usize, text: &str) -> String {
        match self.link.line_numbering {
            Some(n) => numbered_line(index, text, n),
            None => text.to_string(),
        }
    }

    /// Ejecuta el trabajo en el hilo actual hasta que termine, informando cada
    /// evento a `on_event`. Las líneas se validan antes de enviar nada.
    pub fn run<S: AsRef<str>>(
        &mut self,
        lines: &[S],
        mut on_event: impl FnMut(Event),
    ) -> Result<Outcome, StreamError> {
        let job = self.prepare(lines)?;
        Ok(self.execute(&job, &mut on_event))
    }

    /// Como [`run`](Self::run), pero en un hilo propio; los eventos llegan por
    /// `Job::events` y el mando es `Job::control`.
    pub fn spawn<S: AsRef<str>>(self, lines: &[S]) -> Result<Job<T>, StreamError>
    where
        T: 'static,
    {
        let job = self.prepare(lines)?;
        let control = self.control();
        let (tx, events) = mpsc::channel();
        let handle = thread::spawn(move || {
            let mut streamer = self;
            let outcome = streamer.execute(&job, &mut |e| {
                let _ = tx.send(e);
            });
            (streamer, outcome)
        });
        Ok(Job {
            control,
            events,
            handle,
        })
    }

    fn execute(&mut self, job: &[Prepared], emit: &mut dyn FnMut(Event)) -> Outcome {
        while self.cmd_rx.try_recv().is_ok() {}
        self.reader.clear();
        self.stop = None;
        let mut st = RunState {
            next: 0,
            acked: 0,
            errors: 0,
            used: 0,
            pending: VecDeque::new(),
            paused: false,
            idle_seen: false,
            last_poll: None,
            last_rx: Instant::now(),
            last_state: None,
            hidden: job.iter().take_while(|p| p.silent).count(),
            sync_sent: false,
            resend_until: None,
            resends: 0,
        };
        let outcome = if job.len() == st.hidden {
            Outcome::Completed { errors: 0 }
        } else {
            loop {
                match self.tick(job, &mut st, emit) {
                    Ok(Some(outcome)) => break outcome,
                    Ok(None) => {}
                    Err(e) => break Outcome::Disconnected(e.to_string()),
                }
            }
        };
        emit(Event::Finished(outcome.clone()));
        outcome
    }

    /// Intervalo del sondeo `?` durante el trabajo: solo si el enlace consulta con un byte en
    /// tiempo real (GRBL). En Marlin y Smoothie no hay sondeo.
    fn polling(&self) -> Option<Duration> {
        if self.link.polls_during_job() {
            self.config.status_interval
        } else {
            None
        }
    }

    /// Una vuelta del ciclo: mandos, envío, sondeo, lectura y comprobaciones de fin.
    fn tick(
        &mut self,
        job: &[Prepared],
        st: &mut RunState,
        emit: &mut dyn FnMut(Event),
    ) -> io::Result<Option<Outcome>> {
        while let Ok(cmd) = self.cmd_rx.try_recv() {
            if let Some(outcome) = self.handle_command(cmd, st, emit)? {
                return Ok(Some(outcome));
            }
        }

        // Tras un `Resend:` se espera el `ok` que lo acompaña (con un tope de tiempo).
        if st.resend_until.is_some_and(|t| Instant::now() >= t) {
            st.resend_until = None;
        }
        // Tras una línea que escribe en la EEPROM no se envía nada hasta su `ok`.
        let blocked = st
            .pending
            .back()
            .is_some_and(|&(i, _)| job.get(i).is_some_and(|p| p.sync));
        if !st.paused && !blocked && st.resend_until.is_none() && st.next < job.len() {
            let mut batch = Vec::new();
            while let Some(line) = job.get(st.next) {
                let wire = self.wire_text_for(st.next, &line.text);
                let len = wire.len() + 1;
                if line.sync && !st.pending.is_empty() {
                    break;
                }
                let fits = match self.link.flow {
                    FlowControl::CharCounting { rx_buffer } => st.used + len <= rx_buffer,
                    FlowControl::Window { max_lines, .. } => st.pending.len() < max_lines.max(1),
                };
                if !fits {
                    break;
                }
                batch.extend_from_slice(wire.as_bytes());
                batch.push(b'\n');
                st.pending.push_back((st.next, len));
                st.used += len;
                st.next += 1;
                if line.sync {
                    break;
                }
            }
            if !batch.is_empty() {
                self.transport.write_all(&batch)?;
            }
        }

        let polling = self.polling();
        if let Some(interval) = polling
            && st.last_poll.is_none_or(|t| t.elapsed() >= interval)
        {
            self.transport.write_all(&[Realtime::StatusQuery.byte()])?;
            st.last_poll = Some(Instant::now());
        }

        let mut buf = [0u8; 256];
        let n = self.transport.read_timeout(&mut buf, TICK)?;
        if n > 0 {
            st.last_rx = Instant::now();
            self.reader.feed(&buf[..n]);
        }
        while let Some(line) = self.reader.next_line() {
            if let Some(outcome) = self.handle_line(&line, job, st, emit)? {
                return Ok(Some(outcome));
            }
        }

        if let Some(outcome) = self.check_finished(job, st, polling.is_some())? {
            return Ok(Some(outcome));
        }
        // `?` no se contesta durante el homing: con una línea `$H` sin responder se espera
        // `home_timeout` en lugar de `silence_timeout`.
        let homing = st
            .pending
            .iter()
            .any(|&(i, _)| job.get(i).is_some_and(|p| is_homing_line(&p.text)));
        let allowance = if homing {
            self.config.home_timeout.max(self.config.silence_timeout)
        } else {
            self.config.silence_timeout
        };
        if polling.is_some() && st.last_rx.elapsed() > allowance {
            return Ok(Some(Outcome::Disconnected(
                "la controladora dejó de responder".into(),
            )));
        }
        Ok(None)
    }

    /// ¿Terminó el trabajo? GRBL: todas las líneas respondidas y, con sondeo, estado `Idle`.
    /// Marlin y Smoothie: todas respondidas y el `ok` de `M400` (los `ok` de los movimientos
    /// llegan al encolarlos, no al ejecutarlos).
    fn check_finished(
        &mut self,
        job: &[Prepared],
        st: &mut RunState,
        polling: bool,
    ) -> io::Result<Option<Outcome>> {
        if st.acked != job.len() {
            return Ok(None);
        }
        match &self.link.finish {
            FinishDetection::StatusIdle => {
                if st.idle_seen || !(self.config.wait_idle && polling) {
                    return Ok(Some(Outcome::Completed { errors: st.errors }));
                }
            }
            FinishDetection::SyncCommand(cmd) => {
                if !self.config.wait_idle
                    || job.last().is_some_and(|l| l.text.eq_ignore_ascii_case(cmd))
                {
                    // O no se pide esperar el fin, o la última línea ya es la sincronización.
                    return Ok(Some(Outcome::Completed { errors: st.errors }));
                }
                if !st.sync_sent {
                    let cmd = cmd.clone();
                    self.send_extra(&cmd, st)?;
                    st.sync_sent = true;
                } else if st.pending.is_empty() {
                    return Ok(Some(Outcome::Completed { errors: st.errors }));
                }
            }
        }
        Ok(None)
    }

    /// Envía una línea que no es del trabajo; su `ok` solo libera el búfer.
    fn send_extra(&mut self, line: &str, st: &mut RunState) -> io::Result<()> {
        let len = line.len() + 1;
        self.transport.write_all(format!("{line}\n").as_bytes())?;
        st.pending.push_back((EXTRA, len));
        st.used += len;
        Ok(())
    }

    /// Ejecuta pausa o reanudación del enlace: bytes en tiempo real, líneas auxiliares o
    /// "dejar de enviar" (que ya hace `st.paused`).
    fn run_actions(&mut self, actions: &[ControlAction], st: &mut RunState) -> io::Result<()> {
        for a in actions {
            match a {
                ControlAction::Realtime(b) => self.transport.write_all(&[*b])?,
                ControlAction::Line(l) => self.send_extra(l, st)?,
                ControlAction::HostSide => {}
            }
        }
        Ok(())
    }

    /// Detiene la máquina como corresponde al firmware: GRBL con feed hold y soft reset;
    /// Marlin y Smoothie con su secuencia de parada rápida.
    fn stop_machine(&mut self, st: &mut RunState, emit: &mut dyn FnMut(Event)) -> io::Result<()> {
        match self.link.cancel.clone() {
            CancelStyle::SafeStopReset => self.safe_stop(st, emit),
            CancelStyle::Sequence(actions) => self.sequence_stop(&actions, st, emit),
        }
    }

    fn handle_command(
        &mut self,
        cmd: Command,
        st: &mut RunState,
        emit: &mut dyn FnMut(Event),
    ) -> io::Result<Option<Outcome>> {
        match cmd {
            Command::Pause if !st.paused => {
                let actions = self.link.pause.clone();
                self.run_actions(&actions, st)?;
                st.paused = true;
            }
            Command::Resume if st.paused => {
                let actions = self.link.resume.clone();
                self.run_actions(&actions, st)?;
                st.paused = false;
            }
            Command::Pause | Command::Resume => {}
            Command::Cancel => {
                self.stop_machine(st, emit)?;
                return Ok(Some(Outcome::Cancelled));
            }
            // Reset pedido a propósito por el usuario: sin esperar el fin del hold.
            Command::Realtime(Realtime::SoftReset) if self.link.family == LinkFamily::Grbl => {
                let stationary = st.last_state.is_some_and(MachineState::is_stationary);
                self.reset(st, stationary, emit)?;
                return Ok(Some(Outcome::Cancelled));
            }
            // Marlin y Smoothie no tienen reinicio en caliente: se cancela con su secuencia.
            Command::Realtime(Realtime::SoftReset) => {
                self.stop_machine(st, emit)?;
                return Ok(Some(Outcome::Cancelled));
            }
            // Overrides y demás bytes son de GRBL: en otro firmware serían basura en la línea.
            Command::Realtime(_) if self.link.family != LinkFamily::Grbl => {}
            Command::Realtime(r) => self.transport.write_all(&[r.byte()])?,
        }
        Ok(None)
    }

    /// Parada de Marlin y Smoothie: se deja de enviar, se descarta lo pendiente y se manda la
    /// secuencia del enlace (`M410` + apagado del láser; `M112` + `M999` + apagado). Tras cada
    /// línea se espera un `ok` (el de lo pendiente, el suyo o ninguno si el firmware no
    /// contesta, como tras `M112`): como mucho [`SEQUENCE_ACK_WAIT`] de silencio, y en total
    /// [`StreamerConfig::hold_timeout`].
    ///
    /// Como no hay reinicio en caliente y las órdenes son líneas encoladas, el láser se apaga
    /// con el comando del dialecto, no al detenerse el movimiento. La posición se da por
    /// perdida: la parada rápida frena sin desaceleración.
    fn sequence_stop(
        &mut self,
        actions: &[ControlAction],
        st: &mut RunState,
        emit: &mut dyn FnMut(Event),
    ) -> io::Result<()> {
        st.pending.clear();
        st.used = 0;
        st.last_state = None;
        let deadline = Instant::now() + self.config.hold_timeout;
        let mut buf = [0u8; 256];
        let mut waiting = false;
        let mut quiet_since = Instant::now();
        let mut acked = false;
        // Un paso más que las acciones: tras la última línea también se espera su respuesta.
        for step in 0..=actions.len() {
            if waiting {
                // Espera un `ok` (o silencio) antes de mandar la siguiente.
                while !acked
                    && Instant::now() < deadline
                    && quiet_since.elapsed() < SEQUENCE_ACK_WAIT
                {
                    let Ok(n) = self.transport.read_timeout(&mut buf, TICK) else {
                        break;
                    };
                    if n > 0 {
                        quiet_since = Instant::now();
                    }
                    self.reader.feed(&buf[..n]);
                    while let Some(line) = self.reader.next_line() {
                        if self.link.is_ack(&line) {
                            acked = true;
                        } else {
                            emit(Event::Message(line));
                        }
                    }
                }
            }
            let Some(a) = actions.get(step) else { break };
            match a {
                ControlAction::Realtime(b) => self.transport.write_all(&[*b])?,
                ControlAction::Line(l) => {
                    self.transport.write_all(format!("{l}\n").as_bytes())?;
                    waiting = true;
                    acked = false;
                    quiet_since = Instant::now();
                }
                ControlAction::HostSide => {}
            }
        }
        self.reader.clear();
        let _ = self.transport.clear_input();
        emit(Event::Message(format!("; {QUICK_STOP_NOTE}")));
        self.stop = Some(StopInfo {
            settled: false,
            position_maybe_lost: true,
            alarm: None,
        });
        Ok(())
    }

    /// Detiene la máquina sin perder la posición: feed hold, consulta `?` hasta ver que
    /// frenó (`Hold:0`, reposo...) con un tope de [`StreamerConfig::hold_timeout`] y solo
    /// entonces soft reset. Un reset mientras aún frena (`Hold:1`) lanza `ALARM:3`. Después
    /// del reset se lee el saludo y cualquier alarma en lugar de descartarlos.
    ///
    /// grblHAL repite el último `error:N` en las líneas siguientes (y Creality puede seguir
    /// hablando): todo lo que llega durante la espera se absorbe sin contarlo como fallo nuevo.
    fn safe_stop(&mut self, st: &mut RunState, emit: &mut dyn FnMut(Event)) -> io::Result<()> {
        self.transport.write_all(&[Realtime::FeedHold.byte()])?;
        // Los informes anteriores al hold ya no valen.
        st.last_state = None;
        // Sin sondeo no se puede saber cuándo frenó: una espera corta y a resetear.
        let polling = self.config.status_interval.is_some();
        let deadline = Instant::now()
            + if polling {
                self.config.hold_timeout
            } else {
                self.config.cancel_delay
            };
        let mut next_poll = Instant::now();
        let mut settled = false;
        let mut alarm = None;
        let mut buf = [0u8; 256];
        while !settled && Instant::now() < deadline {
            if polling && Instant::now() >= next_poll {
                self.transport.write_all(&[Realtime::StatusQuery.byte()])?;
                next_poll = Instant::now() + self.config.hold_poll_interval;
            }
            let n = self.transport.read_timeout(&mut buf, TICK)?;
            self.reader.feed(&buf[..n]);
            while let Some(line) = self.reader.next_line() {
                match absorb_line(&line, emit) {
                    Absorbed::State(state) => {
                        st.last_state = Some(state);
                        settled |= state.is_stationary();
                    }
                    Absorbed::Alarm(code) => {
                        alarm = Some(code);
                        // Una alarma ya detuvo la máquina.
                        settled = true;
                    }
                    Absorbed::Greeting => settled = true,
                    Absorbed::Other => {}
                }
            }
        }
        self.reset(st, settled, emit)?;
        if let Some(info) = self.stop.as_mut()
            && info.alarm.is_none()
        {
            info.alarm = alarm;
            info.position_maybe_lost |= alarm == Some(3);
        }
        Ok(())
    }

    /// Soft reset y lectura de lo que responde la controladora (saludo, `ALARM:3`...).
    /// `stationary` indica si se sabía la máquina quieta al resetear.
    fn reset(
        &mut self,
        st: &mut RunState,
        stationary: bool,
        emit: &mut dyn FnMut(Event),
    ) -> io::Result<()> {
        self.transport.write_all(&[Realtime::SoftReset.byte()])?;
        let started = Instant::now();
        let mut greeted: Option<Instant> = None;
        let mut alarm = None;
        let mut buf = [0u8; 256];
        loop {
            let over = match greeted {
                Some(t) => t.elapsed() >= self.config.cancel_delay,
                None => started.elapsed() >= self.config.reset_wait,
            };
            if over {
                break;
            }
            // El reset ya salió: si el canal se cae ahora, el siguiente uso del puerto lo
            // dirá; la parada en sí se hizo.
            let Ok(n) = self.transport.read_timeout(&mut buf, TICK) else {
                break;
            };
            self.reader.feed(&buf[..n]);
            while let Some(line) = self.reader.next_line() {
                match absorb_line(&line, emit) {
                    Absorbed::Alarm(code) => alarm = Some(code),
                    Absorbed::Greeting => {
                        greeted.get_or_insert_with(Instant::now);
                    }
                    Absorbed::State(_) | Absorbed::Other => {}
                }
            }
        }
        // Lo que quede (otro informe, un `ok` rezagado) no debe confundirse con futuras respuestas.
        self.reader.clear();
        let _ = self.transport.clear_input();
        st.last_state = None;
        let position_maybe_lost = !stationary || alarm == Some(3);
        if position_maybe_lost {
            emit(Event::Message(format!("; {POSITION_LOST_NOTE}")));
        }
        self.stop = Some(StopInfo {
            settled: stationary,
            position_maybe_lost,
            alarm,
        });
        Ok(())
    }

    fn handle_line(
        &mut self,
        line: &str,
        job: &[Prepared],
        st: &mut RunState,
        emit: &mut dyn FnMut(Event),
    ) -> io::Result<Option<Outcome>> {
        // Las líneas propias del streamer (`M110 N0`) no cuentan en el avance.
        let hidden = st.hidden;
        let progress = |st: &RunState| Event::Progress {
            acked: st.acked.saturating_sub(hidden),
            sent: st.next.saturating_sub(hidden),
            total: job.len() - hidden,
            buffer_used: st.used,
        };
        if self.link.is_ack(line) {
            // El `ok` que acompaña a un `Resend:` no confirma ninguna línea.
            if st.resend_until.take().is_some() {
                return Ok(None);
            }
            st.resends = 0;
            if let Some((idx, len)) = st.pending.pop_front() {
                st.used -= len;
                if idx != EXTRA {
                    st.acked += 1;
                    if !job[idx].silent {
                        emit(progress(st));
                    }
                }
            }
        } else if self.link.family == LinkFamily::Marlin
            && let Some(outcome) = self.handle_marlin_line(line, job, st, emit)
        {
            return Ok(outcome);
        } else if let Some(code) = line.strip_prefix("error:") {
            // Minúsculas: el `error:N` de GRBL. El `ERROR:NN` de Creality va más abajo.
            let Ok(code) = code.trim().parse::<u32>() else {
                emit(Event::Message(line.to_string()));
                return Ok(None);
            };
            let Some((idx, len)) = st.pending.pop_front() else {
                emit(Event::Error {
                    code,
                    source_line: None,
                    text: None,
                });
                return Ok(None);
            };
            st.used -= len;
            if idx == EXTRA {
                // Una línea auxiliar (`M0`, `M400`...) rechazada: no es del trabajo.
                emit(Event::Error {
                    code,
                    source_line: None,
                    text: None,
                });
                return Ok(None);
            }
            st.acked += 1;
            st.errors += 1;
            let source_line = job[idx].source_line;
            emit(Event::Error {
                code,
                source_line: Some(source_line),
                text: Some(job[idx].text.clone()),
            });
            emit(progress(st));
            if self.config.error_policy == ErrorPolicy::Stop {
                // La máquina sigue ejecutando lo que ya tiene en el búfer: se detiene.
                self.stop_machine(st, emit)?;
                return Ok(Some(Outcome::LineError { code, source_line }));
            }
        } else if let Some(code) = parse_creality_error(line) {
            emit(Event::Message(format!(
                "ERROR:{code:02}: {}",
                creality_error_message(code)
            )));
            self.stop_machine(st, emit)?;
            return Ok(Some(Outcome::MachineFault(code)));
        } else if let Some(code) = line.strip_prefix("ALARM:") {
            if let Ok(code) = code.trim().parse::<u32>() {
                emit(Event::Alarm(code));
                return Ok(Some(Outcome::Alarm(code)));
            }
            emit(Event::Message(line.to_string()));
        } else if line.starts_with('<') {
            match StatusReport::parse(line) {
                Some(report) => {
                    st.last_state = Some(report.state);
                    if st.acked == job.len() && report.state == MachineState::Idle {
                        st.idle_seen = true;
                    }
                    emit(Event::Status(Box::new(report)));
                }
                None => emit(Event::Message(line.to_string())),
            }
        } else if self.link.is_reset_banner(line) {
            emit(Event::Reset(line.to_string()));
            return Ok(Some(Outcome::Reset(line.to_string())));
        } else {
            emit(Event::Message(line.to_string()));
        }
        Ok(None)
    }

    /// Líneas propias de Marlin: `Resend: N` (reenvía desde la línea N) y `Error:...`.
    /// `None` = no es una de ellas; `Some(resultado)` = atendida (con el resultado de
    /// `handle_line`).
    fn handle_marlin_line(
        &mut self,
        line: &str,
        job: &[Prepared],
        st: &mut RunState,
        emit: &mut dyn FnMut(Event),
    ) -> Option<Option<Outcome>> {
        let low = line.to_ascii_lowercase();
        if let Some(rest) = low.strip_prefix("resend:") {
            let Some(n) = rest
                .trim()
                .parse::<usize>()
                .ok()
                .filter(|&n| self.link.line_numbering.is_some() && n < job.len())
            else {
                emit(Event::Message(line.to_string()));
                return Some(None);
            };
            st.resends += 1;
            if st.resends > MAX_RESENDS {
                return Some(Some(Outcome::Disconnected(format!(
                    "el enlace pierde líneas: la {n} se reenvió {MAX_RESENDS} veces sin éxito \
                     (cable, baudios o ruido)"
                ))));
            }
            emit(Event::Message(line.to_string()));
            // Marlin descarta lo que tuviera en cola desde esa línea: se vuelve a enviar todo.
            st.pending.clear();
            st.used = 0;
            st.next = n;
            st.acked = st.acked.min(n);
            // El `ok` que sigue al `Resend:` no confirma nada; si no llega, se sigue pasado un rato.
            st.resend_until = Some(Instant::now() + RESEND_WAIT);
            return Some(None);
        }
        if low.starts_with("error:") {
            // Una parada total (`M112`, `kill()`) no se recupera sin reiniciar la placa.
            if low.contains("halted") || low.contains("kill()") {
                emit(Event::Message(line.to_string()));
                return Some(Some(Outcome::Reset(line.to_string())));
            }
            // Los demás (`checksum mismatch`, `Line Number is not Last Line Number+1`) van
            // seguidos de `Resend:`; se informan sin más.
            emit(Event::Message(line.to_string()));
            return Some(None);
        }
        None
    }
}

/// Silencio tras el que, al cancelar en Marlin o Smoothie, se sigue con la siguiente orden de
/// la secuencia aunque la anterior no haya contestado (`M112` no contesta).
const SEQUENCE_ACK_WAIT: Duration = Duration::from_millis(300);

/// Reenvíos seguidos de una línea (sin un `ok` entre medias) tras los que se abandona.
const MAX_RESENDS: u32 = 8;

/// Cuánto se espera el `ok` posterior a un `Resend:` antes de reenviar de todos modos.
const RESEND_WAIT: Duration = Duration::from_millis(500);

/// ¿Es el saludo de la controladora (`Grbl 1.1h [...]`, `GrblHAL ...`)? Compara bytes: el
/// texto puede traer `U+FFFD` por errores de transmisión (CH340, cables con ruido) y cortarlo
/// por índice de bytes haría panic dentro de un carácter.
pub fn is_greeting(line: &str) -> bool {
    line.as_bytes()
        .get(..4)
        .is_some_and(|p| p.eq_ignore_ascii_case(b"grbl"))
}

/// Lo que importa de una línea recibida mientras se detiene la máquina.
enum Absorbed {
    State(MachineState),
    Alarm(u32),
    Greeting,
    Other,
}

/// Clasifica una línea recibida durante la parada, emite el evento que corresponda y
/// descarta el resto (`ok`, `error:N` repetidos de grblHAL...).
fn absorb_line(line: &str, emit: &mut dyn FnMut(Event)) -> Absorbed {
    if line == "ok" || line.starts_with("error:") {
        return Absorbed::Other;
    }
    if let Some(code) = parse_creality_error(line) {
        emit(Event::Message(format!(
            "ERROR:{code:02}: {}",
            creality_error_message(code)
        )));
        return Absorbed::Other;
    }
    if let Some(code) = line
        .strip_prefix("ALARM:")
        .and_then(|c| c.trim().parse::<u32>().ok())
    {
        emit(Event::Alarm(code));
        return Absorbed::Alarm(code);
    }
    if line.starts_with('<') {
        return match StatusReport::parse(line) {
            Some(report) => {
                let state = report.state;
                emit(Event::Status(Box::new(report)));
                Absorbed::State(state)
            }
            None => {
                emit(Event::Message(line.to_string()));
                Absorbed::Other
            }
        };
    }
    // El saludo tras nuestro propio reset es esperado: se registra como mensaje, no como
    // un reinicio inesperado.
    if is_greeting(line) {
        emit(Event::Message(line.to_string()));
        return Absorbed::Greeting;
    }
    emit(Event::Message(line.to_string()));
    Absorbed::Other
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::serial::memory_pair;

    #[test]
    fn prepare_strips_comments_and_spaces() {
        assert_eq!(
            prepare_line("G1 X10.5 Y-2 F1000").unwrap(),
            "G1X10.5Y-2F1000"
        );
        assert_eq!(prepare_line("G0 X1 ; mover").unwrap(), "G0X1");
        assert_eq!(prepare_line("G0 (rápido) X1 (otro) Y2").unwrap(), "G0X1Y2");
        assert_eq!(prepare_line("(solo comentario)").unwrap(), "");
        assert_eq!(prepare_line("   \t ").unwrap(), "");
        assert_eq!(prepare_line("; todo ! ? ~").unwrap(), "");
        assert_eq!(prepare_line("G1 X1 (sin cerrar ! ?").unwrap(), "G1X1");
        assert_eq!(prepare_line("$J=G91 X10 F500").unwrap(), "$J=G91X10F500");
    }

    #[test]
    fn prepare_rejects_realtime_lookalikes() {
        assert_eq!(prepare_line("G1 X1 !"), Err('!'));
        assert_eq!(prepare_line("G1 ~"), Err('~'));
        assert_eq!(prepare_line("?"), Err('?'));
        assert_eq!(prepare_line("G1 Xé"), Err('é'));
        assert_eq!(prepare_line("G1\u{18}"), Err('\u{18}'));
    }

    #[test]
    fn prepare_line_with_keeps_single_spaces_for_reprap_firmwares() {
        assert_eq!(
            prepare_line_with("  M3   S0 I ; láser", false, false).unwrap(),
            "M3 S0 I"
        );
        assert_eq!(
            prepare_line_with("G1 (c) X1  Y2", false, false).unwrap(),
            "G1 X1 Y2"
        );
        assert_eq!(prepare_line_with("   ", false, false).unwrap(), "");
        // Marlin admite `?` y `!`; Smoothie y GRBL los rechazan.
        assert_eq!(
            prepare_line_with("M117 Hola!", false, false).unwrap(),
            "M117 Hola!"
        );
        assert_eq!(prepare_line_with("M117 Hola!", false, true), Err('!'));
        // Lo no ASCII se rechaza siempre.
        assert_eq!(prepare_line_with("M117 é", false, false), Err('é'));
        // Con `strip_spaces` es el de GRBL.
        assert_eq!(
            prepare_line_with("G1 X1", true, true).unwrap(),
            prepare_line("G1 X1").unwrap()
        );
    }

    /// Lee lo que llegue al extremo de la máquina en `ms` milisegundos.
    fn read_for(dev: &mut crate::serial::MemoryTransport, ms: u64) -> String {
        let mut out = Vec::new();
        let mut buf = [0u8; 256];
        let end = Instant::now() + Duration::from_millis(ms);
        while Instant::now() < end {
            let n = dev
                .read_timeout(&mut buf, Duration::from_millis(5))
                .unwrap();
            out.extend_from_slice(&buf[..n]);
        }
        String::from_utf8(out).unwrap()
    }

    #[test]
    fn ping_pong_sends_one_line_per_ok_and_closes_with_m400() {
        use tlaku_doc::GcodeDialect;
        let (host, mut dev) = memory_pair();
        let link = LinkProfile::marlin(&GcodeDialect::marlin_inline());
        let cfg = StreamerConfig::for_link(&link);
        assert_eq!(cfg.status_interval, None);
        let job = Streamer::with_link(host, cfg, link)
            .spawn(&["G90", "G21 ; mm", "", "M3 S0 I"])
            .unwrap();
        assert_eq!(read_for(&mut dev, 60), "G90\n");
        // `ok` con texto (ADVANCED_OK) vale; `echo:` y `busy:` no cuentan.
        dev.write_all(b"echo:busy: processing\r\nok N0 P15 B3\r\n")
            .unwrap();
        assert_eq!(read_for(&mut dev, 60), "G21\n");
        dev.write_all(b"ok\r\n").unwrap();
        assert_eq!(read_for(&mut dev, 60), "M3 S0 I\n");
        dev.write_all(b"ok\r\n").unwrap();
        // Todas respondidas: cierra con M400 y espera su ok.
        assert_eq!(read_for(&mut dev, 60), "M400\n");
        assert!(job.events.recv_timeout(Duration::from_millis(1)).is_ok());
        dev.write_all(b"ok\r\n").unwrap();
        let (_, out) = job.join().unwrap();
        assert_eq!(out, Outcome::Completed { errors: 0 });
    }

    #[test]
    fn ping_pong_pause_resume_cancel_use_the_link_actions() {
        use tlaku_doc::GcodeDialect;
        let (host, mut dev) = memory_pair();
        let link = LinkProfile::marlin(&GcodeDialect::marlin_inline());
        let mut cfg = StreamerConfig::for_link(&link);
        cfg.hold_timeout = Duration::from_millis(500);
        let job = Streamer::with_link(host, cfg, link)
            .spawn(&["G90", "G21", "G0 X1"])
            .unwrap();
        assert_eq!(read_for(&mut dev, 60), "G90\n");
        job.control.pause();
        dev.write_all(b"ok\r\n").unwrap();
        // En pausa no se envía la siguiente, y no se escribe `!`.
        assert_eq!(read_for(&mut dev, 80), "");
        job.control.resume();
        assert_eq!(read_for(&mut dev, 60), "G21\n");
        job.control.cancel();
        dev.write_all(b"ok\r\nok\r\nok\r\n").unwrap();
        let (streamer, out) = job.join().unwrap();
        assert_eq!(out, Outcome::Cancelled);
        assert_eq!(read_for(&mut dev, 20), "M410\nM5\n");
        let stop = streamer.last_stop().unwrap();
        assert!(stop.position_maybe_lost);
    }

    #[test]
    fn grbl_link_from_new_matches_the_config_buffer() {
        let s = streamer(200);
        assert_eq!(s.link().flow, FlowControl::CharCounting { rx_buffer: 200 });
        assert_eq!(s.link().family, LinkFamily::Grbl);
    }

    fn streamer(rx_buffer: usize) -> Streamer<crate::serial::MemoryTransport> {
        let (host, _dev) = memory_pair();
        let config = StreamerConfig {
            rx_buffer,
            ..Default::default()
        };
        Streamer::new(host, config)
    }

    #[test]
    fn line_must_fit_with_newline() {
        let s = streamer(8);
        assert!(s.prepare(&["G1X1Y22"]).is_ok()); // 7 + '\n' = 8
        assert_eq!(
            s.prepare(&["G0", "G1X1Y223"]).err(),
            Some(StreamError::LineTooLong {
                source_line: 2,
                len: 9,
                max: 8
            })
        );
    }

    #[test]
    fn empty_lines_are_skipped_but_keep_source_numbers() {
        let s = streamer(128);
        let job = s.prepare(&["G0 X0", "", "; nada", "G1 X1"]).unwrap();
        let got: Vec<_> = job
            .iter()
            .map(|p| (p.source_line, p.text.as_str()))
            .collect();
        assert_eq!(got, [(1, "G0X0"), (4, "G1X1")]);
    }

    #[test]
    fn empty_job_completes_without_io() {
        let mut s = streamer(128);
        let mut events = Vec::new();
        let out = s.run::<&str>(&[], |e| events.push(e)).unwrap();
        assert_eq!(out, Outcome::Completed { errors: 0 });
        assert_eq!(events, [Event::Finished(Outcome::Completed { errors: 0 })]);
    }

    #[test]
    fn realtime_bytes_match_grbl_11() {
        assert_eq!(Realtime::SoftReset.byte(), 0x18);
        assert_eq!(Realtime::JogCancel.byte(), 0x85);
        assert_eq!(Realtime::FeedPlus10.byte(), 0x91);
        assert_eq!(Realtime::SpindleStop.byte(), 0x9E);
    }

    #[test]
    fn status_edge_cases() {
        assert_eq!(StatusReport::parse("ok"), None);
        assert_eq!(StatusReport::parse("<>"), None);
        assert_eq!(StatusReport::parse("<|MPos:0,0,0>"), None);
        assert_eq!(StatusReport::parse("<Idle|MPos:0,0,0"), None);
        // Basura con forma de línea `<...>` pero sin un nombre de estado.
        assert_eq!(StatusReport::parse("<\u{fffd}\u{fffd}|MPos:0,0,0>"), None);
        assert_eq!(StatusReport::parse("<12345|MPos:0,0,0>"), None);
        assert_eq!(StatusReport::parse("<Nope>"), None);
        let r = StatusReport::parse("<Run|MPos:abc|Zz:1|Bf:1|FS:5,6>").unwrap();
        assert_eq!((r.mpos, r.buffer), (None, None));
        assert_eq!((r.feed, r.spindle), (Some(5.0), Some(6.0)));
        let r = StatusReport::parse(" <Idle|MPos:1.5,2.5>\r").unwrap();
        assert_eq!(r.mpos.unwrap().z, 0.0);
        let r = StatusReport::parse("<Idle|MPos:1,2,3,4,5>").unwrap();
        assert_eq!(r.mpos.unwrap().extra, [4.0, 5.0]);
        assert_eq!(
            StatusReport::parse("<Hold|MPos:0,0,0>").unwrap().state,
            MachineState::Hold(None)
        );
    }

    #[test]
    fn legacy_comma_report_parses_state_and_positions() {
        let r = StatusReport::parse_legacy(
            "<Idle,MPos:1.5000,2.0000,0.0000,WPos:1.0000,2.0000,0.0000>",
        )
        .unwrap();
        assert_eq!(r.state, MachineState::Idle);
        assert_eq!(r.mpos.unwrap().x, 1.5);
        assert_eq!(r.wpos.unwrap().y, 2.0);
        let r = StatusReport::parse_legacy("<Run,MPos:0,0,0>").unwrap();
        assert_eq!(r.state, MachineState::Run);
        assert_eq!(r.wpos, None);
        assert!(StatusReport::parse_legacy("<Nope>").is_none());
        assert!(StatusReport::parse_legacy("ok").is_none());
        // El informe de GRBL 1.1 no es de este formato.
        assert!(StatusReport::parse_legacy("<Idle|MPos:0,0,0>").is_none());
    }

    #[test]
    fn unknown_states_do_not_discard_the_report() {
        // grblHAL: estado `Tool` con campos extra y FS de tres valores.
        let r = StatusReport::parse("<Tool|MPos:1.000,2.000,0.000|FS:0,0,0|WCS:G54|TLR:0|H:1>")
            .unwrap();
        assert_eq!(r.state, MachineState::Other);
        assert_eq!(r.other_state.as_deref(), Some("Tool"));
        assert_eq!(r.mpos.unwrap().x, 1.0);
        assert!(!MachineState::Other.is_stationary());
        let r = StatusReport::parse("<Bogus|MPos:0,0,0>").unwrap();
        assert_eq!(r.other_state.as_deref(), Some("Bogus"));
        // Subcódigo ilegible: el estado se conserva sin él.
        assert_eq!(
            StatusReport::parse("<Hold:x|MPos:0,0,0>").unwrap().state,
            MachineState::Hold(None)
        );
        // Los conocidos no llevan nombre.
        assert_eq!(
            StatusReport::parse("<Idle|MPos:0,0,0>")
                .unwrap()
                .other_state,
            None
        );
    }

    #[test]
    fn fs_accepts_two_or_three_values() {
        let r = StatusReport::parse("<Run|MPos:1.000,1.000,0.000|FS:1500,255,240>").unwrap();
        assert_eq!((r.feed, r.spindle), (Some(1500.0), Some(255.0)));
        let r = StatusReport::parse("<Alarm:1|MPos:0.000,0.000,0.000|FS:0,0,0|Pn:X>").unwrap();
        assert_eq!(r.state, MachineState::Alarm);
        assert_eq!(r.pins.as_deref(), Some("X"));
        // Uno o cuatro valores no son un FS válido.
        let r = StatusReport::parse("<Run|FS:1500>").unwrap();
        assert_eq!((r.feed, r.spindle), (None, None));
        let r = StatusReport::parse("<Run|FS:1,2,3,4>").unwrap();
        assert_eq!((r.feed, r.spindle), (None, None));
    }

    #[test]
    fn nan_and_inf_are_unknown_not_positions() {
        let r = StatusReport::parse(
            "<Idle|MPos:0.000,0.000,0.000|Bf:127,65535|FS:0,0|WCO:nan,1.579,1.619>",
        )
        .unwrap();
        assert!(r.mpos.is_some());
        assert_eq!(r.wco, None);
        let r = StatusReport::parse("<Idle|MPos:-nan,1,2|WPos:inf,0,0|F:NaN|FS:nan,0|Bf:nan,1>")
            .unwrap();
        assert_eq!(
            (r.mpos, r.wpos, r.feed, r.spindle, r.buffer),
            (None, None, None, None, None)
        );
        // Una línea de `$#` real de la Falcon2.
        assert_eq!(
            parse_parameter_line("[G54:-nan,1.579,1.619]"),
            Some(("G54".to_string(), None))
        );
        assert_eq!(
            parse_parameter_line("[G55:0.000,0.000,0.000]"),
            Some(("G55".to_string(), Some(vec![0.0, 0.0, 0.0])))
        );
        assert_eq!(
            parse_parameter_line("[PRB:1.000,2.000,3.000:1]"),
            Some(("PRB".to_string(), Some(vec![1.0, 2.0, 3.0])))
        );
        assert_eq!(parse_parameter_line("ok"), None);
    }

    #[test]
    fn greeting_detection_never_slices_inside_a_character() {
        // Bytes inválidos llegan como U+FFFD (3 bytes cada uno): `line[..4]` haría panic.
        let lossy = |b: &[u8]| String::from_utf8_lossy(b).into_owned();
        for raw in [
            &b"ab\xff\xfe"[..],
            b"\xff\xfe\xfd\xfc\xfb",
            b"gr\xff",
            b"grb\xe2\x82",
            b"\xc3",
            b"\xff",
            b"gr\xc3\xa9b",
            "ñandú".as_bytes(),
            b"",
        ] {
            let line = lossy(raw);
            assert!(!is_greeting(&line), "{line:?}");
            let _ = StatusReport::parse(&line);
            let _ = parse_creality_error(&line);
            let _ = writes_eeprom(&line);
            let _ = parse_parameter_line(&line);
        }
        assert!(is_greeting("Grbl 1.1h ['$' for help]"));
        assert!(is_greeting("GrblHAL 1.1f"));
        assert!(is_greeting("GRBL"));
        assert!(!is_greeting("grb"));
    }

    #[test]
    fn creality_errors_are_not_grbl_errors() {
        assert_eq!(parse_creality_error("ERROR:05"), Some(5));
        assert_eq!(parse_creality_error("ERROR:06."), Some(6));
        assert_eq!(
            parse_creality_error("ERROR:24. Machine not sealed"),
            Some(24)
        );
        assert_eq!(parse_creality_error("  ERROR: 01 \r"), Some(1));
        // El `error:N` de GRBL (minúsculas) no es una falla de Creality.
        assert_eq!(parse_creality_error("error:5"), None);
        assert_eq!(parse_creality_error("Error:5"), None);
        assert_eq!(parse_creality_error("ERROR:"), None);
        assert_eq!(parse_creality_error("ERROR:0x"), None);
        assert_eq!(parse_creality_error("ERROR:12345"), None);
        assert_eq!(parse_creality_error("ERRORS:05"), None);
        assert!(creality_error_message(5).contains("llama"));
        assert!(creality_error_message(1).contains("inclinación"));
        assert!(creality_error_message(6).contains("lente"));
        assert!(creality_error_message(24).contains("tapa"));
        assert!(creality_error_message(4).contains("aire"));
        assert!(creality_error_message(99).contains("sin descripción"));
    }

    #[test]
    fn eeprom_writing_lines_are_detected() {
        for yes in [
            "$100=80",
            "$N0=G54",
            "$I=Mi láser",
            "$RST=$",
            "G10L2P1X0Y0",
            "G10L20P1X0",
            "G28.1",
            "G30.1",
            "G90G10L2P1X0",
            "g10l2p1x0",
        ] {
            assert!(writes_eeprom(yes), "{yes}");
        }
        for no in [
            "G1X10F1000",
            "G0X1Y1",
            "G28",
            "G30",
            "G100",
            "$J=G91X10F500",
            "$$",
            "$H",
            "$X",
            "M3S500",
            "G1X10G10",
        ] {
            // `G1X10G10` sí tiene un G10 pegado; se comprueba aparte.
            if no == "G1X10G10" {
                assert!(writes_eeprom(no));
            } else {
                assert!(!writes_eeprom(no), "{no}");
            }
        }
        assert!(is_homing_line("$H") && is_homing_line("$HX") && is_homing_line("$h"));
        assert!(!is_homing_line("$X") && !is_homing_line("H"));
    }

    #[test]
    fn firmware_family_is_detected_from_identification_lines() {
        let l = |v: &[&str]| Firmware::detect(v);
        assert_eq!(
            l(&["[VER:1.1h.20190825:]", "[OPT:V,15,128]"]),
            Firmware::Grbl
        );
        assert_eq!(
            l(&["[VER:3.9 FluidNC v3.9.7:]", "[OPT:MPHS]"]),
            Firmware::FluidNc
        );
        assert_eq!(
            l(&[
                "[VER:1.1f.20240101:]",
                "[OPT:VNM,35,1024,0]",
                "[NEWOPT:ENUMS,RT+]"
            ]),
            Firmware::GrblHal
        );
        assert_eq!(l(&["[FIRMWARE:grblHAL]"]), Firmware::GrblHal);
        assert_eq!(l(&[]), Firmware::Unknown);
        assert_eq!(
            Firmware::from_banner("GrblHAL 1.1f ['$' or '$HELP' for help]"),
            Some(Firmware::GrblHal)
        );
        assert_eq!(
            Firmware::from_banner("Grbl 3.4 [FluidNC v3.4.3 (wifi) '$' for help]"),
            Some(Firmware::FluidNc)
        );
        assert_eq!(Firmware::from_banner("Grbl 1.1h ['$' for help]"), None);
    }

    #[test]
    fn error_tables_cover_known_codes() {
        assert_eq!(error_message(20), "comando G-code no soportado o no válido");
        assert_eq!(alarm_message(1), "límite físico activado");
        assert_eq!(alarm_message(99), "alarma desconocida");
        // 1 a 17 y 20 a 38 tienen texto en cualquier firmware.
        for c in (1..=17).chain(20..=38) {
            for fw in [
                Firmware::Unknown,
                Firmware::Grbl,
                Firmware::GrblHal,
                Firmware::FluidNc,
            ] {
                let m = error_message_for(fw, c);
                assert!(
                    !m.contains("desconocido") && !m.contains("extendido"),
                    "{fw:?} {c}: {m}"
                );
            }
        }
        for c in 1..=9 {
            assert_ne!(alarm_message(c), "alarma desconocida", "{c}");
        }
    }

    #[test]
    fn extended_codes_depend_on_the_firmware_family() {
        // ALARM:10 es autoescuadre en GRBL 1.1 y paro de emergencia en grblHAL.
        assert!(alarm_message_for(Firmware::Grbl, 10).contains("autoescuadre"));
        assert!(alarm_message_for(Firmware::GrblHal, 10).contains("emergencia"));
        assert!(alarm_message(10).contains("depende del firmware"));
        for c in 11..=22 {
            assert!(
                alarm_message_for(Firmware::GrblHal, c).contains("grblHAL"),
                "{c}"
            );
        }
        // Errores extendidos: texto de grblHAL solo si se sabe que lo es.
        assert_eq!(
            error_message_for(Firmware::GrblHal, 50),
            "parada de emergencia activa"
        );
        assert!(error_message_for(Firmware::GrblHal, 46).contains("homing"));
        assert!(error_message_for(Firmware::GrblHal, 63).contains("SD"));
        assert!(error_message_for(Firmware::FluidNc, 63).contains("SD"));
        assert!(error_message_for(Firmware::FluidNc, 39).contains("segundos"));
        assert!(error_message_for(Firmware::GrblHal, 39).contains("fuera de rango"));
        assert!(error_message(50).contains("depende de la familia"));
        assert!(error_message(999).contains("depende de la familia"));
        // En GRBL 1.1 puro no existen: sin texto inventado.
        assert_eq!(error_message_for(Firmware::Grbl, 50), "error desconocido");
        assert_eq!(error_message(0), "error desconocido");
    }
}
