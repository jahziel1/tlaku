//! Tiempos, la subida ([`Upload`]), solicitudes al hilo y el mando [`RuidaHandle`].

use super::*;
use std::path::PathBuf;
use tlaku_doc::{
    RuidaApplyReport, RuidaCalibration, RuidaControllerSettings, RuidaDocResult,
    RuidaSettingChange, RuidaSettingsPlan,
};
use tlaku_i18n::tr;

// ---------------------------------------------------------------------------
// Tiempos
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct RuidaTiming {
    /// Cada cuánto se lee el estado en reposo.
    pub status_interval: Duration,
    /// Lo mismo mientras se recorre un encuadre.
    pub frame_interval: Duration,
    /// Espera del ACK y de la respuesta de una lectura al sondear el estado. Corta: si no
    /// contesta, el siguiente sondeo ya la intenta de nuevo.
    pub poll_timeout: Duration,
    /// Espera del ACK de cada paquete al subir un trabajo.
    pub upload_ack: Duration,
    /// Sondeos seguidos sin respuesta (fuera de un trabajo) tras los cuales se da por perdida
    /// la conexión.
    pub lost_after: u32,
    /// Tras subir un trabajo en modo ejecutar, cuánto se espera a ver la máquina en marcha antes
    /// de darlo por terminado si ya está en reposo (un trabajo muy corto).
    pub start_grace: Duration,
}

impl Default for RuidaTiming {
    fn default() -> Self {
        Self {
            status_interval: Duration::from_millis(500),
            frame_interval: Duration::from_millis(200),
            poll_timeout: Duration::from_millis(400),
            upload_ack: Duration::from_secs(1),
            lost_after: 3,
            start_grace: Duration::from_secs(5),
        }
    }
}

// ---------------------------------------------------------------------------
// Mando
// ---------------------------------------------------------------------------

/// Lo que hace falta para subir un trabajo.
pub struct Upload {
    /// Flujo `.rd` en claro (`tlaku_pipeline::export_rd_for_device`).
    pub rd: Vec<u8>,
    /// Ejecutar al llegar o guardar como documento.
    pub mode: SendMode,
    /// Cama y tope de velocidad del perfil (los mismos con que se generó el archivo).
    pub limits: SafetyLimits,
    /// Esquina del home con que se generó el archivo.
    pub home: HomeCorner,
    /// Duración estimada del trabajo; solo la usa la máquina virtual para simularlo.
    pub estimate_s: f64,
    /// Avisos para el registro antes de enviar (p. ej. el rotativo).
    pub notes: Vec<String>,
    /// Con `Store`: si ya hay un documento con ese nombre, borrarlo antes de subir (el usuario
    /// lo confirmó). Sin esto, un nombre repetido falla sin tocar nada.
    pub overwrite: bool,
    /// Con `Store`: el nombre es el automático. Si ya existe uno igual se elige otro libre.
    pub auto_name: bool,
}

pub(super) type Reply<T> = Sender<Result<T, String>>;

pub(super) enum Request {
    Upload(Box<Upload>, Reply<()>),
    /// Recorre el rectángulo con el láser apagado. Esquinas en mm del sistema de la máquina.
    Frame {
        corners: Vec<(f64, f64)>,
        speed_mm_min: f64,
        reply: Reply<()>,
    },
    Jog {
        dx: f64,
        dy: f64,
        speed_mm_min: f64,
        reply: Reply<()>,
    },
    JogTo {
        x: f64,
        y: f64,
        speed_mm_min: f64,
        reply: Reply<()>,
    },
    Pause(Reply<()>),
    Resume(Reply<()>),
    Stop(Reply<()>),
    Info(Reply<Box<RuidaInfo>>),
    Files(Reply<Vec<RuidaFile>>),
    SettingsRead(Reply<RuidaControllerSettings>),
    SettingsPreview(Vec<RuidaSettingChange>, Reply<RuidaSettingsPlan>),
    SettingsPreviewFile(String, Reply<RuidaSettingsPlan>),
    SettingsApply {
        changes: Vec<RuidaSettingChange>,
        confirmed: bool,
        dir: PathBuf,
        reply: Reply<RuidaApplyReport>,
    },
    SettingsRestore {
        json: String,
        confirmed: bool,
        dir: PathBuf,
        reply: Reply<RuidaApplyReport>,
    },
    SettingsExport(Reply<String>),
    Calibrate {
        key: String,
        commanded_mm: f64,
        measured_mm: f64,
        reply: Reply<RuidaCalibration>,
    },
    FileDelete {
        index: u32,
        name: String,
        confirmed: bool,
        reply: Reply<RuidaDocResult>,
    },
    FileRun {
        index: u32,
        name: String,
        confirmed: bool,
        reply: Reply<RuidaDocResult>,
    },
    Disconnect,
}

/// Nombre del documento que se guarda y si puede reemplazar a uno existente; lo fija la interfaz
/// justo al confirmar el inicio (`ruida_prepare_store`). Vale para UN envío y caduca: así un
/// permiso de reemplazar que quedó de un diálogo cancelado no sirve para un inicio posterior.
#[derive(Debug, Default, Clone)]
pub(super) struct StorePlan {
    pub(super) name: Option<String>,
    pub(super) overwrite: bool,
    pub(super) at: Option<Instant>,
}

/// Cuánto tiempo vale un [`StorePlan`] desde que la interfaz lo fijó.
pub(super) const STORE_PLAN_TTL: Duration = Duration::from_secs(120);

#[derive(Default)]
pub(super) struct Shared {
    /// El hilo terminó (desconexión pedida o perdida).
    pub(super) dead: AtomicBool,
    /// Se pidió cancelar: la subida en curso se interrumpe entre paquetes.
    pub(super) cancel: AtomicBool,
    pub(super) status: Mutex<Option<StatusReport>>,
    pub(super) virtual_machine: AtomicBool,
    pub(super) store: Mutex<StorePlan>,
}

/// Mando de la conexión; se clona y se usa desde cualquier hilo. Las llamadas bloquean hasta que
/// el hilo responde (en Tauri, dentro de `spawn_blocking`).
#[derive(Clone)]
pub struct RuidaHandle {
    pub(super) tx: Sender<Request>,
    pub(super) shared: Arc<Shared>,
}

/// Las escrituras en la controladora (ajustes, calibración, rotativo, borrar, ejecutar y reemplazar
/// documentos) solo se aceptan si la app pasó la bandera de la preferencia `ruidaExperimentalWrites`
/// (apagada de fábrica). Segunda comprobación, independiente de la interfaz.
fn require_writes(experimental_writes: bool) -> Result<(), String> {
    if experimental_writes {
        Ok(())
    } else {
        Err(tr!("desktop.ruida.writes_disabled"))
    }
}

impl RuidaHandle {
    fn call<T>(&self, build: impl FnOnce(Reply<T>) -> Request) -> Result<T, String> {
        if self.is_closed() {
            return Err(crate::machine::not_connected());
        }
        let (tx, rx) = mpsc::channel();
        self.tx
            .send(build(tx))
            .map_err(|_| crate::machine::not_connected())?;
        rx.recv()
            .map_err(|_| tr!("desktop.machine.connection_lost"))?
    }

    pub fn is_closed(&self) -> bool {
        self.shared.dead.load(Ordering::SeqCst)
    }

    #[cfg(test)]
    pub fn is_virtual(&self) -> bool {
        self.shared.virtual_machine.load(Ordering::SeqCst)
    }

    /// Posición del cabezal en mm del sistema de la máquina, del último sondeo.
    pub fn work_pos(&self) -> Option<[f64; 2]> {
        self.shared.status.lock().ok()?.as_ref().map(|s| s.work_pos)
    }

    /// Último reporte de estado completo (para la automatización: `STATUS`).
    pub fn status_report(&self) -> Option<StatusReport> {
        self.shared.status.lock().ok()?.clone()
    }

    /// Calcula el trabajo, genera el `.rd` y lo sube: ejecuta o guarda según el perfil
    /// (`ruida.run_after_upload`).
    pub fn start_job(
        &self,
        doc: &tlaku_doc::Document,
        device: &DeviceProfile,
        options: &tlaku_doc::JobOptions,
    ) -> Result<(), String> {
        let mut up = build_upload(doc, device, options, self.work_pos())?;
        // El nombre elegido por el usuario manda sobre el automático. El plan vale para UN envío y
        // caduca: se consume aquí.
        if let SendMode::Store { name } = &mut up.mode {
            let plan =
                std::mem::take(&mut *self.shared.store.lock().unwrap_or_else(|e| e.into_inner()));
            if plan.at.is_some_and(|t| t.elapsed() <= STORE_PLAN_TTL) {
                if let Some(chosen) = plan.name {
                    *name = chosen;
                    up.auto_name = false;
                }
                up.overwrite = plan.overwrite;
            }
        }
        self.upload(up)
    }

    /// Encuadre del trabajo (rectángulo de todos sus movimientos) con el láser apagado, a
    /// `speed` mm/min.
    pub fn frame_job(
        &self,
        doc: &tlaku_doc::Document,
        device: &DeviceProfile,
        options: &tlaku_doc::JobOptions,
        speed: f64,
    ) -> Result<(), String> {
        if !(speed.is_finite() && speed > 0.0) {
            return Err(tr!("desktop.common.frame_speed"));
        }
        let corners = ruida_frame_corners(doc, device, options, self.work_pos())?;
        self.frame(corners, speed)
    }

    /// Sube el trabajo. Vuelve cuando terminó de subirse (con `Run` el trabajo sigue en la
    /// máquina; su final llega como evento).
    pub fn upload(&self, up: Upload) -> Result<(), String> {
        self.call(|reply| Request::Upload(Box::new(up), reply))
    }

    /// Recorre el rectángulo de `corners` (mm del sistema de la máquina) sin disparar el láser.
    /// Vuelve en cuanto empieza; el hilo sigue el recorrido.
    pub fn frame(&self, corners: Vec<(f64, f64)>, speed_mm_min: f64) -> Result<(), String> {
        self.call(|reply| Request::Frame {
            corners,
            speed_mm_min,
            reply,
        })
    }

    /// Movimiento relativo (mm del sistema de la máquina) con el láser apagado.
    pub fn jog(&self, dx: f64, dy: f64, speed_mm_min: f64) -> Result<(), String> {
        self.call(|reply| Request::Jog {
            dx,
            dy,
            speed_mm_min,
            reply,
        })
    }

    /// Movimiento absoluto (mm del sistema de la máquina) con el láser apagado.
    pub fn jog_to(&self, x: f64, y: f64, speed_mm_min: f64) -> Result<(), String> {
        self.call(|reply| Request::JogTo {
            x,
            y,
            speed_mm_min,
            reply,
        })
    }

    pub fn pause(&self) -> Result<(), String> {
        self.call(Request::Pause)
    }

    pub fn resume(&self) -> Result<(), String> {
        self.call(Request::Resume)
    }

    /// Detiene el trabajo (`D8 01`); durante una subida, la interrumpe antes.
    pub fn cancel(&self) -> Result<(), String> {
        self.shared.cancel.store(true, Ordering::SeqCst);
        let r = self.call(Request::Stop);
        // Si la conexión ya no existe, la bandera no importa; si la hay, el hilo la apaga.
        if r.is_err() {
            self.shared.cancel.store(false, Ordering::SeqCst);
        }
        r
    }

    /// Información de la controladora (M25 y M26), solo lectura.
    pub fn info(&self) -> Result<RuidaInfo, String> {
        self.call(Request::Info).map(|b| *b)
    }

    /// Documentos guardados en la memoria (M24), solo lectura y experimental.
    pub fn files(&self) -> Result<Vec<RuidaFile>, String> {
        self.call(Request::Files)
    }

    /// Fija el nombre con que se guardará el próximo trabajo (M23) y si puede reemplazar a un
    /// documento que ya se llame así (el usuario lo confirmó). `name = None` vuelve al nombre
    /// automático. Un nombre inválido se rechaza aquí.
    pub fn prepare_store(
        &self,
        name: Option<String>,
        overwrite: bool,
        experimental_writes: bool,
    ) -> Result<(), String> {
        // Reemplazar borra un documento: es una escritura. Elegir un nombre, no.
        if overwrite {
            require_writes(experimental_writes)?;
        }
        if let Some(n) = &name
            && !tlaku_ruida::session::valid_doc_name(n)
        {
            return Err(tr!("desktop.ruida.bad_store_name", name = n));
        }
        *self.shared.store.lock().unwrap_or_else(|e| e.into_inner()) = StorePlan {
            name,
            overwrite,
            at: Some(Instant::now()),
        };
        Ok(())
    }

    /// Todos los ajustes escribibles, leídos de la controladora (EXPERIMENTAL).
    pub fn settings_read(&self) -> Result<RuidaControllerSettings, String> {
        self.call(Request::SettingsRead)
    }

    /// Diferencias que produciría una petición, sin escribir nada.
    pub fn settings_preview(
        &self,
        changes: Vec<RuidaSettingChange>,
    ) -> Result<RuidaSettingsPlan, String> {
        self.call(|reply| Request::SettingsPreview(changes, reply))
    }

    /// Diferencias de un respaldo `.tlakuruida` (su texto JSON) con la máquina.
    pub fn settings_preview_file(&self, json: String) -> Result<RuidaSettingsPlan, String> {
        self.call(|reply| Request::SettingsPreviewFile(json, reply))
    }

    /// Escribe ajustes: respaldo previo en `dir`, confirmación, escritura y verificación.
    pub fn settings_apply(
        &self,
        changes: Vec<RuidaSettingChange>,
        confirmed: bool,
        dir: PathBuf,
        experimental_writes: bool,
    ) -> Result<RuidaApplyReport, String> {
        require_writes(experimental_writes)?;
        self.call(|reply| Request::SettingsApply {
            changes,
            confirmed,
            dir,
            reply,
        })
    }

    /// Restaura un respaldo `.tlakuruida` con las mismas protecciones que `settings_apply`.
    pub fn settings_restore(
        &self,
        json: String,
        confirmed: bool,
        dir: PathBuf,
        experimental_writes: bool,
    ) -> Result<RuidaApplyReport, String> {
        require_writes(experimental_writes)?;
        self.call(|reply| Request::SettingsRestore {
            json,
            confirmed,
            dir,
            reply,
        })
    }

    /// Texto JSON de un respaldo manual de lo que hay ahora en la máquina.
    pub fn settings_export(&self) -> Result<String, String> {
        self.call(Request::SettingsExport)
    }

    /// Calcula la corrección de un paso de eje a partir de una distancia medida.
    pub fn calibrate(
        &self,
        key: String,
        commanded_mm: f64,
        measured_mm: f64,
        experimental_writes: bool,
    ) -> Result<RuidaCalibration, String> {
        require_writes(experimental_writes)?;
        self.call(|reply| Request::Calibrate {
            key,
            commanded_mm,
            measured_mm,
            reply,
        })
    }

    /// Borra un documento de la memoria (pide confirmación; EXPERIMENTAL).
    pub fn file_delete(
        &self,
        index: u32,
        name: String,
        confirmed: bool,
        experimental_writes: bool,
    ) -> Result<RuidaDocResult, String> {
        require_writes(experimental_writes)?;
        self.call(|reply| Request::FileDelete {
            index,
            name,
            confirmed,
            reply,
        })
    }

    /// Ejecuta un documento de la memoria. La interfaz ya pasó la lista de comprobación.
    pub fn file_run(
        &self,
        index: u32,
        name: String,
        confirmed: bool,
        experimental_writes: bool,
    ) -> Result<RuidaDocResult, String> {
        require_writes(experimental_writes)?;
        self.call(|reply| Request::FileRun {
            index,
            name,
            confirmed,
            reply,
        })
    }
}
