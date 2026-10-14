//! Biblioteca de materiales de fábrica: valores de partida con su fuente y su confianza.
//!
//! Los datos viven en `data/materials-default.json`, en un formato compacto (un renglón por
//! ajuste) y se incluyen en la app al compilar. [`factory_library`] los expande a
//! [`MaterialLibrary`], el mismo formato que la biblioteca del usuario. La biblioteca de fábrica
//! es de solo lectura: la interfaz la muestra aparte y, para editar un ajuste, se duplica a la
//! biblioteca del usuario.
//!
//! Reglas de los datos:
//! - Cada ajuste lleva la dirección de donde salen sus valores y su nivel de confianza.
//! - Las velocidades están en mm/min (las fuentes en mm/s se multiplicaron por 60).
//! - Donde la fuente da un rango, el ajuste toma un extremo y lleva el aviso `range_edge`: el de
//!   menos energía en un grabado y el de más energía en un corte.
//! - Donde no hay dato, no hay ajuste.

use crate::layer::{LayerMode, LayerSettings};
use crate::materials::{
    Confidence, MATERIAL_LIBRARY_VERSION, MachineClass, MaterialLibrary, MaterialPreset,
    MaterialRestriction, MaterialRole, MaterialWarning,
};
use serde::Deserialize;
use std::collections::{BTreeMap, HashSet};
use tlaku_core::Rgb;
use tlaku_i18n::tr;

/// Datos de fábrica incluidos en el programa.
const DATA: &str = include_str!("../data/materials-default.json");

/// Prefijo de los identificadores de la biblioteca de fábrica. La del usuario no los usa.
pub const FACTORY_ID_PREFIX: &str = "factory.";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct FactoryData {
    /// Sube cada vez que cambian los datos; la app reinstala la copia si no coincide.
    revision: u32,
    /// Nombre en español de cada material (la interfaz lo traduce con la clave).
    pub(crate) materials: BTreeMap<String, String>,
    /// Nombre en español de cada operación.
    pub(crate) titles: BTreeMap<String, String>,
    #[serde(default)]
    restrictions: Vec<MaterialRestriction>,
    presets: Vec<FactoryEntry>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct FactoryEntry {
    pub(crate) id: String,
    pub(crate) material: String,
    pub(crate) thickness: Option<f64>,
    pub(crate) role: MaterialRole,
    pub(crate) title: String,
    pub(crate) machine_class: MachineClass,
    /// Si falta: línea para cortar y marcar, relleno para grabar.
    pub(crate) mode: Option<LayerMode>,
    /// mm/min.
    pub(crate) speed: f64,
    pub(crate) power: f64,
    pub(crate) min_power: Option<f64>,
    pub(crate) passes: Option<u32>,
    /// mm entre líneas del relleno; si falta se queda el valor normal de la capa.
    pub(crate) interval: Option<f64>,
    #[serde(default)]
    pub(crate) crosshatch: bool,
    #[serde(default)]
    pub(crate) air: bool,
    /// Solo fibra y otras fuentes de galvo.
    pub(crate) frequency_khz: Option<f64>,
    pub(crate) source: String,
    pub(crate) confidence: Confidence,
    #[serde(default)]
    pub(crate) warnings: Vec<MaterialWarning>,
}

impl FactoryEntry {
    fn expand(&self, data: &FactoryData) -> Result<MaterialPreset, String> {
        let material = data.materials.get(&self.material).ok_or_else(|| {
            tr!(
                "doc.factory.material_unnamed",
                id = self.id,
                material = self.material
            )
        })?;
        let title = data.titles.get(&self.title).ok_or_else(|| {
            tr!(
                "doc.factory.title_unnamed",
                id = self.id,
                title = self.title
            )
        })?;
        Ok(self.expand_with(material, title))
    }

    /// Expande el ajuste con los nombres ya resueltos (los datos de fábrica los sacan de sus tablas;
    /// los de la comunidad, de las mismas o del texto que escribió quien los aportó).
    pub(crate) fn expand_with(&self, material: &str, title: &str) -> MaterialPreset {
        let material = &material.to_string();
        let title = &title.to_string();
        let mut settings = LayerSettings::for_color(Rgb(0, 0, 0));
        settings.name = title.clone();
        settings.mode = self.mode.unwrap_or(match self.role {
            MaterialRole::Engrave => LayerMode::Fill,
            MaterialRole::Cut | MaterialRole::Score => LayerMode::Line,
        });
        settings.speed = self.speed;
        settings.power = self.power;
        settings.min_power = self.min_power.unwrap_or(0.0);
        settings.passes = self.passes.unwrap_or(1);
        settings.air = self.air;
        settings.crosshatch = self.crosshatch;
        if let Some(interval) = self.interval {
            settings.interval = interval;
        }
        if let Some(khz) = self.frequency_khz {
            settings.galvo.frequency_khz = khz;
        }
        MaterialPreset {
            id: self.id.clone(),
            material: material.clone(),
            thickness: self.thickness,
            title: title.clone(),
            notes: String::new(),
            device_id: None,
            settings,
            role: Some(self.role),
            machine_class: Some(self.machine_class),
            source: Some(self.source.clone()),
            confidence: Some(self.confidence),
            material_key: Some(self.material.clone()),
            title_key: Some(self.title.clone()),
            warnings: self.warnings.clone(),
        }
    }
}

/// Lee y valida datos de fábrica en el formato compacto (sin los aportes de la comunidad).
pub fn parse_factory(text: &str) -> Result<MaterialLibrary, String> {
    parse_with_community(text, "[]")
}

/// Datos de fábrica más los aportes de la comunidad (`community`: una lista JSON de ajustes, ver
/// [`crate::community`]). Los de la comunidad van después, validados con sus propias reglas.
pub(crate) fn parse_with_community(text: &str, community: &str) -> Result<MaterialLibrary, String> {
    let data: FactoryData =
        serde_json::from_str(text).map_err(|e| tr!("doc.factory.invalid", error = e))?;
    let presets = data
        .presets
        .iter()
        .map(|e| e.expand(&data))
        .collect::<Result<Vec<_>, _>>()?;
    let mut library = MaterialLibrary {
        version: MATERIAL_LIBRARY_VERSION,
        presets,
        revision: data.revision,
        restrictions: data.restrictions.clone(),
    };
    check_factory(&library)?;
    library
        .presets
        .extend(crate::community::expand_all(community, &data)?);
    Ok(library)
}

/// La biblioteca de fábrica incluida en esta versión de la app: los ajustes de fábrica y, al final,
/// los de la comunidad (`community/materials/*.json` del repositorio, incluidos al compilar).
pub fn factory_library() -> Result<MaterialLibrary, String> {
    parse_with_community(DATA, crate::community::DATA)
}

fn is_url(text: &str) -> bool {
    text.strip_prefix("https://")
        .is_some_and(|rest| rest.contains('.') && !rest.contains(char::is_whitespace))
}

/// Comprueba lo que se exige a todo ajuste de fábrica: identificador propio y único, valores
/// razonables, y **fuente (dirección) y confianza**.
pub fn check_factory(library: &MaterialLibrary) -> Result<(), String> {
    if library.revision == 0 {
        return Err(tr!("doc.factory.no_revision"));
    }
    let mut seen = HashSet::new();
    for p in &library.presets {
        let id = p.id.as_str();
        if !id.starts_with(FACTORY_ID_PREFIX) || !seen.insert(id) {
            return Err(tr!("doc.factory.bad_id", id = id));
        }
        if !p.source.as_deref().is_some_and(is_url) {
            return Err(tr!("doc.factory.no_source", id = id));
        }
        if p.confidence.is_none() {
            return Err(tr!("doc.factory.no_confidence", id = id));
        }
        if p.machine_class.is_none() || p.role.is_none() {
            return Err(tr!("doc.factory.no_class", id = id));
        }
        if p.material_key.is_none() || p.title_key.is_none() {
            return Err(tr!("doc.factory.no_keys", id = id));
        }
        let s = &p.settings;
        let ok = s.speed.is_finite()
            && s.speed > 0.0
            && (0.0..=100.0).contains(&s.power)
            && (0.0..=s.power.max(0.0)).contains(&s.min_power)
            && s.passes >= 1
            && s.interval > 0.0
            && p.thickness.is_none_or(|t| t.is_finite() && t > 0.0);
        if !ok {
            return Err(tr!("doc.factory.out_of_range", id = id));
        }
    }
    for r in &library.restrictions {
        if !is_url(&r.source) {
            return Err(tr!(
                "doc.factory.restriction_no_source",
                material = r.material
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::materials::LaserKind;

    fn library() -> MaterialLibrary {
        factory_library().expect("los datos incluidos son válidos")
    }

    #[test]
    fn los_datos_incluidos_son_validos_y_no_estan_vacios() {
        let lib = library();
        assert!(lib.presets.len() > 50, "{}", lib.presets.len());
        assert!(lib.revision >= 1);
    }

    #[test]
    fn todo_ajuste_incluido_tiene_fuente_y_confianza() {
        for p in &library().presets {
            let src = p.source.as_deref().unwrap_or_default();
            assert!(src.starts_with("https://"), "{}: {src}", p.id);
            assert!(p.confidence.is_some(), "{}", p.id);
            assert!(p.machine_class.is_some(), "{}", p.id);
        }
    }

    #[test]
    fn un_ajuste_sin_fuente_se_rechaza() {
        let mut lib = library();
        lib.presets[0].source = None;
        assert!(check_factory(&lib).unwrap_err().contains("fuente"));
        let mut lib = library();
        lib.presets[0].source = Some("sin dirección".into());
        assert!(check_factory(&lib).unwrap_err().contains("fuente"));
        let mut lib = library();
        lib.presets[0].confidence = None;
        assert!(check_factory(&lib).unwrap_err().contains("confianza"));
        let mut lib = library();
        lib.presets[1].id = lib.presets[0].id.clone();
        assert!(check_factory(&lib).unwrap_err().contains("repetido"));
    }

    #[test]
    fn los_datos_compactos_con_un_campo_de_mas_se_rechazan() {
        let bad = DATA.replacen("\"speed\"", "\"velocidad\"", 1);
        assert!(parse_factory(&bad).unwrap_err().contains("inválida"));
    }

    #[test]
    fn el_acrilico_transparente_no_tiene_ajustes_de_diodo() {
        let lib = library();
        assert!(
            lib.presets
                .iter()
                .filter(|p| p.material_key.as_deref() == Some("acrylic_clear"))
                .all(|p| p.machine_class.unwrap().kind() != LaserKind::Diode)
        );
        assert!(
            lib.restrictions
                .iter()
                .any(|r| r.material == "acrylic_clear"
                    && r.laser == LaserKind::Diode
                    && r.warning == MaterialWarning::NotCutWithDiode)
        );
    }

    #[test]
    fn los_cueros_avisan_del_cromo_y_los_rangos_se_marcan() {
        let lib = library();
        for p in lib
            .presets
            .iter()
            .filter(|p| p.material_key.as_deref() == Some("leather"))
        {
            assert!(
                p.warnings.contains(&MaterialWarning::ChromeLeather),
                "{}",
                p.id
            );
        }
        for p in lib.presets.iter().filter(|p| {
            p.source
                .as_deref()
                .is_some_and(|s| s.contains("zapcraft.net"))
        }) {
            assert!(p.warnings.contains(&MaterialWarning::RangeEdge), "{}", p.id);
            assert_eq!(p.confidence, Some(Confidence::Guide));
        }
    }

    #[test]
    fn la_expansion_pone_modo_velocidad_y_frecuencia() {
        let lib = library();
        let get = |id: &str| lib.presets.iter().find(|p| p.id == id).unwrap();
        // Creality, MDF no: tilo de 3 mm en el diodo de 20 W (A1 Pro): 100 %, 500 mm/min.
        let cut = get("factory.basswood.3.cut.diode_20");
        assert_eq!(cut.settings.mode, LayerMode::Line);
        assert_eq!((cut.settings.speed, cut.settings.power), (500.0, 100.0));
        assert!(cut.settings.air);
        assert_eq!(cut.effective_role(), MaterialRole::Cut);
        assert_eq!(cut.confidence, Some(Confidence::Official));
        // Thunder, CO2 de 60 W: 1200 mm/s = 72 000 mm/min, 300 DPI.
        let eng = get("factory.mdf.x.engrave.co2_60");
        assert_eq!(eng.settings.mode, LayerMode::Fill);
        assert_eq!(eng.settings.speed, 72_000.0);
        assert!((eng.settings.interval - 25.4 / 300.0).abs() < 1e-3);
        // Fibra: frecuencia y relleno cruzado.
        let white = get("factory.stainless_steel.x.mark_white.fiber_20");
        assert_eq!(white.settings.galvo.frequency_khz, 35.0);
        assert!(white.settings.crosshatch);
    }

    #[test]
    fn las_clases_se_leen_y_se_escriben_con_nombres_estables() {
        for (c, name) in [
            (MachineClass::Diode10, "diode_10"),
            (MachineClass::Co2_60, "co2_60"),
            (MachineClass::Fiber50, "fiber_50"),
        ] {
            let json = serde_json::to_string(&c).unwrap();
            assert_eq!(json, format!("\"{name}\""));
            assert_eq!(serde_json::from_str::<MachineClass>(&json).unwrap(), c);
        }
        assert_eq!(MachineClass::ALL.len(), 11);
    }

    #[test]
    fn la_clase_mas_cercana_respeta_el_tipo_de_laser() {
        assert_eq!(
            MachineClass::nearest(LaserKind::Diode, 22.0),
            MachineClass::Diode20
        );
        assert_eq!(
            MachineClass::nearest(LaserKind::Diode, 12.0),
            MachineClass::Diode10
        );
        assert_eq!(
            MachineClass::nearest(LaserKind::Diode, 100.0),
            MachineClass::Diode40
        );
        // Empate: gana la de menos potencia.
        assert_eq!(
            MachineClass::nearest(LaserKind::Diode, 15.0),
            MachineClass::Diode10
        );
        assert_eq!(
            MachineClass::nearest(LaserKind::Co2, 50.0),
            MachineClass::Co2_40
        );
        assert_eq!(
            MachineClass::nearest(LaserKind::Co2, 130.0),
            MachineClass::Co2_100
        );
        assert_eq!(
            MachineClass::nearest(LaserKind::Fiber, 60.0),
            MachineClass::Fiber50
        );
    }

    #[test]
    fn un_ajuste_guardado_antes_de_los_campos_nuevos_se_sigue_leyendo() {
        let old = r##"{"version":1,"presets":[{"id":"a","material":"MDF","thickness":3,
            "title":"Corte","notes":"","device_id":null,"settings":
            {"color":"#000000","name":"Corte","mode":"line","output":true,"visible":true,
             "speed":300,"power":100,"min_power":0,"passes":2,"air":true,"interval":0.1,
             "angle":0,"crosshatch":false,"bidirectional":true,"overscan":"auto",
             "kerf":0,"image":{"mode":"stucki","threshold":0.5,"brightness":0,"contrast":0,
             "gamma":1,"invert":false,"dot_width":0.1},"order":null}}]}"##;
        let lib: MaterialLibrary = serde_json::from_str(old).unwrap();
        let p = &lib.presets[0];
        assert_eq!(
            (p.role, p.machine_class, p.source.as_deref()),
            (None, None, None)
        );
        assert!(p.warnings.is_empty());
        assert_eq!(p.effective_role(), MaterialRole::Cut);
        assert_eq!(lib.revision, 0);
        // Al volver a escribirla no aparecen los campos nuevos.
        let back = serde_json::to_value(&lib).unwrap();
        assert!(back["presets"][0].get("source").is_none());
        assert!(back.get("revision").is_none());
    }
}
