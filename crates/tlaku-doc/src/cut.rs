//! Ajustes avanzados de corte de una capa (ola 2): perforación, entradas y salidas, overcut,
//! pausas, modo punto, rampa y agrupación del relleno. Todos tienen un valor por defecto que
//! no cambia nada, así que los documentos viejos se comportan igual.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Corte intermitente: tramos encendidos de `cut_mm` separados por tramos apagados de
/// `skip_mm` (por ejemplo, líneas de doblez en cartón).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case", default)]
#[ts(export)]
pub struct Perforation {
    pub enabled: bool,
    pub cut_mm: f64,
    pub skip_mm: f64,
}

impl Default for Perforation {
    fn default() -> Self {
        Self {
            enabled: false,
            cut_mm: 1.0,
            skip_mm: 1.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum LeadKind {
    #[default]
    Straight,
    Arc,
}

/// Entrada o salida de un contorno cerrado: el láser empieza (o termina) fuera de la pieza
/// para que la marca del arranque no quede en el borde.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case", default)]
#[ts(export)]
pub struct Lead {
    pub enabled: bool,
    pub kind: LeadKind,
    /// mm.
    pub length: f64,
    /// Grados respecto a la tangente del contorno (recto) o barrido del arco.
    pub angle: f64,
}

impl Default for Lead {
    fn default() -> Self {
        Self {
            enabled: false,
            kind: LeadKind::Straight,
            length: 1.0,
            angle: 45.0,
        }
    }
}

/// Arranque de un contorno cerrado con potencia extra durante un tiempo, para atravesar
/// material grueso sin dejar un punto quemado.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case", default)]
#[ts(export)]
pub struct CutThrough {
    pub enabled: bool,
    /// %.
    pub power: f64,
    pub ms: f64,
}

impl Default for CutThrough {
    fn default() -> Self {
        Self {
            enabled: false,
            power: 100.0,
            ms: 100.0,
        }
    }
}

/// Modo punto: en lugar de una línea continua, disparos de `dwell_ms` cada `spacing_mm`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case", default)]
#[ts(export)]
pub struct DotMode {
    pub enabled: bool,
    pub dwell_ms: f64,
    pub spacing_mm: f64,
}

impl Default for DotMode {
    fn default() -> Self {
        Self {
            enabled: false,
            dwell_ms: 1.0,
            spacing_mm: 0.5,
        }
    }
}

/// Rampa de potencia al empezar y al terminar cada trazo (sellos de goma, biselados).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case", default)]
#[ts(export)]
pub struct Ramp {
    pub enabled: bool,
    pub length_mm: f64,
    /// % de la potencia de la capa con que empieza la rampa.
    pub start_pct: f64,
    /// Solo en contornos exteriores (la pieza queda biselada hacia fuera).
    pub outer_only: bool,
}

impl Default for Ramp {
    fn default() -> Self {
        Self {
            enabled: false,
            length_mm: 1.0,
            start_pct: 0.0,
            outer_only: false,
        }
    }
}

/// Cómo se agrupan las formas al rellenar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum FillGrouping {
    /// Todas las formas de la capa en un solo barrido (lo más rápido).
    #[default]
    All,
    /// Cada forma (o isla) se rellena completa antes de pasar a la siguiente.
    ByShape,
    /// Cada grupo del documento se rellena junto.
    ByGroup,
}

/// Forma paramétrica: guarda los parámetros con que se dibujó para poder editarlos después.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
#[ts(export)]
pub enum ShapeParams {
    Rect {
        width: f64,
        height: f64,
        radius: f64,
    },
    Ellipse {
        rx: f64,
        ry: f64,
    },
    Polygon {
        sides: u32,
        radius: f64,
        corner: f64,
    },
    Star {
        points: u32,
        outer: f64,
        inner: f64,
        corner: f64,
    },
}

/// Cómo se ordena el trabajo al enviarlo (ventana de ajustes de optimización).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case", default)]
#[ts(export)]
pub struct Optimization {
    /// Respetar el orden de las capas.
    pub by_layer: bool,
    /// Terminar cada grupo antes de pasar al siguiente.
    pub by_group: bool,
    /// Respetar la prioridad de cada objeto (`DesignObject.priority`).
    pub by_priority: bool,
    /// Formas interiores antes que las que las contienen.
    pub inner_first: bool,
    /// Elegir el mejor punto de inicio de cada forma cerrada.
    pub best_start: bool,
    /// Permitir invertir el sentido de las formas para acortar recorridos.
    pub choose_direction: bool,
    /// Quitar al enviar los tramos que se cortarían dos veces.
    pub remove_overlaps: bool,
    /// Tolerancia de `remove_overlaps`, en mm.
    pub overlap_tolerance: f64,
    /// Recorrer siempre en el mismo sentido para esconder el juego mecánico (más lento).
    pub hide_backlash: bool,
    /// Reducir los recorridos en vacío (vecino más cercano y mejoras locales).
    pub reduce_travel: bool,
}

impl Default for Optimization {
    fn default() -> Self {
        Self {
            by_layer: true,
            by_group: false,
            by_priority: true,
            inner_first: true,
            best_start: true,
            choose_direction: true,
            remove_overlaps: false,
            overlap_tolerance: 0.01,
            hide_backlash: false,
            reduce_travel: true,
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Ola 3: diseño avanzado.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum GuideAxis {
    /// Guía vertical (x fija).
    X,
    /// Guía horizontal (y fija).
    Y,
}

/// Línea guía del documento: no se corta, sirve para alinear y ajustar.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub struct Guide {
    pub axis: GuideAxis,
    /// mm en coordenadas del documento.
    pub pos: f64,
}

/// Punto de inicio y sentido elegidos a mano para un objeto (manda sobre el planificador).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub struct StartSpec {
    /// Punto en coordenadas del documento; se usa el punto más cercano del contorno.
    pub point: Option<[f64; 2]>,
    /// Recorrer al revés del sentido dibujado.
    pub reverse: bool,
}

/// Copias que no existen como objetos: se generan al enviar y en la vista previa.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
#[ts(export)]
pub enum VirtualArray {
    /// `cols` x `rows` copias separadas `dx`, `dy` mm (de borde a borde de la caja).
    Grid {
        cols: u32,
        rows: u32,
        dx: f64,
        dy: f64,
    },
    /// `count` copias alrededor de `center` (documento), repartidas en `sweep_deg`, girando
    /// cada copia si `rotate`.
    Circular {
        count: u32,
        center: [f64; 2],
        sweep_deg: f64,
        rotate: bool,
    },
}

/// Ajustes del texto variable del documento: los textos con `{serie}`, `{csv:columna}`,
/// `{fecha}`, etc. se resuelven al enviar con estos valores.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case", default)]
#[ts(export)]
pub struct VariableText {
    /// Próximo número de serie.
    pub serial: i64,
    /// Cuánto avanza la serie por cada copia o trabajo.
    pub serial_step: i64,
    /// Dígitos mínimos (rellena con ceros).
    pub serial_digits: u32,
    /// Ruta del CSV con los datos (columnas por nombre de encabezado).
    pub csv_path: Option<String>,
    /// Fila actual del CSV (0 = la primera de datos).
    pub csv_row: u32,
    /// Avanzar serie y fila automáticamente al terminar un trabajo.
    pub advance_after_job: bool,
}

impl Default for VariableText {
    fn default() -> Self {
        Self {
            serial: 1,
            serial_step: 1,
            serial_digits: 0,
            csv_path: None,
            csv_row: 0,
            advance_after_job: true,
        }
    }
}
