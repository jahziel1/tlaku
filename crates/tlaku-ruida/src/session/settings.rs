//! Lectura y escritura de los ajustes de la controladora (M25, M26 y M28).
//!
//! **EXPERIMENTAL: sin probar con una controladora real.** Escribir la memoria (`DA 01`) puede
//! dejar la controladora inutilizable. Las garantías que da este módulo, en orden:
//!
//! 1. solo se escriben las direcciones de [`crate::settings::WRITABLE`], dentro de su rango; lo
//!    demás se rechaza antes de enviar nada;
//! 2. antes de escribir se lee **toda** la lista y se entrega a quien llama para que la guarde
//!    en un archivo con fecha; si guardarla falla, no se escribe nada;
//! 3. sin la confirmación explícita (`confirmed`) no se escribe nada;
//! 4. la máquina debe estar en reposo;
//! 5. al terminar se vuelve a leer cada valor escrito y se compara con lo pedido; si no
//!    coincide, el informe lo dice y recomienda restaurar el respaldo.
//!
//! La forma de la orden (`DA 01 dirección v v`, el valor repetido) es la única que describen las
//! fuentes públicas: MeerK40t `rdjob.py` (`set_setting`, MIT) y ruida-pa (`SET_SETTING`, dos
//! valores de 35 bits, marcado como verificado en una RDC6442S). Qué hace la segunda copia del
//! valor no está documentado.

use super::*;
use crate::cmd::Command;
use crate::link::PacketLink;
use crate::settings::{
    self, PlannedChange, SettingChange, SettingValue, SettingsError, SettingsFile, SettingsPlan,
    SettingsSnapshot,
};

/// Resultado de un valor escrito tras releerlo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedSetting {
    pub addr: u16,
    pub key: String,
    pub expected: i64,
    /// Lo que se leyó después; `None` si la máquina no contestó.
    pub actual: Option<i64>,
}

impl VerifiedSetting {
    pub fn matches(&self) -> bool {
        self.actual == Some(self.expected)
    }
}

/// Informe de aplicar (o restaurar) ajustes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplyReport {
    /// Lo que cambió (valor anterior y nuevo).
    pub plan: SettingsPlan,
    /// Descripción del respaldo previo (la devuelve quien lo guardó, p. ej. la ruta).
    pub backup: Option<String>,
    /// Releídos tras escribir, en el orden de escritura.
    pub verified: Vec<VerifiedSetting>,
    /// Si una escritura falló, qué dirección y por qué; las siguientes no se enviaron.
    pub write_error: Option<(u16, String)>,
}

impl ApplyReport {
    /// ¿Todo coincide y no hubo errores?
    pub fn is_ok(&self) -> bool {
        self.write_error.is_none() && self.verified.iter().all(VerifiedSetting::matches)
    }

    /// ¿Conviene restaurar el respaldo? (algo no coincide o una escritura falló tras empezar).
    pub fn restore_advised(&self) -> bool {
        !self.is_ok()
    }
}

impl<L: PacketLink> RuidaSession<L> {
    /// Lee todos los ajustes de la lista cerrada. Un valor que no contesta queda en `None`;
    /// tras el primer silencio no se insiste con los demás.
    pub fn read_settings(&mut self) -> Result<SettingsSnapshot, SessionError> {
        let card_id = u32::try_from(self.read_mem(addr::CARD_ID)?).unwrap_or(u32::MAX);
        let mut silent = false;
        let mut values = Vec::with_capacity(settings::WRITABLE.len());
        for def in settings::WRITABLE {
            let value = if silent {
                None
            } else {
                match self.read_mem(def.addr) {
                    Ok(raw) => Some(def.decode(raw)),
                    Err(SessionError::Timeout { .. }) => {
                        silent = true;
                        None
                    }
                    Err(_) => None,
                }
            };
            values.push(SettingValue {
                addr: def.addr,
                key: def.key.to_string(),
                value,
            });
        }
        Ok(SettingsSnapshot {
            card_id,
            model: model_name(card_id).map(str::to_string),
            values,
        })
    }

    /// Calcula las diferencias de `requested` con lo que hay en la máquina sin escribir nada.
    pub fn preview_settings(
        &mut self,
        requested: &[SettingChange],
    ) -> Result<(SettingsSnapshot, SettingsPlan), SessionError> {
        let snap = self.read_settings()?;
        let plan = settings::plan(&snap, requested)?;
        Ok((snap, plan))
    }

    /// Aplica `requested`. Ver el documento del módulo para las garantías. `backup` recibe la
    /// lectura completa de antes de escribir y devuelve una descripción de dónde se guardó; si
    /// falla, no se escribe nada.
    pub fn apply_settings(
        &mut self,
        requested: &[SettingChange],
        confirmed: bool,
        backup: &mut dyn FnMut(&SettingsSnapshot) -> Result<String, String>,
    ) -> Result<ApplyReport, SessionError> {
        self.apply_inner(requested, confirmed, None, backup)
    }

    /// Restaura un respaldo `.tlakuruida` con las mismas protecciones que [`Self::apply_settings`]
    /// y, además, exige que sea de la misma tarjeta que la conectada.
    pub fn restore_settings(
        &mut self,
        file: &SettingsFile,
        confirmed: bool,
        backup: &mut dyn FnMut(&SettingsSnapshot) -> Result<String, String>,
    ) -> Result<ApplyReport, SessionError> {
        let card = file.card()?;
        self.apply_inner(&file.changes(), confirmed, Some(card), backup)
    }

    fn apply_inner(
        &mut self,
        requested: &[SettingChange],
        confirmed: bool,
        expect_card: Option<u32>,
        backup: &mut dyn FnMut(&SettingsSnapshot) -> Result<String, String>,
    ) -> Result<ApplyReport, SessionError> {
        if !confirmed {
            return Err(SessionError::NotConfirmed);
        }
        // Todo lo que se va a escribir se valida antes de tocar la máquina, también con una
        // lectura imposible.
        for c in requested {
            settings::check_write(c.addr, c.value)?;
        }
        self.ensure_idle()?;
        let snap = self.read_settings()?;
        if let Some(card) = expect_card
            && card != snap.card_id
        {
            return Err(SettingsError::CardMismatch {
                file: card,
                machine: snap.card_id,
            }
            .into());
        }
        let plan = settings::plan(&snap, requested)?;
        if plan.changes.is_empty() {
            return Ok(ApplyReport {
                plan,
                backup: None,
                verified: Vec::new(),
                write_error: None,
            });
        }
        // Respaldo de la lectura completa; sin él no se escribe.
        let saved = backup(&snap).map_err(SessionError::BackupFailed)?;

        let mut write_error = None;
        let mut written: Vec<&PlannedChange> = Vec::new();
        for ch in &plan.changes {
            match self.write_setting(ch.addr, ch.new) {
                Ok(()) => written.push(ch),
                Err(e) => {
                    write_error = Some((ch.addr, e.to_string()));
                    break;
                }
            }
        }
        // Verificación: se relee todo lo que se escribió y también lo que falló (puede haberse
        // aplicado con el ACK perdido).
        let mut verified = Vec::new();
        for ch in plan.changes.iter().take(written.len() + 1) {
            let def = settings::find(ch.addr).expect("la lista cerrada contiene la dirección");
            let actual = self.read_mem(ch.addr).ok().map(|raw| def.decode(raw));
            verified.push(VerifiedSetting {
                addr: ch.addr,
                key: ch.key.clone(),
                expected: ch.new,
                actual,
            });
        }
        Ok(ApplyReport {
            plan,
            backup: Some(saved),
            verified,
            write_error,
        })
    }

    /// Escribe UN ajuste: `DA 01 dirección v v`. Único punto por el que Tlaku envía una escritura
    /// de memoria con valores propios; repite la validación de la lista cerrada.
    fn write_setting(&mut self, addr: u16, value: i64) -> Result<(), SessionError> {
        let raw = settings::check_write(addr, value)?;
        self.command(
            &[Command::MemWrite {
                addr,
                values: vec![raw, raw],
            }],
            self.cfg.ack_timeout,
        )
    }
}
