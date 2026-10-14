//! Ajustes y documentos de la controladora (M23 a M28). EXPERIMENTAL.
//!
//! Todo lo que cambia la máquina pasa por las protecciones de `tlaku_ruida::session`: lista
//! cerrada, respaldo previo en un archivo con fecha, confirmación explícita, máquina en reposo y
//! verificación posterior.

use super::*;
use std::path::PathBuf;
use tlaku_doc::{
    RuidaApplyReport, RuidaCalibration, RuidaControllerSettings, RuidaDocResult,
    RuidaPlannedChange, RuidaSetting, RuidaSettingChange, RuidaSettingsPlan, RuidaVerified,
};
use tlaku_i18n::tr;
use tlaku_ruida::session::ApplyReport;
use tlaku_ruida::settings::{
    self, PlannedChange, SettingChange, SettingKind, SettingsFile, SettingsSnapshot,
};

fn kind_name(k: SettingKind) -> &'static str {
    match k {
        SettingKind::Speed => "speed",
        SettingKind::Accel => "accel",
        SettingKind::Length => "length",
        SettingKind::Step => "step",
        SettingKind::Raw => "raw",
    }
}

fn to_settings(snap: &SettingsSnapshot) -> RuidaControllerSettings {
    RuidaControllerSettings {
        model: snap.model.clone(),
        card_id: format!("0x{:08X}", snap.card_id),
        settings: settings::WRITABLE
            .iter()
            .map(|d| RuidaSetting {
                key: d.key.to_string(),
                addr: format!("0x{:03X}", d.addr),
                group: d.group.key().to_string(),
                kind: kind_name(d.kind).to_string(),
                value: snap.get(d.addr).map(|v| v as f64),
                min: d.min as f64,
                max: d.max as f64,
                source_name: d.source_name.to_string(),
            })
            .collect(),
    }
}

fn planned(c: &PlannedChange) -> RuidaPlannedChange {
    RuidaPlannedChange {
        key: c.key.clone(),
        addr: format!("0x{:03X}", c.addr),
        current: c.current as f64,
        new: c.new as f64,
    }
}

fn to_plan(p: &settings::SettingsPlan, warnings: Vec<String>) -> RuidaSettingsPlan {
    RuidaSettingsPlan {
        changes: p.changes.iter().map(planned).collect(),
        unchanged: u32::try_from(p.unchanged).unwrap_or(u32::MAX),
        warnings,
    }
}

fn to_report(r: &ApplyReport) -> RuidaApplyReport {
    RuidaApplyReport {
        changes: r.plan.changes.iter().map(planned).collect(),
        unchanged: u32::try_from(r.plan.unchanged).unwrap_or(u32::MAX),
        backup: r.backup.clone(),
        verified: r
            .verified
            .iter()
            .map(|v| RuidaVerified {
                key: v.key.clone(),
                expected: v.expected as f64,
                actual: v.actual.map(|a| a as f64),
                ok: v.matches(),
            })
            .collect(),
        write_error: r.write_error.as_ref().map(|(addr, why)| {
            tr!(
                "desktop.ruida.write_error",
                addr = format!("0x{addr:03X}"),
                why = why
            )
        }),
        ok: r.is_ok(),
        restore_advised: r.restore_advised(),
    }
}

/// Cambios de la interfaz (por clave) a cambios del motor (por dirección). Un valor que no es
/// un entero exacto o una clave fuera de la lista se rechaza con su motivo.
fn to_changes(changes: &[RuidaSettingChange]) -> Result<Vec<SettingChange>, String> {
    changes
        .iter()
        .map(|c| {
            let def = settings::find_key(&c.key)
                .ok_or_else(|| tr!("desktop.ruida.unknown_setting", key = c.key))?;
            if !c.value.is_finite() || c.value.fract() != 0.0 || c.value.abs() > 1e15 {
                return Err(tr!(
                    "desktop.ruida.setting_not_integer",
                    key = c.key,
                    value = c.value
                ));
            }
            Ok(SettingChange {
                addr: def.addr,
                value: c.value as i64,
            })
        })
        .collect()
}

impl<L: PacketLink> Worker<L> {
    /// Las operaciones de ajustes y documentos no se mezclan con un trabajo ni con un encuadre.
    fn ensure_free(&self) -> Result<(), String> {
        match self.busy_error() {
            Some(e) => Err(e),
            None => Ok(()),
        }
    }

    pub(super) fn settings_read(&mut self) -> Result<RuidaControllerSettings, String> {
        self.ensure_free()?;
        self.log(LogDir::Out, &tr!("desktop.ruida.log_settings_read"));
        let snap = self.session.read_settings().map_err(op_error)?;
        Ok(to_settings(&snap))
    }

    pub(super) fn settings_preview(
        &mut self,
        changes: &[RuidaSettingChange],
    ) -> Result<RuidaSettingsPlan, String> {
        self.ensure_free()?;
        let req = to_changes(changes)?;
        let (_, plan) = self.session.preview_settings(&req).map_err(op_error)?;
        Ok(to_plan(&plan, Vec::new()))
    }

    /// Diferencias de un respaldo `.tlakuruida` con la máquina. No escribe nada.
    pub(super) fn settings_preview_file(
        &mut self,
        json: &str,
    ) -> Result<RuidaSettingsPlan, String> {
        self.ensure_free()?;
        let file = settings::parse_file(json).map_err(|e| e.to_string())?;
        let (snap, plan) = self
            .session
            .preview_settings(&file.changes())
            .map_err(op_error)?;
        let mut warnings = Vec::new();
        let card = file.card().map_err(|e| e.to_string())?;
        if card != snap.card_id {
            // Aviso y no error: la vista previa sirve para ver qué pasa; restaurar sí lo rechaza.
            warnings.push(tr!(
                "desktop.ruida.backup_other_card",
                file = format!("{card:08X}"),
                machine = format!("{:08X}", snap.card_id)
            ));
        }
        Ok(to_plan(&plan, warnings))
    }

    pub(super) fn settings_apply(
        &mut self,
        changes: &[RuidaSettingChange],
        confirmed: bool,
        dir: &std::path::Path,
    ) -> Result<RuidaApplyReport, String> {
        self.ensure_free()?;
        let req = to_changes(changes)?;
        self.log(
            LogDir::Out,
            &tr!("desktop.ruida.log_settings_apply", n = req.len()),
        );
        let dir: PathBuf = dir.to_path_buf();
        let report = self
            .session
            .apply_settings(&req, confirmed, &mut |snap| backup::save_now(&dir, snap))
            .map_err(op_error)?;
        self.log_report(&report);
        Ok(to_report(&report))
    }

    pub(super) fn settings_restore(
        &mut self,
        json: &str,
        confirmed: bool,
        dir: &std::path::Path,
    ) -> Result<RuidaApplyReport, String> {
        self.ensure_free()?;
        let file: SettingsFile = settings::parse_file(json).map_err(|e| e.to_string())?;
        self.log(LogDir::Out, &tr!("desktop.ruida.log_settings_restore"));
        let dir: PathBuf = dir.to_path_buf();
        let report = self
            .session
            .restore_settings(&file, confirmed, &mut |snap| backup::save_now(&dir, snap))
            .map_err(op_error)?;
        self.log_report(&report);
        Ok(to_report(&report))
    }

    fn log_report(&self, r: &ApplyReport) {
        if let Some(b) = &r.backup {
            self.log(LogDir::In, &tr!("desktop.ruida.log_backup_saved", path = b));
        }
        for c in &r.plan.changes {
            self.log(
                LogDir::In,
                &format!("; {} (0x{:03X}): {} -> {}", c.key, c.addr, c.current, c.new),
            );
        }
        if r.is_ok() {
            self.log(LogDir::In, &tr!("desktop.ruida.log_settings_verified"));
        } else {
            self.log(LogDir::In, &tr!("desktop.ruida.log_settings_mismatch"));
        }
    }

    /// Texto JSON del respaldo manual (`.tlakuruida`) de lo que hay ahora en la máquina.
    pub(super) fn settings_export(&mut self) -> Result<String, String> {
        self.ensure_free()?;
        let snap = self.session.read_settings().map_err(op_error)?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        Ok(SettingsFile::from_snapshot(&snap, &settings::format_utc(now)).to_json())
    }

    pub(super) fn calibrate(
        &mut self,
        key: &str,
        commanded_mm: f64,
        measured_mm: f64,
    ) -> Result<RuidaCalibration, String> {
        self.ensure_free()?;
        let def = settings::find_key(key)
            .filter(|d| d.kind == SettingKind::Step)
            .ok_or_else(|| tr!("desktop.ruida.not_a_step", key = key))?;
        let raw = self.session.read_mem(def.addr).map_err(op_error)?;
        let c = settings::calibrate_step(def.decode(raw), commanded_mm, measured_mm)
            .map_err(|e| e.to_string())?;
        Ok(RuidaCalibration {
            key: key.to_string(),
            current: c.current as f64,
            proposed: c.proposed as f64,
            ratio: c.ratio,
            steps_per_mm_before: c.steps_per_mm_before,
            steps_per_mm_after: c.steps_per_mm_after,
        })
    }

    pub(super) fn file_delete(
        &mut self,
        index: u32,
        name: &str,
        confirmed: bool,
    ) -> Result<RuidaDocResult, String> {
        self.ensure_free()?;
        self.log(
            LogDir::Out,
            &tr!("desktop.ruida.log_file_delete", index = index, name = name),
        );
        let r = self
            .session
            .delete_document(index, name, confirmed)
            .map_err(op_error)?;
        if r.verified() {
            self.log(LogDir::In, &tr!("desktop.ruida.log_file_deleted"));
        } else {
            self.log(LogDir::In, &tr!("desktop.ruida.log_file_not_deleted"));
        }
        Ok(RuidaDocResult {
            index,
            name: r.name.clone(),
            confirmed: r.verified(),
        })
    }

    pub(super) fn file_run(
        &mut self,
        index: u32,
        name: &str,
        confirmed: bool,
    ) -> Result<RuidaDocResult, String> {
        self.ensure_free()?;
        self.log(
            LogDir::Out,
            &tr!("desktop.ruida.log_file_run", index = index, name = name),
        );
        let r = self
            .session
            .run_document(index, name, confirmed)
            .map_err(op_error)?;
        if r.started {
            if let Some(sim) = self.sim.as_mut() {
                // La máquina virtual no sabe cuánto dura un documento guardado: unos segundos.
                sim.start(5.0);
            }
            // Desde aquí el trabajo se sigue como cualquier otro en marcha.
            self.job = Some(ActiveJob {
                since: Instant::now(),
                seen_running: true,
                paused: false,
            });
            self.job_event(
                JobState::Started,
                &tr!("desktop.ruida.job_doc_running", name = name),
            );
        } else {
            self.log(LogDir::In, &tr!("desktop.ruida.log_file_not_started"));
        }
        Ok(RuidaDocResult {
            index,
            name: r.name.clone(),
            confirmed: r.started,
        })
    }
}
