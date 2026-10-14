//! Familia del firmware y textos de `error:N`, `ALARM:N` y fallas de Creality.

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
