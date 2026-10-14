//! Ola 5: galvo (tarjetas JCZ/BJJCZ tipo EZCAD2) y opciones de trabajo profesionales.
//! Ver `docs/hardware/galvo.md`, sección 8. Todo tiene valores por defecto que no cambian nada
//! en máquinas de pórtico.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum LaserSource {
    /// Fibra Q-switched: frecuencia, sin ancho de pulso.
    #[default]
    FiberQ,
    /// Fibra MOPA: frecuencia y ancho de pulso.
    FiberMopa,
    Co2,
    Uv,
}

/// Retardos de la tarjeta en microsegundos (pueden ser negativos).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case", default)]
#[ts(export)]
pub struct GalvoTiming {
    pub laser_on_us: f64,
    pub laser_off_us: f64,
    pub end_us: f64,
    pub polygon_us: f64,
}

impl Default for GalvoTiming {
    fn default() -> Self {
        Self {
            laser_on_us: 100.0,
            laser_off_us: 100.0,
            end_us: 300.0,
            polygon_us: 100.0,
        }
    }
}

/// Saltos (movimientos con el láser apagado) del galvo.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case", default)]
#[ts(export)]
pub struct GalvoJump {
    /// mm/s.
    pub speed: f64,
    pub delay_min_us: f64,
    pub delay_max_us: f64,
    /// Distancia (mm) a partir de la cual se usa el retardo máximo.
    pub distance_limit: f64,
}

impl Default for GalvoJump {
    fn default() -> Self {
        Self {
            speed: 4000.0,
            delay_min_us: 200.0,
            delay_max_us: 400.0,
            distance_limit: 10.0,
        }
    }
}

/// Bamboleo: el haz describe pequeños círculos a lo largo del trazo para ensanchar la línea.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case", default)]
#[ts(export)]
pub struct Wobble {
    pub enabled: bool,
    /// Diámetro del círculo, mm.
    pub size: f64,
    /// Avance por vuelta, mm.
    pub step: f64,
}

impl Default for Wobble {
    fn default() -> Self {
        Self {
            enabled: false,
            size: 0.2,
            step: 0.05,
        }
    }
}

/// Ajustes de capa que solo usan las máquinas galvo (y, `ppi`, algunas de CO2).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case", default)]
#[ts(export)]
pub struct GalvoLayer {
    /// kHz.
    pub frequency_khz: f64,
    /// ns; solo fuentes MOPA o UV.
    pub pulse_width_ns: Option<u32>,
    /// Usar estos retardos y saltos en vez de los del perfil.
    pub override_timings: bool,
    pub timing: GalvoTiming,
    pub jump: GalvoJump,
    pub wobble: Wobble,
    /// Repeticiones rápidas de la capa completa en la tarjeta (en vez de repetir movimientos).
    pub repeat: u32,
    pub pause_before_ms: f64,
    pub pause_after_ms: f64,
    /// Pulsos por pulgada (modo PPI de algunas controladoras de CO2); 0 = apagado.
    pub ppi: f64,
}

impl Default for GalvoLayer {
    fn default() -> Self {
        Self {
            frequency_khz: 30.0,
            pulse_width_ns: None,
            override_timings: false,
            timing: GalvoTiming::default(),
            jump: GalvoJump::default(),
            wobble: Wobble::default(),
            repeat: 1,
            pause_before_ms: 0.0,
            pause_after_ms: 0.0,
            ppi: 0.0,
        }
    }
}

/// Corrección geométrica del campo (sin archivo `.cor`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case", default)]
#[ts(export)]
pub struct FieldCorrection {
    pub scale: [f64; 2],
    pub offset: [f64; 2],
    pub rotation_deg: f64,
    pub invert: [bool; 2],
    pub swap_xy: bool,
    /// Barril/cojín (+/-) por eje.
    pub bulge: [f64; 2],
    pub trapezoid: [f64; 2],
    pub skew: [f64; 2],
    /// Ruta de una tabla de corrección `.cor` de EZCAD; `None` = solo los parámetros anteriores.
    pub cor_file: Option<String>,
}

impl Default for FieldCorrection {
    fn default() -> Self {
        Self {
            scale: [1.0, 1.0],
            offset: [0.0, 0.0],
            rotation_deg: 0.0,
            invert: [false, false],
            swap_xy: false,
            bulge: [0.0, 0.0],
            trapezoid: [0.0, 0.0],
            skew: [0.0, 0.0],
            cor_file: None,
        }
    }
}

/// Perfil de una máquina galvo.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case", default)]
#[ts(export)]
pub struct GalvoDevice {
    /// Tamaño del campo de la lente F-theta, mm (por ejemplo 110, 150, 200, 300).
    pub field_mm: f64,
    pub source: LaserSource,
    /// Potencia nominal de la fuente, W (solo informativo y para límites).
    pub watts: f64,
    pub freq_min_khz: f64,
    pub freq_max_khz: f64,
    /// Anchos de pulso admitidos por la fuente MOPA (ns).
    pub pulse_widths_ns: Vec<u32>,
    pub timing: GalvoTiming,
    pub jump: GalvoJump,
    pub correction: FieldCorrection,
    /// Potencia máxima permitida (%), límite de seguridad del perfil.
    pub max_power_pct: f64,
    /// Encuadre con el puntero rojo: velocidad (mm/s).
    pub light_speed: f64,
}

impl Default for GalvoDevice {
    fn default() -> Self {
        Self {
            field_mm: 110.0,
            source: LaserSource::FiberQ,
            watts: 20.0,
            freq_min_khz: 20.0,
            freq_max_khz: 80.0,
            pulse_widths_ns: Vec::new(),
            timing: GalvoTiming::default(),
            jump: GalvoJump::default(),
            correction: FieldCorrection::default(),
            max_power_pct: 100.0,
            light_speed: 2000.0,
        }
    }
}

/// Alimentador automático (cinta o rodillos que avanzan el material entre trabajos o tramos).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case", default)]
#[ts(export)]
pub struct FeederSettings {
    pub enabled: bool,
    /// G-code que avanza el material; `{mm}` se sustituye por la distancia.
    pub feed_gcode: String,
    /// Distancia de avance por tramo, mm.
    pub feed_mm: f64,
}

impl Default for FeederSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            feed_gcode: "G91\nG0 Y{mm}\nG90".into(),
            feed_mm: 300.0,
        }
    }
}

/// Un tramo de un trabajo dividido (tiling): solo se envía lo que cae dentro de `rect` (mm del
/// documento) y se desplaza `offset` mm para que quepa en la cama.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub struct TileSpec {
    pub rect: [[f64; 2]; 2],
    pub offset: [f64; 2],
}
