use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum DeviceKind {
    Grbl,
}

/// Esquina de la cama donde la máquina tiene su X0 Y0. El G-code crece hacia dentro de la
/// cama desde esa esquina; el documento siempre se dibuja con el origen abajo a la izquierda.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum Origin {
    BottomLeft,
    TopLeft,
    BottomRight,
    TopRight,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum PowerMode {
    /// M4: la controladora escala la potencia con la velocidad real.
    Dynamic,
    /// M3: potencia fija.
    Constant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum RotaryKind {
    /// La pieza descansa sobre dos rodillos; gira según el diámetro del rodillo.
    #[default]
    Roller,
    /// La pieza va sujeta al eje (mandril); gira según su propio diámetro.
    Chuck,
}

/// Accesorio rotativo conectado en lugar del eje Y.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case", default)]
#[ts(export)]
pub struct RotarySettings {
    pub enabled: bool,
    pub kind: RotaryKind,
    /// mm de Y que la controladora debe recorrer para una vuelta completa del rotativo.
    pub mm_per_rotation: f64,
    /// Diámetro del rodillo motriz en mm (solo `roller`).
    pub roller_diameter: f64,
    /// Diámetro de la pieza en mm (solo `chuck`, y para la vista previa).
    pub object_diameter: f64,
}

impl Default for RotarySettings {
    fn default() -> Self {
        Self {
            enabled: false,
            kind: RotaryKind::Roller,
            mm_per_rotation: 360.0,
            roller_diameter: 20.0,
            object_diameter: 80.0,
        }
    }
}

/// Corrección del desfase entre las líneas de ida y de vuelta en barridos bidireccionales,
/// medida a una velocidad. Entre velocidades se interpola linealmente.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub struct ScanOffset {
    /// mm/min.
    pub speed: f64,
    /// mm que se desplazan las líneas de vuelta en el sentido del barrido.
    pub offset: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub struct DeviceProfile {
    pub id: String,
    pub name: String,
    pub kind: DeviceKind,
    /// Ancho y alto de la cama, en mm.
    pub bed: [f64; 2],
    pub origin: Origin,
    /// Valor de S para 100 % de potencia (`$30`).
    pub s_max: f64,
    pub power_mode: PowerMode,
    /// Aceleración por eje, mm/s².
    pub accel: [f64; 2],
    /// Velocidad máxima por eje, mm/min.
    pub max_rate: [f64; 2],
    /// Velocidad de los movimientos rápidos, mm/min.
    pub rapid_rate: f64,
    pub baud: u32,
    /// Bytes del búfer de recepción de la controladora.
    pub rx_buffer: u32,
    pub start_gcode: String,
    pub end_gcode: String,
    pub return_to_origin: bool,
    /// La máquina tiene eje Z motorizado (habilita `z_offset`/`z_step` de las capas).
    #[serde(default)]
    pub z_axis: bool,
    #[serde(default)]
    pub rotary: RotarySettings,
    /// Ordenadas por velocidad; vacío = sin corrección.
    #[serde(default)]
    pub scan_offsets: Vec<ScanOffset>,
}

impl Default for DeviceProfile {
    /// Láser de diodo con GRBL de fábrica y cama de 400x415 mm.
    fn default() -> Self {
        Self {
            id: "diodo-grbl-400x415".into(),
            name: "Diodo GRBL 400x415".into(),
            kind: DeviceKind::Grbl,
            bed: [400.0, 415.0],
            origin: Origin::BottomLeft,
            s_max: 1000.0,
            power_mode: PowerMode::Dynamic,
            accel: [1000.0, 1000.0],
            max_rate: [12000.0, 12000.0],
            rapid_rate: 12000.0,
            baud: 115_200,
            rx_buffer: 127,
            start_gcode: String::new(),
            end_gcode: String::new(),
            return_to_origin: true,
            z_axis: false,
            rotary: RotarySettings::default(),
            scan_offsets: Vec::new(),
        }
    }
}
