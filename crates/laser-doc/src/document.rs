use crate::affine::{self, Transform};
use crate::layer::LayerSettings;
use laser_core::{Bounds, Point, Polyline, Rgb};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Versión del formato del documento (y del archivo `.lsproj`).
pub const FORMAT_VERSION: u32 = 1;

/// Trayectoria en coordenadas locales del objeto.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub struct PathData {
    pub points: Vec<[f64; 2]>,
    pub closed: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub struct VectorObject {
    pub paths: Vec<PathData>,
}

/// Imagen enlazada por ruta. Su caja local es `[0, size_mm[0]] x [0, size_mm[1]]` (Y hacia
/// arriba); el `transform` del objeto la coloca y la escala.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub struct ImageObject {
    pub source_path: String,
    pub width_px: u32,
    pub height_px: u32,
    pub size_mm: [f64; 2],
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
#[ts(export)]
pub enum ObjectKind {
    Vector(VectorObject),
    Image(ImageObject),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub struct DesignObject {
    /// Único en el documento.
    pub id: String,
    /// Color de la capa a la que pertenece.
    #[ts(type = "string")]
    pub layer: Rgb,
    /// Afín `[a b c d e f]`: `x' = a·x + c·y + e`, `y' = b·x + d·y + f`.
    pub transform: Transform,
    pub kind: ObjectKind,
    pub locked: bool,
    pub hidden: bool,
}

fn to_point(p: [f64; 2]) -> Point {
    Point::new(p[0], p[1])
}

impl DesignObject {
    /// Geometría en coordenadas de máquina: los trazos de un objeto vectorial con el
    /// `transform` aplicado, o el rectángulo (cerrado) que ocupa una imagen.
    pub fn polylines(&self) -> Vec<Polyline> {
        let place = |p: [f64; 2]| to_point(affine::apply(&self.transform, p));
        match &self.kind {
            ObjectKind::Vector(v) => v
                .paths
                .iter()
                .map(|path| Polyline {
                    points: path.points.iter().map(|&p| place(p)).collect(),
                    closed: path.closed,
                })
                .collect(),
            ObjectKind::Image(img) => {
                let [w, h] = img.size_mm;
                vec![Polyline {
                    points: [[0.0, 0.0], [w, 0.0], [w, h], [0.0, h]]
                        .into_iter()
                        .map(place)
                        .collect(),
                    closed: true,
                }]
            }
        }
    }

    /// Caja envolvente en coordenadas de máquina; `None` si el objeto no tiene puntos.
    pub fn bounds(&self) -> Option<Bounds> {
        bounds_of(&self.polylines())
    }
}

/// Caja envolvente de un conjunto de polilíneas.
pub fn bounds_of(paths: &[Polyline]) -> Option<Bounds> {
    let mut points = paths.iter().flat_map(|p| &p.points);
    let first = *points.next()?;
    let mut b = Bounds {
        min: first,
        max: first,
    };
    for p in points {
        b.min.x = b.min.x.min(p.x);
        b.min.y = b.min.y.min(p.y);
        b.max.x = b.max.x.max(p.x);
        b.max.y = b.max.y.max(p.y);
    }
    Some(b)
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub struct Document {
    #[ts(type = "1")]
    pub version: u32,
    pub objects: Vec<DesignObject>,
    /// Orden del archivo: el que usa `layer_order = "file"`.
    pub layers: Vec<LayerSettings>,
    pub device_id: Option<String>,
}

impl Default for Document {
    fn default() -> Self {
        Self {
            version: FORMAT_VERSION,
            objects: Vec::new(),
            layers: Vec::new(),
            device_id: None,
        }
    }
}

impl Document {
    pub fn to_json(&self) -> Result<String, String> {
        serde_json::to_string_pretty(self).map_err(|e| format!("no se pudo serializar: {e}"))
    }

    /// Lee un documento; rechaza versiones que esta compilación no conoce.
    pub fn from_json(text: &str) -> Result<Self, String> {
        let value: serde_json::Value =
            serde_json::from_str(text).map_err(|e| format!("documento inválido: {e}"))?;
        match value.get("version") {
            None => return Err("documento inválido: falta el campo \"version\"".into()),
            Some(v) if v.as_u64() == Some(u64::from(FORMAT_VERSION)) => {}
            Some(v) => {
                return Err(format!(
                    "versión de documento no soportada: {v} (esta aplicación entiende la {FORMAT_VERSION}); \
                     actualiza la aplicación para abrirlo"
                ));
            }
        }
        serde_json::from_value(value).map_err(|e| format!("documento inválido: {e}"))
    }

    pub fn object(&self, id: &str) -> Option<&DesignObject> {
        self.objects.iter().find(|o| o.id == id)
    }

    pub fn layer(&self, color: Rgb) -> Option<&LayerSettings> {
        self.layers.iter().find(|l| l.color == color)
    }

    /// Caja envolvente de todos los objetos, en coordenadas de máquina.
    pub fn bounds(&self) -> Option<Bounds> {
        self.objects
            .iter()
            .filter_map(DesignObject::bounds)
            .reduce(union)
    }
}

pub fn union(a: Bounds, b: Bounds) -> Bounds {
    Bounds {
        min: Point::new(a.min.x.min(b.min.x), a.min.y.min(b.min.y)),
        max: Point::new(a.max.x.max(b.max.x), a.max.y.max(b.max.y)),
    }
}
