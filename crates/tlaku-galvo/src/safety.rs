//! Límites del perfil, validación de listas y secuencia de apagado.
//!
//! Este módulo es la **segunda línea de defensa**: `compile` ya respeta los límites, pero aquí se
//! revisan los bytes resultantes sin confiar en quien los generó, y la sesión no envía nada que no
//! haya pasado por aquí. Reglas (docs/hardware/galvo.md, sección 5):
//!
//! - una lista empieza con `listReadyMark`, termina en `listEndOfList` y no lleva nada después
//!   (salvo el relleno de la tanda);
//! - ningún `MarkTo` ni `LaserOnPoint` antes de haber fijado velocidad, retardos, frecuencia,
//!   potencia (mayor que cero) y, en MOPA, el ancho de pulso; y antes de habilitar el puerto del
//!   láser en la propia lista;
//! - potencia, frecuencia y ancho de pulso dentro del perfil; un dwell continuo acotado;
//! - los códigos que no están en la lista blanca (`codes::is_sent`), incluidos los de
//!   configuración, tabla de corrección y EEPROM, se rechazan siempre;
//! - una lista de encuadre (luz roja) no puede llevar marcas, potencia ni el bit del láser.

use crate::cmd::{Imm, ListOp};
use crate::codes;
use crate::params;
use crate::wire::{Kind, Raw};
use std::fmt;
use tlaku_doc::{GalvoDevice, LaserSource};

/// Dwell continuo máximo por defecto (ms).
pub const DEFAULT_MAX_DWELL_MS: f64 = 5000.0;
/// Máximo de comandos de una lista (unos 50 MB).
pub const DEFAULT_MAX_COMMANDS: usize = 4_000_000;
/// Bit del puerto de salida que habilita el láser (galvoplotter `laser_pin = 0`; sin verificar).
pub const DEFAULT_LASER_BIT: u8 = 0;
/// Bit del puerto de salida del puntero rojo (galvoplotter `light_pin = 8`; sin verificar).
pub const DEFAULT_LIGHT_BIT: u8 = 8;

/// Qué clase de lista se valida.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListKind {
    /// Marcado real: el puerto del láser se habilita en la propia lista.
    Mark,
    /// Mismo trabajo pero con el láser deshabilitado y el puntero rojo encendido.
    DryRun,
    /// Encuadre: solo saltos y puntero rojo.
    Light,
}

/// Límites que salen del perfil de la máquina.
#[derive(Debug, Clone, PartialEq)]
pub struct SafetyLimits {
    pub source: LaserSource,
    pub freq_min_khz: f64,
    pub freq_max_khz: f64,
    /// Anchos de pulso permitidos (ns). Obligatorio que el valor esté en la lista.
    pub pulse_widths_ns: Vec<u32>,
    pub max_power_pct: f64,
    pub max_dwell_ms: f64,
    pub max_commands: usize,
    pub laser_bit: u8,
    pub light_bit: u8,
}

impl SafetyLimits {
    /// Límites del perfil. Falla si el perfil no es coherente.
    pub fn from_device(dev: &GalvoDevice) -> Result<Self, SafetyError> {
        let lim = SafetyLimits {
            source: dev.source,
            freq_min_khz: dev.freq_min_khz,
            freq_max_khz: dev.freq_max_khz,
            pulse_widths_ns: dev.pulse_widths_ns.clone(),
            max_power_pct: dev.max_power_pct,
            max_dwell_ms: DEFAULT_MAX_DWELL_MS,
            max_commands: DEFAULT_MAX_COMMANDS,
            laser_bit: DEFAULT_LASER_BIT,
            light_bit: DEFAULT_LIGHT_BIT,
        };
        lim.validate()?;
        Ok(lim)
    }

    /// Comprueba que los límites son coherentes.
    pub fn validate(&self) -> Result<(), SafetyError> {
        let bad = |why: &'static str| Err(SafetyError::BadLimits(why));
        if !(self.freq_min_khz.is_finite() && self.freq_max_khz.is_finite()) {
            return bad("las frecuencias del perfil no son números");
        }
        if self.freq_min_khz <= 0.0 || self.freq_min_khz > self.freq_max_khz {
            return bad("el rango de frecuencia del perfil debe cumplir 0 < mín <= máx");
        }
        if params::period(self.source, self.freq_max_khz).is_none()
            || params::period(self.source, self.freq_min_khz).is_none()
        {
            return bad("el rango de frecuencia no cabe en el periodo de 16 bits de la tarjeta");
        }
        if !(self.max_power_pct.is_finite()
            && self.max_power_pct > 0.0
            && self.max_power_pct <= 100.0)
        {
            return bad("la potencia máxima del perfil debe estar entre 0 y 100 %");
        }
        if !(self.max_dwell_ms.is_finite() && self.max_dwell_ms > 0.0) {
            return bad("el dwell máximo debe ser positivo");
        }
        if self.laser_bit > 15 || self.light_bit > 15 || self.laser_bit == self.light_bit {
            return bad(
                "los bits del láser y del puntero rojo deben ser distintos y menores que 16",
            );
        }
        if self.source == LaserSource::FiberMopa && self.pulse_widths_ns.is_empty() {
            return bad("una fuente MOPA necesita la lista de anchos de pulso del perfil");
        }
        Ok(())
    }

    pub fn laser_mask(&self) -> u16 {
        1 << self.laser_bit
    }

    pub fn light_mask(&self) -> u16 {
        1 << self.light_bit
    }

    /// El ancho de pulso lleva comando propio en esta fuente.
    pub fn uses_pulse_width(&self) -> bool {
        matches!(self.source, LaserSource::FiberMopa | LaserSource::Uv)
    }

    /// Una fuente MOPA exige fijar el pulso; en UV es opcional (si hay lista, se valida contra
    /// ella).
    pub fn requires_pulse_width(&self) -> bool {
        self.source == LaserSource::FiberMopa
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum SafetyError {
    BadLimits(&'static str),
    /// La lista está vacía o no empieza con `listReadyMark`.
    MissingReady,
    /// Falta `listEndOfList`.
    MissingEnd,
    /// Hay comandos distintos del relleno después de `listEndOfList`.
    AfterEnd {
        index: usize,
    },
    TooLong {
        commands: usize,
        max: usize,
    },
    /// Un comando que no es de lista dentro de una lista.
    NotAListCommand {
        index: usize,
        code: u16,
    },
    /// Código fuera de la lista blanca (configuración, EEPROM, tabla de corrección...).
    Forbidden {
        index: usize,
        code: u16,
    },
    /// El comando no está permitido en esta clase de lista.
    NotAllowedHere {
        index: usize,
        name: &'static str,
    },
    /// Comando de otra fuente (por ejemplo, periodo de CO2 en una fibra).
    WrongSource {
        index: usize,
        name: &'static str,
    },
    /// Marca o disparo sin haber fijado todo lo necesario.
    Unprepared {
        index: usize,
        missing: &'static str,
    },
    ZeroSpeed {
        index: usize,
    },
    ZeroPowerMark {
        index: usize,
    },
    PowerAbove {
        index: usize,
        pct: f64,
        max: f64,
    },
    FrequencyOutside {
        index: usize,
        khz: f64,
        min: f64,
        max: f64,
    },
    PulseNotAllowed {
        index: usize,
        ns: u16,
    },
    DwellTooLong {
        index: usize,
        ms: f64,
        max: f64,
    },
    BadPort {
        index: usize,
        value: u16,
    },
    /// Un inmediato que no se puede enviar en este momento.
    ImmediateRefused {
        name: &'static str,
        why: &'static str,
    },
}

impl fmt::Display for SafetyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use SafetyError::*;
        match self {
            BadLimits(w) => write!(f, "perfil de seguridad inválido: {w}"),
            MissingReady => write!(f, "la lista no empieza con listReadyMark"),
            MissingEnd => write!(f, "la lista no termina en listEndOfList"),
            AfterEnd { index } => {
                write!(f, "comando {index} después de listEndOfList")
            }
            TooLong { commands, max } => {
                write!(f, "la lista tiene {commands} comandos (máximo {max})")
            }
            NotAListCommand { index, code } => {
                write!(f, "el comando {index} ({code:#06x}) no es de lista")
            }
            Forbidden { index, code } => write!(
                f,
                "comando {index}: el código {code:#06x} ({}) no se envía nunca desde Tlaku",
                codes::name(*code)
            ),
            NotAllowedHere { index, name } => {
                write!(
                    f,
                    "comando {index}: {name} no se permite en esta clase de lista"
                )
            }
            WrongSource { index, name } => write!(
                f,
                "comando {index}: {name} no corresponde al tipo de fuente del perfil"
            ),
            Unprepared { index, missing } => write!(
                f,
                "comando {index}: marca o disparo sin haber fijado {missing}"
            ),
            ZeroSpeed { index } => write!(f, "comando {index}: velocidad 0"),
            ZeroPowerMark { index } => {
                write!(f, "comando {index}: marca con potencia efectiva 0")
            }
            PowerAbove { index, pct, max } => write!(
                f,
                "comando {index}: potencia {pct:.1} % por encima del máximo del perfil ({max} %)"
            ),
            FrequencyOutside {
                index,
                khz,
                min,
                max,
            } => write!(
                f,
                "comando {index}: frecuencia {khz:.2} kHz fuera del rango del perfil ({min} a {max} kHz)"
            ),
            PulseNotAllowed { index, ns } => write!(
                f,
                "comando {index}: ancho de pulso {ns} ns que no está en la lista del perfil"
            ),
            DwellTooLong { index, ms, max } => write!(
                f,
                "comando {index}: disparo continuo de {ms:.0} ms (máximo {max:.0} ms)"
            ),
            BadPort { index, value } => write!(
                f,
                "comando {index}: salida {value:#06x} no permitida (solo 0, el bit del láser o \
                 el del puntero rojo, nunca juntos)"
            ),
            ImmediateRefused { name, why } => write!(f, "{name} rechazado: {why}"),
        }
    }
}

impl std::error::Error for SafetyError {}

/// Resumen de una lista que pasó la validación.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ListReport {
    pub commands: usize,
    pub marks: usize,
    pub jumps: usize,
    pub dots: usize,
    /// El dwell continuo más largo (ms).
    pub longest_dwell_ms: f64,
}

#[derive(Default)]
struct State {
    jump_speed: bool,
    mark_speed: bool,
    delay_on: bool,
    delay_off: bool,
    delay_poly: bool,
    /// Periodo de frecuencia vigente.
    freq_period: Option<u16>,
    /// Potencia vigente (en fibra la corriente; en CO2/UV, la razón) y si ya es válida.
    power: Option<u16>,
    pulse: bool,
    port_laser: bool,
    dwell_ticks: u64,
}

impl State {
    fn missing_for_fire(&self, lim: &SafetyLimits, kind: ListKind) -> Option<&'static str> {
        if !self.jump_speed {
            return Some("la velocidad de salto");
        }
        if !self.mark_speed {
            return Some("la velocidad de marcado");
        }
        if !(self.delay_on && self.delay_off && self.delay_poly) {
            return Some("los retardos de encendido, apagado y polígono");
        }
        if self.freq_period.is_none() {
            return Some("la frecuencia");
        }
        if self.power.is_none() {
            return Some("la potencia");
        }
        if lim.requires_pulse_width() && !self.pulse {
            return Some("el ancho de pulso");
        }
        if kind == ListKind::Mark && !self.port_laser {
            return Some("el puerto del láser");
        }
        None
    }
}

/// Valida una lista de comandos. Ver las reglas en la documentación del módulo.
pub fn validate_list(
    cmds: &[Raw],
    kind: ListKind,
    lim: &SafetyLimits,
) -> Result<ListReport, SafetyError> {
    lim.validate()?;
    if cmds.len() > lim.max_commands {
        return Err(SafetyError::TooLong {
            commands: cmds.len(),
            max: lim.max_commands,
        });
    }
    if cmds.first().map(ListOp::decode) != Some(ListOp::Ready) {
        return Err(SafetyError::MissingReady);
    }
    let fiber = params::is_fiber(lim.source);
    let max_current = params::fiber_current(lim.max_power_pct);
    let p_fast = params::period(lim.source, lim.freq_max_khz).unwrap_or(1);
    let p_slow = params::period(lim.source, lim.freq_min_khz).unwrap_or(u16::MAX);
    let mut st = State::default();
    let mut rep = ListReport {
        commands: cmds.len(),
        ..ListReport::default()
    };
    let mut ended = false;
    let mut longest_ticks = 0u64;

    for (index, raw) in cmds.iter().enumerate() {
        if raw.kind() != Kind::List {
            return Err(SafetyError::NotAListCommand {
                index,
                code: raw.code,
            });
        }
        let op = ListOp::decode(raw);
        if ended {
            if op == ListOp::EndOfList {
                continue;
            }
            return Err(SafetyError::AfterEnd { index });
        }
        // Un dwell continuo se corta con cualquier comando que no sea LaserOnPoint.
        if !matches!(op, ListOp::LaserOnPoint { .. }) {
            st.dwell_ticks = 0;
        }
        let name = codes::name(raw.code);
        let not_here = || Err(SafetyError::NotAllowedHere { index, name });
        let wrong_source = || Err(SafetyError::WrongSource { index, name });
        match op {
            ListOp::Unknown(r) => {
                return Err(SafetyError::Forbidden {
                    index,
                    code: r.code,
                });
            }
            ListOp::Ready => {
                if index != 0 {
                    return not_here();
                }
            }
            ListOp::EndOfList => ended = true,
            ListOp::JumpSpeed(v) => {
                if v == 0 {
                    return Err(SafetyError::ZeroSpeed { index });
                }
                st.jump_speed = true;
            }
            ListOp::JumpDelay(_) | ListOp::DelayTime { .. } => {}
            ListOp::JumpTo { .. } => {
                if !st.jump_speed {
                    return Err(SafetyError::Unprepared {
                        index,
                        missing: "la velocidad de salto",
                    });
                }
                rep.jumps += 1;
            }
            ListOp::WritePort(v) => {
                let want = match kind {
                    ListKind::Mark => lim.laser_mask(),
                    ListKind::DryRun | ListKind::Light => lim.light_mask(),
                };
                if v != want {
                    return Err(SafetyError::BadPort { index, value: v });
                }
                st.port_laser = kind == ListKind::Mark;
            }
            // Todo lo demás es de marcado: no cabe en un encuadre.
            _ if kind == ListKind::Light => return not_here(),
            ListOp::MarkSpeed(v) => {
                if v == 0 {
                    return Err(SafetyError::ZeroSpeed { index });
                }
                st.mark_speed = true;
            }
            ListOp::LaserOnDelay(_) => st.delay_on = true,
            ListOp::LaserOffDelay(_) => st.delay_off = true,
            ListOp::PolygonDelay(_) => st.delay_poly = true,
            ListOp::QSwitchPeriod(p) | ListOp::MarkFreq(p) => {
                let is_q = matches!(op, ListOp::QSwitchPeriod(_));
                if is_q != fiber {
                    return wrong_source();
                }
                // Dentro del rango del perfil, con el mismo redondeo con que se generó.
                if p == 0 || p < p_fast || p > p_slow {
                    return Err(SafetyError::FrequencyOutside {
                        index,
                        khz: params::freq_from_period(lim.source, p),
                        min: lim.freq_min_khz,
                        max: lim.freq_max_khz,
                    });
                }
                st.freq_period = Some(p);
                if !fiber {
                    // La razón de potencia de CO2/UV depende de la frecuencia: hay que repetirla.
                    st.power = None;
                }
            }
            ListOp::MarkCurrent(c) => {
                if !fiber {
                    return wrong_source();
                }
                if c > max_current {
                    return Err(SafetyError::PowerAbove {
                        index,
                        pct: f64::from(c) * 100.0 / f64::from(params::CURRENT_MAX),
                        max: lim.max_power_pct,
                    });
                }
                st.power = Some(c);
            }
            ListOp::PowerRatio(r) => {
                if fiber {
                    return wrong_source();
                }
                let Some(p) = st.freq_period else {
                    return Err(SafetyError::Unprepared {
                        index,
                        missing: "la frecuencia antes de la potencia",
                    });
                };
                // potencia% = razón x kHz / 200 = razón x 50 / periodo; una unidad de tolerancia.
                let pct = f64::from(r) * 50.0 / f64::from(p);
                let allowed = (lim.max_power_pct * f64::from(p) / 50.0).round() as u32 + 1;
                if u32::from(r) > allowed {
                    return Err(SafetyError::PowerAbove {
                        index,
                        pct,
                        max: lim.max_power_pct,
                    });
                }
                st.power = Some(r);
            }
            ListOp::Co2Fpk(_) => {
                if fiber {
                    return wrong_source();
                }
            }
            ListOp::PulseWidth(ns) => {
                if !lim.uses_pulse_width() {
                    return wrong_source();
                }
                if !lim.pulse_widths_ns.contains(&u32::from(ns)) {
                    return Err(SafetyError::PulseNotAllowed { index, ns });
                }
                st.pulse = true;
            }
            ListOp::MarkTo { .. } | ListOp::LaserOnPoint { .. } => {
                if let Some(missing) = st.missing_for_fire(lim, kind) {
                    return Err(SafetyError::Unprepared { index, missing });
                }
                if st.power == Some(0) {
                    return Err(SafetyError::ZeroPowerMark { index });
                }
                match op {
                    ListOp::MarkTo { .. } => rep.marks += 1,
                    ListOp::LaserOnPoint { ticks } => {
                        rep.dots += 1;
                        st.dwell_ticks += u64::from(ticks);
                        longest_ticks = longest_ticks.max(st.dwell_ticks);
                        let ms = params::ms_from_ticks(st.dwell_ticks);
                        if ms > lim.max_dwell_ms {
                            return Err(SafetyError::DwellTooLong {
                                index,
                                ms,
                                max: lim.max_dwell_ms,
                            });
                        }
                    }
                    _ => unreachable!(),
                }
            }
        }
    }
    if !ended {
        return Err(SafetyError::MissingEnd);
    }
    rep.longest_dwell_ms = params::ms_from_ticks(longest_ticks);
    Ok(rep)
}

/// Situación de la sesión al enviar un inmediato.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImmContext {
    /// Se permite habilitar el láser (solo durante un marcado real, nunca en encuadres ni pruebas).
    pub arming_allowed: bool,
    /// La fuente está armada ahora mismo.
    pub armed: bool,
}

/// Revisa un inmediato antes de enviarlo. Todo lo que no esté en la lista blanca se rechaza.
pub fn check_immediate(imm: &Imm, lim: &SafetyLimits, ctx: ImmContext) -> Result<(), SafetyError> {
    let refuse = |why: &'static str| {
        Err(SafetyError::ImmediateRefused {
            name: codes::name(imm.to_raw().code),
            why,
        })
    };
    match *imm {
        Imm::Unknown(_) => refuse("el código no está en la lista blanca"),
        Imm::EnableLaser if !ctx.arming_allowed => {
            refuse("solo se habilita el láser durante un marcado real")
        }
        Imm::FiberSetMo(v) => {
            if v > 1 {
                refuse("el valor del MO debe ser 0 o 1")
            } else if !params::is_fiber(lim.source) {
                refuse("la fuente del perfil no es de fibra")
            } else if v == 1 && !ctx.arming_allowed {
                refuse("solo se abre el MO durante un marcado real")
            } else {
                Ok(())
            }
        }
        Imm::WritePort(v) => {
            if v == 0 || v == lim.light_mask() {
                Ok(())
            } else if v == lim.laser_mask() {
                if ctx.arming_allowed {
                    Ok(())
                } else {
                    refuse("el bit del láser solo se escribe durante un marcado real")
                }
            } else {
                refuse("solo se escribe 0, el bit del láser o el del puntero rojo")
            }
        }
        Imm::GotoXY { .. } if ctx.armed => refuse("la fuente está armada"),
        _ => Ok(()),
    }
}

/// Secuencia de apagado: se intenta **entera** aunque falle algún paso. Primero se inhibe el
/// láser y se detiene la lista; después se cierra el MO (solo fibra), se ponen las salidas a 0 y
/// se vacía la lista. Basada en `abort` de galvoplotter (MIT) más `DisableLaser`.
pub fn shutdown_sequence(source: LaserSource) -> Vec<Imm> {
    let mut seq = vec![Imm::DisableLaser, Imm::StopExecute];
    if params::is_fiber(source) {
        seq.push(Imm::FiberSetMo(0));
    }
    seq.push(Imm::WritePort(0));
    seq.push(Imm::ResetList);
    seq.push(Imm::DisableLaser);
    seq
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cmd::Delay;

    fn limits(source: LaserSource) -> SafetyLimits {
        SafetyLimits::from_device(&GalvoDevice {
            source,
            pulse_widths_ns: vec![4, 6, 9, 13],
            ..GalvoDevice::default()
        })
        .unwrap()
    }

    fn raws(ops: &[ListOp]) -> Vec<Raw> {
        ops.iter().map(ListOp::to_raw).collect()
    }

    fn fiber_prefix() -> Vec<ListOp> {
        vec![
            ListOp::Ready,
            ListOp::WritePort(1),
            ListOp::JumpSpeed(2000),
            ListOp::QSwitchPeriod(667),
            ListOp::MarkCurrent(2048),
            ListOp::MarkSpeed(100),
            ListOp::LaserOnDelay(Delay::from_us(100)),
            ListOp::LaserOffDelay(Delay::from_us(100)),
            ListOp::PolygonDelay(10),
        ]
    }

    fn with(mut a: Vec<ListOp>, more: &[ListOp]) -> Vec<Raw> {
        a.extend_from_slice(more);
        a.push(ListOp::EndOfList);
        raws(&a)
    }

    const MARK: ListOp = ListOp::MarkTo {
        x: 0x8100,
        y: 0x8000,
        dist: 256,
    };

    #[test]
    fn a_complete_fiber_list_passes() {
        let lim = limits(LaserSource::FiberQ);
        let rep = validate_list(&with(fiber_prefix(), &[MARK]), ListKind::Mark, &lim).unwrap();
        assert_eq!(rep.marks, 1);
    }

    #[test]
    fn marking_requires_every_parameter_first() {
        let lim = limits(LaserSource::FiberQ);
        for drop_idx in 1..fiber_prefix().len() {
            let mut p = fiber_prefix();
            p.remove(drop_idx);
            let err = validate_list(&with(p, &[MARK]), ListKind::Mark, &lim).unwrap_err();
            assert!(
                matches!(err, SafetyError::Unprepared { .. }),
                "sin el comando {drop_idx}: {err}"
            );
        }
        // La primera marca no puede ir antes del puerto del láser.
        let err = validate_list(
            &raws(&[ListOp::Ready, MARK, ListOp::EndOfList]),
            ListKind::Mark,
            &lim,
        )
        .unwrap_err();
        assert!(matches!(err, SafetyError::Unprepared { .. }));
    }

    #[test]
    fn lists_must_start_with_ready_and_end_cleanly() {
        let lim = limits(LaserSource::FiberQ);
        assert_eq!(
            validate_list(&[], ListKind::Mark, &lim),
            Err(SafetyError::MissingReady)
        );
        let no_end = raws(&fiber_prefix());
        assert_eq!(
            validate_list(&no_end, ListKind::Mark, &lim),
            Err(SafetyError::MissingEnd)
        );
        let mut after = with(fiber_prefix(), &[]);
        after.push(MARK.to_raw());
        assert!(matches!(
            validate_list(&after, ListKind::Mark, &lim),
            Err(SafetyError::AfterEnd { .. })
        ));
        // El relleno de EndOfList después del final sí se acepta.
        let mut padded = with(fiber_prefix(), &[]);
        padded.extend(std::iter::repeat_n(Raw::bare(0x8002), 100));
        validate_list(&padded, ListKind::Mark, &lim).unwrap();
    }

    #[test]
    fn power_frequency_and_pulse_are_checked_against_the_profile() {
        let mut dev = GalvoDevice {
            max_power_pct: 50.0,
            freq_min_khz: 20.0,
            freq_max_khz: 80.0,
            ..GalvoDevice::default()
        };
        let lim = SafetyLimits::from_device(&dev).unwrap();
        // 51 %.
        let mut p = fiber_prefix();
        p[4] = ListOp::MarkCurrent(params::fiber_current(51.0));
        assert!(matches!(
            validate_list(&with(p, &[MARK]), ListKind::Mark, &lim),
            Err(SafetyError::PowerAbove { .. })
        ));
        // Justo en el límite pasa.
        let mut p = fiber_prefix();
        p[4] = ListOp::MarkCurrent(params::fiber_current(50.0));
        validate_list(&with(p, &[MARK]), ListKind::Mark, &lim).unwrap();
        // 10 kHz y 100 kHz están fuera de 20 a 80.
        for period in [2000u16, 200] {
            let mut p = fiber_prefix();
            p[3] = ListOp::QSwitchPeriod(period);
            assert!(matches!(
                validate_list(&with(p, &[MARK]), ListKind::Mark, &lim),
                Err(SafetyError::FrequencyOutside { .. })
            ));
        }
        // MOPA: pulso fuera de la lista.
        dev.source = LaserSource::FiberMopa;
        dev.pulse_widths_ns = vec![4, 9];
        let lim = SafetyLimits::from_device(&dev).unwrap();
        let mut p = fiber_prefix();
        p.insert(4, ListOp::PulseWidth(5));
        assert!(matches!(
            validate_list(&with(p, &[MARK]), ListKind::Mark, &lim),
            Err(SafetyError::PulseNotAllowed { ns: 5, .. })
        ));
        let mut p = fiber_prefix();
        p.insert(4, ListOp::PulseWidth(9));
        validate_list(&with(p, &[MARK]), ListKind::Mark, &lim).unwrap();
        // MOPA sin pulso fijado: no se marca.
        assert!(matches!(
            validate_list(&with(fiber_prefix(), &[MARK]), ListKind::Mark, &lim),
            Err(SafetyError::Unprepared { .. })
        ));
    }

    #[test]
    fn q_switched_sources_refuse_pulse_width_commands() {
        let lim = limits(LaserSource::FiberQ);
        let mut p = fiber_prefix();
        p.insert(4, ListOp::PulseWidth(4));
        assert!(matches!(
            validate_list(&with(p, &[MARK]), ListKind::Mark, &lim),
            Err(SafetyError::WrongSource { .. })
        ));
    }

    #[test]
    fn co2_uses_its_own_frequency_and_power_commands() {
        let lim = limits(LaserSource::Co2);
        let p = vec![
            ListOp::Ready,
            ListOp::WritePort(1),
            ListOp::JumpSpeed(2000),
            ListOp::MarkFreq(333),
            ListOp::PowerRatio(333),
            ListOp::MarkSpeed(100),
            ListOp::LaserOnDelay(Delay::from_us(100)),
            ListOp::LaserOffDelay(Delay::from_us(100)),
            ListOp::PolygonDelay(10),
        ];
        validate_list(&with(p.clone(), &[MARK]), ListKind::Mark, &lim).unwrap();
        // Una razón enorme equivale a más del 100 %.
        let mut hot = p.clone();
        hot[4] = ListOp::PowerRatio(2000);
        assert!(matches!(
            validate_list(&with(hot, &[MARK]), ListKind::Mark, &lim),
            Err(SafetyError::PowerAbove { .. })
        ));
        // Cambiar la frecuencia invalida la potencia: hay que repetirla.
        let mut changed = p.clone();
        changed.push(ListOp::MarkFreq(250));
        assert!(matches!(
            validate_list(&with(changed, &[MARK]), ListKind::Mark, &lim),
            Err(SafetyError::Unprepared { .. })
        ));
        // Comandos de fibra en CO2.
        let mut wrong = p;
        wrong[3] = ListOp::QSwitchPeriod(667);
        assert!(matches!(
            validate_list(&with(wrong, &[MARK]), ListKind::Mark, &lim),
            Err(SafetyError::WrongSource { .. })
        ));
    }

    #[test]
    fn zero_power_marks_and_zero_speeds_are_rejected() {
        let lim = limits(LaserSource::FiberQ);
        let mut p = fiber_prefix();
        p[4] = ListOp::MarkCurrent(0);
        assert!(matches!(
            validate_list(&with(p, &[MARK]), ListKind::Mark, &lim),
            Err(SafetyError::ZeroPowerMark { .. })
        ));
        let mut p = fiber_prefix();
        p[5] = ListOp::MarkSpeed(0);
        assert!(matches!(
            validate_list(&with(p, &[MARK]), ListKind::Mark, &lim),
            Err(SafetyError::ZeroSpeed { .. })
        ));
    }

    #[test]
    fn continuous_dwell_is_bounded() {
        let lim = limits(LaserSource::FiberQ);
        let dot = |ticks| ListOp::LaserOnPoint { ticks };
        // 3 x 600 ms = 1,8 s seguidos: pasa; con un salto en medio se reinicia.
        validate_list(
            &with(fiber_prefix(), &[dot(60000), dot(60000), dot(60000)]),
            ListKind::Mark,
            &lim,
        )
        .unwrap();
        let long = vec![dot(60000); 9];
        assert!(matches!(
            validate_list(&with(fiber_prefix(), &long), ListKind::Mark, &lim),
            Err(SafetyError::DwellTooLong { .. })
        ));
    }

    #[test]
    fn foreign_and_forbidden_codes_are_refused() {
        let lim = limits(LaserSource::FiberQ);
        for code in [0x801Cu16, 0x8028, 0x8050, 0x8123] {
            let mut cmds = with(fiber_prefix(), &[]);
            cmds.insert(2, Raw::bare(code));
            assert!(
                matches!(
                    validate_list(&cmds, ListKind::Mark, &lim),
                    Err(SafetyError::Forbidden { .. } | SafetyError::NotAListCommand { .. })
                ),
                "{code:#06x}"
            );
        }
        // Un inmediato dentro de una lista.
        let mut cmds = with(fiber_prefix(), &[]);
        cmds.insert(2, Imm::EnableLaser.to_raw());
        assert!(matches!(
            validate_list(&cmds, ListKind::Mark, &lim),
            Err(SafetyError::NotAListCommand { .. })
        ));
    }

    #[test]
    fn ports_are_restricted_by_list_kind() {
        let lim = limits(LaserSource::FiberQ);
        // Marcado: solo el bit del láser; el del puntero o ambos juntos, no.
        for bad in [0u16, 0x100, 0x101, 0x2] {
            let mut p = fiber_prefix();
            p[1] = ListOp::WritePort(bad);
            assert!(
                matches!(
                    validate_list(&with(p, &[MARK]), ListKind::Mark, &lim),
                    Err(SafetyError::BadPort { .. })
                ),
                "{bad:#x}"
            );
        }
        // Prueba en rojo: lleva el trabajo completo pero con el puerto del puntero.
        let mut p = fiber_prefix();
        p[1] = ListOp::WritePort(0x100);
        validate_list(&with(p.clone(), &[MARK]), ListKind::DryRun, &lim).unwrap();
        // ... y con el bit del láser se rechaza.
        p[1] = ListOp::WritePort(1);
        assert!(validate_list(&with(p, &[MARK]), ListKind::DryRun, &lim).is_err());
    }

    #[test]
    fn light_lists_cannot_fire() {
        let lim = limits(LaserSource::FiberQ);
        let ok = raws(&[
            ListOp::Ready,
            ListOp::WritePort(0x100),
            ListOp::JumpSpeed(1000),
            ListOp::JumpDelay(Delay::from_us(200)),
            ListOp::JumpTo {
                x: 1,
                y: 2,
                dist: 3,
            },
            ListOp::EndOfList,
        ]);
        let rep = validate_list(&ok, ListKind::Light, &lim).unwrap();
        assert_eq!((rep.jumps, rep.marks), (1, 0));
        for bad in [
            ListOp::MarkTo {
                x: 1,
                y: 2,
                dist: 3,
            },
            ListOp::LaserOnPoint { ticks: 1 },
            ListOp::MarkCurrent(100),
            ListOp::QSwitchPeriod(667),
            ListOp::MarkSpeed(10),
        ] {
            let mut cmds = ok.clone();
            cmds.insert(4, bad.to_raw());
            assert!(
                matches!(
                    validate_list(&cmds, ListKind::Light, &lim),
                    Err(SafetyError::NotAllowedHere { .. })
                ),
                "{bad:?}"
            );
        }
        // El bit del láser en un encuadre.
        let mut cmds = ok.clone();
        cmds[1] = ListOp::WritePort(1).to_raw();
        assert!(matches!(
            validate_list(&cmds, ListKind::Light, &lim),
            Err(SafetyError::BadPort { .. })
        ));
        // Saltar sin velocidad de salto.
        let mut cmds = ok;
        cmds.remove(2);
        assert!(matches!(
            validate_list(&cmds, ListKind::Light, &lim),
            Err(SafetyError::Unprepared { .. })
        ));
    }

    #[test]
    fn immediates_are_whitelisted() {
        let lim = limits(LaserSource::FiberQ);
        let idle = ImmContext {
            arming_allowed: false,
            armed: false,
        };
        let live = ImmContext {
            arming_allowed: true,
            armed: true,
        };
        // Apagar siempre se puede.
        for imm in [
            Imm::DisableLaser,
            Imm::StopExecute,
            Imm::ResetList,
            Imm::FiberSetMo(0),
            Imm::WritePort(0),
            Imm::WritePort(0x100),
            Imm::GetVersion,
            Imm::ReadPort,
        ] {
            check_immediate(&imm, &lim, idle).unwrap();
        }
        // Armar, no.
        for imm in [
            Imm::EnableLaser,
            Imm::FiberSetMo(1),
            Imm::WritePort(1),
            Imm::WritePort(0x101),
            Imm::WritePort(0x4),
            Imm::FiberSetMo(7),
            Imm::Unknown(Raw::bare(0x0015)),
        ] {
            assert!(check_immediate(&imm, &lim, idle).is_err(), "{imm:?}");
        }
        // Armado y con permiso se puede armar; pero no mover con GotoXY.
        check_immediate(&Imm::EnableLaser, &lim, live).unwrap();
        check_immediate(&Imm::FiberSetMo(1), &lim, live).unwrap();
        check_immediate(&Imm::WritePort(1), &lim, live).unwrap();
        assert!(check_immediate(&Imm::WritePort(0x101), &lim, live).is_err());
        let goto = Imm::GotoXY {
            x: 0x8000,
            y: 0x8000,
            dist: 0,
        };
        check_immediate(&goto, &lim, idle).unwrap();
        assert!(check_immediate(&goto, &lim, live).is_err());
    }

    #[test]
    fn mo_commands_are_refused_for_non_fiber_sources() {
        let lim = limits(LaserSource::Co2);
        let ctx = ImmContext {
            arming_allowed: true,
            armed: false,
        };
        assert!(check_immediate(&Imm::FiberSetMo(1), &lim, ctx).is_err());
        assert!(check_immediate(&Imm::FiberSetMo(0), &lim, ctx).is_err());
        assert!(
            !shutdown_sequence(LaserSource::Co2).contains(&Imm::FiberSetMo(0)),
            "el apagado de CO2 no manda comandos de fibra"
        );
    }

    #[test]
    fn shutdown_sequence_inhibits_first_and_closes_everything() {
        let seq = shutdown_sequence(LaserSource::FiberMopa);
        assert_eq!(seq.first(), Some(&Imm::DisableLaser));
        assert_eq!(seq.last(), Some(&Imm::DisableLaser));
        assert!(seq.contains(&Imm::StopExecute));
        assert!(seq.contains(&Imm::FiberSetMo(0)));
        assert!(seq.contains(&Imm::WritePort(0)));
        // Todos pasan la lista blanca sin permiso para armar.
        let lim = limits(LaserSource::FiberMopa);
        let ctx = ImmContext {
            arming_allowed: false,
            armed: true,
        };
        for imm in &seq {
            check_immediate(imm, &lim, ctx).unwrap();
        }
    }

    #[test]
    fn inconsistent_profiles_are_refused() {
        let bad = |f: &dyn Fn(&mut GalvoDevice)| {
            let mut d = GalvoDevice::default();
            f(&mut d);
            SafetyLimits::from_device(&d).unwrap_err()
        };
        assert!(matches!(
            bad(&|d| d.freq_min_khz = 0.0),
            SafetyError::BadLimits(_)
        ));
        assert!(matches!(
            bad(&|d| d.freq_max_khz = 10.0),
            SafetyError::BadLimits(_)
        ));
        assert!(matches!(
            bad(&|d| d.freq_min_khz = f64::NAN),
            SafetyError::BadLimits(_)
        ));
        assert!(matches!(
            bad(&|d| d.max_power_pct = 0.0),
            SafetyError::BadLimits(_)
        ));
        assert!(matches!(
            bad(&|d| d.max_power_pct = 120.0),
            SafetyError::BadLimits(_)
        ));
        assert!(matches!(
            bad(&|d| d.source = LaserSource::FiberMopa),
            SafetyError::BadLimits(_)
        ));
    }
}
