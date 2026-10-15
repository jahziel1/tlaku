//! Catálogo publicado de ajustes de material de la comunidad (mejora 5 del plan de usuarios).
//!
//! El flujo de aportes (`community/materials/<n>.json`, ver [`crate::community`]) se compila en CI
//! a un índice JSON versionado que el sitio estático publica en `catalog/v1/index.json`. La app lo
//! baja a pedido (nunca en segundo plano), lo valida con las mismas reglas del aporte y lo guarda en
//! caché para usarlo sin conexión. Este módulo es el contrato de ese índice:
//!
//! - [`CatalogIndex`] / [`CatalogEntry`]: el formato (se exporta a TypeScript).
//! - [`build_index`]: lo arma desde los archivos de aportes (lo usa el ejemplo
//!   `examples/community_catalog.rs` en el flujo del sitio).
//! - [`validate_index`]: lo comprueba **antes de mostrarlo**: tamaño, número de entradas, versión y,
//!   por entrada, las reglas de [`crate::community::check_entry`] (potencia 0 a 100 %, velocidades
//!   positivas y acotadas, clase de máquina contra vatios...) más la coherencia de los campos
//!   calculados (`operation`, `mode`, `verified_by`). Un solo error rechaza todo el índice.
//!
//! El índice es **solo datos**: nada de él se ejecuta ni se envía a la máquina. Usar un ajuste pasa
//! por la biblioteca o la capa del usuario y por la confirmación normal de la interfaz.

use crate::community::{
    COMMUNITY_LICENSE, CommunityEntry, Confirmation, check_entry, is_date, issue_number,
};
use crate::factory::FactoryData;
use crate::layer::LayerMode;
use crate::materials::{Confidence, MachineClass, MaterialRole};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::Path;
use std::sync::OnceLock;
use tlaku_i18n::tr;
use ts_rs::TS;

/// Versión del esquema del índice. Va también en la ruta publicada (`catalog/v1/`): un cambio que
/// rompa a las apps viejas publica un `v2/` aparte y deja el `v1/` como estaba.
pub const CATALOG_VERSION: u32 = 1;
/// Valor de `schema` del índice.
pub const CATALOG_SCHEMA: &str = "tlaku.community-materials";
/// Ruta del índice dentro del sitio (`<sitio>/catalog/v1/index.json`).
pub const CATALOG_PATH: &str = "catalog/v1/index.json";
/// Un índice más grande que esto se rechaza (también al descargarlo, sin bajar el resto).
pub const MAX_INDEX_BYTES: usize = 4 * 1024 * 1024;
/// Más entradas que estas se rechazan.
pub const MAX_ENTRIES: usize = 5000;

/// Qué se hace con el material, para filtrar. Se calcula de `role` y `mode`:
/// imagen y relleno por el modo; si no, por el rol (corte, marcado o grabado en línea).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum CatalogOperation {
    Cut,
    Score,
    Engrave,
    Fill,
    Image,
}

impl CatalogOperation {
    /// La operación que corresponde a un rol y un modo de capa.
    pub fn of(role: MaterialRole, mode: LayerMode) -> Self {
        match (mode, role) {
            (LayerMode::Image, _) => Self::Image,
            (LayerMode::Fill, _) => Self::Fill,
            (LayerMode::Line, MaterialRole::Cut) => Self::Cut,
            (LayerMode::Line, MaterialRole::Score) => Self::Score,
            (LayerMode::Line, MaterialRole::Engrave) => Self::Engrave,
        }
    }
}

/// Modo de capa que se usa si el aporte no dice cuál: línea para cortar y marcar, relleno para grabar.
fn default_mode(role: MaterialRole) -> LayerMode {
    match role {
        MaterialRole::Engrave => LayerMode::Fill,
        MaterialRole::Cut | MaterialRole::Score => LayerMode::Line,
    }
}

/// Un ajuste del catálogo. Los campos ausentes no se escriben.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub struct CatalogEntry {
    /// `community.<material>.<grosor>.<rol>.<clase>.<issue>`; único.
    pub id: String,
    /// Clave del material (`basswood`...). Si no está en la biblioteca de fábrica, `material_name` la nombra.
    pub material: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub material_name: Option<String>,
    /// mm; ausente para grabar la superficie.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub thickness: Option<f64>,
    pub role: MaterialRole,
    /// Clave del título de la operación (`mark_black`...), si no es la del rol.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub title: Option<String>,
    /// Calculada: ver [`CatalogOperation`].
    pub operation: CatalogOperation,
    /// Modo de la capa con el que se probó (siempre presente).
    pub mode: LayerMode,
    pub machine_class: MachineClass,
    /// Potencia óptica real del láser, en W.
    pub laser_watts: f64,
    /// Máquina tal como la escribió quien aportó (declarada, no demostrada).
    pub machine: String,
    /// Modelo del catálogo de máquinas de Tlaku (su `id`), si quien revisó pudo identificarlo.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub machine_model: Option<String>,
    /// mm/min.
    pub speed: f64,
    /// 0 a 100 %.
    pub power: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub passes: Option<u32>,
    /// mm entre líneas del relleno.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub interval: Option<f64>,
    #[serde(default)]
    pub air: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub frequency_khz: Option<f64>,
    /// Resultado y enfoque, texto libre de quien aportó (hasta 400 caracteres).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub notes: Option<String>,
    /// Issue del aporte (ahí están la foto y las confirmaciones).
    pub source: String,
    /// Siempre `CC0-1.0`.
    pub license: String,
    /// Usuario de GitHub de quien aportó.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub contributor: Option<String>,
    /// Día en que se abrió el issue, `AAAA-MM-DD`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub date: Option<String>,
    /// Versión de Tlaku con la que se probó.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub tlaku_version: Option<String>,
    /// Personas distintas que lo confirmaron (`confirmations.len()`). 0 = sin verificar.
    pub verified_by: u32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[ts(optional, as = "Option<Vec<Confirmation>>")]
    pub confirmations: Vec<Confirmation>,
}

impl CatalogEntry {
    /// Entrada del catálogo a partir de un aporte ya comprobado.
    pub fn from_community(e: &CommunityEntry) -> Self {
        let mode = e.mode.unwrap_or_else(|| default_mode(e.role));
        Self {
            id: e.id.clone(),
            material: e.material.clone(),
            material_name: e.material_name.clone(),
            thickness: e.thickness,
            role: e.role,
            title: e.title.clone(),
            operation: CatalogOperation::of(e.role, mode),
            mode,
            machine_class: e.machine_class,
            laser_watts: e.laser_watts,
            machine: e.machine.clone(),
            machine_model: e.machine_model.clone(),
            speed: e.speed,
            power: e.power,
            passes: e.passes,
            interval: e.interval,
            air: e.air,
            frequency_khz: e.frequency_khz,
            notes: e.notes.clone(),
            source: e.source.clone(),
            license: e.license.clone(),
            contributor: e.contributor.clone(),
            date: e.date.clone(),
            tlaku_version: e.tlaku_version.clone(),
            verified_by: e.confirmations.len() as u32,
            confirmations: e.confirmations.clone(),
        }
    }

    /// La misma entrada con la forma del archivo de aporte (para volver a pasar sus reglas).
    fn as_community(&self) -> CommunityEntry {
        CommunityEntry {
            id: self.id.clone(),
            material: self.material.clone(),
            material_name: self.material_name.clone(),
            thickness: self.thickness,
            role: self.role,
            title: self.title.clone(),
            machine_class: self.machine_class,
            laser_watts: self.laser_watts,
            machine: self.machine.clone(),
            mode: Some(self.mode),
            speed: self.speed,
            power: self.power,
            passes: self.passes,
            interval: self.interval,
            air: self.air,
            frequency_khz: self.frequency_khz,
            notes: self.notes.clone(),
            source: self.source.clone(),
            // Los aportes siempre son «foro»; el índice no lo repite.
            confidence: Confidence::Forum,
            license: self.license.clone(),
            contributor: self.contributor.clone(),
            date: self.date.clone(),
            tlaku_version: self.tlaku_version.clone(),
            machine_model: self.machine_model.clone(),
            confirmations: self.confirmations.clone(),
        }
    }
}

/// El índice completo: `catalog/v1/index.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub struct CatalogIndex {
    /// Siempre [`CATALOG_SCHEMA`].
    pub schema: String,
    /// Versión del esquema ([`CATALOG_VERSION`]).
    #[ts(type = "1")]
    pub version: u32,
    /// Día en que se compiló el índice, `AAAA-MM-DD`.
    pub generated_at: String,
    /// Ordenadas por `id`.
    pub entries: Vec<CatalogEntry>,
}

/// De dónde salió el índice que se muestra.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum CatalogSource {
    /// Recién bajado del sitio.
    Network,
    /// La última copia guardada (sin conexión o sin cambios en el sitio).
    Cache,
    /// No hay copia ni se pudo bajar.
    None,
}

/// Lo que devuelven los comandos del catálogo a la interfaz.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub struct CatalogState {
    /// Índice ya validado; `null` si no hay ninguno.
    pub index: Option<CatalogIndex>,
    pub source: CatalogSource,
    /// Cuándo se bajó la copia que se muestra, en ms desde 1970 (la fecha de la caché).
    #[ts(type = "number | null")]
    pub fetched_at: Option<f64>,
    /// Qué falló al bajar o leer (ya traducido), si algo falló. Con caché válida el índice igual se muestra.
    pub error: Option<String>,
}

fn factory_data() -> &'static FactoryData {
    static DATA: OnceLock<FactoryData> = OnceLock::new();
    DATA.get_or_init(|| {
        serde_json::from_str(include_str!("../data/materials-default.json"))
            .expect("los datos de fábrica incluidos son válidos")
    })
}

/// Comprueba una entrada del índice: las reglas del aporte y la coherencia de lo calculado.
fn check_catalog_entry(e: &CatalogEntry) -> Result<(), String> {
    let id = e.id.as_str();
    let as_community = e.as_community();
    check_entry(&as_community, factory_data())?;
    let incoherent = |field: &str| tr!("doc.community.out_of_range", id = id, field = field);
    if e.license != COMMUNITY_LICENSE {
        return Err(incoherent("license"));
    }
    if e.operation != CatalogOperation::of(e.role, e.mode) {
        return Err(incoherent("operation"));
    }
    if e.verified_by as usize != e.confirmations.len() {
        return Err(incoherent("verified_by"));
    }
    Ok(())
}

/// Lee y comprueba un índice. Rechaza el texto entero ante el primer problema: no se muestra nada a medias.
pub fn validate_index(text: &str) -> Result<CatalogIndex, String> {
    if text.len() > MAX_INDEX_BYTES {
        return Err(tr!(
            "doc.catalog.too_big",
            size = text.len(),
            max = MAX_INDEX_BYTES
        ));
    }
    // La versión se mira primero: un índice de otra versión mayor no tiene por qué parecerse a este.
    let value: serde_json::Value =
        serde_json::from_str(text).map_err(|e| tr!("doc.catalog.invalid", error = e))?;
    match value.get("version").and_then(serde_json::Value::as_u64) {
        Some(v) if v == u64::from(CATALOG_VERSION) => {}
        Some(v) => {
            return Err(tr!(
                "doc.catalog.unsupported_version",
                version = v,
                supported = CATALOG_VERSION
            ));
        }
        None => return Err(tr!("doc.catalog.no_version")),
    }
    let index: CatalogIndex =
        serde_json::from_value(value).map_err(|e| tr!("doc.catalog.invalid", error = e))?;
    if index.schema != CATALOG_SCHEMA {
        return Err(tr!("doc.catalog.bad_schema", schema = CATALOG_SCHEMA));
    }
    if !is_date(&index.generated_at) {
        return Err(tr!("doc.catalog.bad_date"));
    }
    if index.entries.len() > MAX_ENTRIES {
        return Err(tr!(
            "doc.catalog.too_many",
            count = index.entries.len(),
            max = MAX_ENTRIES
        ));
    }
    let mut seen = HashSet::new();
    for e in &index.entries {
        check_catalog_entry(e)?;
        if !seen.insert(e.id.as_str()) {
            return Err(tr!("doc.catalog.duplicate", id = e.id));
        }
    }
    Ok(index)
}

/// Arma el índice con los archivos de aportes `(nombre de archivo, texto)`. Cada archivo se llama
/// como el número de su issue y debe pasar las reglas del aporte; no puede haber ids ni issues repetidos.
pub fn build_index(files: &[(String, String)], generated_at: &str) -> Result<CatalogIndex, String> {
    if !is_date(generated_at) {
        return Err(tr!("doc.catalog.bad_date"));
    }
    let mut entries = Vec::with_capacity(files.len());
    let mut issues = HashSet::new();
    for (name, text) in files {
        let entry: CommunityEntry = serde_json::from_str(text)
            .map_err(|e| tr!("doc.catalog.file_invalid", file = name, error = e))?;
        check_entry(&entry, factory_data())?;
        let issue = issue_number(&entry.source).unwrap_or(0);
        if *name != format!("{issue}.json") || !issues.insert(issue) {
            return Err(tr!("doc.catalog.file_name", file = name, issue = issue));
        }
        entries.push(CatalogEntry::from_community(&entry));
    }
    entries.sort_by(|a, b| a.id.cmp(&b.id));
    let index = CatalogIndex {
        schema: CATALOG_SCHEMA.to_string(),
        version: CATALOG_VERSION,
        generated_at: generated_at.to_string(),
        entries,
    };
    // Lo que se publica es exactamente lo que la app aceptará.
    let text = serde_json::to_string(&index).map_err(|e| e.to_string())?;
    validate_index(&text)
}

/// Lee los `*.json` de `dir` (los aportes) y arma el índice.
pub fn build_index_from_dir(dir: &Path, generated_at: &str) -> Result<CatalogIndex, String> {
    let mut paths: Vec<_> = std::fs::read_dir(dir)
        .map_err(|e| format!("{}: {e}", dir.display()))?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .collect();
    paths.sort();
    let mut files = Vec::with_capacity(paths.len());
    for path in paths {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let text =
            std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        files.push((name, text));
    }
    build_index(&files, generated_at)
}

#[cfg(test)]
mod tests;
