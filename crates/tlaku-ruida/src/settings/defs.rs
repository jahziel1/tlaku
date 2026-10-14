//! Lista cerrada de ajustes de la controladora que Tlaku puede escribir, con su rango.
//!
//! **EXPERIMENTAL: sin probar con una controladora real.** Cada dirección y cada nombre salen de
//! fuentes públicas con licencia MIT o de documentación en prosa; los **rangos son cotas de
//! seguridad propias de Tlaku**, no límites del firmware (ninguna fuente los da). Una dirección
//! que no está aquí no se escribe nunca, aunque exista en la memoria de la máquina.
//!
//! Fuentes (ver `docs/hardware/ruida.md` sección 16 y `THIRD_PARTY.md`):
//! - [RPAC] ruida-pa, `protocols/ruida/ruida_protocol.py`, tabla `MT` (MIT): nombres
//!   (`MEM_PRECISION_X`...) y tipo (`dim` en µm, `speed` en µm/s) de las direcciones por eje,
//!   marcadas allí como verificadas en una RDC8445S; `MEM_G0_VELOCITY` y `MEM_HOME_VELOCITY`
//!   sin marca; `MEM_ROTATE_PULSE` y `MEM_ROTATE_D` sin tipo (`TBDU35`).
//! - [EDU] https://edutechwiki.unige.ch/en/Ruida, tabla de propiedades (`DA 00` / `DA 01`):
//!   mismos nombres y direcciones; coincide con [RPAC].
//! - [MK40] MeerK40t, `meerk40t/ruida/emulator.py`: valores de ejemplo en µm/s para `0x005`
//!   (200 000) y la forma de escritura `DA 01 dirección v v` (`rdjob.py`, `set_setting`).

use super::SettingsError;

/// Cómo interpretar y mostrar el valor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingKind {
    /// Velocidad en µm/s (el valor de 35 bits es µm/s: [RPAC] `speed`, [MK40]).
    Speed,
    /// Aceleración en µm/s².
    Accel,
    /// Longitud o desplazamiento en µm ([RPAC] `dim`).
    Length,
    /// Longitud de un paso del eje ("Axis Precision"; [RPAC] la muestra como `dim`).
    Step,
    /// Valor sin unidad documentada (el rotativo): se muestra y se escribe tal cual.
    Raw,
}

/// Grupo para presentarlos en la interfaz.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingGroup {
    General,
    AxisX,
    AxisY,
    AxisZ,
    AxisU,
    Rotary,
}

impl SettingGroup {
    pub fn key(self) -> &'static str {
        match self {
            SettingGroup::General => "general",
            SettingGroup::AxisX => "axis_x",
            SettingGroup::AxisY => "axis_y",
            SettingGroup::AxisZ => "axis_z",
            SettingGroup::AxisU => "axis_u",
            SettingGroup::Rotary => "rotary",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SettingDef {
    /// Dirección de 14 bits (`DA 00 hi lo`; p. ej. `0x023` = bytes `00 23`).
    pub addr: u16,
    /// Clave estable (archivo de respaldo y textos de la interfaz).
    pub key: &'static str,
    pub kind: SettingKind,
    pub group: SettingGroup,
    /// El valor de 35 bits se interpreta con signo (complemento a dos).
    pub signed: bool,
    /// Rango permitido (inclusive), en la unidad de `kind`. **Cota propia de Tlaku.**
    pub min: i64,
    pub max: i64,
    /// Nombre del dato en la fuente.
    pub source_name: &'static str,
}

#[allow(clippy::too_many_arguments)]
const fn d(
    addr: u16,
    key: &'static str,
    kind: SettingKind,
    group: SettingGroup,
    signed: bool,
    min: i64,
    max: i64,
    source_name: &'static str,
) -> SettingDef {
    SettingDef {
        addr,
        key,
        kind,
        group,
        signed,
        min,
        max,
        source_name,
    }
}

use SettingGroup::{AxisU, AxisX, AxisY, AxisZ, General, Rotary};
use SettingKind::{Accel, Length, Raw, Speed, Step};

/// 600 mm/s: el tope de velocidad que usa todo Tlaku para Ruida (`safety::MAX_SPEED_MM_MIN`).
const SPEED_MAX: i64 = 600_000;
/// Velocidades de arranque y de botón: 100 mm/s como cota.
const SLOW_MAX: i64 = 100_000;
/// 50 m/s² como cota de las aceleraciones.
const ACCEL_MAX: i64 = 50_000_000;
/// Recorrido de un eje: de 10 mm a 3 m.
const TRAVEL_MIN: i64 = 10_000;
const TRAVEL_MAX: i64 = 3_000_000;
/// Desplazamiento de home: hasta ±1 m.
const OFFSET_MAX: i64 = 1_000_000;
/// Paso de un eje: cota absoluta amplia; el cambio relativo lo limita [`super::plan`].
const STEP_MAX: i64 = 1_000_000;

/// Los ajustes escribibles, en el orden en que se leen, se respaldan y se muestran.
pub static WRITABLE: &[SettingDef] = &[
    // Generales: [RPAC] MT 0x00/0x05 y 0x00/0x0C; [EDU] "G0 Velocity", "Home Velocity".
    d(
        0x005,
        "g0_speed",
        Speed,
        General,
        false,
        1_000,
        SPEED_MAX,
        "MEM_G0_VELOCITY",
    ),
    d(
        0x00C,
        "home_speed",
        Speed,
        General,
        false,
        1_000,
        SPEED_MAX,
        "MEM_HOME_VELOCITY",
    ),
    // Eje X: [RPAC] MT 0x00/0x21..0x2A; [EDU] "Axis ... 1".
    d(
        0x021,
        "x_step",
        Step,
        AxisX,
        false,
        1,
        STEP_MAX,
        "MEM_PRECISION_X",
    ),
    d(
        0x023,
        "x_max_speed",
        Speed,
        AxisX,
        false,
        1_000,
        SPEED_MAX,
        "MEM_MAX_VELOCITY_X",
    ),
    d(
        0x024,
        "x_start_speed",
        Speed,
        AxisX,
        false,
        0,
        SLOW_MAX,
        "MEM_START_VELOCITY_X",
    ),
    d(
        0x025,
        "x_max_accel",
        Accel,
        AxisX,
        false,
        1_000,
        ACCEL_MAX,
        "MEM_MAX_ACC_X",
    ),
    d(
        0x026,
        "x_travel",
        Length,
        AxisX,
        false,
        TRAVEL_MIN,
        TRAVEL_MAX,
        "MEM_BED_SIZE_X",
    ),
    d(
        0x027,
        "x_button_speed",
        Speed,
        AxisX,
        false,
        0,
        SLOW_MAX,
        "MEM_BTN_START_VEL_X",
    ),
    d(
        0x028,
        "x_button_accel",
        Accel,
        AxisX,
        false,
        1_000,
        ACCEL_MAX,
        "MEM_BTN_ACC_X",
    ),
    d(
        0x029,
        "x_estop_accel",
        Accel,
        AxisX,
        false,
        1_000,
        ACCEL_MAX,
        "MEM_ESTP_ACC_X",
    ),
    d(
        0x02A,
        "x_home_offset",
        Length,
        AxisX,
        true,
        -OFFSET_MAX,
        OFFSET_MAX,
        "MEM_HOME_OFFSET_X",
    ),
    // Eje Y: [RPAC] MT 0x00/0x31..0x3A; [EDU] "Axis ... 2".
    d(
        0x031,
        "y_step",
        Step,
        AxisY,
        false,
        1,
        STEP_MAX,
        "MEM_PRECISION_Y",
    ),
    d(
        0x033,
        "y_max_speed",
        Speed,
        AxisY,
        false,
        1_000,
        SPEED_MAX,
        "MEM_MAX_VELOCITY_Y",
    ),
    d(
        0x034,
        "y_start_speed",
        Speed,
        AxisY,
        false,
        0,
        SLOW_MAX,
        "MEM_START_VELOCITY_Y",
    ),
    d(
        0x035,
        "y_max_accel",
        Accel,
        AxisY,
        false,
        1_000,
        ACCEL_MAX,
        "MEM_MAX_ACC_Y",
    ),
    d(
        0x036,
        "y_travel",
        Length,
        AxisY,
        false,
        TRAVEL_MIN,
        TRAVEL_MAX,
        "MEM_BED_SIZE_Y",
    ),
    d(
        0x037,
        "y_button_speed",
        Speed,
        AxisY,
        false,
        0,
        SLOW_MAX,
        "MEM_BTN_START_VEL_Y",
    ),
    d(
        0x038,
        "y_button_accel",
        Accel,
        AxisY,
        false,
        1_000,
        ACCEL_MAX,
        "MEM_BTN_ACC_Y",
    ),
    d(
        0x039,
        "y_estop_accel",
        Accel,
        AxisY,
        false,
        1_000,
        ACCEL_MAX,
        "MEM_ESTP_ACC_Y",
    ),
    d(
        0x03A,
        "y_home_offset",
        Length,
        AxisY,
        true,
        -OFFSET_MAX,
        OFFSET_MAX,
        "MEM_HOME_OFFSET_Y",
    ),
    // Z y U: solo el paso (calibración); [RPAC] MT 0x00/0x41 y 0x51.
    d(
        0x041,
        "z_step",
        Step,
        AxisZ,
        false,
        1,
        STEP_MAX,
        "MEM_PRECISION_Z",
    ),
    d(
        0x051,
        "u_step",
        Step,
        AxisU,
        false,
        1,
        STEP_MAX,
        "MEM_PRECISION_U",
    ),
    // Rotativo: [RPAC] MT 0x02/0x1F `MEM_ROTATE_PULSE` y 0x02/0x21 `MEM_ROTATE_D` (sin tipo);
    // [EDU] "Rotate Pulse" y "Rotate D". Ninguna fuente dice la unidad ni cómo se activa.
    d(
        0x11F,
        "rotary_pulse",
        Raw,
        Rotary,
        false,
        1,
        100_000_000,
        "MEM_ROTATE_PULSE",
    ),
    d(
        0x121,
        "rotary_diameter",
        Raw,
        Rotary,
        false,
        1,
        100_000_000,
        "MEM_ROTATE_D",
    ),
];

/// Máximo de 35 bits.
pub const MAX_RAW: u64 = (1 << 35) - 1;

/// Busca la definición de una dirección; `None` si no está en la lista cerrada.
pub fn find(addr: u16) -> Option<&'static SettingDef> {
    WRITABLE.iter().find(|d| d.addr == addr)
}

/// Busca por clave.
pub fn find_key(key: &str) -> Option<&'static SettingDef> {
    WRITABLE.iter().find(|d| d.key == key)
}

impl SettingDef {
    /// Valor de 35 bits leído de la máquina a número (con signo si corresponde).
    pub fn decode(&self, raw: u64) -> i64 {
        let raw = raw & MAX_RAW;
        if self.signed && raw & (1 << 34) != 0 {
            raw as i64 - (1 << 35)
        } else {
            raw as i64
        }
    }

    /// Valida `value` contra la lista cerrada y devuelve el valor de 35 bits que se escribe.
    pub fn encode(&self, value: i64) -> Result<u64, SettingsError> {
        if value < self.min || value > self.max {
            return Err(SettingsError::OutOfRange {
                addr: self.addr,
                key: self.key,
                value,
                min: self.min,
                max: self.max,
            });
        }
        Ok((value as u64) & MAX_RAW)
    }
}

/// Valida `value` para la dirección `addr`: error si la dirección no está en la lista cerrada o
/// el valor sale de su rango. Devuelve el valor de 35 bits que se escribiría.
pub fn check_write(addr: u16, value: i64) -> Result<u64, SettingsError> {
    find(addr)
        .ok_or(SettingsError::NotWritable { addr })?
        .encode(value)
}
