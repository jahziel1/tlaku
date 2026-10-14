use crate::affine::{self, Transform};
use crate::cut::{Guide, ShapeParams, StartSpec, VariableText, VirtualArray};
use crate::layer::LayerSettings;
use crate::text::TextObject;
use serde::{Deserialize, Serialize};
use tlaku_core::bezpath::BezPath;
use tlaku_core::{Bounds, Point, Polyline, Rgb};
use tlaku_i18n::tr;
use ts_rs::TS;

/// Versión del formato del documento (y del archivo `.tlaku`). La 1 guardaba polilíneas
/// (`points`/`closed`); la 2 guarda curvas Bézier, grupos y objetos de texto.
pub const DOC_VERSION: u32 = 2;

/// Error máximo (mm) al aplanar curvas para el trabajo (`polylines`, colocación y G-code).
/// 0.01 mm es bastante menos que el ancho del haz de un láser de diodo (~0.1 mm).
pub const FLATTEN_TOLERANCE_MM: f64 = 0.01;

/// Curvas en coordenadas locales del objeto.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub struct VectorObject {
    pub paths: Vec<BezPath>,
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
    /// Máscara: solo se graba lo que queda dentro de estas curvas (coordenadas locales de la
    /// imagen, mm, Y hacia arriba); vacío = la imagen completa.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[ts(as = "Option<Vec<BezPath>>", optional)]
    pub mask: Vec<BezPath>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
#[ts(export)]
pub enum ObjectKind {
    Vector(VectorObject),
    Image(ImageObject),
    Text(TextObject),
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
    /// Grupo plano al que pertenece (los objetos con el mismo id se seleccionan juntos).
    pub group: Option<String>,
    /// Prioridad en el orden de corte dentro de su capa (menor = antes); `None` = automática.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub priority: Option<i32>,
    /// Escala de potencia del objeto, % de la potencia de su capa; `None` = 100 %.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub power_scale: Option<f64>,
    /// Parámetros de la forma si se dibujó como forma paramétrica.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub shape: Option<ShapeParams>,
    /// Punto de inicio y sentido elegidos a mano.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub start: Option<StartSpec>,
    /// Puentes puestos a mano: puntos del documento sobre el contorno (se usa el más cercano).
    /// Si hay alguno, sustituyen a los automáticos de la capa para este objeto.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[ts(as = "Option<Vec<[f64; 2]>>", optional)]
    pub manual_tabs: Vec<[f64; 2]>,
    /// Copias virtuales del objeto.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub array: Option<VirtualArray>,
}

fn to_point(p: [f64; 2]) -> Point {
    Point::new(p[0], p[1])
}

impl DesignObject {
    /// Curvas del objeto en coordenadas de máquina (con el `transform` aplicado): las del
    /// vector, el contorno generado de un texto (vacío si aún no se generó) o el rectángulo
    /// cerrado que ocupa una imagen.
    pub fn curves(&self) -> Vec<BezPath> {
        let place = |p: Point| to_point(affine::apply(&self.transform, [p.x, p.y]));
        let placed = |paths: &[BezPath]| paths.iter().map(|b| b.map_points(place)).collect();
        match &self.kind {
            ObjectKind::Vector(v) => placed(&v.paths),
            ObjectKind::Text(t) => placed(&t.outline),
            ObjectKind::Image(img) => {
                let [w, h] = img.size_mm;
                let rect = Polyline {
                    points: [[0.0, 0.0], [w, 0.0], [w, h], [0.0, h]]
                        .into_iter()
                        .map(|p| place(to_point(p)))
                        .collect(),
                    closed: true,
                };
                vec![BezPath::from(&rect)]
            }
        }
    }

    /// Geometría en coordenadas de máquina aplanada con [`FLATTEN_TOLERANCE_MM`].
    pub fn polylines(&self) -> Vec<Polyline> {
        self.polylines_with(FLATTEN_TOLERANCE_MM)
    }

    /// Como [`polylines`](Self::polylines) con una tolerancia (mm) a elección. El aplanado se
    /// hace después del `transform`, así que la tolerancia vale en mm reales aunque el objeto
    /// esté escalado.
    pub fn polylines_with(&self, tolerance_mm: f64) -> Vec<Polyline> {
        self.curves()
            .iter()
            .flat_map(|c| c.flatten(tolerance_mm))
            .collect()
    }

    /// Caja envolvente en coordenadas de máquina; `None` si el objeto no dibuja nada.
    ///
    /// Es **exacta** para curvas: se calculan los extremos de cada Bézier ya transformada
    /// (una afín de una Bézier es otra Bézier), no la caja de los puntos de control ni la de
    /// una versión aplanada.
    pub fn bounds(&self) -> Option<Bounds> {
        self.curves()
            .iter()
            .filter_map(BezPath::bounds)
            .reduce(union)
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
    #[ts(type = "2")]
    pub version: u32,
    pub objects: Vec<DesignObject>,
    /// Orden del archivo: el que usa `layer_order = "file"`.
    pub layers: Vec<LayerSettings>,
    pub device_id: Option<String>,
    /// Notas del proyecto (texto libre; no se corta).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    #[ts(as = "Option<String>", optional)]
    pub notes: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[ts(as = "Option<Vec<Guide>>", optional)]
    pub guides: Vec<Guide>,
    #[serde(default)]
    #[ts(as = "Option<VariableText>", optional)]
    pub variable_text: VariableText,
}

impl Default for Document {
    fn default() -> Self {
        Self {
            version: DOC_VERSION,
            objects: Vec::new(),
            layers: Vec::new(),
            device_id: None,
            notes: String::new(),
            guides: Vec::new(),
            variable_text: Default::default(),
        }
    }
}

impl Document {
    pub fn to_json(&self) -> Result<String, String> {
        serde_json::to_string_pretty(self).map_err(|e| tr!("doc.document.serialize", error = e))
    }

    /// Lee un documento; migra la versión 1 (polilíneas) a la actual y rechaza versiones que
    /// esta compilación no conoce.
    pub fn from_json(text: &str) -> Result<Self, String> {
        let mut value: serde_json::Value =
            serde_json::from_str(text).map_err(|e| tr!("doc.document.invalid", error = e))?;
        match value.get("version").map(|v| v.as_u64()) {
            None => return Err(tr!("doc.document.no_version")),
            Some(Some(1)) => migrate_v1(&mut value),
            Some(Some(v)) if v == u64::from(DOC_VERSION) => {}
            Some(_) => {
                return Err(tr!(
                    "doc.document.unsupported_version",
                    found = value["version"],
                    supported = DOC_VERSION
                ));
            }
        }
        serde_json::from_value(value).map_err(|e| tr!("doc.document.invalid", error = e))
    }

    pub fn object(&self, id: &str) -> Option<&DesignObject> {
        self.objects.iter().find(|o| o.id == id)
    }

    pub fn layer(&self, color: Rgb) -> Option<&LayerSettings> {
        self.layers.iter().find(|l| l.color == color)
    }

    /// Caja envolvente de todos los objetos y de sus copias virtuales, en coordenadas de máquina.
    pub fn bounds(&self) -> Option<Bounds> {
        self.objects
            .iter()
            .filter_map(DesignObject::bounds_with_copies)
            .reduce(union)
    }
}

pub fn union(a: Bounds, b: Bounds) -> Bounds {
    Bounds {
        min: Point::new(a.min.x.min(b.min.x), a.min.y.min(b.min.y)),
        max: Point::new(a.max.x.max(b.max.x), a.max.y.max(b.max.y)),
    }
}

/// Versión 1 a 2: cada `{points, closed}` pasa a `move_to`/`line_to`(/`close`) y los objetos
/// ganan `group: null`. Lo que no reconoce lo deja tal cual para que falle la lectura normal.
fn migrate_v1(doc: &mut serde_json::Value) {
    use serde_json::{Value, json};
    let Some(objects) = doc.get_mut("objects").and_then(Value::as_array_mut) else {
        return;
    };
    for obj in objects {
        if let Some(map) = obj.as_object_mut() {
            map.entry("group").or_insert(Value::Null);
        }
        let Some(paths) = obj.pointer_mut("/kind/paths").and_then(Value::as_array_mut) else {
            continue;
        };
        for path in paths {
            let (Some(points), closed) = (
                path.get("points").and_then(Value::as_array),
                path.get("closed").and_then(Value::as_bool).unwrap_or(false),
            ) else {
                continue;
            };
            let mut segs: Vec<Value> = points
                .iter()
                .enumerate()
                .map(
                    |(i, p)| json!({ "type": if i == 0 { "move_to" } else { "line_to" }, "to": p }),
                )
                .collect();
            if closed && !segs.is_empty() {
                segs.push(json!({ "type": "close" }));
            }
            *path = Value::Array(segs);
        }
    }
    doc["version"] = DOC_VERSION.into();
}
