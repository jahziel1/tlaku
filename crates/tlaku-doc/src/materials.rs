//! Biblioteca de materiales y prueba de materiales.

use crate::layer::{LayerMode, LayerSettings};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Versión del archivo de biblioteca (`.tlakumat`, JSON).
pub const MATERIAL_LIBRARY_VERSION: u32 = 1;

/// Para qué sirve un ajuste. Lo usa el modo sencillo para elegir "cortar", "grabar" o "marcar".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum MaterialRole {
    /// Atravesar el material (capa de líneas).
    Cut,
    /// Rellenar o grabar la superficie.
    Engrave,
    /// Marcar una línea sin atravesar (rayado, plegado).
    Score,
}

/// Tipo de fuente láser.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum LaserKind {
    Diode,
    Co2,
    Fiber,
}

/// Clase de máquina: tipo de láser y potencia nominal. Un ajuste de la biblioteca puede valer
/// para toda una clase ("diodo de 10 W") y no solo para un perfil concreto.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum MachineClass {
    #[serde(rename = "diode_5")]
    Diode5,
    #[serde(rename = "diode_10")]
    Diode10,
    #[serde(rename = "diode_20")]
    Diode20,
    #[serde(rename = "diode_40")]
    Diode40,
    #[serde(rename = "co2_40")]
    Co2_40,
    #[serde(rename = "co2_60")]
    Co2_60,
    #[serde(rename = "co2_80")]
    Co2_80,
    #[serde(rename = "co2_100")]
    Co2_100,
    #[serde(rename = "fiber_20")]
    Fiber20,
    #[serde(rename = "fiber_30")]
    Fiber30,
    #[serde(rename = "fiber_50")]
    Fiber50,
}

impl MachineClass {
    /// Todas las clases, de menor a mayor potencia dentro de cada tipo de láser.
    pub const ALL: [MachineClass; 11] = [
        MachineClass::Diode5,
        MachineClass::Diode10,
        MachineClass::Diode20,
        MachineClass::Diode40,
        MachineClass::Co2_40,
        MachineClass::Co2_60,
        MachineClass::Co2_80,
        MachineClass::Co2_100,
        MachineClass::Fiber20,
        MachineClass::Fiber30,
        MachineClass::Fiber50,
    ];

    pub fn kind(self) -> LaserKind {
        match self {
            MachineClass::Diode5
            | MachineClass::Diode10
            | MachineClass::Diode20
            | MachineClass::Diode40 => LaserKind::Diode,
            MachineClass::Co2_40
            | MachineClass::Co2_60
            | MachineClass::Co2_80
            | MachineClass::Co2_100 => LaserKind::Co2,
            MachineClass::Fiber20 | MachineClass::Fiber30 | MachineClass::Fiber50 => {
                LaserKind::Fiber
            }
        }
    }

    /// Potencia nominal de la clase, en W.
    pub fn watts(self) -> f64 {
        match self {
            MachineClass::Diode5 => 5.0,
            MachineClass::Diode10 => 10.0,
            MachineClass::Diode20 | MachineClass::Fiber20 => 20.0,
            MachineClass::Fiber30 => 30.0,
            MachineClass::Diode40 | MachineClass::Co2_40 => 40.0,
            MachineClass::Fiber50 => 50.0,
            MachineClass::Co2_60 => 60.0,
            MachineClass::Co2_80 => 80.0,
            MachineClass::Co2_100 => 100.0,
        }
    }

    /// La clase de `kind` con la potencia más cercana a `watts`; en un empate, la de menos
    /// potencia (sus ajustes son los más prudentes).
    pub fn nearest(kind: LaserKind, watts: f64) -> MachineClass {
        let mut best = None::<MachineClass>;
        for c in Self::ALL.into_iter().filter(|c| c.kind() == kind) {
            let better = match best {
                None => true,
                Some(b) => (c.watts() - watts).abs() < (b.watts() - watts).abs(),
            };
            if better {
                best = Some(c);
            }
        }
        best.expect("cada tipo de láser tiene al menos una clase")
    }
}

/// Qué tan confiable es la fuente de un ajuste.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum Confidence {
    /// Tabla publicada por el fabricante.
    Official,
    /// Guía o artículo publicado (de un fabricante en general o de un sitio especializado).
    Guide,
    /// Mensaje de un foro o de la comunidad, sin verificar.
    Forum,
}

/// Avisos que acompañan a un ajuste (el texto vive en la interfaz, en cada idioma).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum MaterialWarning {
    /// El diodo azul atraviesa el material sin cortarlo (acrílico transparente).
    NotCutWithDiode,
    /// No procesar cuero curtido al cromo (gases peligrosos).
    ChromeLeather,
    /// La marca solo se ve con un recubrimiento (spray de marcado, cinta oscura).
    NeedsCoating,
    /// La fuente da un rango y aquí está un extremo de él: prueba antes de usarlo.
    RangeEdge,
    /// La fuente no distingue color, especie o marca del material.
    GenericRow,
}

/// Ajustes probados para un material y una operación.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub struct MaterialPreset {
    /// Único en la biblioteca.
    pub id: String,
    /// Por ejemplo "MDF", "Contrachapado de abedul", "Acrílico negro".
    pub material: String,
    /// Grosor en mm; `null` = no aplica (grabados).
    pub thickness: Option<f64>,
    /// Operación, por ejemplo "Corte", "Grabado ligero".
    pub title: String,
    pub notes: String,
    /// Máquina para la que se probó; `null` = cualquiera.
    pub device_id: Option<String>,
    /// Al aplicarse a una capa se copian todos los campos menos `color`, `name`, `output`,
    /// `visible` y `order`.
    pub settings: LayerSettings,
    /// Para qué sirve; ausente en bibliotecas viejas (se deduce del modo, ver
    /// [`MaterialPreset::effective_role`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub role: Option<MaterialRole>,
    /// Clase de máquina para la que vale; ausente = no se dice. Con `device_id` también `null`,
    /// el ajuste es genérico.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub machine_class: Option<MachineClass>,
    /// Dirección de donde salen los valores; ausente en ajustes propios del usuario.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub source: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub confidence: Option<Confidence>,
    /// Identificador estable del material en la biblioteca de fábrica (para traducir su nombre);
    /// ausente en ajustes del usuario.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub material_key: Option<String>,
    /// Lo mismo para el título de la operación ("cut", "engrave", "mark_black"...).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub title_key: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[ts(optional, as = "Option<Vec<MaterialWarning>>")]
    pub warnings: Vec<MaterialWarning>,
}

impl MaterialPreset {
    /// El rol declarado o, si falta, el que sugiere el modo de la capa (línea = corte, relleno
    /// o imagen = grabado).
    pub fn effective_role(&self) -> MaterialRole {
        self.role.unwrap_or(match self.settings.mode {
            LayerMode::Line => MaterialRole::Cut,
            LayerMode::Fill | LayerMode::Image => MaterialRole::Engrave,
        })
    }
}

/// Algo que una clase de láser no puede hacer con un material: no hay ajuste que ofrecer y la
/// interfaz debe explicarlo.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub struct MaterialRestriction {
    /// `material_key` (o nombre) del material afectado.
    pub material: String,
    pub laser: LaserKind,
    /// `null` = cualquier operación.
    pub role: Option<MaterialRole>,
    pub warning: MaterialWarning,
    pub source: String,
    pub confidence: Confidence,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub struct MaterialLibrary {
    #[ts(type = "1")]
    pub version: u32,
    pub presets: Vec<MaterialPreset>,
    /// Revisión del contenido; solo la lleva la biblioteca de fábrica (la app la usa para saber
    /// si la copia instalada está al día). 0 en las del usuario.
    #[serde(default, skip_serializing_if = "is_zero")]
    #[ts(optional, as = "Option<u32>")]
    pub revision: u32,
    /// Lo que ciertas clases de láser no pueden hacer con ciertos materiales.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[ts(optional, as = "Option<Vec<MaterialRestriction>>")]
    pub restrictions: Vec<MaterialRestriction>,
}

fn is_zero(v: &u32) -> bool {
    *v == 0
}

impl Default for MaterialLibrary {
    fn default() -> Self {
        Self {
            version: MATERIAL_LIBRARY_VERSION,
            presets: Vec::new(),
            revision: 0,
            restrictions: Vec::new(),
        }
    }
}

/// Parámetro que varía a lo largo de filas o columnas en la prueba de materiales.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum TestParam {
    /// mm/min.
    Speed,
    /// %.
    Power,
    Passes,
    /// mm (solo fill e image).
    Interval,
}

/// Eje de la cuadrícula de prueba: qué parámetro varía y entre qué valores (inclusive).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub struct TestAxis {
    pub param: TestParam,
    pub count: u32,
    pub from: f64,
    pub to: f64,
}

/// Cuadrícula de prueba: cada celda es una capa con sus propios ajustes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub struct MaterialTestParams {
    /// Line (cuadros) o fill (cuadros rellenos).
    pub mode: LayerMode,
    /// Varía de una fila a otra (de arriba hacia abajo).
    pub rows: TestAxis,
    /// Varía de una columna a otra (de izquierda a derecha).
    pub cols: TestAxis,
    /// Ancho y alto de cada celda en mm.
    pub cell: [f64; 2],
    /// Separación entre celdas en mm.
    pub gap: f64,
    /// Ajustes base de todas las celdas (lo que no varía).
    pub base: LayerSettings,
    /// Rotular valores de filas y columnas con trazo sencillo.
    pub labels: bool,
    /// Velocidad (mm/min) y potencia (%) de los rótulos.
    pub label_speed: f64,
    pub label_power: f64,
    /// Esquina inferior izquierda de la cuadrícula (con rótulos), en mm del documento.
    pub origin: [f64; 2],
}
