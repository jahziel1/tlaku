//! Política de retención del historial de versiones: qué versiones se borran para no crecer sin fin.
//!
//! Es una función pura (`plan`): recibe las versiones y la hora «de ahora» y devuelve los ids que
//! sobran. Así se prueba con relojes simulados, sin tocar el disco.
//!
//! Escalones, de la versión más nueva a la más vieja (la antigüedad se mide desde ahora):
//!
//! 1. hasta `recent_hours`: se conservan todas;
//! 2. hasta `daily_days`: una por día (la más nueva de cada día UTC);
//! 3. hasta `weekly_weeks`: una por semana (la más nueva de cada bloque de 7 días UTC);
//! 4. más viejas: se borran.
//!
//! Después se aplican los topes de número y de tamaño borrando primero las más viejas. **Nunca se
//! borran** la versión más nueva ni las manuales con nombre (las puso el usuario a propósito: solo
//! él las quita); por eso un tope puede quedar rebasado si solo quedan versiones protegidas.

use super::store::{Kind, VersionInfo};
use std::collections::HashSet;

pub const HOUR_MS: u64 = 3_600_000;
pub const DAY_MS: u64 = 24 * HOUR_MS;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Policy {
    pub recent_hours: u32,
    pub daily_days: u32,
    pub weekly_weeks: u32,
    pub max_bytes: u64,
    pub max_versions: usize,
}

impl Policy {
    fn recent_end(&self) -> u64 {
        u64::from(self.recent_hours) * HOUR_MS
    }
    fn daily_end(&self) -> u64 {
        (u64::from(self.daily_days) * DAY_MS).max(self.recent_end())
    }
    fn weekly_end(&self) -> u64 {
        (u64::from(self.weekly_weeks) * 7 * DAY_MS).max(self.daily_end())
    }
}

/// `true` si la poda automática no puede borrarla.
fn protected(v: &VersionInfo) -> bool {
    v.kind == Kind::Manual
}

/// Ids que hay que borrar para cumplir `policy` a la hora `now_ms` (de la más vieja a la más nueva).
pub fn plan(versions: &[VersionInfo], now_ms: u64, policy: Policy) -> Vec<String> {
    let mut sorted: Vec<&VersionInfo> = versions.iter().collect();
    // De la más nueva a la más vieja.
    sorted.sort_by(|a, b| b.created_ms.cmp(&a.created_ms).then(b.id.cmp(&a.id)));
    let newest = sorted.first().map(|v| v.id.clone());
    let mut doomed: HashSet<String> = HashSet::new();
    let mut taken: HashSet<(u8, u64)> = HashSet::new();

    for v in &sorted {
        if protected(v) || Some(&v.id) == newest.as_ref() {
            continue;
        }
        let age = now_ms.saturating_sub(v.created_ms);
        if age <= policy.recent_end() {
            continue;
        }
        let day = v.created_ms / DAY_MS;
        let bucket = if age <= policy.daily_end() {
            (0u8, day)
        } else if age <= policy.weekly_end() {
            (1u8, day / 7)
        } else {
            doomed.insert(v.id.clone());
            continue;
        };
        // La primera de cada bloque (la más nueva) se queda.
        if !taken.insert(bucket) {
            doomed.insert(v.id.clone());
        }
    }

    // Topes de número y de tamaño: caen las más viejas que no estén protegidas.
    let mut count = sorted.iter().filter(|v| !doomed.contains(&v.id)).count();
    let mut bytes: u64 = sorted
        .iter()
        .filter(|v| !doomed.contains(&v.id))
        .map(|v| v.stored_bytes)
        .sum();
    for v in sorted.iter().rev() {
        if count <= policy.max_versions && bytes <= policy.max_bytes {
            break;
        }
        if doomed.contains(&v.id) || protected(v) || Some(&v.id) == newest.as_ref() {
            continue;
        }
        doomed.insert(v.id.clone());
        count -= 1;
        bytes = bytes.saturating_sub(v.stored_bytes);
    }

    sorted
        .into_iter()
        .rev()
        .filter(|v| doomed.contains(&v.id))
        .map(|v| v.id.clone())
        .collect()
}
