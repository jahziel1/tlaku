//! Tabla de códigos de comando conocidos, con su fuente, su nivel de confianza y si Tlaku los
//! envía alguna vez.
//!
//! Los códigos y nombres vienen de galvoplotter (MIT) y de MeerK40t `balormk` (MIT), que
//! coinciden entre sí; el nombre `listJumpDelay` sustituye al `listMarkFreq` duplicado de las
//! tablas de edutechwiki (docs/hardware/galvo.md, sección 2.2). **Ningún código está verificado
//! con hardware.**
//!
//! La columna [`Policy`] es la regla de seguridad: solo se pueden construir y enviar los comandos
//! de `Policy::Sent` (ver `cmd` y `safety`). Escribir EEPROM, la tabla de corrección o los
//! parámetros de configuración de la tarjeta **no está implementado** y no hay forma de
//! construirlo desde la API.

/// Códigos de lista (byte alto `0x80`).
pub mod list {
    pub const JUMP_TO: u16 = 0x8001;
    pub const END_OF_LIST: u16 = 0x8002;
    pub const LASER_ON_POINT: u16 = 0x8003;
    pub const DELAY_TIME: u16 = 0x8004;
    pub const MARK_TO: u16 = 0x8005;
    pub const JUMP_SPEED: u16 = 0x8006;
    pub const LASER_ON_DELAY: u16 = 0x8007;
    pub const LASER_OFF_DELAY: u16 = 0x8008;
    pub const MARK_FREQ: u16 = 0x800A;
    pub const MARK_POWER_RATIO: u16 = 0x800B;
    pub const MARK_SPEED: u16 = 0x800C;
    pub const JUMP_DELAY: u16 = 0x800D;
    pub const POLYGON_DELAY: u16 = 0x800F;
    pub const WRITE_PORT: u16 = 0x8011;
    pub const MARK_CURRENT: u16 = 0x8012;
    pub const MARK_FREQ2: u16 = 0x8013;
    pub const FLY_ENABLE: u16 = 0x801A;
    pub const QSWITCH_PERIOD: u16 = 0x801B;
    pub const DIRECT_LASER_SWITCH: u16 = 0x801C;
    pub const FLY_DELAY: u16 = 0x801D;
    pub const SET_CO2_FPK: u16 = 0x801E;
    pub const FLY_WAIT_INPUT: u16 = 0x801F;
    pub const FIBER_OPEN_MO: u16 = 0x8021;
    pub const WAIT_FOR_INPUT: u16 = 0x8022;
    pub const CHANGE_MARK_COUNT: u16 = 0x8023;
    pub const SET_WELD_POWER_WAVE: u16 = 0x8024;
    pub const ENABLE_WELD_POWER_WAVE: u16 = 0x8025;
    pub const FIBER_YLPM_PULSE_WIDTH: u16 = 0x8026;
    pub const FLY_ENCODER_COUNT: u16 = 0x8028;
    pub const SET_DA_Z_WORD: u16 = 0x8029;
    pub const JPT_SET_PARAM: u16 = 0x8050;
    pub const READY_MARK: u16 = 0x8051;
}

/// Códigos inmediatos (byte alto `0x00`).
pub mod imm {
    pub const DISABLE_LASER: u16 = 0x0002;
    pub const ENABLE_LASER: u16 = 0x0004;
    pub const EXECUTE_LIST: u16 = 0x0005;
    pub const SET_PWM_PULSE_WIDTH: u16 = 0x0006;
    pub const GET_VERSION: u16 = 0x0007;
    pub const GET_SERIAL_NO: u16 = 0x0009;
    pub const GET_LIST_STATUS: u16 = 0x000A;
    pub const GET_POSITION_XY: u16 = 0x000C;
    pub const GOTO_XY: u16 = 0x000D;
    pub const LASER_SIGNAL_OFF: u16 = 0x000E;
    pub const LASER_SIGNAL_ON: u16 = 0x000F;
    pub const WRITE_COR_LINE: u16 = 0x0010;
    pub const RESET_LIST: u16 = 0x0012;
    pub const RESTART_LIST: u16 = 0x0013;
    pub const WRITE_COR_TABLE: u16 = 0x0015;
    pub const SET_CONTROL_MODE: u16 = 0x0016;
    pub const SET_DELAY_MODE: u16 = 0x0017;
    pub const SET_MAX_POLY_DELAY: u16 = 0x0018;
    pub const SET_END_OF_LIST: u16 = 0x0019;
    pub const SET_FIRST_PULSE_KILLER: u16 = 0x001A;
    pub const SET_LASER_MODE: u16 = 0x001B;
    pub const SET_TIMING: u16 = 0x001C;
    pub const SET_STANDBY: u16 = 0x001D;
    pub const SET_PWM_HALF_PERIOD: u16 = 0x001E;
    pub const STOP_EXECUTE: u16 = 0x001F;
    pub const STOP_LIST: u16 = 0x0020;
    pub const WRITE_PORT: u16 = 0x0021;
    pub const WRITE_ANALOG_PORT1: u16 = 0x0022;
    pub const WRITE_ANALOG_PORT2: u16 = 0x0023;
    pub const WRITE_ANALOG_PORT_X: u16 = 0x0024;
    pub const READ_PORT: u16 = 0x0025;
    pub const SET_AXIS_MOTION_PARAM: u16 = 0x0026;
    pub const SET_AXIS_ORIGIN_PARAM: u16 = 0x0027;
    pub const AXIS_GO_ORIGIN: u16 = 0x0028;
    pub const MOVE_AXIS_TO: u16 = 0x0029;
    pub const GET_AXIS_POS: u16 = 0x002A;
    pub const GET_FLY_WAIT_COUNT: u16 = 0x002B;
    pub const GET_MARK_COUNT: u16 = 0x002D;
    pub const SET_FPK_PARAM2: u16 = 0x002E;
    pub const FIBER_PULSE_WIDTH: u16 = 0x002F;
    pub const FIBER_GET_CONFIG_EXTEND: u16 = 0x0030;
    pub const INPUT_PORT: u16 = 0x0031;
    pub const SET_FLY_RES: u16 = 0x0032;
    pub const FIBER_SET_MO: u16 = 0x0033;
    pub const FIBER_GET_ST_MO_AP: u16 = 0x0034;
    pub const GET_USER_DATA: u16 = 0x0036;
    pub const GET_FLY_SPEED: u16 = 0x0038;
    pub const DISABLE_Z: u16 = 0x0039;
    pub const ENABLE_Z: u16 = 0x003A;
    pub const SET_Z_DATA: u16 = 0x003B;
    pub const SET_SPI_SIMMER_CURRENT: u16 = 0x003C;
    pub const RESET: u16 = 0x0040;
    pub const GET_MARK_TIME: u16 = 0x0041;
    pub const SET_FPK_PARAM: u16 = 0x0062;
}

/// Qué tan seguro es el comando y sus parámetros.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Confidence {
    /// Código y parámetros descritos por dos fuentes MIT que coinciden (galvoplotter y MeerK40t)
    /// y por las tablas públicas. Sin hardware propio, "conocido" no significa "verificado".
    Known,
    /// Se conoce el código y el nombre; los parámetros son parcialmente conocidos o hay una
    /// discrepancia entre fuentes (se anota en `notes`).
    Partial,
    /// Solo el nombre y el código: parámetros y efecto desconocidos.
    CodeOnly,
}

/// Qué hace el comando, para decidir si se puede enviar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Class {
    /// Solo consulta.
    Read,
    /// Movimiento de espejos con el láser apagado.
    Motion,
    /// Movimiento con el láser encendido o disparo en un punto.
    Fire,
    /// Parámetro de marcado dentro de una lista (velocidad, potencia, frecuencia, retardos).
    Param,
    /// Control de la ejecución de listas.
    Flow,
    /// Arma o desarma la fuente (habilitar láser, MO, puertos de salida).
    Arm,
    /// Escribe configuración o tablas de la tarjeta, o toca ejes y marcado al vuelo.
    Config,
    /// Emite o puede dañar: señal de láser directa, y todo lo desconocido.
    Direct,
}

/// Política de Tlaku para el comando.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Policy {
    /// Se construye y se envía (con las validaciones de `safety`).
    Sent,
    /// No se envía nunca desde Tlaku.
    NeverSent(&'static str),
}

#[derive(Debug, Clone, Copy)]
pub struct CodeInfo {
    pub code: u16,
    pub name: &'static str,
    pub confidence: Confidence,
    pub class: Class,
    pub policy: Policy,
    /// Fuente del código y de los parámetros.
    pub source: &'static str,
    pub notes: &'static str,
}

impl CodeInfo {
    pub fn is_list(&self) -> bool {
        self.code & 0x8000 != 0
    }
}

const GP: &str = "galvoplotter (MIT) + MeerK40t balormk (MIT)";
const GP_ONLY: &str = "galvoplotter (MIT)";
const NAMES: &str = "edutechwiki / galvoplotter: solo el nombre";
const CFG: Policy = Policy::NeverSent("configuración de la tarjeta: Tlaku no la escribe");
const AXES: Policy = Policy::NeverSent("ejes, Z y marcado al vuelo: fuera de alcance");

macro_rules! row {
    ($code:expr, $name:expr, $conf:ident, $class:ident, $policy:expr, $src:expr, $notes:expr) => {
        CodeInfo {
            code: $code,
            name: $name,
            confidence: Confidence::$conf,
            class: Class::$class,
            policy: $policy,
            source: $src,
            notes: $notes,
        }
    };
}

use Policy::Sent;

/// Toda la tabla, ordenada por código de lista y luego inmediatos.
pub static TABLE: &[CodeInfo] = &[
    row!(
        list::JUMP_TO,
        "listJumpTo",
        Known,
        Motion,
        Sent,
        GP,
        "x, y (u16 centrados en 0x8000), ángulo (0) y distancia en unidades (saturada a 0xFFFF)"
    ),
    row!(
        list::END_OF_LIST,
        "listEndOfList",
        Known,
        Flow,
        Sent,
        GP,
        "fin de la lista; también rellena la última tanda"
    ),
    row!(
        list::LASER_ON_POINT,
        "listLaserOnPoint",
        Known,
        Fire,
        Sent,
        GP,
        "dispara en el punto actual durante N x 10 us (máx. 60000 por comando en galvoplotter)"
    ),
    row!(
        list::DELAY_TIME,
        "listDelayTime",
        Known,
        Param,
        Sent,
        GP,
        "espera de N x 10 us con el láser apagado"
    ),
    row!(
        list::MARK_TO,
        "listMarkTo",
        Known,
        Fire,
        Sent,
        GP,
        "como listJumpTo pero con el láser encendido"
    ),
    row!(
        list::JUMP_SPEED,
        "listJumpSpeed",
        Known,
        Param,
        Sent,
        GP,
        "unidades galvo por ms = mm/s x unidades_por_mm / 1000, máx. 0xFFFF"
    ),
    row!(
        list::LASER_ON_DELAY,
        "listLaserOnDelay",
        Known,
        Param,
        Sent,
        GP,
        "|retardo| en us; segundo parámetro 0x8000 si es negativo"
    ),
    row!(
        list::LASER_OFF_DELAY,
        "listLaserOffDelay",
        Known,
        Param,
        Sent,
        GP,
        "igual que listLaserOnDelay"
    ),
    row!(
        list::MARK_FREQ,
        "listMarkFreq",
        Partial,
        Param,
        Sent,
        GP,
        "CO2 y UV: periodo = round(10000 / kHz). Edutechwiki lo asigna también a otros nombres"
    ),
    row!(
        list::MARK_POWER_RATIO,
        "listMarkPowerRatio",
        Partial,
        Param,
        Sent,
        GP,
        "CO2 y UV: round(200 x potencia% / kHz). Las fuentes lo llaman 'poco entendido'"
    ),
    row!(
        list::MARK_SPEED,
        "listMarkSpeed",
        Known,
        Param,
        Sent,
        GP,
        "misma conversión que listJumpSpeed"
    ),
    row!(
        list::JUMP_DELAY,
        "listJumpDelay",
        Known,
        Param,
        Sent,
        GP,
        "|retardo| en us más signo; se fija antes de cada salto según su distancia"
    ),
    row!(
        list::POLYGON_DELAY,
        "listPolygonDelay",
        Partial,
        Param,
        Sent,
        GP,
        "DISCREPANCIA: galvoplotter manda |us| y signo; MeerK40t manda us/10 en un parámetro. \
         Tlaku usa us/10, que ante la duda da la espera más corta"
    ),
    row!(
        list::WRITE_PORT,
        "listWritePort",
        Known,
        Arm,
        Sent,
        GP,
        "puertos de salida como máscara de 16 bits (bit 0 láser, bit 8 puntero rojo en las fuentes)"
    ),
    row!(
        list::MARK_CURRENT,
        "listMarkCurrent",
        Partial,
        Param,
        Sent,
        GP,
        "fibra: potencia = round(% x 0xFFF / 100). 'No correctamente entendido' en las fuentes"
    ),
    row!(
        list::MARK_FREQ2,
        "listMarkFreq2",
        CodeOnly,
        Param,
        Policy::NeverSent("parámetros desconocidos"),
        GP,
        "las fuentes lo dejan sin implementar"
    ),
    row!(
        list::FLY_ENABLE,
        "listFlyEnable",
        CodeOnly,
        Config,
        AXES,
        GP,
        ""
    ),
    row!(
        list::QSWITCH_PERIOD,
        "listQSwitchPeriod",
        Partial,
        Param,
        Sent,
        GP,
        "fibra: periodo = round(20000 / kHz) (unidades de 50 ns)"
    ),
    row!(
        list::DIRECT_LASER_SWITCH,
        "listDirectLaserSwitch",
        CodeOnly,
        Direct,
        Policy::NeverSent("conmuta el láser directamente"),
        GP,
        "sin implementar en las fuentes"
    ),
    row!(
        list::FLY_DELAY,
        "listFlyDelay",
        CodeOnly,
        Config,
        AXES,
        GP,
        ""
    ),
    row!(
        list::SET_CO2_FPK,
        "listSetCo2FPK",
        Partial,
        Param,
        Sent,
        GP,
        "CO2 y UV: first pulse killer = round(2000 / kHz), dos parámetros iguales"
    ),
    row!(
        list::FLY_WAIT_INPUT,
        "listFlyWaitInput",
        CodeOnly,
        Config,
        AXES,
        GP,
        ""
    ),
    row!(
        list::FIBER_OPEN_MO,
        "listFiberOpenMO",
        CodeOnly,
        Arm,
        Policy::NeverSent("el MO se abre con el inmediato FiberSetMo, que sí está en las fuentes"),
        GP,
        ""
    ),
    row!(
        list::WAIT_FOR_INPUT,
        "listWaitForInput",
        CodeOnly,
        Flow,
        Policy::NeverSent("espera de entradas: bits desconocidos"),
        GP,
        ""
    ),
    row!(
        list::CHANGE_MARK_COUNT,
        "listChangeMarkCount",
        CodeOnly,
        Config,
        CFG,
        GP,
        ""
    ),
    row!(
        list::SET_WELD_POWER_WAVE,
        "listSetWeldPowerWave",
        CodeOnly,
        Config,
        CFG,
        GP,
        "soldadura"
    ),
    row!(
        list::ENABLE_WELD_POWER_WAVE,
        "listEnableWeldPowerWave",
        CodeOnly,
        Config,
        CFG,
        GP,
        "soldadura"
    ),
    row!(
        list::FIBER_YLPM_PULSE_WIDTH,
        "listFiberYLPMPulseWidth",
        Partial,
        Param,
        Sent,
        GP,
        "ancho de pulso de fuentes MOPA/YLPM; la unidad se supone ns (INFERIDO: galvoplotter dice \
         'un valor típico es 4')"
    ),
    row!(
        list::FLY_ENCODER_COUNT,
        "listFlyEncoderCount",
        CodeOnly,
        Config,
        AXES,
        GP,
        ""
    ),
    row!(
        list::SET_DA_Z_WORD,
        "listSetDaZWord",
        CodeOnly,
        Config,
        AXES,
        GP,
        ""
    ),
    row!(
        list::JPT_SET_PARAM,
        "listJptSetParam",
        CodeOnly,
        Param,
        Policy::NeverSent("parámetros de fuentes JPT desconocidos"),
        GP,
        ""
    ),
    row!(
        list::READY_MARK,
        "listReadyMark",
        Known,
        Flow,
        Sent,
        GP,
        "primer comando de toda lista nueva"
    ),
    // --- inmediatos ---
    row!(
        imm::DISABLE_LASER,
        "DisableLaser",
        Partial,
        Arm,
        Sent,
        GP,
        "se envía al empezar, al terminar y en cualquier parada. Efecto exacto sin verificar"
    ),
    row!(
        imm::ENABLE_LASER,
        "EnableLaser",
        Partial,
        Arm,
        Sent,
        GP,
        "solo se envía justo antes de ejecutar un marcado; nunca en el encuadre"
    ),
    row!(
        imm::EXECUTE_LIST,
        "ExecuteList",
        Known,
        Flow,
        Sent,
        GP,
        "empieza a ejecutar la lista"
    ),
    row!(
        imm::SET_PWM_PULSE_WIDTH,
        "SetPwmPulseWidth",
        CodeOnly,
        Config,
        CFG,
        GP,
        ""
    ),
    row!(
        imm::GET_VERSION,
        "GetVersion",
        Known,
        Read,
        Sent,
        GP,
        "la palabra 3 de la respuesta lleva el estado (listo 0x20, ocupado 0x04, eje 0x40)"
    ),
    row!(
        imm::GET_SERIAL_NO,
        "GetSerialNo",
        Known,
        Read,
        Sent,
        GP,
        "4 palabras"
    ),
    row!(
        imm::GET_LIST_STATUS,
        "GetListStatus",
        Partial,
        Read,
        Sent,
        GP,
        "MeerK40t: palabra 3, bit 0x04 en marcha y 0x08 en pausa"
    ),
    row!(
        imm::GET_POSITION_XY,
        "GetPositionXY",
        Partial,
        Read,
        Sent,
        GP,
        "formato sin verificar"
    ),
    row!(
        imm::GOTO_XY,
        "GotoXY",
        Known,
        Motion,
        Sent,
        GP,
        "x, y, ángulo 0, distancia; mueve los espejos sin láser"
    ),
    row!(
        imm::LASER_SIGNAL_OFF,
        "LaserSignalOff",
        CodeOnly,
        Direct,
        Policy::NeverSent("señal de láser directa: se usa DisableLaser"),
        GP,
        ""
    ),
    row!(
        imm::LASER_SIGNAL_ON,
        "LaserSignalOn",
        CodeOnly,
        Direct,
        Policy::NeverSent("enciende la señal de láser sin lista: prohibido"),
        GP,
        ""
    ),
    row!(
        imm::WRITE_COR_LINE,
        "WriteCorLine",
        Known,
        Config,
        CFG,
        GP,
        "tabla de corrección: Tlaku corrige en el anfitrión y no escribe la tabla"
    ),
    row!(
        imm::RESET_LIST,
        "ResetList",
        Known,
        Flow,
        Sent,
        GP,
        "vacía la lista de la tarjeta"
    ),
    row!(
        imm::RESTART_LIST,
        "RestartList",
        Known,
        Flow,
        Policy::NeverSent("reanudar tras pausa: Tlaku no implementa pausa"),
        GP,
        ""
    ),
    row!(
        imm::WRITE_COR_TABLE,
        "WriteCorTable",
        Known,
        Config,
        CFG,
        GP,
        "tabla de corrección de la tarjeta: no se escribe"
    ),
    row!(
        imm::SET_CONTROL_MODE,
        "SetControlMode",
        Partial,
        Config,
        CFG,
        GP,
        "init de EZCAD"
    ),
    row!(
        imm::SET_DELAY_MODE,
        "SetDelayMode",
        Partial,
        Config,
        CFG,
        GP,
        "init de EZCAD"
    ),
    row!(
        imm::SET_MAX_POLY_DELAY,
        "SetMaxPolyDelay",
        CodeOnly,
        Config,
        CFG,
        GP,
        ""
    ),
    row!(
        imm::SET_END_OF_LIST,
        "SetEndOfList",
        Partial,
        Flow,
        Sent,
        GP,
        "galvoplotter lo manda con 0 tras cada tanda; efecto exacto sin verificar"
    ),
    row!(
        imm::SET_FIRST_PULSE_KILLER,
        "SetFirstPulseKiller",
        Partial,
        Config,
        CFG,
        GP,
        "init de EZCAD"
    ),
    row!(
        imm::SET_LASER_MODE,
        "SetLaserMode",
        Partial,
        Config,
        CFG,
        GP,
        "init de EZCAD"
    ),
    row!(
        imm::SET_TIMING,
        "SetTiming",
        Partial,
        Config,
        CFG,
        GP,
        "init de EZCAD"
    ),
    row!(
        imm::SET_STANDBY,
        "SetStandby",
        Partial,
        Config,
        CFG,
        GP,
        "init de EZCAD"
    ),
    row!(
        imm::SET_PWM_HALF_PERIOD,
        "SetPwmHalfPeriod",
        Partial,
        Config,
        CFG,
        GP,
        "init de EZCAD"
    ),
    row!(
        imm::STOP_EXECUTE,
        "StopExecute",
        Known,
        Flow,
        Sent,
        GP,
        "detiene la ejecución"
    ),
    row!(
        imm::STOP_LIST,
        "StopList",
        Known,
        Flow,
        Policy::NeverSent("pausa: en reportes de usuarios el haz queda encendido"),
        GP,
        ""
    ),
    row!(
        imm::WRITE_PORT,
        "WritePort",
        Known,
        Arm,
        Sent,
        GP,
        "salidas como máscara de 16 bits; solo se permiten 0, el bit del láser o el del puntero"
    ),
    row!(
        imm::WRITE_ANALOG_PORT1,
        "WriteAnalogPort1",
        Partial,
        Config,
        CFG,
        GP,
        "init de EZCAD"
    ),
    row!(
        imm::WRITE_ANALOG_PORT2,
        "WriteAnalogPort2",
        CodeOnly,
        Config,
        CFG,
        GP,
        ""
    ),
    row!(
        imm::WRITE_ANALOG_PORT_X,
        "WriteAnalogPortX",
        CodeOnly,
        Config,
        CFG,
        GP,
        ""
    ),
    row!(
        imm::READ_PORT,
        "ReadPort",
        Known,
        Read,
        Sent,
        GP,
        "las entradas llegan en la palabra 1 de la respuesta"
    ),
    row!(
        imm::SET_AXIS_MOTION_PARAM,
        "SetAxisMotionParam",
        CodeOnly,
        Config,
        AXES,
        GP,
        ""
    ),
    row!(
        imm::SET_AXIS_ORIGIN_PARAM,
        "SetAxisOriginParam",
        CodeOnly,
        Config,
        AXES,
        GP,
        ""
    ),
    row!(
        imm::AXIS_GO_ORIGIN,
        "AxisGoOrigin",
        CodeOnly,
        Config,
        AXES,
        GP,
        ""
    ),
    row!(
        imm::MOVE_AXIS_TO,
        "MoveAxisTo",
        CodeOnly,
        Config,
        AXES,
        GP,
        ""
    ),
    row!(
        imm::GET_AXIS_POS,
        "GetAxisPos",
        CodeOnly,
        Read,
        AXES,
        GP,
        ""
    ),
    row!(
        imm::GET_FLY_WAIT_COUNT,
        "GetFlyWaitCount",
        CodeOnly,
        Read,
        AXES,
        GP,
        ""
    ),
    row!(
        imm::GET_MARK_COUNT,
        "GetMarkCount",
        CodeOnly,
        Read,
        Policy::NeverSent("formato desconocido"),
        GP,
        ""
    ),
    row!(
        imm::SET_FPK_PARAM2,
        "SetFpkParam2",
        Partial,
        Config,
        CFG,
        GP,
        "init de EZCAD"
    ),
    row!(
        imm::FIBER_PULSE_WIDTH,
        "FiberPulseWidth",
        CodeOnly,
        Config,
        CFG,
        GP,
        ""
    ),
    row!(
        imm::FIBER_GET_CONFIG_EXTEND,
        "FiberGetConfigExtend",
        CodeOnly,
        Read,
        Policy::NeverSent("formato desconocido"),
        GP,
        ""
    ),
    row!(
        imm::INPUT_PORT,
        "InputPort",
        CodeOnly,
        Config,
        CFG,
        GP,
        "bloqueo de entradas"
    ),
    row!(
        imm::SET_FLY_RES,
        "SetFlyRes",
        Partial,
        Config,
        AXES,
        GP,
        "init de EZCAD"
    ),
    row!(
        imm::FIBER_SET_MO,
        "Fiber_SetMo",
        Partial,
        Arm,
        Sent,
        GP,
        "0 cierra y 1 abre el oscilador maestro (MO) de la fibra. Solo fuentes de fibra"
    ),
    row!(
        imm::FIBER_GET_ST_MO_AP,
        "Fiber_GetStMO_AP",
        CodeOnly,
        Read,
        Policy::NeverSent("formato desconocido"),
        NAMES,
        ""
    ),
    row!(
        imm::GET_USER_DATA,
        "GetUserData",
        CodeOnly,
        Read,
        Policy::NeverSent("formato desconocido"),
        GP_ONLY,
        ""
    ),
    row!(
        imm::GET_FLY_SPEED,
        "GetFlySpeed",
        CodeOnly,
        Read,
        AXES,
        GP_ONLY,
        ""
    ),
    row!(imm::DISABLE_Z, "DisableZ", CodeOnly, Config, AXES, GP, ""),
    row!(
        imm::ENABLE_Z,
        "EnableZ",
        CodeOnly,
        Config,
        AXES,
        GP,
        "init de EZCAD"
    ),
    row!(imm::SET_Z_DATA, "SetZData", CodeOnly, Config, AXES, GP, ""),
    row!(
        imm::SET_SPI_SIMMER_CURRENT,
        "SetSPISimmerCurrent",
        CodeOnly,
        Config,
        CFG,
        GP,
        ""
    ),
    row!(
        imm::RESET,
        "Reset",
        CodeOnly,
        Config,
        Policy::NeverSent("reinicia estado de la tarjeta; efecto desconocido"),
        GP,
        "init de EZCAD"
    ),
    row!(
        imm::GET_MARK_TIME,
        "GetMarkTime",
        Partial,
        Read,
        Policy::NeverSent("formato desconocido"),
        GP,
        "siempre con parámetro 3"
    ),
    row!(
        imm::SET_FPK_PARAM,
        "SetFpkParam",
        CodeOnly,
        Config,
        CFG,
        NAMES,
        ""
    ),
];

/// Busca un código en la tabla.
pub fn lookup(code: u16) -> Option<&'static CodeInfo> {
    TABLE.iter().find(|c| c.code == code)
}

/// Nombre del código, o `"?"` si no se conoce.
pub fn name(code: u16) -> &'static str {
    lookup(code).map_or("?", |c| c.name)
}

/// `true` si Tlaku puede enviar este código (conocido y con `Policy::Sent`). Todo lo
/// desconocido, incluido cualquier comando de escritura de EEPROM, es falso.
pub fn is_sent(code: u16) -> bool {
    lookup(code).is_some_and(|c| c.policy == Policy::Sent)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn codes_are_unique_and_the_flag_matches_the_range() {
        let mut seen = HashSet::new();
        for c in TABLE {
            assert!(seen.insert(c.code), "código repetido {:#06x}", c.code);
            assert_eq!(c.is_list(), c.code >> 8 == 0x80, "{}", c.name);
            assert!(c.is_list() || c.code >> 8 == 0, "{}", c.name);
        }
    }

    #[test]
    fn nothing_that_configures_or_fires_directly_is_sent() {
        for c in TABLE {
            if matches!(c.class, Class::Config | Class::Direct) {
                assert!(
                    matches!(c.policy, Policy::NeverSent(_)),
                    "{} no debe enviarse",
                    c.name
                );
            }
        }
        // Escritura de EEPROM: no hay código conocido, y lo desconocido no se envía.
        assert!(!is_sent(0x0042));
        assert!(!is_sent(0xFFFF));
        assert!(!is_sent(imm::WRITE_COR_TABLE));
        assert!(!is_sent(imm::LASER_SIGNAL_ON));
        assert!(!is_sent(imm::RESET));
    }

    #[test]
    fn the_sent_set_is_exactly_the_documented_one() {
        let mut sent: Vec<&str> = TABLE
            .iter()
            .filter(|c| c.policy == Policy::Sent)
            .map(|c| c.name)
            .collect();
        sent.sort_unstable();
        let mut want = vec![
            "listJumpTo",
            "listEndOfList",
            "listLaserOnPoint",
            "listDelayTime",
            "listMarkTo",
            "listJumpSpeed",
            "listLaserOnDelay",
            "listLaserOffDelay",
            "listMarkFreq",
            "listMarkPowerRatio",
            "listMarkSpeed",
            "listJumpDelay",
            "listPolygonDelay",
            "listWritePort",
            "listMarkCurrent",
            "listQSwitchPeriod",
            "listSetCo2FPK",
            "listFiberYLPMPulseWidth",
            "listReadyMark",
            "DisableLaser",
            "EnableLaser",
            "ExecuteList",
            "GetVersion",
            "GetSerialNo",
            "GetListStatus",
            "GetPositionXY",
            "GotoXY",
            "ResetList",
            "SetEndOfList",
            "StopExecute",
            "WritePort",
            "ReadPort",
            "Fiber_SetMo",
        ];
        want.sort_unstable();
        assert_eq!(sent, want);
    }

    #[test]
    fn lookups_return_names() {
        assert_eq!(name(0x8005), "listMarkTo");
        assert_eq!(name(0x0004), "EnableLaser");
        assert_eq!(name(0x1234), "?");
    }
}
