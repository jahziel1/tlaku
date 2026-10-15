//! Lector de perfiles de dispositivo `.lbdev` de LightBurn (interoperabilidad, sala limpia).
//!
//! Un `.lbdev` es un JSON de texto que fabricantes y usuarios publican. Las claves y su
//! estructura se aprendieron **solo de un ejemplo público**: el perfil «Creality Falcon2» pegado
//! en <https://forum.creality.com/t/profile-settings/36308> (envoltorio `{"DeviceList":[...]}`),
//! más el nombre de los controladores que ese archivo y la documentación pública de importación
//! mencionan. No se usa código, texto ni ícono de LightBurn.
//!
//! ## Estructura observada
//!
//! ```text
//! { "DeviceList": [ {
//!     "DisplayName", "Name" (controlador), "Type" (conexión), "Width", "Height",
//!     "MirrorX", "MirrorY", "HomeOnStartup", "Info", "LastCamera",
//!     "EnableLaser2Offset", "Laser2OffsetX/Y", "EnableProcessOffset", "ProcessOffsetX/Y",
//!     "Settings": { "BaudRate", "CommPort", "S_Scale", "StartGCode", "EndGCode", "EnableZ",
//!                   "NegativeZ", "AirAssistM7", "EnableBoundsCheck", "TransferMode",
//!                   "Sim_MaxSpeedX/Y", "Sim_RapidSpeed", "Sim_MaxAccelX/Y", "UserFinishX/Y", ... }
//! } ] }
//! ```
//!
//! ## Mapeo a [`DeviceProfile`]
//!
//! | Clave | Campo | Nota |
//! |---|---|---|
//! | `DisplayName` | `name` | sin nombre, se usa `Name`; si ya existe, sufijo ` (2)` |
//! | `Name` | `kind` | ver [`kind_from_name`]; los controladores sin soporte **no se importan** |
//! | `Width`, `Height` | `bed` | mm |
//! | `MirrorX`, `MirrorY` | `origin` | ver abajo |
//! | `Type`, `Settings.BaudRate` | `connection`, `baud` | solo serie; Ruida usa UDP |
//! | `Settings.S_Scale` | `s_max` | entero positivo |
//! | `Settings.EnableZ` | `z_axis` | el sentido de Z se confirma a mano (aviso) |
//! | `Settings.StartGCode`, `EndGCode` | `start_gcode`, `end_gcode` | con aviso de revisarlos |
//! | `Settings.AirAssistM7` | `dialect.air_on` = `M7` | si es falso, el aire de fábrica (`M8`) |
//! | `HomeOnStartup` | `home_on_connect` | |
//! | `Settings.TransferMode` | `sync_streaming` | 1 = síncrono (inferido; 0 = con búfer) |
//! | `Settings.Sim_MaxSpeedX/Y`, `Sim_RapidSpeed` | `max_rate`, `rapid_rate` | mm/s a mm/min (x60) |
//! | `Settings.Sim_MaxAccelX/Y` | `accel` | mm/s², igual |
//!
//! **Origen.** Los dos únicos datos públicos son el Falcon2 (`MirrorX` y `MirrorY` falsos, una
//! máquina GRBL con el cero adelante a la izquierda, la esquina que la documentación pública de
//! LightBurn dice habitual en GRBL) y los proyectos `.lbrn2` de un Ruida con ambos verdaderos
//! (cero atrás a la derecha). Se deduce que cada marca refleja un eje desde «adelante a la
//! izquierda»: `(F,F)` abajo-izquierda, `(V,F)` abajo-derecha, `(F,V)` arriba-izquierda y `(V,V)`
//! arriba-derecha. Es una deducción: cualquier marca verdadera deja un aviso para confirmarla.
//!
//! **Velocidades de simulación.** `Sim_*` son los valores del simulador de tiempos de LightBurn,
//! no los del firmware. Se usan como punto de partida y se avisa que conviene contrastarlos con
//! `$$` (Gestor de dispositivos > «Leer de la máquina», que propone `$110/$111`, `$120/$121`,
//! `$130/$131` y `$30` sin cambiar nada sin confirmación).
//!
//! **Controladores sin soporte** (Trocen, TopWisdom, desconocidos): el dispositivo se omite con
//! aviso en vez de crear un perfil GRBL, porque mandar G-code a un DSP sería peligroso.
//!
//! Este módulo solo **lee** texto y fabrica perfiles en memoria; no escribe nada en la máquina.

use crate::device::{
    Connection, DeviceKind, DeviceProfile, GcodeDialect, Origin, RuidaSettings, SpeedUnit,
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use tlaku_i18n::tr;
use ts_rs::TS;

/// Qué tipo de aviso es; la interfaz le pone el texto en el idioma del usuario.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum LbdevNoticeKind {
    /// Claves con valor que Tlaku no tiene (`detail`: lista separada por comas).
    NotImported,
    /// Valor ilegible o fuera de rango; se usó el de fábrica (`detail`: `clave: valor`).
    InvalidValue,
    /// Falta un dato necesario; se usó el de fábrica (`detail`: la clave).
    MissingValue,
    /// Controlador sin soporte: dispositivo omitido (`detail`: el valor de `Name`).
    UnsupportedController,
    /// Entrada que no es un dispositivo: omitida (`detail`: su posición, desde 1).
    SkippedEntry,
    /// Velocidades y aceleraciones del simulador: confirmar con `$$`.
    SimulatorValues,
    /// Trae G-code de inicio o fin (`detail`: `start`, `end` o `start, end`): revisarlo.
    ScriptCode,
    /// Tiene eje Z: confirmar el sentido con una prueba.
    ZAxisCheck,
    /// El origen se dedujo de `MirrorX`/`MirrorY`: confirmarlo (`detail`: `X`, `Y` o `X, Y`).
    OriginInferred,
    /// Tipo de conexión que no se importa (`detail`: valor de `Type`).
    ConnectionType,
    /// Ruida: falta la dirección IP de la controladora.
    RuidaHost,
}

/// Aviso de una importación, ligado al dispositivo que lo causó.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub struct LbdevNotice {
    /// Nombre del dispositivo en el archivo (`DisplayName`).
    pub device: String,
    pub kind: LbdevNoticeKind,
    pub detail: String,
}

/// Resultado de leer un `.lbdev`: perfiles listos para guardar y avisos.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub struct LbdevImport {
    pub devices: Vec<DeviceProfile>,
    pub notices: Vec<LbdevNotice>,
}

/// Claves que no se avisan: datos locales del usuario o de presentación.
const LOCAL_KEYS: &[&str] = &[
    "CommPort",
    "LastMachineFilePath",
    "LastMachineFileExtension",
    "LastCamera",
    "LastExportExt",
    "Info",
    "DefaultCutList",
    "DefaultToolCutList",
];

/// Claves que este lector consume (aunque algunas solo para decidir un aviso).
const USED_KEYS: &[&str] = &[
    "DisplayName",
    "Name",
    "Type",
    "Width",
    "Height",
    "MirrorX",
    "MirrorY",
    "HomeOnStartup",
    "Settings",
    "BaudRate",
    "S_Scale",
    "StartGCode",
    "EndGCode",
    "EnableZ",
    "AirAssistM7",
    "EnableBoundsCheck",
    "TransferMode",
    "Sim_MaxSpeedX",
    "Sim_MaxSpeedY",
    "Sim_RapidSpeed",
    "Sim_MaxAccelX",
    "Sim_MaxAccelY",
];

/// Claves que solo tienen sentido en un controlador de G-code.
const GCODE_ONLY_KEYS: &[&str] = &[
    "BaudRate",
    "S_Scale",
    "StartGCode",
    "EndGCode",
    "EnableZ",
    "AirAssistM7",
    "HomeOnStartup",
    "TransferMode",
];

/// Claves donde puede venir la dirección IP de una controladora de red (nombres no publicados:
/// se aceptan los habituales).
const HOST_KEYS: &[&str] = &["IPAddress", "IpAddress", "IP", "Address", "Host"];

const MAX_DEVICES: usize = 100;

/// Controlador de Tlaku para el valor de `Name`, o `None` si no hay soporte. Se compara sin
/// mayúsculas ni separadores (`GRBL-M3` y `grbl m3` son lo mismo).
pub fn kind_from_name(name: &str) -> Option<DeviceKind> {
    let key: String = name
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| c.to_ascii_lowercase())
        .collect();
    Some(match key.as_str() {
        "grbl" => DeviceKind::Grbl,
        "grblm3" => DeviceKind::GrblM3,
        "grbllpc" => DeviceKind::GrblLpc,
        "grblhal" => DeviceKind::GrblHal,
        "fluidnc" => DeviceKind::FluidNc,
        "smoothieware" | "smoothie" => DeviceKind::Smoothie,
        "marlin" => DeviceKind::Marlin,
        "gcode" | "customgcode" => DeviceKind::CustomGcode,
        "ruida" => DeviceKind::Ruida,
        _ => return None,
    })
}

/// Origen a partir de `MirrorX`/`MirrorY` (ver la deducción en la documentación del módulo).
pub fn origin_from_mirror(mirror_x: bool, mirror_y: bool) -> Origin {
    match (mirror_x, mirror_y) {
        (false, false) => Origin::BottomLeft,
        (true, false) => Origin::BottomRight,
        (false, true) => Origin::TopLeft,
        (true, true) => Origin::TopRight,
    }
}

/// Lee un `.lbdev` y fabrica los perfiles. `existing` son los (id, nombre) que ya hay, para que
/// los perfiles nuevos tengan id único y no repitan nombre. Falla si el texto no es JSON o si no
/// queda ningún dispositivo que importar.
pub fn parse_lbdev(text: &str, existing: &[(String, String)]) -> Result<LbdevImport, String> {
    let root = parse_json(text)?;
    let entries = device_entries(&root).ok_or_else(|| tr!("doc.lbdev.no_device_list"))?;
    let mut taken_ids: Vec<String> = existing.iter().map(|(id, _)| id.clone()).collect();
    let mut taken_names: Vec<String> = existing.iter().map(|(_, n)| norm_name(n)).collect();
    let mut out = LbdevImport {
        devices: Vec::new(),
        notices: Vec::new(),
    };
    let mut skipped = Vec::new();
    for (i, entry) in entries.iter().take(MAX_DEVICES).enumerate() {
        let Some(map) = entry.as_object() else {
            out.notices.push(LbdevNotice {
                device: String::new(),
                kind: LbdevNoticeKind::SkippedEntry,
                detail: (i + 1).to_string(),
            });
            continue;
        };
        match convert(map, i, &mut taken_ids, &mut taken_names) {
            Ok((device, mut notices)) => {
                out.devices.push(device);
                out.notices.append(&mut notices);
            }
            Err((label, notice)) => {
                skipped.push(label);
                out.notices.push(notice);
            }
        }
    }
    if out.devices.is_empty() {
        return Err(if skipped.is_empty() {
            tr!("doc.lbdev.empty")
        } else {
            tr!("doc.lbdev.nothing_importable", names = skipped.join(", "))
        });
    }
    Ok(out)
}

fn parse_json(text: &str) -> Result<Value, String> {
    let text = text.trim_start_matches('\u{feff}').trim();
    match serde_json::from_str::<Value>(text) {
        Ok(v) => Ok(v),
        Err(first) => {
            // Un perfil copiado de un foro suele traer comillas tipográficas y `\_`.
            let fixed = text
                .replace(['\u{201c}', '\u{201d}'], "\"")
                .replace("\\_", "_");
            serde_json::from_str::<Value>(&fixed)
                .map_err(|_| tr!("doc.lbdev.invalid_json", error = first))
        }
    }
}

/// Lista de dispositivos: `{"DeviceList":[..]}`, `{"Devices":[..]}`, una lista suelta o un solo
/// dispositivo con sus claves en la raíz.
fn device_entries(root: &Value) -> Option<Vec<Value>> {
    match root {
        Value::Array(a) => Some(a.clone()),
        Value::Object(m) => {
            for key in ["DeviceList", "Devices"] {
                if let Some(Value::Array(a)) = m.get(key) {
                    return Some(a.clone());
                }
            }
            (m.contains_key("Name") || m.contains_key("DisplayName")).then(|| vec![root.clone()])
        }
        _ => None,
    }
}

fn norm_name(name: &str) -> String {
    name.trim().to_lowercase()
}

fn slugify(text: &str) -> String {
    let mut s = String::new();
    for c in text.chars().flat_map(char::to_lowercase) {
        let c = match c {
            'á' | 'à' | 'ä' | 'â' => 'a',
            'é' | 'è' | 'ë' | 'ê' => 'e',
            'í' | 'ì' | 'ï' | 'î' => 'i',
            'ó' | 'ò' | 'ö' | 'ô' => 'o',
            'ú' | 'ù' | 'ü' | 'û' => 'u',
            'ñ' => 'n',
            c => c,
        };
        if c.is_ascii_alphanumeric() {
            s.push(c);
        } else if !s.ends_with('-') {
            s.push('-');
        }
    }
    let s = s.trim_matches('-').to_string();
    if s.is_empty() { "maquina".into() } else { s }
}

fn unique_id(name: &str, taken: &[String]) -> String {
    let slug = slugify(name);
    if !taken.contains(&slug) {
        return slug;
    }
    (2..)
        .map(|i| format!("{slug}-{i}"))
        .find(|c| !taken.contains(c))
        .unwrap_or(slug)
}

fn unique_name(name: &str, taken: &[String]) -> String {
    if !taken.contains(&norm_name(name)) {
        return name.to_string();
    }
    (2..)
        .map(|i| format!("{name} ({i})"))
        .find(|c| !taken.contains(&norm_name(c)))
        .unwrap_or_else(|| name.to_string())
}

// ---------------------------------------------------------------------------
// Lectura tolerante de valores
// ---------------------------------------------------------------------------

fn as_num(v: &Value) -> Option<f64> {
    let n = match v {
        Value::Number(n) => n.as_f64()?,
        Value::String(s) => s.trim().parse::<f64>().ok()?,
        _ => return None,
    };
    n.is_finite().then_some(n)
}

fn as_bool(v: &Value) -> Option<bool> {
    match v {
        Value::Bool(b) => Some(*b),
        Value::Number(n) => n.as_f64().map(|x| x != 0.0),
        Value::String(s) => match s.trim().to_ascii_lowercase().as_str() {
            "true" | "1" | "yes" => Some(true),
            "false" | "0" | "no" => Some(false),
            _ => None,
        },
        _ => None,
    }
}

fn as_text(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        _ => None,
    }
}

/// ¿El valor cambia algo respecto de «apagado»? Verdadero, número distinto de 0, texto o lista
/// no vacíos.
fn is_active(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().is_some_and(|x| x != 0.0),
        Value::String(s) => !s.trim().is_empty() && !matches!(s.trim(), "0" | "false" | "False"),
        Value::Array(a) => !a.is_empty(),
        Value::Object(o) => !o.is_empty(),
    }
}

/// Lector de claves del dispositivo y de su `Settings`, con registro de lo usado y de los avisos.
struct Reader<'a> {
    top: &'a Map<String, Value>,
    settings: Option<&'a Map<String, Value>>,
    device: String,
    notices: Vec<LbdevNotice>,
}

impl<'a> Reader<'a> {
    fn get(&self, key: &str) -> Option<&'a Value> {
        self.top
            .get(key)
            .or_else(|| self.settings.and_then(|s| s.get(key)))
            .filter(|v| !v.is_null())
    }

    fn notice(&mut self, kind: LbdevNoticeKind, detail: impl Into<String>) {
        self.notices.push(LbdevNotice {
            device: self.device.clone(),
            kind,
            detail: detail.into(),
        });
    }

    fn bad(&mut self, key: &str, v: &Value) {
        let shown = match v {
            Value::String(s) => s.clone(),
            other => other.to_string(),
        };
        self.notice(LbdevNoticeKind::InvalidValue, format!("{key}: {shown}"));
    }

    fn bool_or(&mut self, key: &str) -> bool {
        match self.get(key) {
            None => false,
            Some(v) => as_bool(v).unwrap_or_else(|| {
                self.bad(key, v);
                false
            }),
        }
    }

    /// Número positivo y razonable (hasta `max`), o `None` con aviso si estaba presente pero mal.
    fn positive(&mut self, key: &str, max: f64) -> Option<f64> {
        let v = self.get(key)?;
        match as_num(v) {
            Some(n) if n > 0.0 && n <= max => Some(n),
            _ => {
                self.bad(key, v);
                None
            }
        }
    }
}

type Skip = (String, LbdevNotice);

/// Convierte una entrada. `Err` = dispositivo omitido (etiqueta para el mensaje y su aviso).
fn convert(
    map: &Map<String, Value>,
    index: usize,
    taken_ids: &mut Vec<String>,
    taken_names: &mut Vec<String>,
) -> Result<(DeviceProfile, Vec<LbdevNotice>), Skip> {
    let raw_name = map.get("Name").and_then(as_text).unwrap_or_default();
    let display = map
        .get("DisplayName")
        .and_then(as_text)
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .or_else(|| Some(raw_name.trim().to_string()).filter(|s| !s.is_empty()))
        .unwrap_or_else(|| format!("LightBurn {}", index + 1));
    let Some(kind) = kind_from_name(&raw_name) else {
        let shown = if raw_name.trim().is_empty() {
            "?".to_string()
        } else {
            raw_name.trim().to_string()
        };
        return Err((
            format!("{display} ({shown})"),
            LbdevNotice {
                device: display,
                kind: LbdevNoticeKind::UnsupportedController,
                detail: shown,
            },
        ));
    };

    let mut r = Reader {
        top: map,
        settings: map.get("Settings").and_then(Value::as_object),
        device: display.clone(),
        notices: Vec::new(),
    };
    let mut d = DeviceProfile::for_kind(kind);
    let ruida = kind == DeviceKind::Ruida;

    // Identidad: nombre sin repetir e id nuevo.
    d.name = unique_name(&display, taken_names);
    d.id = unique_id(&d.name, taken_ids);
    taken_names.push(norm_name(&d.name));
    taken_ids.push(d.id.clone());

    // Cama.
    for (i, key) in ["Width", "Height"].into_iter().enumerate() {
        match r.positive(key, 10_000.0) {
            Some(n) => d.bed[i] = n,
            None if r.get(key).is_none() => r.notice(LbdevNoticeKind::MissingValue, key),
            None => {}
        }
    }

    // Origen.
    let (mx, my) = (r.bool_or("MirrorX"), r.bool_or("MirrorY"));
    d.origin = origin_from_mirror(mx, my);
    if mx || my {
        let axes = match (mx, my) {
            (true, true) => "X, Y",
            (true, false) => "X",
            _ => "Y",
        };
        r.notice(LbdevNoticeKind::OriginInferred, axes);
    }

    // Conexión.
    let conn_type = r.get("Type").and_then(as_text).unwrap_or_default();
    let conn_key = conn_type.trim().to_ascii_lowercase();
    if ruida {
        let host = HOST_KEYS
            .iter()
            .find_map(|k| r.get(k).and_then(as_text))
            .map(|s| s.trim().to_string())
            .unwrap_or_default();
        if host.is_empty() {
            r.notice(LbdevNoticeKind::RuidaHost, "");
        }
        d.connection = Connection::Udp { host };
        if !conn_key.is_empty() && conn_key != "ethernet" && conn_key != "udp" {
            r.notice(LbdevNoticeKind::ConnectionType, conn_type.trim());
        }
    } else if !conn_key.is_empty() && conn_key != "serial" && conn_key != "usb" {
        r.notice(LbdevNoticeKind::ConnectionType, conn_type.trim());
    }

    if !ruida {
        if let Some(b) = r.positive("BaudRate", 4_000_000.0) {
            if b.fract() == 0.0 {
                d.baud = b as u32;
            } else {
                r.bad("BaudRate", &Value::from(b));
            }
        }
        if let Some(s) = r.positive("S_Scale", 100_000.0) {
            if s.fract() == 0.0 {
                d.s_max = s;
            } else {
                r.bad("S_Scale", &Value::from(s));
            }
        }
        d.home_on_connect = r.bool_or("HomeOnStartup");
        d.z_axis = r.bool_or("EnableZ");
        if d.z_axis {
            r.notice(LbdevNoticeKind::ZAxisCheck, "");
        }
        if r.bool_or("AirAssistM7") {
            d.dialect = GcodeDialect {
                air_on: Some("M7".into()),
                air_off: Some("M9".into()),
                ..d.dialect
            };
        }
        match r.get("TransferMode").map(|v| (v, as_num(v))) {
            Some((_, Some(1.0))) => d.sync_streaming = true,
            Some((_, Some(0.0))) | None => {}
            Some((v, _)) => r.bad("TransferMode", v),
        }
        let mut scripts = Vec::new();
        for (key, label) in [("StartGCode", "start"), ("EndGCode", "end")] {
            let code = r
                .get(key)
                .and_then(as_text)
                .map(|s| s.replace("\r\n", "\n").replace('\r', "\n"))
                .map(|s| s.trim_end().to_string())
                .unwrap_or_default();
            if !code.trim().is_empty() {
                scripts.push(label);
                if label == "start" {
                    d.start_gcode = code;
                } else {
                    d.end_gcode = code;
                }
            }
        }
        if !scripts.is_empty() {
            r.notice(LbdevNoticeKind::ScriptCode, scripts.join(", "));
        }
    }

    // Velocidades y aceleraciones del simulador (mm/s y mm/s²).
    let mut sim = false;
    for (i, key) in ["Sim_MaxSpeedX", "Sim_MaxSpeedY"].into_iter().enumerate() {
        if let Some(n) = r.positive(key, 1_000.0) {
            d.max_rate[i] = n * 60.0;
            sim = true;
        }
    }
    if let Some(n) = r.positive("Sim_RapidSpeed", 1_000.0) {
        d.rapid_rate = n * 60.0;
        sim = true;
    }
    for (i, key) in ["Sim_MaxAccelX", "Sim_MaxAccelY"].into_iter().enumerate() {
        if let Some(n) = r.positive(key, 1_000_000.0) {
            d.accel[i] = n;
            sim = true;
        }
    }
    if sim {
        r.notice(LbdevNoticeKind::SimulatorValues, "");
    }
    // Las velocidades del perfil se muestran en mm/min, como un perfil nuevo.
    d.speed_unit = SpeedUnit::default();
    if ruida {
        d.ruida = RuidaSettings::default();
    }

    // Lo que no tiene equivalente y está activo.
    let mut unmapped = Vec::new();
    let mut consider = |key: &str, v: &Value| {
        if LOCAL_KEYS.contains(&key) || (ruida && HOST_KEYS.contains(&key)) {
            return;
        }
        if USED_KEYS.contains(&key) {
            // Tlaku siempre comprueba los límites: solo se avisa si ahí estaba apagado. En Ruida
            // las claves de G-code no se usan y, si traen algo, se avisan.
            let off = key == "EnableBoundsCheck" && as_bool(v) == Some(false);
            let unused = ruida && GCODE_ONLY_KEYS.contains(&key) && is_active(v);
            if off || unused {
                unmapped.push(key.to_string());
            }
            return;
        }
        if is_active(v) {
            unmapped.push(key.to_string());
        }
    };
    for (k, v) in map.iter().filter(|(k, _)| k.as_str() != "Settings") {
        consider(k, v);
    }
    if let Some(s) = r.settings {
        for (k, v) in s {
            consider(k, v);
        }
    }
    unmapped.sort();
    unmapped.dedup();
    if !unmapped.is_empty() {
        r.notice(LbdevNoticeKind::NotImported, unmapped.join(", "));
    }
    Ok((d, r.notices))
}

#[cfg(test)]
mod tests;
