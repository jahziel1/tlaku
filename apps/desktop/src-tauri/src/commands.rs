//! Comandos de Tauri (`invoke`): adaptan argumentos y delegan en `engine` (motor y archivos)
//! y en `machine` (conexión). Los pesados corren con `spawn_blocking`, fuera del hilo principal.

use crate::engine::{self, Head};
use crate::machine::active::{ActiveMachine, Machine};
use crate::machine::galvo::{self, GalvoConnection, GalvoTimingCfg};
use crate::machine::lhymicro::{self, LhyConnection, LhyTiming};
use crate::machine::ruida::{self, RuidaConnection, RuidaTiming};
use crate::machine::{Connection, Emit, Handle, NOT_CONNECTED, Sink, Timing, frame, virtual_port};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager, State};
use tlaku_doc::{
    ConnectResult, DeviceKind, DeviceProfile, Document, ExportResult, GcodeDialect, ImportResult,
    JobOptions, JobPreview, OverrideKind, RealtimeCommand, SimulatedFault,
};
use tlaku_grbl::link::{LinkFamily, LinkProfile};
use tlaku_grbl::serial::{Grbl, SerialOptions};

/// Estado administrado por Tauri: la conexión con la máquina (si hay una). Es una sola, de
/// cualquier respaldo (GRBL, Ruida...): ver `machine::active`.
#[derive(Default)]
pub struct AppState {
    conn: Mutex<Option<ActiveMachine>>,
    /// Hay un `connect` en curso (abrir el puerto tarda ~2 s por el reinicio de la placa).
    connecting: AtomicBool,
}

impl AppState {
    /// Mando de la conexión viva, de cualquier respaldo. Si la conexión se perdió, la suelta y
    /// avisa.
    pub(crate) fn machine(&self) -> Result<Machine, String> {
        let mut g = self.conn.lock().map_err(|_| "estado interno dañado")?;
        match g.as_ref().map(ActiveMachine::handle) {
            Some(m) if !m.is_closed() => Ok(m),
            Some(_) => {
                *g = None;
                Err(NOT_CONNECTED.into())
            }
            None => Err(NOT_CONNECTED.into()),
        }
    }

    /// Mando GRBL de la conexión viva. Con otro respaldo (Ruida) falla explicando que la
    /// operación es solo de GRBL: así consola, overrides, ajustes `$` y macros no necesitan
    /// saber de los demás.
    pub(crate) fn handle(&self) -> Result<Handle, String> {
        self.machine()?.grbl().cloned()
    }

    /// Posición de trabajo de la cabeza, si hay máquina conectada con estado conocido.
    fn head(&self) -> Head {
        self.machine().ok().and_then(|m| m.work_pos())
    }

    /// Al cerrar la app: cancela un trabajo en curso y suelta el puerto.
    pub fn shutdown(&self) {
        if let Ok(mut g) = self.conn.lock()
            && let Some(c) = g.take()
        {
            c.close();
        }
    }
}

/// Emite los eventos de la conexión a la ventana (`machine://*`).
struct TauriSink(AppHandle);

impl Sink for TauriSink {
    fn emit(&self, event: Emit) {
        let name = event.name();
        let _ = match event {
            Emit::Status(p) => self.0.emit(name, p),
            Emit::Progress(p) => self.0.emit(name, p),
            Emit::Log(p) => self.0.emit(name, p),
            Emit::Job(p) => self.0.emit(name, p),
            Emit::Connection(p) => self.0.emit(name, p),
            Emit::Galvo(p) => self.0.emit(name, p),
        };
    }
}

/// Ejecuta `f` en el pool de hilos bloqueantes.
async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| format!("La tarea se interrumpió: {e}"))?
}

fn config_dir(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_config_dir()
        .map_err(|e| format!("No se encontró la carpeta de configuración: {e}"))
}

/// Permite que la interfaz muestre estas imágenes con `convertFileSrc` (protocolo de assets).
fn allow_images(app: &AppHandle, paths: impl IntoIterator<Item = PathBuf>) {
    let scope = app.asset_protocol_scope();
    for p in paths {
        let _ = scope.allow_file(p);
    }
}

// ---------------------------------------------------------------------------
// Aplicación
// ---------------------------------------------------------------------------

/// Comando de prueba: confirma que la interfaz habla con Rust.
#[tauri::command]
pub fn app_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

// ---------------------------------------------------------------------------
// Archivos y proyecto
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn import_file(app: AppHandle, path: String) -> Result<ImportResult, String> {
    let result = blocking(move || engine::import(&path)).await?;
    let images = result.objects.iter().filter_map(|o| match &o.kind {
        tlaku_doc::ObjectKind::Image(i) => Some(PathBuf::from(&i.source_path)),
        tlaku_doc::ObjectKind::Vector(_) | tlaku_doc::ObjectKind::Text(_) => None,
    });
    allow_images(&app, images);
    Ok(result)
}

#[tauri::command]
pub async fn save_project(path: String, doc: Document) -> Result<(), String> {
    blocking(move || engine::save_project(&path, &doc)).await
}

#[tauri::command]
pub async fn load_project(app: AppHandle, path: String) -> Result<Document, String> {
    let doc = blocking(move || engine::load_project(&path)).await?;
    allow_images(&app, engine::image_paths(&doc));
    Ok(doc)
}

// ---------------------------------------------------------------------------
// Trabajo
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn compute_job(
    state: State<'_, AppState>,
    doc: Document,
    device: DeviceProfile,
    options: JobOptions,
) -> Result<JobPreview, String> {
    let head = state.head();
    blocking(move || engine::compute_job(&doc, &device, &options, head)).await
}

/// Aviso de que el enlace serie del perfil no da abasto para el trabajo (diálogo de inicio).
/// Solo informa: nunca bloquea el inicio. `None` si el enlace alcanza o no aplica (red, Ruida...).
#[tauri::command]
pub async fn job_link_warning(
    state: State<'_, AppState>,
    doc: Document,
    device: DeviceProfile,
    options: JobOptions,
) -> Result<Option<String>, String> {
    let head = state.head();
    blocking(move || engine::link_warning(&doc, &device, &options, head)).await
}

#[tauri::command]
pub async fn export_gcode(
    state: State<'_, AppState>,
    doc: Document,
    device: DeviceProfile,
    options: JobOptions,
    path: String,
) -> Result<ExportResult, String> {
    let head = state.head();
    blocking(move || engine::export_gcode(&doc, &device, &options, &path, head)).await
}

// ---------------------------------------------------------------------------
// Dispositivos
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn list_devices(app: AppHandle) -> Result<Vec<DeviceProfile>, String> {
    let dir = config_dir(&app)?;
    blocking(move || engine::list_devices(&dir)).await
}

#[tauri::command]
pub async fn save_device(app: AppHandle, device: DeviceProfile) -> Result<(), String> {
    let dir = config_dir(&app)?;
    blocking(move || engine::save_device(&dir, device)).await
}

/// Ordena la lista de perfiles como `ids` (arrastrar o subir y bajar en el gestor).
#[tauri::command]
pub async fn reorder_devices(app: AppHandle, ids: Vec<String>) -> Result<(), String> {
    let dir = config_dir(&app)?;
    blocking(move || engine::reorder_devices(&dir, &ids)).await
}

#[tauri::command]
pub async fn delete_device(app: AppHandle, id: String) -> Result<(), String> {
    let dir = config_dir(&app)?;
    blocking(move || engine::delete_device(&dir, &id)).await
}

// ---------------------------------------------------------------------------
// Máquina
// ---------------------------------------------------------------------------

/// Puertos serie reales y, al final, las máquinas virtuales (`virtual:<perfil>`).
#[tauri::command]
pub async fn list_ports() -> Result<Vec<String>, String> {
    blocking(|| {
        let mut ports = tlaku_grbl::serial::list_ports()
            .map_err(|e| format!("No se pudieron listar los puertos serie: {e}"))?;
        // Las placas del K40 que haya por USB (`usb:k40:<n>`), antes de las máquinas virtuales.
        ports.extend(lhymicro::usb_ports());
        // Las tarjetas galvo (BJJCZ 9588:xxxx) que haya por USB (`usb:galvo:<n>`).
        ports.extend(galvo::usb_ports());
        ports.extend(virtual_port::ports());
        ports.extend(crate::netlink::virtual_ports());
        // La máquina virtual de Ruida (emulador UDP); la interfaz solo la ofrece con un perfil Ruida.
        ports.push(ruida::VIRTUAL_PORT.to_string());
        // La placa simulada del K40; la interfaz solo la ofrece con un perfil K40.
        ports.push(lhymicro::VIRTUAL_PORT.to_string());
        // La tarjeta galvo simulada; la interfaz solo la ofrece con un perfil galvo.
        ports.push(galvo::VIRTUAL_PORT.to_string());
        Ok(ports)
    })
    .await
}

#[tauri::command]
pub async fn connect(
    app: AppHandle,
    state: State<'_, AppState>,
    port: String,
    baud: u32,
    device: Option<DeviceProfile>,
    galvo_ack: Option<galvo::SafetyAck>,
) -> Result<ConnectResult, String> {
    if baud == 0 {
        return Err("Los baudios deben ser mayores que 0.".into());
    }
    if state.machine().is_ok() {
        return Err(
            "Ya hay una máquina conectada. Desconéctala antes de abrir otro puerto.".into(),
        );
    }
    if state.connecting.swap(true, Ordering::SeqCst) {
        return Err("Ya se está conectando con una máquina; espera un momento.".into());
    }
    // Abrir el puerto sin subir DTR (la placa no se reinicia), si el perfil lo pide.
    let serial_options = SerialOptions {
        no_dtr_reset: device.as_ref().is_some_and(|d| d.no_dtr_reset),
    };
    let sink: Arc<dyn Sink> = Arc::new(TauriSink(app));
    // Ruida: UDP a la IP del perfil (o su máquina virtual), sin puerto serie.
    if let Some(device) = device.clone().filter(|d| d.kind == DeviceKind::Ruida) {
        let result = blocking(move || {
            RuidaConnection::connect(&device, &port, sink, RuidaTiming::default())
        })
        .await;
        state.connecting.store(false, Ordering::SeqCst);
        let (conn, info) = result?;
        *state.conn.lock().map_err(|_| "estado interno dañado")? = Some(ActiveMachine::Ruida(conn));
        return Ok(info);
    }
    // K40 (M2 Nano): USB (`usb:k40:<n>`) o su máquina virtual, sin puerto serie.
    if let Some(device) = device.clone().filter(|d| d.kind == DeviceKind::LhyMicro) {
        let result =
            blocking(move || LhyConnection::connect(&device, &port, sink, LhyTiming::default()))
                .await;
        state.connecting.store(false, Ordering::SeqCst);
        let (conn, info) = result?;
        *state.conn.lock().map_err(|_| "estado interno dañado")? = Some(ActiveMachine::Lhy(conn));
        return Ok(info);
    }
    // Galvo (JCZ/BJJCZ): USB (`usb:galvo:<n>`, con el reconocimiento de seguridad de ESTA conexión)
    // o su tarjeta virtual. Sin el reconocimiento no se toca el USB.
    if let Some(device) = device.filter(|d| d.kind == DeviceKind::Galvo) {
        let result = blocking(move || {
            GalvoConnection::connect(
                &device,
                &port,
                galvo_ack.as_ref(),
                sink,
                GalvoTimingCfg::default(),
            )
        })
        .await;
        state.connecting.store(false, Ordering::SeqCst);
        let (conn, info) = result?;
        *state.conn.lock().map_err(|_| "estado interno dañado")? = Some(ActiveMachine::Galvo(conn));
        return Ok(info);
    }
    if galvo::is_virtual(&port) || galvo::usb_index(&port).is_some() {
        state.connecting.store(false, Ordering::SeqCst);
        return Err("La tarjeta galvo solo se abre con un perfil de tipo galvo.".into());
    }
    if ruida::is_virtual(&port) {
        state.connecting.store(false, Ordering::SeqCst);
        return Err("La máquina virtual Ruida solo se abre con un perfil de tipo Ruida.".into());
    }
    if lhymicro::is_virtual(&port) || lhymicro::usb_index(&port).is_some() {
        state.connecting.store(false, Ordering::SeqCst);
        return Err("La placa del K40 solo se abre con un perfil de tipo K40 (M2 Nano).".into());
    }
    let result = blocking(move || {
        // Red (Telnet/TCP, WebSocket o máquina virtual por red): sin puerto serie ni espera de
        // reinicio de la placa.
        if let Some(opened) = crate::netlink::open(&port) {
            let (transport, emu) = opened?;
            return Connection::start_with(transport, &port, sink, Timing::default(), emu);
        }
        if virtual_port::is_virtual(&port) {
            // Sin puerto serie: un emulador con reloj de pared (la velocidad va en el id).
            return Connection::start_virtual(&port, sink, Timing::default());
        }
        // Abrir el puerto reinicia muchas placas: `Grbl::open` espera el saludo y lo descarta.
        let transport = Grbl::open_with(&port, baud, serial_options)
            .map_err(|e| {
                format!(
                    "No se pudo abrir {port}: {e}. Revisa el cable, que la máquina esté \
                     encendida y que ningún otro programa use el puerto."
                )
            })?
            .into_transport();
        Connection::start(transport, &port, sink, Timing::default())
    })
    .await;
    state.connecting.store(false, Ordering::SeqCst);
    let (conn, info) = result?;
    *state.conn.lock().map_err(|_| "estado interno dañado")? = Some(ActiveMachine::Grbl(conn));
    Ok(info)
}

#[tauri::command]
pub async fn disconnect(state: State<'_, AppState>) -> Result<(), String> {
    let conn = state
        .conn
        .lock()
        .map_err(|_| "estado interno dañado")?
        .take();
    if let Some(c) = conn {
        blocking(move || {
            c.close();
            Ok(())
        })
        .await?;
    }
    Ok(())
}

/// Ejecuta `f` con el mando de la conexión activa (cualquier respaldo), fuera del hilo principal.
async fn with_machine<T: Send + 'static>(
    state: &State<'_, AppState>,
    f: impl FnOnce(Machine) -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    let m = state.machine()?;
    blocking(move || f(m)).await
}

/// Ejecuta `f` con el mando GRBL de la conexión, fuera del hilo principal. Con Ruida falla
/// explicando que la operación es solo de GRBL.
async fn with_handle<T: Send + 'static>(
    state: &State<'_, AppState>,
    f: impl FnOnce(Handle) -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    let h = state.handle()?;
    blocking(move || f(h)).await
}

#[tauri::command]
pub async fn machine_command(
    state: State<'_, AppState>,
    line: String,
) -> Result<Vec<String>, String> {
    with_handle(&state, move |h| h.command(&line)).await
}

#[tauri::command]
pub async fn machine_realtime(
    state: State<'_, AppState>,
    cmd: RealtimeCommand,
) -> Result<(), String> {
    with_machine(&state, move |m| m.realtime(cmd)).await
}

#[tauri::command]
pub async fn jog(state: State<'_, AppState>, dx: f64, dy: f64, speed: f64) -> Result<(), String> {
    with_machine(&state, move |m| m.jog(dx, dy, speed)).await
}

#[tauri::command]
pub async fn start_job(
    state: State<'_, AppState>,
    doc: Document,
    device: DeviceProfile,
    options: JobOptions,
) -> Result<(), String> {
    // Ruida: genera el `.rd` y lo sube (guardando o ejecutando, según el perfil).
    if let Machine::Ruida(r) = state.machine()? {
        return blocking(move || r.start_job(&doc, &device, &options)).await;
    }
    // K40: codifica LHYMICRO-GL y lo envía en el hilo de la conexión (hace home si hace falta).
    if let Machine::Lhy(l) = state.machine()? {
        return blocking(move || l.start_job(&doc, &device, &options)).await;
    }
    // Galvo: marca en el hilo de la conexión (con marcado dividido, repetido o rotativo recorre
    // las piezas y espera que muevas el material entre ellas).
    if let Machine::Galvo(g) = state.machine()? {
        return blocking(move || g.start_job(&doc, &device, &options, false)).await;
    }
    let h = state.handle()?;
    blocking(move || {
        let job = engine::job_gcode(&doc, &device, &options, h.work_pos())?;
        // Con la S máx del perfil, la conexión avisa si `$30` de la controladora es distinto. El
        // enlace (conteo de caracteres o una línea por `ok`) sale del tipo de máquina del perfil.
        // La carga del enlace solo sirve para avisar en el registro; no bloquea el envío.
        h.start_job_with_load(
            job.lines,
            LinkProfile::for_device(&device),
            Some(device.s_max),
            job.load,
        )
    })
    .await
}

// La interfaz manda los argumentos en snake_case (`power_pct`, `s_max`, `duration_ms`).
#[tauri::command(rename_all = "snake_case")]
pub async fn frame_job(
    state: State<'_, AppState>,
    doc: Document,
    device: DeviceProfile,
    options: JobOptions,
    speed: f64,
    power_pct: Option<f64>,
) -> Result<(), String> {
    // Ruida recorre el rectángulo con el láser apagado; no admite encuadre con potencia baja.
    if let Machine::Ruida(r) = state.machine()? {
        return blocking(move || r.frame_job(&doc, &device, &options, speed)).await;
    }
    // K40: igual, con el láser apagado y a la velocidad de salto de la placa.
    if let Machine::Lhy(l) = state.machine()? {
        return blocking(move || l.frame_job(&doc, &device, &options)).await;
    }
    // Galvo: solo con el puntero rojo (el láser nunca se habilita), en bucle hasta detener.
    if let Machine::Galvo(g) = state.machine()? {
        return blocking(move || g.frame_job(&doc, &device, &options, false)).await;
    }
    let h = state.handle()?;
    blocking(move || {
        // Con el láser apagado el que se ve es el puntero rojo: se desfasa su `pointer_offset`.
        let pointer = power_pct.is_none();
        let lines = engine::frame_lines(&doc, &device, &options, speed, h.work_pos(), pointer)?;
        let lines = match power_pct {
            Some(p) => frame::low_power(lines, &device, p)?,
            None => lines,
        };
        h.frame_link(lines, LinkProfile::for_device(&device))
    })
    .await
}

/// Encuadre de banda elástica: sigue el casco convexo del trabajo en lugar del rectángulo.
// La interfaz manda los argumentos en snake_case (`power_pct`, `s_max`, `duration_ms`).
#[tauri::command(rename_all = "snake_case")]
pub async fn frame_hull(
    state: State<'_, AppState>,
    doc: Document,
    device: DeviceProfile,
    options: JobOptions,
    speed: f64,
    power_pct: Option<f64>,
) -> Result<(), String> {
    // Ruida no tiene banda elástica: recorre el rectángulo envolvente.
    if let Machine::Ruida(r) = state.machine()? {
        return blocking(move || r.frame_job(&doc, &device, &options, speed)).await;
    }
    // El K40 tampoco: recorre el rectángulo envolvente.
    if let Machine::Lhy(l) = state.machine()? {
        return blocking(move || l.frame_job(&doc, &device, &options)).await;
    }
    // Galvo: el contorno del trabajo con el puntero rojo.
    if let Machine::Galvo(g) = state.machine()? {
        // Casco convexo de lo que se marca (no los trazos).
        return blocking(move || {
            g.frame(&doc, &device, &options, galvo::FrameKind::Hull, None, None)
        })
        .await;
    }
    let h = state.handle()?;
    blocking(move || {
        let pointer = power_pct.is_none();
        let lines = frame::hull_lines(&doc, &device, &options, speed, h.work_pos(), pointer)?;
        let lines = match power_pct {
            Some(p) => frame::low_power(lines, &device, p)?,
            None => lines,
        };
        h.frame_link(lines, LinkProfile::for_device(&device))
    })
    .await
}

/// Mueve la cabeza a (`x`, `y`) en coordenadas de trabajo (las de `work_pos`) con el láser
/// apagado. Para "posicionar el láser" con un clic: la interfaz convierte antes la posición del
/// documento con el origen de la máquina.
#[tauri::command]
pub async fn jog_to(state: State<'_, AppState>, x: f64, y: f64, speed: f64) -> Result<(), String> {
    with_machine(&state, move |m| m.jog_to(x, y, speed)).await
}

/// Familia del protocolo de un tipo de máquina (decide cómo se escribe el movimiento y la pausa).
fn family_of(kind: DeviceKind) -> LinkFamily {
    LinkProfile::for_kind(kind, &GcodeDialect::for_kind(kind), 127).family
}

/// Mueve un eje extra (`Z`, `U`...) `delta` mm respecto a donde está. Falla con un trabajo en
/// curso o en alarma.
#[tauri::command]
pub async fn jog_axis(
    state: State<'_, AppState>,
    kind: DeviceKind,
    axis: char,
    delta: f64,
    speed: f64,
) -> Result<(), String> {
    with_handle(&state, move |h| {
        h.jog_axis(family_of(kind), axis, delta, speed)
    })
    .await
}

/// Lleva un eje extra a la posición `pos` (coordenadas de trabajo, mm). Lo usa "Enfocar".
#[tauri::command]
pub async fn jog_axis_to(
    state: State<'_, AppState>,
    kind: DeviceKind,
    axis: char,
    pos: f64,
    speed: f64,
) -> Result<(), String> {
    with_handle(&state, move |h| {
        h.jog_axis_to(family_of(kind), axis, pos, speed)
    })
    .await
}

/// Disparo de prueba del láser: `power_pct` % de `s_max` durante `duration_ms`. Apaga el láser
/// siempre al terminar, aunque falle.
// La interfaz manda los argumentos en snake_case (`power_pct`, `s_max`, `duration_ms`).
#[tauri::command(rename_all = "snake_case")]
pub async fn fire_test(
    state: State<'_, AppState>,
    kind: DeviceKind,
    power_pct: f64,
    s_max: f64,
    duration_ms: u32,
) -> Result<(), String> {
    with_handle(&state, move |h| {
        h.fire(family_of(kind), power_pct, s_max, duration_ms)
    })
    .await
}

/// Lleva el override en vivo de velocidad o potencia a `percent` por ciento (10 a 200). Solo
/// GRBL; con un trabajo en curso también vale.
#[tauri::command]
pub async fn set_override(
    state: State<'_, AppState>,
    kind: OverrideKind,
    percent: u32,
) -> Result<(), String> {
    with_handle(&state, move |h| h.set_override(kind, percent)).await
}

#[tauri::command]
pub async fn pause_job(state: State<'_, AppState>) -> Result<(), String> {
    with_machine(&state, |m| m.pause()).await
}

#[tauri::command]
pub async fn resume_job(state: State<'_, AppState>) -> Result<(), String> {
    with_machine(&state, |m| m.resume()).await
}

#[tauri::command]
pub async fn cancel_job(state: State<'_, AppState>) -> Result<(), String> {
    with_machine(&state, |m| m.cancel()).await
}

/// Provoca una falla en la máquina virtual conectada (llama, alarma 3, desconexión) para ver
/// cómo reacciona la interfaz. Con una máquina real devuelve un error.
#[tauri::command]
pub async fn simulate_fault(
    state: State<'_, AppState>,
    fault: SimulatedFault,
) -> Result<(), String> {
    with_handle(&state, move |h| h.simulate_fault(fault)).await
}

// ---------------------------------------------------------------------------
// Ruida (solo lectura; EXPERIMENTAL)
// ---------------------------------------------------------------------------

/// Información de la controladora Ruida conectada: modelo, tarjeta, cama, velocidades y
/// contadores (M25 y M26). Solo lectura.
#[tauri::command]
pub async fn ruida_info(state: State<'_, AppState>) -> Result<tlaku_doc::RuidaInfo, String> {
    with_machine(&state, |m| m.ruida()?.info()).await
}

/// Documentos guardados en la memoria de la controladora Ruida (M24). Solo lectura.
#[tauri::command]
pub async fn ruida_files(state: State<'_, AppState>) -> Result<Vec<tlaku_doc::RuidaFile>, String> {
    with_machine(&state, |m| m.ruida()?.files()).await
}

// ---------------------------------------------------------------------------
// K40 con placa M2 Nano (LHYMICRO-GL por USB; EXPERIMENTAL)
// ---------------------------------------------------------------------------

/// Calcula y codifica el trabajo sin enviarlo: devuelve los avisos del codificador (la potencia
/// se ignora, los grises se binarizan...) para mostrarlos antes de empezar. No necesita conexión;
/// si la hay, parte de la posición actual del cabezal.
#[tauri::command]
pub async fn lhy_preview(
    state: State<'_, AppState>,
    doc: Document,
    device: DeviceProfile,
    options: JobOptions,
) -> Result<lhymicro::LhyPreview, String> {
    let head = state
        .machine()
        .ok()
        .and_then(|m| m.lhy().ok().and_then(|l| l.work_pos()));
    blocking(move || lhymicro::preview(&doc, &device, &options, head)).await
}

/// Lleva el cabezal al home (finales de carrera). La placa no informa su posición: se cuenta
/// desde aquí.
#[tauri::command]
pub async fn lhy_home(state: State<'_, AppState>) -> Result<(), String> {
    with_machine(&state, |m| m.lhy()?.home()).await
}

/// Estado propio de la conexión K40: placa, si se conoce la posición y el último código de estado.
#[tauri::command]
pub async fn lhy_state(state: State<'_, AppState>) -> Result<lhymicro::LhyState, String> {
    let m = state.machine()?;
    Ok(m.lhy()?.state())
}

// ---------------------------------------------------------------------------
// Galvo (tarjetas JCZ/BJJCZ por USB; EXPERIMENTAL)
// ---------------------------------------------------------------------------

/// Calcula el trabajo galvo sin enviarlo: pasos del marcado dividido, repetido o rotativo, veces
/// que habrá que mover el material, tiempo y avisos del pipeline. No necesita conexión.
#[tauri::command]
pub async fn galvo_preview(
    doc: Document,
    device: DeviceProfile,
    options: JobOptions,
) -> Result<galvo::GalvoPreview, String> {
    blocking(move || galvo::preview(&doc, &device, &options)).await
}

/// Estado propio de la conexión galvo: tarjeta, estado, entradas (solo lectura), puntero rojo,
/// «la fuente puede estar encendida» y si el trabajo espera que muevas el material.
#[tauri::command]
pub async fn galvo_state(state: State<'_, AppState>) -> Result<galvo::GalvoState, String> {
    let m = state.machine()?;
    Ok(m.galvo()?.state())
}

/// Recorrido del trabajo con el láser DESHABILITADO y el puntero rojo (la fuente nunca se arma).
#[tauri::command]
pub async fn galvo_red_run(
    state: State<'_, AppState>,
    doc: Document,
    device: DeviceProfile,
    options: JobOptions,
) -> Result<(), String> {
    let m = state.machine()?;
    blocking(move || m.galvo()?.start_job(&doc, &device, &options, true)).await
}

/// Enciende o apaga el puntero rojo en su sitio (la fuente queda deshabilitada).
#[tauri::command]
pub async fn galvo_red_light(state: State<'_, AppState>, on: bool) -> Result<(), String> {
    with_machine(&state, move |m| m.galvo()?.set_red_light(on)).await
}

/// Encuadre galvo con el puntero rojo de la forma pedida (rectángulo, trazos, casco convexo u
/// objetos sueltos), en bucle hasta detener o durante `loops` vueltas. El láser nunca se habilita.
#[tauri::command(rename_all = "snake_case")]
pub async fn galvo_frame(
    state: State<'_, AppState>,
    doc: Document,
    device: DeviceProfile,
    options: JobOptions,
    shape: galvo::FrameKind,
    loops: Option<u32>,
    gap_mm: Option<f64>,
) -> Result<(), String> {
    let m = state.machine()?;
    blocking(move || {
        m.galvo()?
            .frame(&doc, &device, &options, shape, loops, gap_mm)
    })
    .await
}

/// Mueve (mm) o escala el encuadre galvo en curso sin detenerlo.
#[tauri::command(rename_all = "snake_case")]
pub async fn galvo_adjust_frame(
    state: State<'_, AppState>,
    dx: f64,
    dy: f64,
    scale: f64,
) -> Result<(), String> {
    with_machine(&state, move |m| m.galvo()?.adjust_frame(dx, dy, scale)).await
}

/// Verificación SIN LÁSER de un perfil galvo (asistente de alta): lee la tarjeta y comprueba que el
/// perfil compila. Con una tarjeta real exige el reconocimiento de seguridad de esta llamada.
#[tauri::command(rename_all = "snake_case")]
pub async fn galvo_verify(
    device: DeviceProfile,
    port: String,
    galvo_ack: Option<galvo::SafetyAck>,
) -> Result<galvo::GalvoVerifyReport, String> {
    blocking(move || galvo::verify(&device, &port, galvo_ack.as_ref())).await
}

/// El usuario movió el material (o el rotativo): el trabajo dividido o repetido sigue.
#[tauri::command]
pub async fn galvo_confirm_move(state: State<'_, AppState>) -> Result<(), String> {
    with_machine(&state, |m| m.galvo()?.confirm_move()).await
}
