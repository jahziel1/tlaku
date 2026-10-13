//! Perfiles de controladora: todo lo que distingue a un firmware de otro es dato aquí (banners,
//! `$I`, tabla de ajustes, tamaños de búfer, qué campos trae el reporte...) y no código aparte.
//!
//! Las fuentes de cada valor están en `docs/hardware/{grbl,falcon2,dialectos-gcode}.md`; lo que
//! esos documentos marcan como inferido o no verificado se anota en el comentario del campo.

use std::time::Duration;

/// Familia de firmware: solo se usa para los pocos comportamientos que no caben en un campo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Family {
    Grbl,
    GrblHal,
    FluidNc,
    GrblLpc,
    Falcon2,
    Marlin,
    Smoothie,
}

/// Estilo de protocolo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Protocol {
    /// Familia GRBL: conteo de caracteres, bytes en tiempo real, `$`.
    Grbl,
    /// Estilo RepRap (Marlin, Smoothie): una línea por `ok`, sin tiempo real ni `$`.
    PingPong,
}

/// Terminador de las respuestas de la controladora.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineEnding {
    CrLf,
    Lf,
}

impl LineEnding {
    pub fn bytes(self) -> &'static [u8] {
        match self {
            Self::CrLf => b"\r\n",
            Self::Lf => b"\n",
        }
    }
}

/// Un ajuste `$N` con su valor inicial y los decimales con que se imprime.
#[derive(Debug, Clone, PartialEq)]
pub struct Setting {
    pub id: u32,
    pub value: f64,
    pub decimals: u8,
}

impl Setting {
    pub fn new(id: u32, value: f64, decimals: u8) -> Self {
        Self {
            id,
            value,
            decimals,
        }
    }

    /// Texto del valor tal como lo imprime `$$`.
    pub fn format(&self) -> String {
        format_value(self.value, self.decimals)
    }
}

pub(crate) fn format_value(value: f64, decimals: u8) -> String {
    if decimals == 0 {
        format!("{}", value.round() as i64)
    } else {
        let v = if value.abs() < 0.5 * 10f64.powi(-i32::from(decimals)) {
            0.0
        } else {
            value
        };
        format!("{:.*}", usize::from(decimals), v)
    }
}

/// Descripción completa de una controladora emulada.
#[derive(Debug, Clone)]
pub struct Profile {
    pub name: String,
    pub family: Family,
    pub protocol: Protocol,
    /// Líneas que imprime al conectar (encendido o apertura del puerto).
    pub banner_connect: Vec<String>,
    /// Líneas que imprime tras un soft-reset (`0x18`).
    pub banner_reset: Vec<String>,
    /// Respuesta de `$I` (GRBL) o `M115` (RepRap), sin el `ok` final.
    pub info: Vec<String>,
    /// Tamaño declarado del búfer RX; el utilizable es uno menos (búfer circular).
    pub rx_size: usize,
    /// Bloques del planificador.
    pub planner_blocks: usize,
    /// Máximo de caracteres por línea (ya sin espacios ni comentarios); `error:11` si se
    /// supera. 80 en GRBL clásico es lo que recuerda el documento, **no verificado**.
    pub max_line_len: Option<usize>,
    /// Ajustes `$N` iniciales (los editables). Para los perfiles RepRap son internos.
    pub settings: Vec<Setting>,
    pub line_ending: LineEnding,
    /// Imprime `[MSG:'$H'|'$X' to unlock]` al arrancar en alarma.
    pub unlock_message: bool,
    /// Hay finales de carrera: el reporte trae `Pn:XY` en la esquina de homing.
    pub has_limit_switches: bool,
    /// Duración simulada de `$H`.
    pub homing_duration: Duration,
    /// Si `?` se contesta durante el homing (el documento dice que puede no hacerlo).
    pub status_during_home: bool,
    /// El `error:N` persiste en las líneas siguientes hasta línea vacía, `$` o reset (grblHAL
    /// con nivel de compatibilidad 0).
    pub persistent_error: bool,
    /// `M7` (mist) compilado.
    pub m7_enabled: bool,
    /// Bytes de tiempo real extra de grblHAL: `0x80`/`0x81`/`0x82` como `?`/`~`/`!`, `0x87`
    /// (reporte completo) y `0x83` (estado del parser).
    pub extended_realtime: bool,
    /// `FS:f,s,real` con tres valores (grblHAL).
    pub fs_three_values: bool,
    /// Estado `Alarm:N` con el código (grblHAL).
    pub alarm_substate: bool,
    /// `M6` entra en el estado `Tool` hasta `~` (grblHAL).
    pub tool_change_state: bool,
    /// Campos extra que grblHAL añade al reporte (`WCS:G54|H:0`) para probar tolerancia.
    pub extra_status_fields: bool,
    /// El reporte trae `Ov:` y `A:`.
    pub overrides_in_status: bool,
    /// El láser en M3 se apaga durante el feed hold (`$63` de grblHAL). GRBL clásico lo deja
    /// encendido: es el riesgo del punto 20 de `grbl.md`.
    pub laser_off_in_hold: bool,
    /// Interpretación de `$10`: bits de grblHAL (`true`) o los dos bits de GRBL 1.1 (`false`).
    pub hal_status_mask: bool,
    /// Marlin con `EMERGENCY_PARSER`: `M410`, `M112` y `M108` actúan al instante, aunque haya
    /// una línea bloqueada esperando sitio en la cola; `M0` pausa hasta `M108`. Sin él, `M410`
    /// y `M112` se encolan detrás de lo ya enviado y `M0` se ignora (sin LCD).
    pub emergency_parser: bool,
    /// Marlin heredado con el láser en una salida de ventilador (`M106 P<n> S<s>` / `M107 P<n>`):
    /// ese ventilador manda sobre el láser y **`G0` no lo apaga**. `None` = el láser va por
    /// `M3`/`M4`/`S` (lo normal).
    pub laser_fan: Option<u32>,
}

fn s(id: u32, value: f64, decimals: u8) -> Setting {
    Setting::new(id, value, decimals)
}

fn set(settings: &mut Vec<Setting>, id: u32, value: f64, decimals: u8) {
    match settings.iter_mut().find(|x| x.id == id) {
        Some(x) => {
            x.value = value;
            x.decimals = decimals;
        }
        None => {
            settings.push(Setting::new(id, value, decimals));
            settings.sort_by_key(|x| x.id);
        }
    }
}

/// Tabla típica de un láser de diodo GRBL 1.1 (vector P-1 de `grbl.md`).
fn classic_settings() -> Vec<Setting> {
    vec![
        s(0, 10.0, 0),
        s(1, 25.0, 0),
        s(2, 0.0, 0),
        s(3, 0.0, 0),
        s(4, 0.0, 0),
        s(5, 0.0, 0),
        s(6, 0.0, 0),
        s(10, 1.0, 0),
        s(11, 0.010, 3),
        s(12, 0.002, 3),
        s(13, 0.0, 0),
        s(20, 0.0, 0),
        s(21, 0.0, 0),
        s(22, 0.0, 0),
        s(23, 0.0, 0),
        s(24, 25.0, 3),
        s(25, 500.0, 3),
        s(26, 250.0, 0),
        s(27, 1.0, 3),
        s(30, 1000.0, 0),
        s(31, 0.0, 0),
        s(32, 1.0, 0),
        s(100, 80.0, 3),
        s(101, 80.0, 3),
        s(102, 250.0, 3),
        s(110, 6000.0, 3),
        s(111, 6000.0, 3),
        s(112, 500.0, 3),
        s(120, 1000.0, 3),
        s(121, 1000.0, 3),
        s(122, 10.0, 3),
        s(130, 400.0, 3),
        s(131, 400.0, 3),
        s(132, 200.0, 3),
    ]
}

fn lines(v: &[&str]) -> Vec<String> {
    v.iter().map(|x| (*x).to_string()).collect()
}

impl Profile {
    /// Base común de la familia GRBL; los constructores ajustan lo que cambia.
    fn grbl_base(name: &str, family: Family) -> Self {
        Self {
            name: name.to_string(),
            family,
            protocol: Protocol::Grbl,
            banner_connect: lines(&["Grbl 1.1h ['$' for help]"]),
            banner_reset: lines(&["Grbl 1.1h ['$' for help]"]),
            info: lines(&["[VER:1.1h.20190825:]", "[OPT:V,15,128]"]),
            rx_size: 128,
            planner_blocks: 15,
            max_line_len: Some(80),
            settings: classic_settings(),
            line_ending: LineEnding::CrLf,
            unlock_message: true,
            has_limit_switches: false,
            homing_duration: Duration::from_secs(3),
            status_during_home: false,
            persistent_error: false,
            m7_enabled: false,
            extended_realtime: false,
            fs_three_values: false,
            alarm_substate: false,
            tool_change_state: false,
            extra_status_fields: false,
            overrides_in_status: true,
            laser_off_in_hold: false,
            hal_status_mask: false,
            emergency_parser: false,
            laser_fan: None,
        }
    }

    /// GRBL 1.1h clásico sobre Arduino: banner `Grbl 1.1h`, `[OPT:V,15,128]` (búfer RX de 128
    /// con 127 utilizables, 15 bloques), `$32=1`.
    pub fn grbl_1_1h() -> Self {
        Self::grbl_base("GRBL 1.1h (Arduino)", Family::Grbl)
    }

    /// Creality Falcon2 Pro (GRBL 1.1f modificado, ESP32-S3): banner de siete líneas con las
    /// propias de Creality, `$I` con `[software versions:]`, RX de 65 536 bytes y 127 bloques
    /// (`Bf:127,65535`), `$10=3`, `$21=1`, `$22=1`, `$23=3`, `$150`-`$161` (volcado B de
    /// `falcon2.md`), homing con `Home` visible en el reporte y MPos creciendo hacia dentro.
    pub fn falcon2_pro() -> Self {
        let banner = lines(&[
            "usb connect succeed",
            "[OK]system start-up CV50-Pro-MASTER-Release V1.0.1 20240325-1042",
            "[OK] ILmp_app_init OK",
            "[OK] gas_init OK",
            "[MSG:Using cpu_map…esp32-s3]",
            "Grbl 1.1f ['$' for help]",
            "[OK] ILmp Link connect ok",
        ]);
        let mut settings = classic_settings();
        settings.retain(|x| !matches!(x.id, 102 | 112 | 122 | 132 | 11 | 12 | 4 | 5 | 6));
        for (id, v, d) in [
            (0, 10.0, 0),
            (1, 250.0, 0),
            (2, 0.0, 0),
            (3, 4.0, 0),
            (10, 3.0, 0),
            (13, 0.0, 0),
            (20, 0.0, 0),
            (21, 1.0, 0),
            (22, 1.0, 0),
            (23, 3.0, 0),
            (24, 2000.0, 3),
            (25, 4000.0, 3),
            (26, 20.0, 0),
            (27, 5.0, 3),
            (30, 1000.0, 0),
            (31, 0.0, 0),
            (32, 1.0, 0),
            (100, 80.0, 3),
            (101, 80.0, 3),
            (110, 50000.0, 3),
            (111, 50000.0, 3),
            (120, 5000.0, 3),
            (121, 5000.0, 3),
            (130, 400.0, 3),
            (131, 400.0, 3),
            (150, 36.0, 0),
            (151, 100.0, 0),
            (152, 30.0, 0),
            (153, 0.0, 0),
            (154, 1.0, 0),
            (155, 1.0, 0),
            (156, 0.0, 0),
            (157, 1.0, 0),
            (158, 1.0, 0),
            (160, 1.0, 0),
            (161, 13.0, 2),
        ] {
            set(&mut settings, id, v, d);
        }
        Self {
            banner_connect: banner.clone(),
            banner_reset: banner,
            info: lines(&[
                "[VER:1.1f.20220810:]",
                "[OPT:VHL,127,65536]",
                "[software versions: CV50-Pro-MASTER-Release V1.0.0 20231129-1024]",
                "inquire_info->power:22W ,CV50-LASER-22W-Release V3.0.5 20231129-1001",
            ]),
            rx_size: 65_536,
            planner_blocks: 127,
            max_line_len: None,
            settings,
            unlock_message: false,
            has_limit_switches: true,
            homing_duration: Duration::from_secs(8),
            status_during_home: true,
            overrides_in_status: false,
            ..Self::grbl_base("Creality Falcon2 Pro 22W", Family::Falcon2)
        }
    }

    /// grblHAL (nivel de compatibilidad 0): banner `GrblHAL`, búfer RX de 1024 y 35 bloques
    /// (forma de `[OPT:]` del vector I-4, valores inferidos), `FS` de tres valores, `Alarm:N`,
    /// estado `Tool`, error persistente, bytes de tiempo real extra y `$10` por bits.
    pub fn grbl_hal() -> Self {
        let mut settings = classic_settings();
        for (id, v, d) in [
            (10, 127.0, 0),
            (33, 5000.0, 0),
            (34, 0.0, 0),
            (35, 0.0, 0),
            (36, 100.0, 0),
            (63, 0.0, 0),
        ] {
            set(&mut settings, id, v, d);
        }
        Self {
            banner_connect: lines(&["GrblHAL 1.1f ['$' or '$HELP' for help]"]),
            banner_reset: lines(&["GrblHAL 1.1f ['$' or '$HELP' for help]"]),
            info: lines(&[
                "[VER:1.1f.20240101:]",
                "[OPT:VNM,35,1024,0]",
                "[NEWOPT:ENUMS,RT+]",
            ]),
            rx_size: 1024,
            planner_blocks: 35,
            max_line_len: None,
            settings,
            persistent_error: true,
            extended_realtime: true,
            fs_three_values: true,
            alarm_substate: true,
            tool_change_state: true,
            extra_status_fields: true,
            hal_status_mask: true,
            ..Self::grbl_base("grblHAL", Family::GrblHal)
        }
    }

    /// FluidNC 3.9.7: líneas `[MSG:INFO: ...]` antes del saludo `Grbl 3.9 [FluidNC ...]`,
    /// `$I` con `[OPT:MPHS]` (sin tamaños: no sirve para leer el búfer). El tamaño de búfer
    /// y de planificador es **[?]** en la documentación; se usan los de GRBL clásico.
    pub fn fluidnc() -> Self {
        let banner = lines(&[
            "[MSG:INFO: FluidNC v3.9.7]",
            "[MSG:INFO: Machine: Tlaku emulada]",
            "Grbl 3.9 [FluidNC v3.9.7 (wifi) '$' for help]",
        ]);
        Self {
            banner_connect: banner.clone(),
            banner_reset: banner,
            info: lines(&["[VER:3.9 FluidNC v3.9.7:]", "[OPT:MPHS]"]),
            max_line_len: None,
            ..Self::grbl_base("FluidNC 3.9.7", Family::FluidNc)
        }
    }

    /// GRBL-LPC (Smoothieboard, Cohesion3D): como GRBL 1.1 pero con `$30=1.0` por defecto, de
    /// modo que un trabajo con `S` hasta 1000 satura al 100 %. Banner y `[OPT:]` inferidos.
    pub fn grbl_lpc() -> Self {
        let mut settings = classic_settings();
        for (id, v, d) in [
            (30, 1.0, 3),
            (31, 0.0, 3),
            (33, 5000.0, 0),
            (34, 0.0, 0),
            (35, 0.0, 0),
            (36, 100.0, 0),
        ] {
            set(&mut settings, id, v, d);
        }
        Self {
            banner_connect: lines(&["Grbl 1.1f ['$' for help]"]),
            banner_reset: lines(&["Grbl 1.1f ['$' for help]"]),
            info: lines(&["[VER:1.1f.20170131:]", "[OPT:V,15,128]"]),
            settings,
            max_line_len: None,
            ..Self::grbl_base("GRBL-LPC", Family::GrblLpc)
        }
    }

    /// Base de los perfiles RepRap: ajustes internos (no visibles por `$$`) para el movimiento.
    fn reprap_base(name: &str, family: Family, s_max: f64, s_decimals: u8) -> Self {
        let mut settings = classic_settings();
        set(&mut settings, 30, s_max, s_decimals);
        set(&mut settings, 32, 1.0, 0);
        Self {
            protocol: Protocol::PingPong,
            rx_size: 128,
            planner_blocks: 16,
            max_line_len: Some(96),
            settings,
            line_ending: LineEnding::Lf,
            overrides_in_status: false,
            ..Self::grbl_base(name, family)
        }
    }

    /// Marlin 2.x con función láser, modo inline (`M3 S0 I`, `G1 ... S`): banner, `ok` por línea,
    /// `M115` (con `Cap:` de `EXTENDED_CAPABILITIES_REPORT`), `M114`, `M400`, el movimiento,
    /// `M106/M107`, `M410` (parada rápida, encolada), `M112` (paro total hasta reiniciar), `M0`
    /// ignorada (sin LCD ni `EMERGENCY_PARSER`) y las líneas numeradas con checksum
    /// (`N<n> ... *<xor>`) con `Error:` y `Resend: n` ante un fallo. Sin `?`, `!`, `~` ni `$`.
    /// Un comando desconocido da `echo:Unknown command` y `ok`.
    ///
    /// Inferido de la documentación (no de una placa): los textos exactos de `Error:`/`Resend:`
    /// y que `G0` apague el láser en modo estándar hasta el siguiente `M3`.
    pub fn marlin() -> Self {
        Self {
            banner_connect: lines(&["start", "echo:Marlin 2.1.2.5"]),
            banner_reset: lines(&["start", "echo:Marlin 2.1.2.5"]),
            info: lines(&[
                "FIRMWARE_NAME:Marlin 2.1.2.5 (Tlaku emulador) PROTOCOL_VERSION:1.0 MACHINE_TYPE:Laser EXTRUDER_COUNT:0",
                "Cap:AUTOREPORT_POS:0",
                "Cap:EMERGENCY_PARSER:0",
            ]),
            ..Self::reprap_base("Marlin 2.x (láser)", Family::Marlin, 255.0, 0)
        }
    }

    /// Marlin con `EMERGENCY_PARSER`: `M410`/`M112`/`M108` actúan al instante y `M0` pausa.
    pub fn marlin_emergency_parser() -> Self {
        let mut p = Self::marlin();
        p.name = "Marlin 2.x (láser, EMERGENCY_PARSER)".into();
        p.info[2] = "Cap:EMERGENCY_PARSER:1".into();
        p.emergency_parser = true;
        p
    }

    /// Marlin heredado con el láser en el ventilador 1 (`M106 P1 S<n>` / `M107 P1`): `G0` no
    /// lo apaga, de modo que el generador debe apagarlo antes de cada desplazamiento.
    pub fn marlin_fan_laser() -> Self {
        let mut p = Self::marlin();
        p.name = "Marlin 2.x (láser en M106 P1)".into();
        p.laser_fan = Some(1);
        p
    }

    /// Smoothieware con `laser_module` (`S` de 0 a 1): banner `Smoothie` (como lo ve LightBurn),
    /// `ok` por línea, `M115`, `M114`, `M400`, `M112` (paro: `!!` hasta `M999`) y `?`.
    ///
    /// La respuesta a `?` es `<Idle,MPos:x,y,z,WPos:x,y,z>` (formato con comas de GRBL 0.9,
    /// cuatro decimales), sin `ok`. **No está confirmada** por la documentación de Smoothie ni
    /// por una máquina real: es el formato que recuerdan los emisores de la familia y hay que
    /// contrastarlo. Pendiente: `M600/M601`, el módulo `switch`.
    pub fn smoothie() -> Self {
        Self {
            banner_connect: lines(&["Smoothie"]),
            banner_reset: lines(&["Smoothie"]),
            info: lines(&["FIRMWARE_NAME:Smoothieware, FIRMWARE_VERSION:1.1 (Tlaku emulador)"]),
            ..Self::reprap_base("Smoothieware", Family::Smoothie, 1.0, 3)
        }
    }

    /// Todos los perfiles disponibles.
    pub fn all() -> Vec<Self> {
        vec![
            Self::grbl_1_1h(),
            Self::falcon2_pro(),
            Self::grbl_hal(),
            Self::fluidnc(),
            Self::grbl_lpc(),
            Self::marlin(),
            Self::marlin_emergency_parser(),
            Self::marlin_fan_laser(),
            Self::smoothie(),
        ]
    }

    /// Valor inicial de un ajuste.
    pub fn setting(&self, id: u32) -> Option<f64> {
        self.settings.iter().find(|x| x.id == id).map(|x| x.value)
    }

    /// Cambia (o añade) un ajuste inicial.
    pub fn with_setting(mut self, id: u32, value: f64) -> Self {
        match self.settings.iter_mut().find(|x| x.id == id) {
            Some(x) => x.value = value,
            None => set(&mut self.settings, id, value, 0),
        }
        self
    }

    /// Primera línea del banner de reset que empieza por `Grbl`/`GrblHAL`, si la hay.
    pub fn greeting(&self) -> Option<&str> {
        self.banner_reset
            .iter()
            .map(String::as_str)
            .find(|l| l.to_ascii_lowercase().starts_with("grbl"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_profiles_have_banner_and_info() {
        for p in Profile::all() {
            assert!(!p.banner_connect.is_empty(), "{}", p.name);
            assert!(!p.info.is_empty(), "{}", p.name);
            assert!(p.rx_size >= 64);
        }
    }

    #[test]
    fn setting_formats() {
        assert_eq!(Setting::new(11, 0.01, 3).format(), "0.010");
        assert_eq!(Setting::new(30, 1000.0, 0).format(), "1000");
        assert_eq!(Setting::new(161, 13.0, 2).format(), "13.00");
        assert_eq!(Setting::new(1, -0.0, 3).format(), "0.000");
    }

    #[test]
    fn falcon_matches_dump_b() {
        let p = Profile::falcon2_pro();
        assert_eq!(p.setting(22), Some(1.0));
        assert_eq!(p.setting(23), Some(3.0));
        assert_eq!(p.setting(30), Some(1000.0));
        assert_eq!(p.setting(150), Some(36.0));
        assert_eq!(p.setting(102), None);
        assert_eq!(p.greeting(), Some("Grbl 1.1f ['$' for help]"));
    }
}
