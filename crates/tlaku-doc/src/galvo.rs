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
    /// Grabado 3D por cortes (solo capas de imagen); funciona también en máquinas de pórtico.
    pub slice3d: Slice3d,
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
            slice3d: Slice3d::default(),
        }
    }
}

/// Grabado 3D por cortes (3D Slice): la imagen en grises es un mapa de profundidad (oscuro =
/// hondo) y se graba en `slices` pasadas con umbrales que bajan de blanco a negro; la pasada `k`
/// graba los píxeles más oscuros que su umbral. Cada corte es su propio bloque.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case", default)]
#[ts(export)]
pub struct Slice3d {
    pub enabled: bool,
    /// Número de cortes (profundidades), de 2 a 256.
    pub slices: u32,
    /// Grados que se suman al ángulo de barrido en cada corte (0 = el mismo ángulo).
    pub angle_step_deg: f64,
}

impl Default for Slice3d {
    fn default() -> Self {
        Self {
            enabled: false,
            slices: 16,
            angle_step_deg: 0.0,
        }
    }
}

/// Eje del cilindro o del cono (su generatriz), en el sistema del documento.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum CylinderAxis {
    /// El cilindro va a lo largo de X: la superficie se curva en Y (como un rotativo).
    #[default]
    X,
    /// El cilindro va a lo largo de Y: la superficie se curva en X.
    Y,
}

/// Corrección de cilindro (ProjectMark en EZCAD): compensa la curvatura al marcar con el galvo
/// sobre un cilindro. El diseño se dibuja plano, con la coordenada a lo largo de la curvatura en
/// mm de arco sobre la superficie; el eje del cilindro pasa por el centro de la cama.
/// Modelo geométrico propio (proyección en perspectiva desde el espejo; no verificado con
/// hardware).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case", default)]
#[ts(export)]
pub struct CylinderCorrection {
    pub enabled: bool,
    /// Diámetro del cilindro, mm.
    pub diameter_mm: f64,
    /// Distancia del espejo del galvo a la línea más cercana de la superficie (la cima de un
    /// cilindro convexo, el fondo de uno cóncavo), mm. Con la pieza enfocada es la distancia focal.
    pub mirror_distance_mm: f64,
    pub axis: CylinderAxis,
    /// Superficie interior (anillo por dentro) en vez de exterior.
    pub concave: bool,
}

impl Default for CylinderCorrection {
    fn default() -> Self {
        Self {
            enabled: false,
            diameter_mm: 70.0,
            mirror_distance_mm: 160.0,
            axis: CylinderAxis::X,
            concave: false,
        }
    }
}

/// Deformación para objetos cónicos (Taper Warp): el diseño se dibuja con el diámetro mayor y el
/// ancho se reduce hacia el extremo más delgado. Solo en mm; no corrige la distancia focal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case", default)]
#[ts(export)]
pub struct TaperWarp {
    pub enabled: bool,
    pub top_diameter_mm: f64,
    pub bottom_diameter_mm: f64,
    /// Largo del cono a lo largo de su eje, centrado en la cama, mm.
    pub length_mm: f64,
    pub axis: CylinderAxis,
    /// El extremo "superior" está en el lado positivo del eje (+X o +Y).
    pub top_at_positive: bool,
}

impl Default for TaperWarp {
    fn default() -> Self {
        Self {
            enabled: false,
            top_diameter_mm: 70.0,
            bottom_diameter_mm: 50.0,
            length_mm: 100.0,
            axis: CylinderAxis::X,
            top_at_positive: true,
        }
    }
}

/// Orden en que se marcan las piezas de un diseño más grande que el campo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum SplitOrder {
    /// Filas de arriba abajo, cada una en zigzag (menos recorrido de la mesa).
    #[default]
    Snake,
    /// Filas de arriba abajo, siempre de izquierda a derecha.
    Rows,
}

/// Qué ejes de la mesa se pueden mover entre piezas (marcado dividido sin rotativo).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum SplitAxes {
    #[default]
    Xy,
    X,
    Y,
}

/// Marcado dividido (Split Marking, M40) y rotativo galvo (M30): un diseño mayor que el campo
/// se parte en piezas del tamaño del campo. Con `DeviceProfile.rotary` activo las piezas son
/// franjas angulares (el rotativo gira entre ellas); si no, la mesa se mueve en `axes`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case", default)]
#[ts(export)]
pub struct SplitMarking {
    pub enabled: bool,
    /// Lado de las piezas, mm; 0 = el campo del galvo (un valor mayor se limita al campo).
    pub piece_mm: f64,
    /// Solape entre piezas contiguas, mm.
    pub overlap_mm: f64,
    /// Pausa tras cada movimiento de la mesa o del rotativo, ms.
    pub pause_after_move_ms: f64,
    pub order: SplitOrder,
    pub axes: SplitAxes,
}

impl Default for SplitMarking {
    fn default() -> Self {
        Self {
            enabled: false,
            piece_mm: 0.0,
            overlap_mm: 0.0,
            pause_after_move_ms: 0.0,
            order: SplitOrder::Snake,
            axes: SplitAxes::Xy,
        }
    }
}

/// Marcado repetido (Repeat Marking, M40): el trabajo (o la secuencia de piezas) se marca
/// `count` veces; entre repeticiones la mesa avanza `increment_mm` (con rotativo, la Y se
/// convierte a giro con la circunferencia de la pieza) y se espera `pause_ms`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case", default)]
#[ts(export)]
pub struct RepeatMarking {
    pub count: u32,
    pub increment_mm: [f64; 2],
    pub pause_ms: f64,
}

impl Default for RepeatMarking {
    fn default() -> Self {
        Self {
            count: 1,
            increment_mm: [0.0, 0.0],
            pause_ms: 0.0,
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

/// Enclavamiento por software (M45): antes de cada trabajo y de cada encuadre se leen las
/// entradas de la tarjeta y se exige `(entradas & mask) == value`. Los bits de EMSTOP y de las
/// alarmas de la fuente no están documentados: quien integra la máquina los mide y los fija aquí.
/// Con `mask = 0` no se exige nada.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case", default)]
#[ts(export)]
pub struct InputRequirement {
    /// Qué bits de la palabra de entradas se miran.
    pub mask: u16,
    /// Valor que deben tener esos bits; los bits fuera de `mask` deben ser 0.
    pub value: u16,
}

impl InputRequirement {
    /// `true` si las entradas cumplen la condición.
    pub fn is_met(&self, inputs: u16) -> bool {
        inputs & self.mask == self.value
    }

    /// `value` solo puede tener bits dentro de `mask`.
    pub fn is_valid(&self) -> bool {
        self.value & !self.mask == 0
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
    /// Corrección de cilindro y de cono (también para rotativo con galvo).
    pub cylinder: CylinderCorrection,
    pub taper: TaperWarp,
    /// Marcado dividido / rotativo galvo y marcado repetido: ver `tlaku_pipeline::mark_sequence`.
    pub split: SplitMarking,
    pub repeat: RepeatMarking,
    /// Enclavamiento por una entrada de la tarjeta (M45); `None` = sin enclavamiento.
    pub require_input: Option<InputRequirement>,
    /// Overscan de rellenos e imágenes, mm (M44); 0 = sin overscan (los espejos casi no tienen
    /// inercia). Con un valor mayor que 0 las líneas de relleno se alargan y el tramo extra se
    /// recorre con el láser apagado a la velocidad de marcado y sin retardo de salto.
    pub overscan_mm: f64,
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
            cylinder: CylinderCorrection::default(),
            taper: TaperWarp::default(),
            split: SplitMarking::default(),
            repeat: RepeatMarking::default(),
            require_input: None,
            overscan_mm: 0.0,
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Un perfil guardado antes de la ola 6C no trae los campos nuevos: se leen con sus valores
    /// de siempre (sin enclavamiento y sin overscan).
    #[test]
    fn old_galvo_profiles_load_with_the_new_fields_off() {
        let d: GalvoDevice = serde_json::from_str(r#"{ "field_mm": 150.0 }"#).unwrap();
        assert_eq!(d.field_mm, 150.0);
        assert_eq!(d.require_input, None);
        assert_eq!(d.overscan_mm, 0.0);
        assert_eq!(GalvoDevice::default().require_input, None);
    }

    #[test]
    fn the_interlock_round_trips_and_checks_its_bits() {
        let d: GalvoDevice =
            serde_json::from_str(r#"{ "require_input": { "mask": 8, "value": 8 } }"#).unwrap();
        let r = d.require_input.unwrap();
        assert!(r.is_valid() && r.is_met(0x0008) && r.is_met(0x00FF) && !r.is_met(0));
        assert!(!InputRequirement { mask: 1, value: 2 }.is_valid());
        let json = serde_json::to_string(&d).unwrap();
        assert_eq!(serde_json::from_str::<GalvoDevice>(&json).unwrap(), d);
        // Una condición a medias toma 0 en lo que falta (no exige nada).
        let r: InputRequirement = serde_json::from_str("{}").unwrap();
        assert_eq!((r.mask, r.value), (0, 0));
    }
}
