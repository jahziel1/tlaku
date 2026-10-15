//! Ajustes de material aportados por la comunidad.
//!
//! Cada aporte es un archivo JSON de `community/materials/` (uno por issue de GitHub). `build.rs`
//! los reúne al compilar y [`crate::factory_library`] los agrega al final de la biblioteca de fábrica
//! con `confidence = forum`: la interfaz los muestra como «Comunidad (sin verificar)». El formato es
//! el compacto de `data/materials-default.json` más los datos de procedencia (máquina, potencia,
//! licencia). Las reglas viven en [`check_entry`] y las comprueba la prueba de este módulo con los
//! archivos reales, así que un aporte inválido no llega a la rama principal.
//!
//! Reglas:
//! - `id` empieza con `community.`, es único y termina con el número del issue.
//! - `source` es la dirección del issue (`https://github.com/jahziel1/tlaku/issues/N`).
//! - `license` es `CC0-1.0`: quien aporta cede los valores al dominio público.
//! - Unidades como en el resto de la biblioteca: mm, mm/min, potencia de 0 a 100 %.
//! - Los valores son de un usuario, no están verificados: son un punto de partida.

use crate::factory::{FactoryData, FactoryEntry};
use crate::layer::LayerMode;
use crate::materials::{Confidence, MachineClass, MaterialPreset, MaterialRole};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use tlaku_i18n::tr;
use ts_rs::TS;

/// Lista JSON con todos los aportes, generada por `build.rs`.
pub(crate) const DATA: &str = include_str!(concat!(env!("OUT_DIR"), "/community.json"));

/// Prefijo de los identificadores de la comunidad.
pub const COMMUNITY_ID_PREFIX: &str = "community.";
/// Licencia exigida a todo aporte.
pub const COMMUNITY_LICENSE: &str = "CC0-1.0";
/// Los issues del repositorio; la fuente de un aporte es uno de ellos.
pub const COMMUNITY_ISSUES_URL: &str = "https://github.com/jahziel1/tlaku/issues/";

/// Más confirmaciones que esto no se guardan en un aporte (con 100 el dato ya está de sobra confirmado).
pub const MAX_CONFIRMATIONS: usize = 100;

/// Una persona que probó el ajuste en su máquina y lo confirmó (comentario en el issue del aporte).
/// La máquina es **declarada**: nadie la demuestra, ni al aportar ni al confirmar.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export)]
pub struct Confirmation {
    /// Usuario de GitHub de quien confirma (distinto de quien aportó).
    pub by: String,
    /// Máquina en la que lo probó, tal como la escribió.
    pub machine: String,
    /// Clase de su máquina, si la dijo.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub machine_class: Option<MachineClass>,
    /// Día de la confirmación, `AAAA-MM-DD`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub date: Option<String>,
    /// Dirección del comentario (`.../issues/N#issuecomment-ID`) o del issue donde lo dijo.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub source: Option<String>,
}

/// Un aporte tal como está en su archivo.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommunityEntry {
    pub id: String,
    /// Clave del material. Si es una de la biblioteca de fábrica (`mdf`, `basswood`...) la interfaz
    /// la traduce; si no, hace falta `material_name`.
    pub material: String,
    pub material_name: Option<String>,
    pub thickness: Option<f64>,
    pub role: MaterialRole,
    /// Clave de la operación (`cut`, `engrave`, `score`...). Si falta se deduce del rol.
    pub title: Option<String>,
    pub machine_class: MachineClass,
    /// Potencia óptica del láser, en W. Debe corresponder a `machine_class`.
    pub laser_watts: f64,
    /// Máquina en la que se probó, tal como la escribió quien aporta.
    pub machine: String,
    pub mode: Option<LayerMode>,
    /// mm/min.
    pub speed: f64,
    /// 0 a 100 %.
    pub power: f64,
    pub passes: Option<u32>,
    /// mm entre líneas del relleno.
    pub interval: Option<f64>,
    #[serde(default)]
    pub air: bool,
    pub frequency_khz: Option<f64>,
    /// Resultado y enfoque, en texto libre.
    pub notes: Option<String>,
    pub source: String,
    pub confidence: Confidence,
    pub license: String,
    /// Usuario de GitHub de quien lo aportó (opcional).
    pub contributor: Option<String>,
    /// Día en que se abrió el issue, `AAAA-MM-DD` (opcional).
    pub date: Option<String>,
    /// Versión de Tlaku con la que se probó (opcional), por ejemplo `0.1.0`.
    pub tlaku_version: Option<String>,
    /// Modelo del catálogo de máquinas de Tlaku (su `id`, por ejemplo `creality-falcon2-22w`) que
    /// puso quien revisó el aporte; sin él solo se compara por clase. Es declarado, no demostrado.
    pub machine_model: Option<String>,
    /// Personas que lo confirmaron (las agrega quien revisa, tras leer el comentario).
    #[serde(default)]
    pub confirmations: Vec<Confirmation>,
}

/// `AAAA-MM-DD` con mes y día posibles (no comprueba los meses de 30 días: es un dato informativo).
pub(crate) fn is_date(text: &str) -> bool {
    let b = text.as_bytes();
    if !text.is_ascii() || b.len() != 10 || b[4] != b'-' || b[7] != b'-' {
        return false;
    }
    let digits = |r: std::ops::Range<usize>| text[r].parse::<u32>().ok();
    match (digits(0..4), digits(5..7), digits(8..10)) {
        (Some(y), Some(m), Some(d)) => {
            (2020..=2100).contains(&y) && (1..=12).contains(&m) && (1..=31).contains(&d)
        }
        _ => false,
    }
}

/// `1.2.3` o `1.2.3-beta.1`, de a lo más 32 caracteres.
fn is_version(text: &str) -> bool {
    let (core, rest) = match text.split_once(['-', '+']) {
        Some((c, r)) => (c, Some(r)),
        None => (text, None),
    };
    let parts: Vec<&str> = core.split('.').collect();
    text.len() <= 32
        && parts.len() == 3
        && parts
            .iter()
            .all(|p| !p.is_empty() && p.len() <= 6 && p.chars().all(|c| c.is_ascii_digit()))
        && rest.is_none_or(|r| {
            !r.is_empty()
                && r.chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '+'))
        })
}

fn is_alias(text: &str) -> bool {
    !text.is_empty()
        && text.len() <= 39
        && text.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

/// `https://github.com/jahziel1/tlaku/issues/N` con, a lo más, `#issuecomment-ID` al final.
fn is_confirmation_source(text: &str) -> bool {
    let (base, fragment) = match text.split_once('#') {
        Some((b, f)) => (b, Some(f)),
        None => (text, None),
    };
    issue_number(base).is_some()
        && fragment.is_none_or(|f| {
            f.strip_prefix("issuecomment-").is_some_and(|id| {
                !id.is_empty() && id.len() <= 20 && id.chars().all(|c| c.is_ascii_digit())
            })
        })
}

fn is_key(text: &str) -> bool {
    !text.is_empty()
        && text.len() <= 40
        && text
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

/// Número del issue de una dirección `https://github.com/jahziel1/tlaku/issues/N`.
pub fn issue_number(source: &str) -> Option<u64> {
    let n = source.strip_prefix(COMMUNITY_ISSUES_URL)?;
    if n.is_empty() || n.starts_with('0') || !n.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    n.parse().ok()
}

fn clean_text(text: &str, max: usize) -> bool {
    !text.trim().is_empty() && text.chars().count() <= max && !text.chars().any(char::is_control)
}

/// Rango de velocidad (mm/min) y de potencia óptica (W) razonables por tipo de láser.
fn limits(class: MachineClass) -> (f64, f64) {
    use crate::materials::LaserKind;
    match class.kind() {
        LaserKind::Diode => (60_000.0, 60.0),
        LaserKind::Co2 => (150_000.0, 300.0),
        LaserKind::Fiber => (600_000.0, 200.0),
    }
}

/// Comprueba un aporte. `data` trae las claves de materiales y operaciones de la biblioteca de fábrica.
pub(crate) fn check_entry(e: &CommunityEntry, data: &FactoryData) -> Result<(), String> {
    let id = e.id.as_str();
    let bad = |field: &str| tr!("doc.community.out_of_range", id = id, field = field);

    let issue = issue_number(&e.source).ok_or_else(|| {
        tr!(
            "doc.community.bad_source",
            id = id,
            prefix = COMMUNITY_ISSUES_URL
        )
    })?;
    let id_ok = id.len() <= 120
        && id.starts_with(COMMUNITY_ID_PREFIX)
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '.' | '_' | '-'))
        && id.ends_with(&format!(".{issue}"));
    if !id_ok {
        return Err(tr!("doc.community.bad_id", id = id, issue = issue));
    }
    if e.license != COMMUNITY_LICENSE {
        return Err(tr!(
            "doc.community.bad_license",
            id = id,
            license = COMMUNITY_LICENSE
        ));
    }
    if e.confidence != Confidence::Forum {
        return Err(tr!("doc.community.bad_confidence", id = id));
    }
    if !is_key(&e.material) {
        return Err(bad("material"));
    }
    if !data.materials.contains_key(&e.material)
        && !e
            .material_name
            .as_deref()
            .is_some_and(|n| clean_text(n, 60))
    {
        return Err(tr!("doc.community.no_name", id = id, material = e.material));
    }
    if let Some(title) = &e.title
        && !data.titles.contains_key(title)
    {
        return Err(tr!("doc.community.bad_title", id = id, title = title));
    }
    let (max_speed, max_watts) = limits(e.machine_class);
    if !e.laser_watts.is_finite() || e.laser_watts <= 0.0 || e.laser_watts > max_watts {
        return Err(bad("laser_watts"));
    }
    let expected = MachineClass::nearest(e.machine_class.kind(), e.laser_watts);
    if expected != e.machine_class {
        return Err(tr!(
            "doc.community.bad_class",
            id = id,
            watts = e.laser_watts,
            class = serde_json::to_string(&expected).unwrap_or_default()
        ));
    }
    if !e.speed.is_finite() || !(1.0..=max_speed).contains(&e.speed) {
        return Err(bad("speed"));
    }
    if !e.power.is_finite() || e.power <= 0.0 || e.power > 100.0 {
        return Err(bad("power"));
    }
    if e.passes.is_some_and(|p| !(1..=100).contains(&p)) {
        return Err(bad("passes"));
    }
    if e.interval
        .is_some_and(|i| !i.is_finite() || !(0.01..=5.0).contains(&i))
    {
        return Err(bad("interval"));
    }
    if e.thickness
        .is_some_and(|t| !t.is_finite() || t <= 0.0 || t > 100.0)
    {
        return Err(bad("thickness"));
    }
    if e.frequency_khz
        .is_some_and(|f| !f.is_finite() || !(1.0..=1000.0).contains(&f))
    {
        return Err(bad("frequency_khz"));
    }
    if !clean_text(&e.machine, 80) {
        return Err(bad("machine"));
    }
    if e.notes.as_deref().is_some_and(|n| !clean_text(n, 400)) {
        return Err(bad("notes"));
    }
    if e.contributor.as_deref().is_some_and(|c| !is_alias(c)) {
        return Err(bad("contributor"));
    }
    if e.date.as_deref().is_some_and(|d| !is_date(d)) {
        return Err(bad("date"));
    }
    if e.tlaku_version.as_deref().is_some_and(|v| !is_version(v)) {
        return Err(bad("tlaku_version"));
    }
    if e.machine_model.as_deref().is_some_and(|m| {
        m.is_empty()
            || m.len() > 60
            || !m
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    }) {
        return Err(bad("machine_model"));
    }
    if e.mode == Some(LayerMode::Image) && e.role != MaterialRole::Engrave {
        return Err(bad("mode"));
    }
    check_confirmations(e, id)
}

/// Las confirmaciones: sin repetir a nadie, sin contar a quien aportó y con la máquina escrita.
fn check_confirmations(e: &CommunityEntry, id: &str) -> Result<(), String> {
    let bad = |field: &str| tr!("doc.community.out_of_range", id = id, field = field);
    if e.confirmations.len() > MAX_CONFIRMATIONS {
        return Err(bad("confirmations"));
    }
    let mut seen = HashSet::new();
    for c in &e.confirmations {
        let valid = is_alias(&c.by)
            && clean_text(&c.machine, 80)
            && c.date.as_deref().is_none_or(is_date)
            && c.source.as_deref().is_none_or(is_confirmation_source)
            && c.machine_class.is_none_or(|k| MachineClass::ALL.contains(&k))
            // Quien aportó no se confirma a sí mismo: no sumaría confianza.
            && e.contributor
                .as_deref()
                .is_none_or(|who| !who.eq_ignore_ascii_case(&c.by))
            && seen.insert(c.by.to_ascii_lowercase());
        if !valid {
            return Err(bad("confirmations"));
        }
    }
    Ok(())
}

/// Convierte un aporte ya comprobado en un ajuste de la biblioteca.
fn expand(e: &CommunityEntry, data: &FactoryData) -> MaterialPreset {
    let title_key = e.title.clone().unwrap_or_else(|| {
        match e.role {
            MaterialRole::Cut => "cut",
            MaterialRole::Engrave => "engrave",
            MaterialRole::Score => "score",
        }
        .to_string()
    });
    let material = data
        .materials
        .get(&e.material)
        .cloned()
        .or_else(|| e.material_name.clone())
        .unwrap_or_else(|| e.material.clone());
    let title = data
        .titles
        .get(&title_key)
        .cloned()
        .unwrap_or_else(|| title_key.clone());
    let entry = FactoryEntry {
        id: e.id.clone(),
        material: e.material.clone(),
        thickness: e.thickness,
        role: e.role,
        title: title_key,
        machine_class: e.machine_class,
        mode: e.mode,
        speed: e.speed,
        power: e.power,
        min_power: None,
        passes: e.passes,
        interval: e.interval,
        crosshatch: false,
        air: e.air,
        frequency_khz: e.frequency_khz,
        source: e.source.clone(),
        confidence: e.confidence,
        warnings: Vec::new(),
    };
    let mut preset = entry.expand_with(&material, &title);
    preset.notes = match e.notes.as_deref() {
        Some(n) => format!("{} · {}", e.machine.trim(), n.trim()),
        None => e.machine.trim().to_string(),
    };
    preset
}

/// Lee una lista JSON de aportes, los comprueba y los convierte. El primero inválido detiene todo.
pub(crate) fn expand_all(text: &str, data: &FactoryData) -> Result<Vec<MaterialPreset>, String> {
    let entries: Vec<CommunityEntry> =
        serde_json::from_str(text).map_err(|e| tr!("doc.community.invalid", error = e))?;
    let mut seen = HashSet::new();
    let mut out = Vec::with_capacity(entries.len());
    for e in &entries {
        check_entry(e, data)?;
        if !seen.insert(e.id.as_str()) {
            return Err(tr!("doc.community.bad_id", id = e.id, issue = "?"));
        }
        out.push(expand(e, data));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::factory::{factory_library, parse_factory};
    use crate::materials::MaterialLibrary;
    use std::fs;
    use std::path::{Path, PathBuf};

    /// Ejemplo válido (inventado para las pruebas; no es un aporte real).
    const SAMPLE: &str = r#"{
        "id": "community.basswood.3.cut.diode_20.7",
        "material": "basswood",
        "thickness": 3,
        "role": "cut",
        "machine_class": "diode_20",
        "laser_watts": 22,
        "machine": "Máquina de prueba 22 W",
        "speed": 500,
        "power": 100,
        "passes": 2,
        "air": true,
        "notes": "Corte limpio; enfoque en la superficie.",
        "source": "https://github.com/jahziel1/tlaku/issues/7",
        "confidence": "forum",
        "license": "CC0-1.0",
        "contributor": "alguien"
    }"#;

    fn data() -> FactoryData {
        serde_json::from_str(include_str!("../data/materials-default.json")).unwrap()
    }

    fn entry(patch: impl FnOnce(&mut serde_json::Value)) -> Result<(), String> {
        let mut v: serde_json::Value = serde_json::from_str(SAMPLE).unwrap();
        patch(&mut v);
        let e: CommunityEntry = serde_json::from_value(v).map_err(|e| e.to_string())?;
        check_entry(&e, &data())
    }

    fn community_dir() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../community/materials")
    }

    #[test]
    fn el_ejemplo_es_valido_y_se_expande_con_confianza_foro() {
        let lib = parse_with_sample(SAMPLE);
        let p = lib.presets.last().unwrap();
        assert_eq!(p.id, "community.basswood.3.cut.diode_20.7");
        assert_eq!(p.confidence, Some(Confidence::Forum));
        assert_eq!(p.machine_class, Some(MachineClass::Diode20));
        assert_eq!(p.material_key.as_deref(), Some("basswood"));
        assert_eq!(p.title_key.as_deref(), Some("cut"));
        assert_eq!((p.settings.speed, p.settings.power), (500.0, 100.0));
        assert_eq!(p.settings.passes, 2);
        assert!(p.settings.air);
        assert!(
            p.notes.starts_with("Máquina de prueba 22 W · "),
            "{}",
            p.notes
        );
        assert_eq!(
            p.source.as_deref(),
            Some("https://github.com/jahziel1/tlaku/issues/7")
        );
    }

    fn parse_with_sample(sample: &str) -> MaterialLibrary {
        let data = include_str!("../data/materials-default.json");
        crate::factory::parse_with_community(data, &format!("[{sample}]")).unwrap()
    }

    #[test]
    fn un_material_desconocido_necesita_su_nombre() {
        assert!(entry(|v| v["material"] = "felt".into()).is_err());
        assert!(
            entry(|v| {
                v["material"] = "felt".into();
                v["material_name"] = "Fieltro".into();
            })
            .is_ok()
        );
    }

    #[test]
    fn se_rechaza_lo_que_incumple_las_reglas() {
        type Patch = Box<dyn Fn(&mut serde_json::Value)>;
        let bad: Vec<(&str, Patch)> = vec![
            (
                "id sin prefijo",
                Box::new(|v| v["id"] = "factory.x.7".into()),
            ),
            (
                "id que no termina en el issue",
                Box::new(|v| v["id"] = "community.basswood.8".into()),
            ),
            (
                "fuente que no es un issue",
                Box::new(|v| v["source"] = "https://example.com/issues/7".into()),
            ),
            (
                "fuente con texto tras el número",
                Box::new(|v| v["source"] = "https://github.com/jahziel1/tlaku/issues/7#x".into()),
            ),
            (
                "licencia distinta",
                Box::new(|v| v["license"] = "MIT".into()),
            ),
            (
                "confianza oficial",
                Box::new(|v| v["confidence"] = "official".into()),
            ),
            ("velocidad cero", Box::new(|v| v["speed"] = 0.into())),
            (
                "velocidad enorme",
                Box::new(|v| v["speed"] = 1_000_000.into()),
            ),
            ("potencia sobre 100", Box::new(|v| v["power"] = 120.into())),
            ("potencia cero", Box::new(|v| v["power"] = 0.into())),
            ("pasadas cero", Box::new(|v| v["passes"] = 0.into())),
            ("intervalo cero", Box::new(|v| v["interval"] = 0.into())),
            (
                "grosor negativo",
                Box::new(|v| v["thickness"] = (-1).into()),
            ),
            (
                "watts que no son de la clase",
                Box::new(|v| v["laser_watts"] = 5.into()),
            ),
            (
                "watts absurdos",
                Box::new(|v| v["laser_watts"] = 5000.into()),
            ),
            ("máquina vacía", Box::new(|v| v["machine"] = "  ".into())),
            (
                "usuario con símbolos",
                Box::new(|v| v["contributor"] = "a/b".into()),
            ),
            (
                "clave con mayúsculas",
                Box::new(|v| v["material"] = "Basswood".into()),
            ),
            (
                "título desconocido",
                Box::new(|v| v["title"] = "nada".into()),
            ),
            ("campo desconocido", Box::new(|v| v["velocidad"] = 1.into())),
        ];
        for (why, patch) in bad {
            assert!(entry(patch).is_err(), "debió rechazarse: {why}");
        }
    }

    #[test]
    fn la_biblioteca_incluida_mantiene_los_ajustes_de_fabrica_primero() {
        let lib = factory_library().expect("datos válidos");
        let plain = parse_factory(include_str!("../data/materials-default.json")).unwrap();
        assert!(lib.presets.len() >= plain.presets.len());
        assert_eq!(lib.presets[..plain.presets.len()], plain.presets[..]);
        assert!(
            lib.presets[plain.presets.len()..]
                .iter()
                .all(|p| p.id.starts_with(COMMUNITY_ID_PREFIX)
                    && p.confidence == Some(Confidence::Forum))
        );
    }

    #[test]
    fn dos_aportes_con_el_mismo_id_se_rechazan() {
        let both = format!("[{SAMPLE},{SAMPLE}]");
        let data = data();
        assert!(expand_all(&both, &data).is_err());
        assert!(expand_all("[]", &data).unwrap().is_empty());
        assert!(expand_all("no es json", &data).is_err());
    }

    /// Lee los archivos reales de `community/materials/` (puede no haber ninguno todavía).
    #[test]
    fn los_archivos_de_la_comunidad_son_validos() {
        let data = data();
        let mut ids = HashSet::new();
        let mut sources = HashSet::new();
        let dir = community_dir();
        assert!(dir.is_dir(), "falta la carpeta {}", dir.display());
        let mut files: Vec<_> = fs::read_dir(&dir)
            .unwrap()
            .filter_map(Result::ok)
            .map(|e| e.path())
            .collect();
        files.sort();
        for path in files {
            let name = path.file_name().unwrap().to_string_lossy().to_string();
            if path.extension().is_none_or(|x| x != "json") {
                // Solo README.md y .gitkeep pueden convivir con los aportes.
                assert!(
                    matches!(name.as_str(), "README.md" | "README.es.md" | ".gitkeep"),
                    "archivo inesperado en community/materials: {name}"
                );
                continue;
            }
            let text = fs::read_to_string(&path).unwrap();
            let e: CommunityEntry = serde_json::from_str(&text)
                .unwrap_or_else(|err| panic!("{name}: JSON inválido: {err}"));
            check_entry(&e, &data).unwrap_or_else(|err| panic!("{name}: {err}"));
            let issue = issue_number(&e.source).unwrap();
            assert_eq!(
                name,
                format!("{issue}.json"),
                "el archivo debe llamarse como el número de su issue"
            );
            assert!(ids.insert(e.id.clone()), "{name}: id repetido {}", e.id);
            assert!(sources.insert(issue), "{name}: issue repetido");
        }
        // Y la lista que se compila coincide con lo que hay en disco.
        let compiled = expand_all(DATA, &data).expect("lo incluido es válido");
        assert_eq!(compiled.len(), ids.len());
    }
}
