//! Familia del firmware y textos de `error:N`, `ALARM:N` y fallas de Creality.

use tlaku_i18n::tr;

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
fn classic_error(code: u32) -> Option<String> {
    Some(match code {
        1 => tr!("grbl.firmware.classic_1"),
        2 => tr!("grbl.firmware.classic_2"),
        3 => tr!("grbl.firmware.classic_3"),
        4 => tr!("grbl.firmware.classic_4"),
        5 => tr!("grbl.firmware.classic_5"),
        6 => tr!("grbl.firmware.classic_6"),
        7 => tr!("grbl.firmware.classic_7"),
        8 => tr!("grbl.firmware.classic_8"),
        9 => tr!("grbl.firmware.classic_9"),
        10 => tr!("grbl.firmware.classic_10"),
        11 => tr!("grbl.firmware.classic_11"),
        12 => tr!("grbl.firmware.classic_12"),
        13 => tr!("grbl.firmware.classic_13"),
        14 => tr!("grbl.firmware.classic_14"),
        15 => tr!("grbl.firmware.classic_15"),
        16 => tr!("grbl.firmware.classic_16"),
        17 => tr!("grbl.firmware.classic_17"),
        20 => tr!("grbl.firmware.classic_20"),
        21 => tr!("grbl.firmware.classic_21"),
        22 => tr!("grbl.firmware.classic_22"),
        23 => tr!("grbl.firmware.classic_23"),
        24 => tr!("grbl.firmware.classic_24"),
        25 => tr!("grbl.firmware.classic_25"),
        26 => tr!("grbl.firmware.classic_26"),
        27 => tr!("grbl.firmware.classic_27"),
        28 => tr!("grbl.firmware.classic_28"),
        29 => tr!("grbl.firmware.classic_29"),
        30 => tr!("grbl.firmware.classic_30"),
        31 => tr!("grbl.firmware.classic_31"),
        32 => tr!("grbl.firmware.classic_32"),
        33 => tr!("grbl.firmware.classic_33"),
        34 => tr!("grbl.firmware.classic_34"),
        35 => tr!("grbl.firmware.classic_35"),
        36 => tr!("grbl.firmware.classic_36"),
        37 => tr!("grbl.firmware.classic_37"),
        38 => tr!("grbl.firmware.classic_38"),
        _ => return None,
    })
}

/// `error:N` de grblHAL por encima de GRBL 1.1 (nombres de su API pública; la documentación
/// consultada no trae descripciones, así que los textos son una traducción fiel del nombre).
fn hal_error(code: u32) -> Option<String> {
    Some(match code {
        18 => tr!("grbl.firmware.hal_18"),
        19 => tr!("grbl.firmware.hal_19"),
        39 => tr!("grbl.firmware.hal_39"),
        40 => tr!("grbl.firmware.hal_40"),
        41 => tr!("grbl.firmware.hal_41"),
        42 => tr!("grbl.firmware.hal_42"),
        43 => tr!("grbl.firmware.hal_43"),
        44 => tr!("grbl.firmware.hal_44"),
        45 => tr!("grbl.firmware.hal_45"),
        46 => tr!("grbl.firmware.hal_46"),
        47 => tr!("grbl.firmware.hal_47"),
        48 => tr!("grbl.firmware.hal_48"),
        49 => tr!("grbl.firmware.hal_49"),
        50 => tr!("grbl.firmware.hal_50"),
        51 => tr!("grbl.firmware.hal_51"),
        52 => tr!("grbl.firmware.hal_52"),
        53 => tr!("grbl.firmware.hal_53"),
        54 => tr!("grbl.firmware.hal_54"),
        55 => tr!("grbl.firmware.hal_55"),
        56 => tr!("grbl.firmware.hal_56"),
        57 => tr!("grbl.firmware.hal_57"),
        58 => tr!("grbl.firmware.hal_58"),
        60..=66 => tr!("grbl.firmware.hal_60_to_66"),
        70 => tr!("grbl.firmware.hal_70"),
        71..=76 => tr!("grbl.firmware.hal_71_to_76"),
        77..=79 => tr!("grbl.firmware.hal_77_to_79"),
        80..=83 => tr!("grbl.firmware.hal_80_to_83"),
        84..=89 => tr!("grbl.firmware.hal_84_to_89"),
        90..=92 => tr!("grbl.firmware.hal_90_to_92"),
        _ => return None,
    })
}

/// `error:N` de FluidNC por encima de GRBL 1.1 ([T]: lista de terceros, sin confirmar con su
/// documentación oficial). FluidNC ofrece `$E=N` para consultar el texto en la propia máquina.
fn fluidnc_error(code: u32) -> Option<String> {
    Some(match code {
        19 => tr!("grbl.firmware.fluidnc_19"),
        39 => tr!("grbl.firmware.fluidnc_39"),
        60..=69 => tr!("grbl.firmware.fluidnc_60_to_69"),
        70 | 71 => tr!("grbl.firmware.fluidnc_70_71"),
        80 | 81 => tr!("grbl.firmware.fluidnc_80_81"),
        90 => tr!("grbl.firmware.fluidnc_90"),
        100 | 101 => tr!("grbl.firmware.fluidnc_100_101"),
        110 => tr!("grbl.firmware.fluidnc_110"),
        120 => tr!("grbl.firmware.fluidnc_120"),
        152 => tr!("grbl.firmware.fluidnc_152"),
        _ => return None,
    })
}

/// Descripción breve de un `error:N` sin conocer el firmware. Los códigos de GRBL 1.1 tienen
/// su texto; los extendidos (18, 19 y desde el 39) devuelven un texto genérico, porque cambian
/// de significado según la familia. Ver [`error_message_for`].
pub fn error_message(code: u32) -> String {
    error_message_for(Firmware::Unknown, code)
}

/// Descripción breve de un `error:N` según la familia del firmware.
pub fn error_message_for(fw: Firmware, code: u32) -> String {
    if let Some(m) = classic_error(code) {
        return m;
    }
    match fw {
        Firmware::GrblHal => hal_error(code),
        Firmware::FluidNc => fluidnc_error(code),
        Firmware::Grbl | Firmware::Unknown => None,
    }
    .unwrap_or_else(|| {
        if code >= 18 && fw != Firmware::Grbl {
            tr!("grbl.firmware.extended_error")
        } else {
            tr!("grbl.firmware.unknown_error")
        }
    })
}

/// Descripción de un `ALARM:N` sin conocer el firmware. El 10 es ambiguo (autoescuadre en
/// GRBL 1.1, paro de emergencia en grblHAL); ver [`alarm_message_for`].
pub fn alarm_message(code: u32) -> String {
    alarm_message_for(Firmware::Unknown, code)
}

/// Descripción breve de un `ALARM:N` según la familia del firmware.
pub fn alarm_message_for(fw: Firmware, code: u32) -> String {
    match code {
        1 => tr!("grbl.firmware.alarm_1"),
        2 => tr!("grbl.firmware.alarm_2"),
        3 => tr!("grbl.firmware.alarm_3"),
        4 => tr!("grbl.firmware.alarm_4"),
        5 => tr!("grbl.firmware.alarm_5"),
        6 => tr!("grbl.firmware.alarm_6"),
        7 => tr!("grbl.firmware.alarm_7"),
        8 => tr!("grbl.firmware.alarm_8"),
        9 => tr!("grbl.firmware.alarm_9"),
        10 => match fw {
            Firmware::Grbl => tr!("grbl.firmware.alarm_10_grbl"),
            Firmware::GrblHal => tr!("grbl.firmware.alarm_10_hal"),
            Firmware::FluidNc => tr!("grbl.firmware.alarm_10_fluidnc"),
            Firmware::Unknown => tr!("grbl.firmware.alarm_10_unknown"),
        },
        // Solo grblHAL define estos números (nombres de su API pública, sin descripciones).
        11 => tr!("grbl.firmware.alarm_11"),
        12 => tr!("grbl.firmware.alarm_12"),
        13 => tr!("grbl.firmware.alarm_13"),
        14 => tr!("grbl.firmware.alarm_14"),
        15 => tr!("grbl.firmware.alarm_15"),
        16 => tr!("grbl.firmware.alarm_16"),
        17 => tr!("grbl.firmware.alarm_17"),
        18 => tr!("grbl.firmware.alarm_18"),
        19 => tr!("grbl.firmware.alarm_19"),
        20 => tr!("grbl.firmware.alarm_20"),
        21 => tr!("grbl.firmware.alarm_21"),
        22 => tr!("grbl.firmware.alarm_22"),
        _ => tr!("grbl.firmware.unknown_alarm"),
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
pub fn creality_error_message(code: u32) -> String {
    match code {
        1 => tr!("grbl.firmware.creality_1"),
        2 => tr!("grbl.firmware.creality_2"),
        4 => tr!("grbl.firmware.creality_4"),
        5 => tr!("grbl.firmware.creality_5"),
        6 => tr!("grbl.firmware.creality_6"),
        7 => tr!("grbl.firmware.creality_7"),
        8..=12 => tr!("grbl.firmware.creality_8_to_12"),
        13 => tr!("grbl.firmware.creality_13"),
        14 => tr!("grbl.firmware.creality_14"),
        24 => tr!("grbl.firmware.creality_24"),
        25 => tr!("grbl.firmware.creality_25"),
        _ => tr!("grbl.firmware.creality_unknown"),
    }
}

/// Qué hacer ante un `ERROR:NN` de Creality. Tlaku no envía `$X` ni nada por su cuenta.
pub fn creality_recovery() -> String {
    tr!("grbl.firmware.creality_recovery")
}
