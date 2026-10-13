//! Lector de proyectos de LightBurn (`.lbrn` y `.lbrn2`, XML) a un modelo neutro.
//!
//! No depende del documento de Tlaku: entrega formas, capas (`CutSetting`) y avisos tal como
//! vienen en el archivo. La conversión a objetos y ajustes de Tlaku vive en `tlaku-pipeline`.
//!
//! Formato (fuentes públicas, ver `THIRD_PARTY.md`; no hay especificación oficial):
//! - Raíz `<LightBurnProject AppVersion FormatVersion MirrorX MirrorY>`. `FormatVersion` es 1 en
//!   `.lbrn2` y 0 (o falta, en versiones muy viejas) en `.lbrn`. Ambos son XML con la misma
//!   estructura.
//! - Capas: `<CutSetting type="Cut|Scan|Scan+Cut|Image|Tool">` (o `<CutSetting_Img>` para las de
//!   imagen) con hijos `<clave Value="..."/>`; `index` enlaza con el `CutIndex` de cada forma.
//! - Formas: `<Shape Type="Rect|Ellipse|Path|Group|Text|Bitmap" CutIndex=".." ...>` con
//!   `<XForm>a b c d e f</XForm>`; las transformaciones se componen padre-hijo.
//! - Unidades: milímetros (velocidad en mm/s). Los ejes del espacio de trabajo van con Y hacia arriba (lo confirman la
//!   orientación de los contornos de texto guardados y el diseño de archivos públicos, inferido).

mod parse;
mod pathdata;

pub use parse::parse;
pub use pathdata::{ellipse_path, rect_path};

use tlaku_core::bezpath::BezPath;

/// Afín `[a b c d e f]`: `x' = a·x + c·y + e`, `y' = b·x + d·y + f` (el orden del `XForm`).
pub type Transform = [f64; 6];

pub const IDENTITY: Transform = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];

/// Primer `CutIndex` de las capas de herramienta (T1); T2 es el siguiente.
pub const TOOL_LAYER_FIRST: i32 = 30;

/// Transformación que aplica primero `inner` y después `outer`.
pub fn compose(outer: &Transform, inner: &Transform) -> Transform {
    [
        outer[0] * inner[0] + outer[2] * inner[1],
        outer[1] * inner[0] + outer[3] * inner[1],
        outer[0] * inner[2] + outer[2] * inner[3],
        outer[1] * inner[2] + outer[3] * inner[3],
        outer[0] * inner[4] + outer[2] * inner[5] + outer[4],
        outer[1] * inner[4] + outer[3] * inner[5] + outer[5],
    ]
}

/// Un proyecto leído.
#[derive(Debug, Clone, PartialEq)]
pub struct Project {
    pub app_version: String,
    /// `None` si el archivo no lo declara (LightBurn 0.9.x viejo).
    pub format_version: Option<u32>,
    pub mirror_x: bool,
    pub mirror_y: bool,
    pub layers: Vec<Layer>,
    pub shapes: Vec<Shape>,
    /// Problemas leyendo el archivo (ya en español, listos para mostrar).
    pub warnings: Vec<String>,
}

/// Una capa (`CutSetting`). Los parámetros se guardan como texto, por clave de elemento.
#[derive(Debug, Clone, PartialEq)]
pub struct Layer {
    pub index: i32,
    /// Atributo `type`: `Cut`, `Scan`, `Scan+Cut`, `Image`, `Tool`...
    pub kind: String,
    pub name: String,
    /// Parámetros por nombre de elemento (`speed`, `maxPower`...), con el texto de su `Value`.
    pub params: std::collections::HashMap<String, String>,
    /// Sub-capas: las capas siguientes del archivo con el mismo índice, en orden.
    pub subs: Vec<Layer>,
}

impl Layer {
    pub fn text(&self, key: &str) -> Option<&str> {
        self.params.get(key).map(String::as_str)
    }

    pub fn num(&self, key: &str) -> Option<f64> {
        self.text(key)?
            .trim()
            .parse()
            .ok()
            .filter(|v: &f64| v.is_finite())
    }

    /// Valores `1`/`0` (también `true`/`false`).
    pub fn flag(&self, key: &str) -> Option<bool> {
        match self.text(key)?.trim() {
            "1" | "true" | "True" => Some(true),
            "0" | "false" | "False" => Some(false),
            other => other.parse::<f64>().ok().map(|v| v != 0.0),
        }
    }

    /// Capa de herramienta (T1/T2): guías que no salen a la máquina.
    pub fn is_tool(&self) -> bool {
        self.kind.eq_ignore_ascii_case("tool") || self.index >= TOOL_LAYER_FIRST
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Shape {
    /// `None` si la forma no trae `CutIndex` (grupos).
    pub cut_index: Option<i32>,
    pub xform: Transform,
    pub locked: bool,
    pub kind: ShapeKind,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ShapeKind {
    /// Caja centrada en el origen local de `w` x `h`, esquinas de radio `corner_radius`.
    Rect {
        w: f64,
        h: f64,
        corner_radius: f64,
    },
    /// Elipse centrada en el origen local con radios `rx`, `ry`.
    Ellipse {
        rx: f64,
        ry: f64,
    },
    /// Trayectoria con sus curvas en coordenadas locales (puede traer varios subtrayectos).
    Path(BezPath),
    Group(Vec<Shape>),
    Text(TextShape),
    Bitmap(BitmapShape),
    /// Tipo que no se entiende (se cuenta para avisar al usuario).
    Unknown(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct TextShape {
    pub text: String,
    /// Cadena `Font` tal cual (`Arial,-1,100,5,50,0,0,0,0,0`).
    pub font: String,
    pub family: String,
    pub bold: bool,
    pub italic: bool,
    /// Atributo `H`: tamaño del em en mm.
    pub height: f64,
    /// `LS` y `LnS` tal cual (unidades sin confirmar).
    pub letter_spacing: f64,
    pub line_spacing: f64,
    /// `Ah`: 0 izquierda, 1 centro, 2 derecha.
    pub align_h: u8,
    /// `Av`: 0 arriba, 1 centro, 2 abajo.
    pub align_v: u8,
    /// Contorno guardado por LightBurn (`BackupPath`) en el sistema local del texto, si lo trae.
    pub outline: Option<BezPath>,
    /// Texto variable (`VariableOffset`/`Eval`): el valor guardado es el de ese momento.
    pub variable: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BitmapShape {
    /// Caja local centrada en el origen: `w` x `h`.
    pub w: f64,
    pub h: f64,
    /// Bytes de la imagen incrustada (PNG, JPEG...), ya sin base64.
    pub data: Vec<u8>,
    /// Ruta original de la imagen en el equipo del autor (atributo `File`).
    pub file: Option<String>,
}
