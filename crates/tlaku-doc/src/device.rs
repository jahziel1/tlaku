use crate::galvo::{FeederSettings, GalvoDevice};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Familia de firmware de la controladora. Decide el dialecto de G-code por defecto
/// ([`GcodeDialect::for_kind`]); el dialecto del perfil se puede editar después.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum DeviceKind {
    /// GRBL 1.1f o posterior con modo láser (`$32=1`): `M4`, `S` 0-1000.
    #[default]
    Grbl,
    /// GRBL 1.1e o anterior (0.9, 1.0): sin `M4` y sin modo láser, así que `G0` no apaga el rayo.
    GrblM3,
    GrblHal,
    FluidNc,
    /// GRBL portado al LPC1769 (Cohesion3D, Smoothieboard con ese firmware).
    GrblLpc,
    Smoothie,
    /// Marlin 2.x con función láser (modo inline).
    Marlin,
    /// G-code propio: el usuario edita el dialecto; parte del de GRBL 1.1.
    CustomGcode,
    /// Controladoras DSP Ruida (CO2): protocolo binario por red (UDP) o archivo `.rd`; no usa
    /// el dialecto de G-code. Ver `docs/hardware/ruida.md` y el crate `tlaku-ruida`.
    Ruida,
    /// Placas Lihuiyu M2 Nano y afines (K40 de serie): protocolo LHYMICRO-GL por USB; no usa
    /// el dialecto de G-code. Ver `docs/hardware/dsp-co2.md`.
    LhyMicro,
    /// Galvo con tarjeta JCZ/BJJCZ (EZCAD2, LMC por USB): fibra, UV o CO2 galvo. No usa G-code.
    /// Ver `docs/hardware/galvo.md` y el crate `tlaku-galvo`.
    Galvo,
}

impl DeviceKind {
    /// Valor habitual de `s_max` (S para el 100 %) del firmware. Solo se usa al crear un perfil
    /// de ese tipo ([`DeviceProfile::for_kind`]): el perfil guarda su propio `s_max`.
    pub fn default_s_max(self) -> f64 {
        match self {
            DeviceKind::GrblM3 => 255.0,
            DeviceKind::GrblLpc => 1.0,
            DeviceKind::Smoothie => 1.0,
            DeviceKind::Marlin => 255.0,
            _ => 1000.0,
        }
    }

    /// Velocidad serie habitual del firmware.
    pub fn default_baud(self) -> u32 {
        match self {
            DeviceKind::Marlin => 250_000,
            _ => 115_200,
        }
    }
}

/// Cómo se escriben los comentarios del G-code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum CommentStyle {
    /// `; texto` hasta el fin de línea.
    #[default]
    Semicolon,
    /// `(texto)`.
    Parentheses,
}

/// Unidad y letra del tiempo de `G4` (pausa). GRBL y Smoothieware en modo GRBL leen `P` en
/// segundos; Marlin (y Smoothieware en modo RepRap, el habitual con láser) lo leen en
/// milisegundos. Como `S`, ambos lo leen en segundos (enteros en Marlin).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum DwellFormat {
    /// `G4 P0.25`: segundos con decimales (GRBL y familia).
    #[default]
    PSeconds,
    /// `G4 P250`: milisegundos enteros (Marlin, Smoothieware).
    PMilliseconds,
    /// `G4 S0.25`: segundos en la letra `S`.
    SSeconds,
}

/// Cómo escribe el G-code una controladora: datos puros que lee el generador de `tlaku-grbl`,
/// sin código propio por firmware. Solo cubre la generación; el enlace (control de flujo,
/// pausa, consulta de estado) vive aparte. La escala de `S` es `DeviceProfile.s_max`.
///
/// El valor por defecto es el de GRBL 1.1; los demás firmwares salen de [`GcodeDialect::for_kind`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case", default)]
#[ts(export)]
pub struct GcodeDialect {
    /// Decimales de `S` (0 en Marlin y GRBL-M3, 3 en GRBL y Smoothie).
    pub s_decimals: u8,
    /// Decimales máximos de las coordenadas (se quitan los ceros sobrantes).
    pub decimals: u8,
    /// Línea que enciende el láser a potencia constante, con `S0` para no disparar todavía
    /// (`M3 S0`, `M3 S0 I`). Vacía = no se emite (el láser se enciende con `power_cmd`).
    pub constant_on: String,
    /// Línea del modo de potencia dinámica (`M4 S0`); `None` = el firmware no la tiene y la
    /// capa dinámica usa `constant_on`.
    pub dynamic_on: Option<String>,
    /// Apaga el láser al final del trabajo (`M5`, `M107 P1`).
    pub laser_off: String,
    /// `true`: la potencia va como `S` dentro de cada `G1`. `false`: va en una línea aparte
    /// (`power_cmd`) antes del movimiento; cada cambio de potencia frena la máquina.
    pub s_inline: bool,
    /// Plantilla de la línea de potencia con `{s}` (`M3 S{s}`, `M106 P1 S{s}`). Solo se usa con
    /// `s_inline = false`.
    pub power_cmd: String,
    /// `G0` apaga el láser por sí solo (modo láser de GRBL 1.1, Smoothie, Marlin). Si no
    /// (GRBL-M3, `M106`), el generador lo apaga antes de cada desplazamiento.
    pub g0_laser_off: bool,
    /// Con `s_inline`: escribe `S0` en el `G0` cuando el láser queda con una `S` distinta de 0.
    /// Es la forma de apagarlo cuando `G0` no lo hace (GRBL-M3) y una defensa en Marlin.
    pub travel_s0: bool,
    /// Escribe `F` también en los `G0` (Marlin comparte la velocidad entre `G0` y `G1`); el valor
    /// es `DeviceProfile.rapid_rate`.
    pub feed_on_g0: bool,
    /// Enciende el aire asistido (`M8`, `M7`, `M106 P2 S255`); `None` = la máquina no lo tiene
    /// y las capas con aire lo ignoran con un aviso.
    pub air_on: Option<String>,
    /// Apaga el aire asistido (`M9`, `M107 P2`).
    pub air_off: Option<String>,
    /// Última línea del trabajo, tras volver al origen (`M400`): su `ok` llega cuando acaba el
    /// último movimiento. `None` = no se emite.
    pub end_sync: Option<String>,
    pub comment: CommentStyle,
    /// Cómo se escribe el tiempo de `G4` (pausas del perforado y del modo punto).
    pub dwell: DwellFormat,
    /// Longitud máxima de una línea con su salto, en bytes (127 en GRBL, ~96 en Marlin);
    /// 0 = sin dato. Los comentarios se recortan para no pasarla.
    pub max_line_len: u32,
    /// Línea que selecciona el láser 1 al volver a él tras usar el 2; `None` = no se emite.
    pub laser1_select: Option<String>,
    /// Línea que selecciona el láser 2 antes de las capas que lo usan (máquinas con
    /// `DeviceProfile::laser2`); `None` = el firmware no la tiene y se avisa.
    pub laser2_select: Option<String>,
}

impl Default for GcodeDialect {
    /// GRBL 1.1.
    fn default() -> Self {
        Self {
            s_decimals: 3,
            decimals: 3,
            constant_on: "M3 S0".into(),
            dynamic_on: Some("M4 S0".into()),
            laser_off: "M5".into(),
            s_inline: true,
            power_cmd: "M3 S{s}".into(),
            g0_laser_off: true,
            travel_s0: false,
            feed_on_g0: false,
            air_on: Some("M8".into()),
            air_off: Some("M9".into()),
            end_sync: None,
            comment: CommentStyle::Semicolon,
            dwell: DwellFormat::PSeconds,
            max_line_len: 127,
            laser1_select: None,
            laser2_select: None,
        }
    }
}

impl GcodeDialect {
    /// Dialecto habitual de cada firmware (ver `docs/hardware/dialectos-gcode.md`, sección 8.3).
    pub fn for_kind(kind: DeviceKind) -> Self {
        let grbl = Self::default();
        match kind {
            DeviceKind::Grbl
            | DeviceKind::GrblHal
            | DeviceKind::FluidNc
            | DeviceKind::GrblLpc
            | DeviceKind::CustomGcode => grbl,
            // No hablan G-code: el dialecto no se usa, se deja el de GRBL para que el perfil
            // siga siendo válido si alguien cambia de tipo.
            DeviceKind::Ruida | DeviceKind::LhyMicro | DeviceKind::Galvo => grbl,
            // Sin `M4`, sin modo láser (`G0` no apaga) y con `S` entera de 0 a 255.
            DeviceKind::GrblM3 => Self {
                s_decimals: 0,
                dynamic_on: None,
                g0_laser_off: false,
                travel_s0: true,
                ..grbl
            },
            // `S` de 0 a 1 por defecto (decimales); el módulo láser solo dispara en `G1`.
            // No hay dato del largo máximo de línea.
            DeviceKind::Smoothie => Self {
                dynamic_on: None,
                end_sync: Some("M400".into()),
                // En el modo RepRap (el que se usa con láser) `G4 P` va en ms; solo en el
                // modo GRBL de Smoothieware es en segundos.
                dwell: DwellFormat::PMilliseconds,
                max_line_len: 0,
                ..grbl
            },
            DeviceKind::Marlin => Self::marlin_inline(),
        }
    }

    /// Marlin 2.x con potencia inline (`M3 S0 I` y `G1 ... S<n>`): el modo recomendado, el
    /// único apto para imágenes.
    pub fn marlin_inline() -> Self {
        Self {
            s_decimals: 0,
            constant_on: "M3 S0 I".into(),
            dynamic_on: None,
            travel_s0: true,
            feed_on_g0: true,
            end_sync: Some("M400".into()),
            dwell: DwellFormat::PMilliseconds,
            max_line_len: 96,
            ..Self::default()
        }
    }

    /// Marlin sin inline: `M3 S<n>` en una línea aparte antes de cada tramo. Sirve para corte
    /// vectorial a potencia constante; cada cambio de `S` frena la máquina.
    pub fn marlin_standard() -> Self {
        Self {
            constant_on: String::new(),
            s_inline: false,
            travel_s0: false,
            ..Self::marlin_inline()
        }
    }

    /// Marlin heredado con el láser en la salida del ventilador (`M106`/`M107`). `G0` no apaga
    /// el láser, así que se apaga antes de cada desplazamiento.
    pub fn marlin_fan() -> Self {
        Self {
            laser_off: "M107 P1".into(),
            power_cmd: "M106 P1 S{s}".into(),
            g0_laser_off: false,
            ..Self::marlin_standard()
        }
    }
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
    /// Diámetro de la pieza en mm (solo `chuck`, y para la circunferencia que se avisa y la
    /// prueba de una vuelta; con `roller` no afecta al G-code).
    pub object_diameter: f64,
    /// Refleja la Y del trabajo dentro de su caja (solo en el G-code) cuando el accesorio gira
    /// al revés según el montaje del motor. No cambia el área que ocupa el trabajo.
    pub mirror: bool,
}

impl Default for RotarySettings {
    /// Rodillo con los datos oficiales de xTool RA2 Pro: 32 mm de Y por vuelta y rodillo de
    /// 17 mm, el único accesorio de rodillo con cifras publicadas por el fabricante. Los demás
    /// se calibran, pero ningún accesorio real se acerca a los 360 mm que usaba Tlaku antes
    /// (con ellos Y se movía casi 6 veces de más y la máquina daba `ALARM:2`).
    fn default() -> Self {
        Self::preset(RotaryKind::Roller)
    }
}

impl RotarySettings {
    /// Valores de partida de cada tipo de accesorio, con el rotativo desactivado: rodillo 32 mm
    /// por vuelta y 17 mm de rodillo (xTool); mandril 40 mm por vuelta (Creality Rotary Kit Pro,
    /// 3200 pasos a `$101=80`). Hay que calibrarlos con la prueba de una vuelta.
    pub fn preset(kind: RotaryKind) -> Self {
        Self {
            enabled: false,
            kind,
            mm_per_rotation: match kind {
                RotaryKind::Roller => 32.0,
                RotaryKind::Chuck => 40.0,
            },
            roller_diameter: 17.0,
            object_diameter: 80.0,
            mirror: false,
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
    /// mm de separación **medida** entre los extremos de las líneas de ida y de vuelta (el
    /// número que se lee al mirar el grabado de prueba). Al generar, cada línea se corrige la
    /// mitad: las de ida `-offset/2` y las de vuelta `+offset/2` en el sentido del barrido,
    /// así el grabado queda centrado sobre los vectores. (LightBurn pide aquí la mitad de lo
    /// medido, porque desplaza cada línea ese valor; en Tlaku se anota lo medido.)
    pub offset: f64,
}

/// Unidad en la que la interfaz muestra y pide las velocidades de la máquina. Solo es de
/// presentación: internamente (perfil, capas, G-code) todo sigue en mm/min.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum SpeedUnit {
    #[default]
    MmPerMin,
    MmPerSec,
}

/// Perfil de una máquina. Se deserializa con [`DeviceProfileRepr`]: si falta `dialect` se usa
/// el de su `kind` ([`GcodeDialect::for_kind`]), cosa que `serde(default)` no puede hacer.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub struct DeviceProfile {
    pub id: String,
    pub name: String,
    pub kind: DeviceKind,
    /// Cómo escribe el G-code este firmware; editable, sobre todo con `custom_gcode`.
    pub dialect: GcodeDialect,
    /// Ancho y alto de la cama, en mm.
    pub bed: [f64; 2],
    pub origin: Origin,
    /// Valor de S para 100 % de potencia (`$30`). Es la escala de `S` para todos los
    /// dialectos (1.0 en Smoothie y GRBL-LPC de fábrica, 255 en Marlin, 1000 en GRBL).
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
    pub z_axis: bool,
    /// Solo afecta al G-code. Convención interna: `z_offset` y `z_step` positivos enfocan más
    /// **hacia dentro del material** (el cabezal baja o la cama sube). Con `false`, el G-code
    /// sigue la convención de GRBL (+Z aleja el cabezal del material), así que una profundidad
    /// positiva sale como Z negativa; con `true` sale con el signo contrario (máquinas donde +Z
    /// acerca el cabezal, o la cama sube con +Z). Se comprueba mandando `z_offset = 1`: el foco
    /// debe entrar 1 mm en el material.
    pub z_invert: bool,
    pub rotary: RotarySettings,
    /// Ordenadas por velocidad; vacío = sin corrección.
    pub scan_offsets: Vec<ScanOffset>,
    /// mm que se corren todas las líneas de barrido (rellenos e imágenes, también
    /// unidireccionales) en el sentido del barrido, para alinear el grabado con los cortes
    /// vectoriales. Positivo = en el sentido de las líneas de ida.
    pub scan_initial_offset: f64,
    /// Cómo se conecta: puerto serie (el que se elige en el panel), red TCP/WebSocket o UDP.
    #[serde(default)]
    pub connection: Connection,
    /// Comandos guardados del usuario (botones en el panel Máquina).
    #[serde(default)]
    pub macros: Vec<Macro>,
    /// Posiciones guardadas de la cabeza, en coordenadas de la máquina.
    #[serde(default)]
    pub positions: Vec<SavedPosition>,
    /// Segundo láser o eje U (máquinas que los tengan); solo habilitan las opciones de la
    /// interfaz y del driver que los usan.
    #[serde(default)]
    pub laser2: bool,
    #[serde(default)]
    pub u_axis: bool,
    #[serde(default)]
    pub ruida: RuidaSettings,
    #[serde(default)]
    pub lhymicro: LhyMicroSettings,
    #[serde(default)]
    pub galvo: GalvoDevice,
    #[serde(default)]
    pub feeder: FeederSettings,
    /// Perfil que se activa al abrir la app (solo uno debería tenerlo; si hay varios gana el
    /// primero de la lista).
    #[serde(default)]
    pub is_default: bool,
    /// Ícono del perfil en la lista (nombre de un juego fijo de la interfaz); vacío = el del tipo.
    #[serde(default)]
    pub icon: String,
    /// Desfase `[x, y]` en mm entre el puntero rojo y el rayo, en el sistema del documento: el
    /// puntero cae en `rayo + desfase`. Lo usan el encuadre con puntero y «ir al puntero».
    #[serde(default)]
    pub pointer_offset: [f64; 2],
    /// Unidad con la que se muestran las velocidades (no cambia lo que se guarda ni el G-code).
    #[serde(default)]
    pub speed_unit: SpeedUnit,
    /// Los ejes de la máquina están cruzados: su X recorre el eje Y del documento y su Y el X.
    /// La salida de G-code y el encuadre intercambian X e Y; `bed` sigue en orientación de
    /// documento. No se combina con el rotativo (se ignora y se avisa).
    #[serde(default)]
    pub swap_xy: bool,
    /// Al conectar, pedir confirmación y hacer homing (`$H`).
    #[serde(default)]
    pub home_on_connect: bool,
    /// Abrir el puerto serie sin subir DTR para que la placa no se reinicie (si el sistema y la
    /// biblioteca serie lo permiten; en Linux el controlador lo sube siempre).
    #[serde(default)]
    pub no_dtr_reset: bool,
    /// Envío síncrono: una línea por `ok` en vez de conteo de caracteres. Más lento; sirve con
    /// placas que pierden bytes con el búfer lleno.
    #[serde(default)]
    pub sync_streaming: bool,
    /// Con `u_axis`: mm que se mueve el eje U al empezar cada trabajo (y que vuelve a deshacer al
    /// terminar). 0 = no se mueve.
    #[serde(default)]
    pub u_offset: f64,
    /// Potencia nominal del láser en W (22 en una Falcon2 Pro 22 W), si se conoce. Sirve para
    /// convertir a la máquina los valores de partida por watt de la vista previa por tonos. El
    /// asistente de máquinas la llena desde el catálogo; `None` = no se sabe.
    #[serde(default)]
    pub laser_watts: Option<f64>,
}

/// Transporte hacia la controladora.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
#[ts(export)]
pub enum Connection {
    /// Puerto serie o USB-serie; el puerto se elige al conectar.
    #[default]
    Serial,
    /// TCP (GRBL por red, FluidNC, grblHAL Ethernet). `port` suele ser 23.
    Tcp { host: String, port: u16 },
    /// WebSocket (FluidNC, ESP3D, algunos módulos WiFi).
    WebSocket { url: String },
    /// UDP (Ruida por red).
    Udp { host: String },
}

/// Comando guardado del usuario: una o varias líneas que se mandan tal cual.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub struct Macro {
    pub name: String,
    /// Una línea por comando.
    pub gcode: String,
    /// Pedir confirmación antes de enviarlo.
    pub confirm: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub struct SavedPosition {
    pub name: String,
    pub x: f64,
    pub y: f64,
}

/// Ajustes propios de Ruida.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case", default)]
#[ts(export)]
pub struct RuidaSettings {
    /// Byte de ofuscación; `None` = detectarlo al conectar (sondeo de solo lectura).
    pub magic: Option<u8>,
    /// Al enviar, ejecutar en vez de solo guardar en la memoria de la controladora.
    pub run_after_upload: bool,
}

/// Ajustes de las placas Lihuiyu (M2 Nano y afines).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case", default)]
#[ts(export)]
pub struct LhyMicroSettings {
    /// Modelo de placa (por ejemplo "M2", "M1", "B2"): cambia las ecuaciones de velocidad.
    pub board: String,
}

impl Default for LhyMicroSettings {
    fn default() -> Self {
        Self { board: "M2".into() }
    }
}

/// Forma en disco/red del perfil: igual que [`DeviceProfile`] pero con `dialect` opcional.
#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
struct DeviceProfileRepr {
    id: String,
    name: String,
    kind: DeviceKind,
    #[serde(default)]
    dialect: Option<GcodeDialect>,
    bed: [f64; 2],
    origin: Origin,
    s_max: f64,
    power_mode: PowerMode,
    accel: [f64; 2],
    max_rate: [f64; 2],
    rapid_rate: f64,
    baud: u32,
    rx_buffer: u32,
    start_gcode: String,
    end_gcode: String,
    return_to_origin: bool,
    #[serde(default)]
    z_axis: bool,
    #[serde(default)]
    z_invert: bool,
    #[serde(default)]
    rotary: RotarySettings,
    #[serde(default)]
    scan_offsets: Vec<ScanOffset>,
    #[serde(default)]
    scan_initial_offset: f64,
    #[serde(default)]
    connection: Connection,
    #[serde(default)]
    macros: Vec<Macro>,
    #[serde(default)]
    positions: Vec<SavedPosition>,
    #[serde(default)]
    laser2: bool,
    #[serde(default)]
    u_axis: bool,
    #[serde(default)]
    ruida: RuidaSettings,
    #[serde(default)]
    lhymicro: LhyMicroSettings,
    #[serde(default)]
    galvo: GalvoDevice,
    #[serde(default)]
    feeder: FeederSettings,
    #[serde(default)]
    is_default: bool,
    #[serde(default)]
    icon: String,
    #[serde(default)]
    pointer_offset: [f64; 2],
    #[serde(default)]
    speed_unit: SpeedUnit,
    #[serde(default)]
    swap_xy: bool,
    #[serde(default)]
    home_on_connect: bool,
    #[serde(default)]
    no_dtr_reset: bool,
    #[serde(default)]
    sync_streaming: bool,
    #[serde(default)]
    u_offset: f64,
    #[serde(default)]
    laser_watts: Option<f64>,
}

impl<'de> Deserialize<'de> for DeviceProfile {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        DeviceProfileRepr::deserialize(deserializer).map(Self::from)
    }
}

impl From<DeviceProfileRepr> for DeviceProfile {
    fn from(r: DeviceProfileRepr) -> Self {
        Self {
            dialect: r.dialect.unwrap_or_else(|| GcodeDialect::for_kind(r.kind)),
            id: r.id,
            name: r.name,
            kind: r.kind,
            bed: r.bed,
            origin: r.origin,
            s_max: r.s_max,
            power_mode: r.power_mode,
            accel: r.accel,
            max_rate: r.max_rate,
            rapid_rate: r.rapid_rate,
            baud: r.baud,
            rx_buffer: r.rx_buffer,
            start_gcode: r.start_gcode,
            end_gcode: r.end_gcode,
            return_to_origin: r.return_to_origin,
            z_axis: r.z_axis,
            z_invert: r.z_invert,
            rotary: r.rotary,
            scan_offsets: r.scan_offsets,
            scan_initial_offset: r.scan_initial_offset,
            connection: r.connection,
            macros: r.macros,
            positions: r.positions,
            laser2: r.laser2,
            u_axis: r.u_axis,
            ruida: r.ruida,
            lhymicro: r.lhymicro,
            galvo: r.galvo,
            feeder: r.feeder,
            is_default: r.is_default,
            icon: r.icon,
            pointer_offset: r.pointer_offset,
            speed_unit: r.speed_unit,
            swap_xy: r.swap_xy,
            home_on_connect: r.home_on_connect,
            no_dtr_reset: r.no_dtr_reset,
            sync_streaming: r.sync_streaming,
            u_offset: r.u_offset,
            laser_watts: r.laser_watts,
        }
    }
}

impl DeviceProfile {
    /// Perfil por defecto de un tipo de firmware: su dialecto, su `s_max`, su modo de potencia y su
    /// velocidad serie.
    /// Es lo que debe aplicar la interfaz al cambiar el `kind` de un perfil nuevo.
    pub fn for_kind(kind: DeviceKind) -> Self {
        let dialect = GcodeDialect::for_kind(kind);
        Self {
            kind,
            // Sin modo dinámico en el firmware (Marlin, Smoothie, GRBL-M3) la potencia es fija.
            power_mode: if dialect.dynamic_on.is_some() {
                PowerMode::Dynamic
            } else {
                PowerMode::Constant
            },
            dialect,
            s_max: kind.default_s_max(),
            baud: kind.default_baud(),
            ..Self::default()
        }
    }
}

impl Default for DeviceProfile {
    /// Láser de diodo con GRBL de fábrica y cama de 400x415 mm.
    fn default() -> Self {
        Self {
            id: "diodo-grbl-400x415".into(),
            name: "Diodo GRBL 400x415".into(),
            kind: DeviceKind::Grbl,
            dialect: GcodeDialect::default(),
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
            z_invert: false,
            rotary: RotarySettings::default(),
            scan_offsets: Vec::new(),
            scan_initial_offset: 0.0,
            connection: Connection::Serial,
            macros: Vec::new(),
            positions: Vec::new(),
            laser2: false,
            u_axis: false,
            ruida: RuidaSettings::default(),
            lhymicro: LhyMicroSettings::default(),
            galvo: GalvoDevice::default(),
            feeder: FeederSettings::default(),
            is_default: false,
            icon: String::new(),
            pointer_offset: [0.0, 0.0],
            speed_unit: SpeedUnit::default(),
            swap_xy: false,
            home_on_connect: false,
            no_dtr_reset: false,
            sync_streaming: false,
            u_offset: 0.0,
            laser_watts: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Un perfil como lo guardaba Tlaku antes de los dialectos, el Z invertido y la corrección
    /// inicial de barrido: sin `dialect`, `z_invert`, `scan_initial_offset` ni `rotary.mirror`.
    fn old_profile_json() -> serde_json::Value {
        serde_json::json!({
            "id": "viejo", "name": "Viejo", "kind": "grbl",
            "bed": [400.0, 415.0], "origin": "bottom_left", "s_max": 1000.0,
            "power_mode": "dynamic", "accel": [1000.0, 1000.0],
            "max_rate": [12000.0, 12000.0], "rapid_rate": 12000.0, "baud": 115200,
            "rx_buffer": 127, "start_gcode": "", "end_gcode": "", "return_to_origin": true,
            "z_axis": true,
            "rotary": {
                "enabled": true, "kind": "chuck", "mm_per_rotation": 360.0,
                "roller_diameter": 20.0, "object_diameter": 80.0
            },
            "scan_offsets": [{ "speed": 3000.0, "offset": 0.2 }]
        })
    }

    #[test]
    fn a_dialect_saved_without_dwell_reads_seconds_and_defaults_follow_the_kind() {
        let mut v = serde_json::to_value(GcodeDialect::marlin_inline()).unwrap();
        assert_eq!(v["dwell"], "p_milliseconds");
        v.as_object_mut().unwrap().remove("dwell");
        let d: GcodeDialect = serde_json::from_value(v).unwrap();
        assert_eq!(d.dwell, DwellFormat::PSeconds);
        for kind in [DeviceKind::Marlin, DeviceKind::Smoothie] {
            assert_eq!(
                GcodeDialect::for_kind(kind).dwell,
                DwellFormat::PMilliseconds
            );
        }
        for kind in [
            DeviceKind::Grbl,
            DeviceKind::GrblM3,
            DeviceKind::GrblHal,
            DeviceKind::FluidNc,
            DeviceKind::GrblLpc,
            DeviceKind::CustomGcode,
        ] {
            assert_eq!(GcodeDialect::for_kind(kind).dwell, DwellFormat::PSeconds);
        }
    }

    #[test]
    fn an_old_profile_loads_with_the_grbl_dialect_and_neutral_new_fields() {
        let d: DeviceProfile = serde_json::from_value(old_profile_json()).unwrap();
        assert_eq!(d.kind, DeviceKind::Grbl);
        assert_eq!(d.dialect, GcodeDialect::default());
        assert!(!d.z_invert && d.scan_initial_offset == 0.0 && !d.rotary.mirror);
        // Lo guardado se conserva (no se pisa con los valores de fábrica nuevos).
        assert_eq!(d.rotary.mm_per_rotation, 360.0);
        assert_eq!(d.scan_offsets.len(), 1);
        // Y sin `z_axis` ni `rotary` (más viejo aún) también carga.
        let mut v = old_profile_json();
        v.as_object_mut().unwrap().remove("z_axis");
        v.as_object_mut().unwrap().remove("rotary");
        v.as_object_mut().unwrap().remove("scan_offsets");
        let d: DeviceProfile = serde_json::from_value(v).unwrap();
        assert!(!d.z_axis && !d.rotary.enabled && d.scan_offsets.is_empty());
    }

    #[test]
    fn a_missing_dialect_follows_the_kind_but_a_saved_one_wins() {
        for (kind, name) in [
            (DeviceKind::GrblM3, "grbl_m3"),
            (DeviceKind::Smoothie, "smoothie"),
            (DeviceKind::Marlin, "marlin"),
            (DeviceKind::CustomGcode, "custom_gcode"),
        ] {
            let mut v = old_profile_json();
            v["kind"] = name.into();
            let d: DeviceProfile = serde_json::from_value(v.clone()).unwrap();
            assert_eq!(d.kind, kind);
            assert_eq!(d.dialect, GcodeDialect::for_kind(kind), "{name}");
            // Con dialecto guardado (aunque sea parcial) se respeta.
            v["dialect"] = serde_json::json!({ "laser_off": "M999" });
            let d: DeviceProfile = serde_json::from_value(v).unwrap();
            assert_eq!(d.dialect.laser_off, "M999");
            assert_eq!(d.dialect.constant_on, GcodeDialect::default().constant_on);
        }
    }

    #[test]
    fn a_profile_with_every_new_field_roundtrips() {
        let mut d = DeviceProfile::for_kind(DeviceKind::Marlin);
        d.z_axis = true;
        d.z_invert = true;
        d.scan_initial_offset = -0.15;
        d.rotary = RotarySettings {
            enabled: true,
            kind: RotaryKind::Chuck,
            mirror: true,
            ..RotarySettings::preset(RotaryKind::Chuck)
        };
        d.scan_offsets = vec![ScanOffset {
            speed: 6000.0,
            offset: 0.4,
        }];
        let json = serde_json::to_string(&d).unwrap();
        assert_eq!(serde_json::from_str::<DeviceProfile>(&json).unwrap(), d);
    }

    #[test]
    fn wave6c_fields_default_to_neutral_and_roundtrip() {
        // Un perfil viejo carga con todo neutro: nada cambia en su G-code ni en su conexión.
        let d: DeviceProfile = serde_json::from_value(old_profile_json()).unwrap();
        assert!(!d.is_default && d.icon.is_empty() && d.pointer_offset == [0.0, 0.0]);
        assert_eq!(d.speed_unit, SpeedUnit::MmPerMin);
        assert!(!d.swap_xy && !d.home_on_connect && !d.no_dtr_reset && !d.sync_streaming);
        assert_eq!(d.u_offset, 0.0);
        // Un perfil de antes no trae los watts del láser: no se sabe.
        assert_eq!(d.laser_watts, None);
        let d = DeviceProfile {
            is_default: true,
            icon: "printer".into(),
            pointer_offset: [1.5, -2.0],
            speed_unit: SpeedUnit::MmPerSec,
            swap_xy: true,
            home_on_connect: true,
            no_dtr_reset: true,
            sync_streaming: true,
            u_offset: 12.5,
            laser_watts: Some(22.0),
            ..DeviceProfile::default()
        };
        let json = serde_json::to_string(&d).unwrap();
        assert!(json.contains("\"speed_unit\":\"mm_per_sec\""));
        assert_eq!(serde_json::from_str::<DeviceProfile>(&json).unwrap(), d);
    }

    #[test]
    fn kind_presets_follow_the_documented_table() {
        let g = |k| GcodeDialect::for_kind(k);
        // Familia GRBL con modo láser: idéntica a GRBL 1.1.
        for k in [
            DeviceKind::GrblHal,
            DeviceKind::FluidNc,
            DeviceKind::GrblLpc,
        ] {
            assert_eq!(g(k), GcodeDialect::default());
        }
        // GRBL-M3: sin M4, G0 no apaga (S0 explícito) y S entera.
        let m3 = g(DeviceKind::GrblM3);
        assert!(m3.dynamic_on.is_none() && !m3.g0_laser_off && m3.travel_s0 && m3.s_decimals == 0);
        // Marlin: inline, F en G0, sin M4, cierre con M400 y líneas cortas.
        let marlin = g(DeviceKind::Marlin);
        assert_eq!(marlin.constant_on, "M3 S0 I");
        assert!(marlin.feed_on_g0 && marlin.dynamic_on.is_none() && marlin.max_line_len == 96);
        assert_eq!(marlin.end_sync.as_deref(), Some("M400"));
        // Perfil nuevo por tipo: potencia, escala y velocidad serie del firmware.
        let p = DeviceProfile::for_kind(DeviceKind::Smoothie);
        assert_eq!((p.s_max, p.power_mode), (1.0, PowerMode::Constant));
        let p = DeviceProfile::for_kind(DeviceKind::Marlin);
        assert_eq!((p.s_max, p.baud), (255.0, 250_000));
        let p = DeviceProfile::for_kind(DeviceKind::Grbl);
        assert_eq!(p, DeviceProfile::default());
    }

    #[test]
    fn rotary_defaults_are_a_real_accessory() {
        let r = RotarySettings::default();
        assert!(!r.enabled && !r.mirror);
        assert_eq!((r.mm_per_rotation, r.roller_diameter), (32.0, 17.0));
        assert_eq!(
            RotarySettings::preset(RotaryKind::Chuck).mm_per_rotation,
            40.0
        );
        // Un rotativo guardado sin `mirror` lo toma de los valores por defecto.
        let r: RotarySettings = serde_json::from_str(r#"{"enabled": true}"#).unwrap();
        assert!(r.enabled && !r.mirror && r.mm_per_rotation == 32.0);
    }
}
