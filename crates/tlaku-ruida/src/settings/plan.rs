//! Instantánea de los ajustes, cambios pedidos y diferencias con lo que hay en la máquina.

use super::SettingsError;
use super::defs::{self, SettingKind, WRITABLE};
use serde::{Deserialize, Serialize};

/// Un valor leído de la máquina. `value` es `None` si no contestó.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SettingValue {
    pub addr: u16,
    pub key: String,
    pub value: Option<i64>,
}

/// Todos los ajustes de la lista cerrada, leídos en un momento dado.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SettingsSnapshot {
    pub card_id: u32,
    pub model: Option<String>,
    pub values: Vec<SettingValue>,
}

impl SettingsSnapshot {
    pub fn get(&self, addr: u16) -> Option<i64> {
        self.values
            .iter()
            .find(|v| v.addr == addr)
            .and_then(|v| v.value)
    }

    /// ¿Contestó la máquina a todos?
    pub fn complete(&self) -> bool {
        self.values.len() == WRITABLE.len() && self.values.iter().all(|v| v.value.is_some())
    }
}

/// Un cambio pedido: dirección y valor nuevo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SettingChange {
    pub addr: u16,
    pub value: i64,
}

/// Un cambio ya validado, con el valor actual junto al nuevo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannedChange {
    pub addr: u16,
    pub key: String,
    pub current: i64,
    pub new: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SettingsPlan {
    /// Solo lo que cambia de verdad, en el orden de la lista cerrada.
    pub changes: Vec<PlannedChange>,
    /// Pedidos cuyo valor ya es el de la máquina.
    pub unchanged: usize,
}

/// Valida `requested` contra la lista cerrada y la instantánea, y devuelve las diferencias.
/// Rechaza (sin ignorar nada en silencio) direcciones ajenas a la lista, valores fuera de rango,
/// direcciones repetidas, ajustes sin valor actual y saltos de paso mayores al doble o menores a
/// la mitad. No toca la máquina.
pub fn plan(
    snapshot: &SettingsSnapshot,
    requested: &[SettingChange],
) -> Result<SettingsPlan, SettingsError> {
    let mut seen: Vec<u16> = Vec::new();
    for c in requested {
        if seen.contains(&c.addr) {
            return Err(SettingsError::Duplicate { addr: c.addr });
        }
        seen.push(c.addr);
    }
    let mut changes = Vec::new();
    let mut unchanged = 0;
    // Se recorre la lista cerrada, no la petición: el orden de escritura es siempre el mismo.
    for def in WRITABLE {
        let Some(c) = requested.iter().find(|c| c.addr == def.addr) else {
            continue;
        };
        def.encode(c.value)?;
        let current = snapshot
            .get(def.addr)
            .ok_or(SettingsError::NoCurrentValue {
                addr: def.addr,
                key: def.key,
            })?;
        if def.kind == SettingKind::Step {
            if current <= 0 {
                return Err(SettingsError::NoCurrentValue {
                    addr: def.addr,
                    key: def.key,
                });
            }
            if c.value > current.saturating_mul(2) || c.value.saturating_mul(2) < current {
                return Err(SettingsError::StepJump {
                    key: def.key,
                    current,
                    value: c.value,
                });
            }
        }
        if current == c.value {
            unchanged += 1;
        } else {
            changes.push(PlannedChange {
                addr: def.addr,
                key: def.key.to_string(),
                current,
                new: c.value,
            });
        }
    }
    // Cualquier dirección pedida que no esté en la lista cerrada es un error, no se descarta.
    for c in requested {
        defs::check_write(c.addr, c.value)?;
    }
    Ok(SettingsPlan { changes, unchanged })
}
