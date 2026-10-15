//! Comandos del área `nest` (ola 3, D76): acomodo de piezas sobre `tlaku-nest`.
//!
//! El cálculo corre en un hilo propio y la interfaz lo sondea: es lo más simple y fiable (no
//! depende de que lleguen eventos). Hay un único trabajo a la vez.
//!
//! - `nest_start(request)` valida, lanza el hilo y devuelve el `job_id`. Uno nuevo cancela al
//!   anterior.
//! - `nest_poll(job_id)` devuelve el estado: `running`, la mejor solución hasta ahora (una
//!   transformación por copia colocada), el aprovechamiento y lo que no cupo.
//! - `nest_cancel(job_id)` pide parar; el siguiente `nest_poll` ya trae `running: false` con la
//!   mejor solución hasta ese momento.
//!
//! Cada pieza es un objeto suelto o un grupo completo (la interfaz manda sus objetos). La
//! transformación de cada copia es una afín en coordenadas de documento que se compone **por
//! encima** del `transform` actual de cada objeto de la pieza. La copia 0 es la pieza original;
//! las copias 1 en adelante las crea la interfaz como objetos nuevos.
//!
//! **Varias hojas y retazos:** `max_sheets` (por omisión 1) permite seguir en hojas nuevas iguales
//! al contenedor cuando las piezas no caben; `remnants` son retazos guardados que se prueban
//! antes que la hoja nueva (los que no admiten nada se saltan). Cada colocación lleva su `sheet`
//! (posición entre las hojas usadas) y el estado trae el resumen de cada hoja. Con
//! `leftover_min_mm > 0`, al terminar se calcula el sobrante de cada hoja (retazos nuevos).

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;
use tlaku_core::Polyline;
use tlaku_doc::{DesignObject, ObjectKind};
use tlaku_i18n::tr;
use tlaku_nest::{
    Gravity, LeftoverOptions, MAX_INSTANCES, NestContainer, NestError, NestOptions, NestPiece,
    PieceRotation, Rotations, SheetsInput, SheetsOutcome, nest_sheets, sheet_leftovers,
};

/// Error máximo (mm) al aplanar las curvas de las piezas.
const TOLERANCE_MM: f64 = 0.01;
const MIN_TIME_S: f64 = 1.0;
const MAX_TIME_S: f64 = 600.0;
/// Tope de hojas usadas en un trabajo (el de la interfaz es el mismo: `MAX_NEST_SHEETS`).
pub const MAX_SHEETS: u32 = 50;
/// Tope de retazos que se prueban en un trabajo.
const MAX_REMNANTS: usize = 200;

/// Giros que admite una pieza: los de las opciones, ninguno o solo la veta (0° y 180°).
#[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum NestPieceRotation {
    #[default]
    Inherit,
    None,
    Grain,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NestPieceInput {
    pub id: String,
    pub objects: Vec<DesignObject>,
    pub copies: u32,
    #[serde(default)]
    pub rotation: NestPieceRotation,
}

/// Un retazo guardado que se ofrece como hoja: contornos cerrados (exterior y huecos) en mm.
#[derive(Debug, Clone, Deserialize)]
pub struct NestRemnantInput {
    pub id: String,
    pub paths: Vec<Vec<[f64; 2]>>,
}

fn one() -> u32 {
    1
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum NestContainerInput {
    Rect { width: f64, height: f64 },
    Object { object: Box<DesignObject> },
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NestRotationMode {
    None,
    Quarter,
    Step,
    Free,
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub struct NestRotationInput {
    pub mode: NestRotationMode,
    #[serde(default)]
    pub step_deg: f64,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NestGravity {
    Gravity,
    BottomLeft,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NestRequest {
    pub pieces: Vec<NestPieceInput>,
    pub container: NestContainerInput,
    pub spacing: f64,
    pub margin: f64,
    pub rotation: NestRotationInput,
    pub allow_mirror: bool,
    pub use_holes: bool,
    pub gravity: NestGravity,
    pub time_limit_s: f64,
    pub seed: u64,
    /// Hojas que se pueden usar en total (1 = una sola, como antes).
    #[serde(default = "one")]
    pub max_sheets: u32,
    /// Retazos que se prueban antes que una hoja nueva.
    #[serde(default)]
    pub remnants: Vec<NestRemnantInput>,
    /// Ancho mínimo (mm) de los retazos que sobran; 0 = no calcular el sobrante.
    #[serde(default)]
    pub leftover_min_mm: f64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct NestPlacementOut {
    pub piece: String,
    pub copy: u32,
    /// Posición entre las hojas usadas (`NestStatus::sheets`).
    pub sheet: usize,
    pub rotation_deg: f64,
    pub mirror: bool,
    pub transform: [f64; 6],
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct NestRefOut {
    pub piece: String,
    pub copy: u32,
}

/// Resumen de una hoja usada.
#[derive(Debug, Clone, Serialize, Default, PartialEq)]
pub struct NestSheetOut {
    /// Posición entre las hojas usadas (la que llevan las colocaciones).
    pub index: usize,
    /// `remnant` si es un retazo guardado, `container` si es la hoja (la cama o el material).
    pub source: String,
    /// Id del retazo guardado cuando `source` es `remnant`.
    pub remnant_id: Option<String>,
    /// Caja de la hoja `[x0, y0, x1, y1]` en las coordenadas en que van las colocaciones.
    pub bounds: [f64; 4],
    pub utilization: f64,
    pub placed: usize,
    pub area_mm2: f64,
    pub placed_area_mm2: f64,
    /// Retazos que sobran (solo al terminar y si se pidió): cada uno son contornos cerrados
    /// (exterior y huecos) en las coordenadas de la hoja.
    pub leftovers: Vec<Vec<Vec<[f64; 2]>>>,
}

#[derive(Debug, Clone, Serialize, Default, PartialEq)]
pub struct NestStatus {
    pub running: bool,
    pub elapsed_ms: u64,
    pub evaluations: u64,
    /// 0 a 100: área neta colocada entre área neta del contenedor.
    pub utilization: f64,
    pub placed: usize,
    pub total: usize,
    pub placements: Vec<NestPlacementOut>,
    pub sheets: Vec<NestSheetOut>,
    pub unplaced: Vec<NestRefOut>,
    /// Terminó por el tope de tiempo (y no porque se pidió parar o porque ya no mejoraba).
    pub timed_out: bool,
    pub error: Option<String>,
}

fn list(ids: &[&str]) -> String {
    ids.iter()
        .map(crate::edit::quoted)
        .collect::<Vec<_>>()
        .join(", ")
}

/// Contornos cerrados de un conjunto de objetos en coordenadas de documento. Los errores
/// nombran los objetos con problemas.
fn closed_paths(objects: &[DesignObject], what: &str) -> Result<Vec<Polyline>, String> {
    let mut images = Vec::new();
    let mut open = Vec::new();
    let mut empty = Vec::new();
    let mut out = Vec::new();
    for o in objects {
        if matches!(o.kind, ObjectKind::Image(_)) {
            images.push(o.id.as_str());
            continue;
        }
        let paths = o.polylines_with(TOLERANCE_MM);
        if paths.is_empty() {
            empty.push(o.id.as_str());
        } else if paths.iter().any(|p| !p.closed) {
            open.push(o.id.as_str());
        } else {
            out.extend(paths);
        }
    }
    if !images.is_empty() {
        return Err(tr!(
            "desktop.edit.nest.images",
            what = what,
            list = list(&images)
        ));
    }
    if !open.is_empty() {
        return Err(tr!(
            "desktop.edit.nest.open_shapes",
            what = what,
            list = list(&open)
        ));
    }
    if !empty.is_empty() {
        return Err(tr!(
            "desktop.edit.nest.no_outline",
            what = what,
            list = list(&empty)
        ));
    }
    Ok(out)
}

fn remnant_region(r: &NestRemnantInput) -> Result<Vec<Polyline>, String> {
    let paths: Vec<Polyline> = r
        .paths
        .iter()
        .map(|ring| Polyline {
            points: ring
                .iter()
                .map(|p| tlaku_core::Point::new(p[0], p[1]))
                .collect(),
            closed: true,
        })
        .collect();
    let ok = !paths.is_empty()
        && paths.iter().all(|p| {
            p.points.len() >= 3 && p.points.iter().all(|q| q.x.is_finite() && q.y.is_finite())
        });
    if ok {
        Ok(paths)
    } else {
        Err(tr!("desktop.edit.nest.bad_remnant", id = r.id))
    }
}

/// Convierte la petición de la interfaz en la entrada del motor.
pub fn to_input(request: &NestRequest) -> Result<SheetsInput, String> {
    if request.pieces.iter().all(|p| p.copies == 0) {
        return Err(NestError::NoPieces.to_string());
    }
    let total: usize = request.pieces.iter().map(|p| p.copies as usize).sum();
    if total > MAX_INSTANCES {
        return Err(NestError::TooMany(total).to_string());
    }
    if request.remnants.len() > MAX_REMNANTS {
        return Err(tr!(
            "desktop.edit.nest.too_many_remnants",
            max = MAX_REMNANTS
        ));
    }
    let mut pieces = Vec::with_capacity(request.pieces.len());
    for p in &request.pieces {
        let region = if p.copies == 0 {
            Vec::new()
        } else {
            closed_paths(&p.objects, &tr!("desktop.edit.nest.what_piece", id = p.id))?
        };
        pieces.push(NestPiece {
            region,
            copies: p.copies,
            rotation: match p.rotation {
                NestPieceRotation::Inherit => PieceRotation::Inherit,
                NestPieceRotation::None => PieceRotation::Fixed,
                NestPieceRotation::Grain => PieceRotation::grain(),
            },
        });
    }
    let container = match &request.container {
        NestContainerInput::Rect { width, height } => NestContainer::Rect {
            width: *width,
            height: *height,
        },
        NestContainerInput::Object { object } => NestContainer::Region(closed_paths(
            std::slice::from_ref(object),
            &tr!("desktop.edit.nest.what_material"),
        )?),
    };
    let mut sheets = Vec::with_capacity(request.remnants.len());
    for r in &request.remnants {
        sheets.push(NestContainer::Region(remnant_region(r)?));
    }
    let rotations = match request.rotation.mode {
        NestRotationMode::None => Rotations::Fixed,
        NestRotationMode::Quarter => Rotations::Quarter,
        NestRotationMode::Step => Rotations::Step(request.rotation.step_deg),
        NestRotationMode::Free => Rotations::Free,
    };
    if !request.time_limit_s.is_finite() {
        return Err(tr!("desktop.edit.nest.bad_time_limit"));
    }
    Ok(SheetsInput {
        pieces,
        sheets,
        refill: Some(container),
        max_sheets: request.max_sheets.clamp(1, MAX_SHEETS) as usize,
        options: NestOptions {
            spacing: request.spacing,
            margin: request.margin,
            rotations,
            allow_mirror: request.allow_mirror,
            use_holes: request.use_holes,
            gravity: match request.gravity {
                NestGravity::Gravity => Gravity::Gravity,
                NestGravity::BottomLeft => Gravity::BottomLeft,
            },
            time_limit: Duration::from_secs_f64(request.time_limit_s.clamp(MIN_TIME_S, MAX_TIME_S)),
            seed: request.seed,
            max_evaluations: None,
        },
    })
}

/// Caja `[x0, y0, x1, y1]` de una hoja en sus propias coordenadas.
fn sheet_bounds(c: &NestContainer) -> [f64; 4] {
    match c {
        NestContainer::Rect { width, height } => [0.0, 0.0, *width, *height],
        NestContainer::Region(paths) => {
            let mut b = [
                f64::INFINITY,
                f64::INFINITY,
                f64::NEG_INFINITY,
                f64::NEG_INFINITY,
            ];
            for q in paths.iter().flat_map(|p| p.points.iter()) {
                b = [b[0].min(q.x), b[1].min(q.y), b[2].max(q.x), b[3].max(q.y)];
            }
            if b[0].is_finite() { b } else { [0.0; 4] }
        }
    }
}

fn status_from(
    request: &NestRequest,
    input: &SheetsInput,
    o: &SheetsOutcome,
    running: bool,
    cancelled: bool,
) -> NestStatus {
    let id = |i: usize| request.pieces[i].id.clone();
    let mut placements = Vec::with_capacity(o.placed_count());
    let mut sheets = Vec::with_capacity(o.sheets.len());
    for (k, sheet) in o.sheets.iter().enumerate() {
        let list = input.sheets.get(sheet.index);
        let remnant = list.and_then(|_| request.remnants.get(sheet.index));
        let container = list.or(input.refill.as_ref());
        sheets.push(NestSheetOut {
            index: k,
            source: if remnant.is_some() {
                "remnant"
            } else {
                "container"
            }
            .into(),
            remnant_id: remnant.map(|r| r.id.clone()),
            bounds: container.map_or([0.0; 4], sheet_bounds),
            utilization: sheet.utilization,
            placed: sheet.placements.len(),
            area_mm2: sheet.container_area,
            placed_area_mm2: sheet.placed_area,
            leftovers: Vec::new(),
        });
        placements.extend(sheet.placements.iter().map(|p| NestPlacementOut {
            piece: id(p.piece),
            copy: p.copy,
            sheet: k,
            rotation_deg: p.rotation_deg,
            mirror: p.mirror,
            transform: p.transform,
        }));
    }
    NestStatus {
        running,
        elapsed_ms: o.elapsed.as_millis() as u64,
        evaluations: o.evaluations,
        utilization: o.utilization,
        placed: placements.len(),
        total: request.pieces.iter().map(|p| p.copies as usize).sum(),
        placements,
        sheets,
        unplaced: o
            .unplaced
            .iter()
            .map(|&(piece, copy)| NestRefOut {
                piece: id(piece),
                copy,
            })
            .collect(),
        timed_out: !running && o.interrupted && !cancelled,
        error: None,
    }
}

/// Agrega a cada hoja el sobrante (retazos nuevos) si la petición lo pidió.
fn add_leftovers(
    request: &NestRequest,
    input: &SheetsInput,
    o: &SheetsOutcome,
    s: &mut NestStatus,
) {
    let width = request.leftover_min_mm;
    if !(width.is_finite() && width > 0.0) {
        return;
    }
    let opts = LeftoverOptions {
        // Los retazos quedan a la separación pedida (al menos 1 mm) de las piezas cortadas.
        clearance: request.spacing.max(1.0),
        min_width: width,
        min_area: width * width,
    };
    for (sheet, found) in s.sheets.iter_mut().zip(sheet_leftovers(input, o, &opts)) {
        sheet.leftovers = found
            .into_iter()
            .map(|region| {
                region
                    .into_iter()
                    .map(|ring| ring.points.iter().map(|q| [q.x, q.y]).collect())
                    .collect()
            })
            .collect();
    }
}

/// Estado compartido de un trabajo: lo escribe el hilo de cálculo y lo lee `nest_poll`.
struct Job {
    id: String,
    stop: Arc<AtomicBool>,
    status: Arc<Mutex<NestStatus>>,
}

fn current() -> &'static Mutex<Option<Job>> {
    static CURRENT: OnceLock<Mutex<Option<Job>>> = OnceLock::new();
    CURRENT.get_or_init(|| Mutex::new(None))
}

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

/// Calcula de forma síncrona y avisa del progreso con `publish`. Es el cuerpo del hilo y lo que
/// prueban los tests.
pub fn run_job(
    request: &NestRequest,
    input: &SheetsInput,
    stop: &AtomicBool,
    publish: &mut dyn FnMut(NestStatus),
) {
    let total: usize = request.pieces.iter().map(|p| p.copies as usize).sum();
    let mut last_revision = 0u64;
    let mut last = NestStatus {
        running: true,
        total,
        ..NestStatus::default()
    };
    publish(last.clone());
    let result = nest_sheets(input, stop, &mut |o| {
        if o.revision != last_revision {
            last_revision = o.revision;
            last = status_from(request, input, o, true, false);
        } else {
            last.evaluations = o.evaluations;
            last.elapsed_ms = o.elapsed.as_millis() as u64;
        }
        publish(last.clone());
    });
    match result {
        Ok(o) => {
            let mut done = status_from(request, input, &o, false, stop.load(Ordering::Relaxed));
            add_leftovers(request, input, &o, &mut done);
            publish(done);
        }
        Err(e) => publish(NestStatus {
            running: false,
            total,
            error: Some(e.to_string()),
            ..NestStatus::default()
        }),
    }
}

pub fn start(request: NestRequest) -> Result<String, String> {
    let input = to_input(&request)?;
    // Cancela el trabajo anterior: la interfaz solo atiende uno.
    let mut slot = current()
        .lock()
        .map_err(|_| tr!("desktop.edit.nest.state_poisoned"))?;
    if let Some(old) = slot.take() {
        old.stop.store(true, Ordering::Relaxed);
    }
    let id = format!("nest-{}", NEXT_ID.fetch_add(1, Ordering::Relaxed));
    let stop = Arc::new(AtomicBool::new(false));
    let status = Arc::new(Mutex::new(NestStatus {
        running: true,
        total: request.pieces.iter().map(|p| p.copies as usize).sum(),
        ..NestStatus::default()
    }));
    *slot = Some(Job {
        id: id.clone(),
        stop: stop.clone(),
        status: status.clone(),
    });
    drop(slot);
    std::thread::Builder::new()
        .name("tlaku-nest".into())
        .spawn(move || {
            let publish_to = status.clone();
            let body = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                run_job(&request, &input, &stop, &mut |s| {
                    if let Ok(mut g) = publish_to.lock() {
                        *g = s;
                    }
                });
            }));
            if body.is_err()
                && let Ok(mut g) = status.lock()
            {
                g.running = false;
                g.error = Some(tr!("desktop.edit.nest.crashed"));
            }
        })
        .map_err(|e| tr!("desktop.edit.nest.start_failed", err = e))?;
    Ok(id)
}

fn with_job<T>(job_id: &str, f: impl FnOnce(&Job) -> T) -> Result<T, String> {
    let slot = current()
        .lock()
        .map_err(|_| tr!("desktop.edit.nest.state_poisoned"))?;
    match slot.as_ref() {
        Some(j) if j.id == job_id => Ok(f(j)),
        _ => Err(tr!("desktop.edit.nest.job_gone")),
    }
}

pub fn poll(job_id: &str) -> Result<NestStatus, String> {
    with_job(job_id, |j| {
        j.status
            .lock()
            .map(|g| g.clone())
            .map_err(|_| tr!("desktop.edit.nest.state_poisoned"))
    })?
}

pub fn cancel(job_id: &str) -> Result<(), String> {
    with_job(job_id, |j| j.stop.store(true, Ordering::Relaxed))
}

// La interfaz manda los argumentos en snake_case (`job_id`), como el resto del contrato.
#[tauri::command(rename_all = "snake_case")]
pub async fn nest_start(request: NestRequest) -> Result<String, String> {
    start(request)
}

#[tauri::command(rename_all = "snake_case")]
pub async fn nest_poll(job_id: String) -> Result<NestStatus, String> {
    poll(&job_id)
}

#[tauri::command(rename_all = "snake_case")]
pub async fn nest_cancel(job_id: String) -> Result<(), String> {
    cancel(&job_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tlaku_core::Point;
    use tlaku_doc::{BezPath, VectorObject, affine};

    fn obj(id: &str, x0: f64, y0: f64, x1: f64, y1: f64, closed: bool) -> DesignObject {
        let poly = Polyline {
            points: vec![
                Point::new(x0, y0),
                Point::new(x1, y0),
                Point::new(x1, y1),
                Point::new(x0, y1),
            ],
            closed,
        };
        DesignObject {
            id: id.into(),
            layer: tlaku_core::Rgb(255, 0, 0),
            transform: affine::IDENTITY,
            kind: ObjectKind::Vector(VectorObject::new(vec![BezPath::from(&poly)])),
            locked: false,
            hidden: false,
            group: None,
            priority: None,
            power_scale: None,
            shape: None,
            start: None,
            manual_tabs: Vec::new(),
            array: None,
        }
    }

    fn request(pieces: Vec<NestPieceInput>, w: f64, h: f64) -> NestRequest {
        NestRequest {
            pieces,
            container: NestContainerInput::Rect {
                width: w,
                height: h,
            },
            spacing: 2.0,
            margin: 0.0,
            rotation: NestRotationInput {
                mode: NestRotationMode::Quarter,
                step_deg: 15.0,
            },
            allow_mirror: false,
            use_holes: false,
            gravity: NestGravity::Gravity,
            time_limit_s: 5.0,
            seed: 1,
            max_sheets: 1,
            remnants: Vec::new(),
            leftover_min_mm: 0.0,
        }
    }

    fn piece(id: &str, w: f64, h: f64, copies: u32) -> NestPieceInput {
        NestPieceInput {
            id: id.into(),
            objects: vec![obj(id, 10.0, 20.0, 10.0 + w, 20.0 + h, true)],
            copies,
            rotation: NestPieceRotation::Inherit,
        }
    }

    fn solve(request: &NestRequest) -> NestStatus {
        let input = to_input(request).expect("petición válida");
        let mut last = NestStatus::default();
        run_job(request, &input, &AtomicBool::new(false), &mut |s| last = s);
        last
    }

    #[test]
    fn la_transformacion_lleva_cada_copia_a_su_lugar() {
        let req = request(vec![piece("a", 30.0, 20.0, 4)], 70.0, 50.0);
        let s = solve(&req);
        assert!(!s.running && s.error.is_none());
        assert_eq!((s.placed, s.total), (4, 4));
        assert!(s.unplaced.is_empty());
        assert!((s.utilization - 4.0 * 600.0 / 3500.0 * 100.0).abs() < 1e-6);
        // Aplicando la transformación al contorno original, cada copia queda dentro de la cama.
        for p in &s.placements {
            let o = obj("a", 10.0, 20.0, 40.0, 40.0, true);
            for poly in o.polylines() {
                for q in poly.points {
                    let [x, y] = affine::apply(&p.transform, [q.x, q.y]);
                    assert!(x > -0.01 && y > -0.01 && x < 70.01 && y < 50.01, "{x} {y}");
                }
            }
        }
        let copies: Vec<u32> = s.placements.iter().map(|p| p.copy).collect();
        for c in 0..4 {
            assert!(copies.contains(&c));
        }
    }

    #[test]
    fn lo_que_no_cabe_se_reporta_con_el_id_de_la_pieza() {
        let req = request(
            vec![
                piece("chica", 10.0, 10.0, 1),
                piece("grande", 90.0, 10.0, 2),
            ],
            50.0,
            50.0,
        );
        let s = solve(&req);
        assert_eq!(s.placed, 1);
        assert_eq!(s.unplaced.len(), 2);
        assert!(s.unplaced.iter().all(|u| u.piece == "grande"));
    }

    #[test]
    fn la_peticion_se_valida_en_espanol() {
        let mut open = request(vec![piece("a", 10.0, 10.0, 1)], 50.0, 50.0);
        open.pieces[0].objects[0] = obj("a", 0.0, 0.0, 5.0, 5.0, false);
        assert!(to_input(&open).unwrap_err().contains("formas abiertas"));
        let none = request(vec![piece("a", 10.0, 10.0, 0)], 50.0, 50.0);
        assert!(to_input(&none).unwrap_err().contains("No hay piezas"));
        let many = request(
            vec![piece("a", 1.0, 1.0, MAX_INSTANCES as u32 + 1)],
            50.0,
            50.0,
        );
        assert!(to_input(&many).unwrap_err().contains("demasiadas"));
        // Contenedor inválido: lo detecta el motor y se informa en el estado.
        let bad = solve(&request(vec![piece("a", 10.0, 10.0, 1)], 0.0, 50.0));
        assert!(!bad.running);
        assert!(bad.error.as_deref().unwrap_or("").contains("contenedor"));
    }

    #[test]
    fn el_material_irregular_se_toma_de_un_objeto() {
        let mut req = request(vec![piece("a", 20.0, 20.0, 5)], 0.0, 0.0);
        req.container = NestContainerInput::Object {
            object: Box::new(obj("material", 100.0, 100.0, 150.0, 140.0, true)),
        };
        let s = solve(&req);
        // 50 x 40 con piezas de 20 x 20 y separación 2: caben 2 x 1 en filas de 42; con giros, 4.
        assert!(s.placed >= 2, "{}", s.placed);
        for p in &s.placements {
            let o = obj("a", 10.0, 20.0, 30.0, 40.0, true);
            for poly in o.polylines() {
                for q in poly.points {
                    let [x, y] = affine::apply(&p.transform, [q.x, q.y]);
                    assert!(
                        x > 99.99 && y > 99.99 && x < 150.01 && y < 140.01,
                        "{x} {y}"
                    );
                }
            }
        }
    }

    #[test]
    fn iniciar_sondear_y_cancelar() {
        let mut req = request(vec![piece("a", 5.0, 5.0, 60)], 400.0, 300.0);
        req.time_limit_s = 120.0;
        let id = start(req).unwrap();
        // Con tiempo de sobra sigue corriendo hasta que se cancela.
        let mut seen_running = false;
        for _ in 0..200 {
            let s = poll(&id).unwrap();
            if s.evaluations >= 2 {
                seen_running = s.running;
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        cancel(&id).unwrap();
        let mut done = None;
        for _ in 0..500 {
            let s = poll(&id).unwrap();
            if !s.running {
                done = Some(s);
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        let done = done.expect("el trabajo debe terminar tras cancelar");
        assert!(seen_running || done.evaluations > 0);
        assert_eq!(done.placed, 60);
        assert!(!done.timed_out, "se canceló, no se agotó el tiempo");
        // Otro trabajo invalida el id anterior.
        let id2 = start(request(vec![piece("a", 5.0, 5.0, 1)], 50.0, 50.0)).unwrap();
        assert!(poll(&id).unwrap_err().contains("ya no existe"));
        assert!(poll(&id2).is_ok());
        cancel(&id2).unwrap();
    }

    #[test]
    fn varias_hojas_reparten_las_piezas_y_resumen_cada_hoja() {
        let mut req = request(vec![piece("a", 40.0, 40.0, 6)], 100.0, 100.0);
        req.spacing = 0.0;
        req.max_sheets = 3;
        let s = solve(&req);
        assert!(s.error.is_none() && s.unplaced.is_empty());
        assert_eq!((s.placed, s.total), (6, 6));
        assert_eq!(s.sheets.len(), 2);
        let counts: Vec<usize> = s.sheets.iter().map(|h| h.placed).collect();
        assert_eq!(counts, vec![4, 2]);
        for (k, h) in s.sheets.iter().enumerate() {
            assert_eq!(h.index, k);
            assert_eq!(h.source, "container");
            assert_eq!(h.bounds, [0.0, 0.0, 100.0, 100.0]);
            assert_eq!(h.area_mm2, 10000.0);
        }
        // Cada colocación dice en qué hoja cayó; ninguna copia se repite.
        for k in 0..2 {
            let n = s.placements.iter().filter(|p| p.sheet == k).count();
            assert_eq!(n, s.sheets[k].placed);
        }
        let mut copies: Vec<u32> = s.placements.iter().map(|p| p.copy).collect();
        copies.sort_unstable();
        assert_eq!(copies, vec![0, 1, 2, 3, 4, 5]);
        // Aprovechamiento global: 6 piezas de 1600 mm² entre 2 hojas de 10000 mm².
        assert!((s.utilization - 6.0 * 1600.0 / 20000.0 * 100.0).abs() < 1e-6);
    }

    #[test]
    fn con_una_sola_hoja_lo_que_no_cabe_se_reporta_como_antes() {
        let req = request(vec![piece("a", 40.0, 40.0, 6)], 100.0, 100.0);
        let s = solve(&req);
        assert_eq!(s.sheets.len(), 1);
        assert_eq!(s.placed, 4);
        assert_eq!(s.unplaced.len(), 2);
    }

    #[test]
    fn los_retazos_se_usan_antes_que_la_hoja_nueva() {
        let mut req = request(vec![piece("a", 40.0, 40.0, 3)], 100.0, 100.0);
        req.spacing = 0.0;
        req.max_sheets = 4;
        req.remnants = vec![
            // Un retazo diminuto donde no cabe nada (se salta) y uno de 90 x 50 donde caben 2.
            NestRemnantInput {
                id: "chico".into(),
                paths: vec![vec![[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0]]],
            },
            NestRemnantInput {
                id: "bueno".into(),
                paths: vec![vec![[0.0, 0.0], [90.0, 0.0], [90.0, 50.0], [0.0, 50.0]]],
            },
        ];
        let s = solve(&req);
        assert!(s.unplaced.is_empty(), "{:?}", s.unplaced);
        assert_eq!(s.sheets.len(), 2);
        assert_eq!(s.sheets[0].source, "remnant");
        assert_eq!(s.sheets[0].remnant_id.as_deref(), Some("bueno"));
        assert_eq!(s.sheets[0].placed, 2);
        assert_eq!(s.sheets[0].bounds, [0.0, 0.0, 90.0, 50.0]);
        assert_eq!(s.sheets[1].source, "container");
        assert_eq!(s.sheets[1].remnant_id, None);
        assert_eq!(s.sheets[1].placed, 1);
    }

    #[test]
    fn un_retazo_invalido_se_rechaza_en_espanol() {
        let mut req = request(vec![piece("a", 10.0, 10.0, 1)], 50.0, 50.0);
        req.remnants = vec![NestRemnantInput {
            id: "malo".into(),
            paths: vec![vec![[0.0, 0.0], [1.0, 1.0]]],
        }];
        let e = to_input(&req).unwrap_err();
        assert!(e.contains("malo") && e.contains("contorno"), "{e}");
    }

    #[test]
    fn la_veta_y_el_sin_giro_llegan_al_motor() {
        // Cabe de pie (20 de ancho) pero no tumbada en una tira de 25: con veta no entra.
        let mut req = request(vec![piece("a", 60.0, 20.0, 1)], 25.0, 100.0);
        let free = solve(&req);
        assert_eq!(free.placed, 1);
        req.pieces[0].rotation = NestPieceRotation::Grain;
        let grain = solve(&req);
        assert_eq!((grain.placed, grain.unplaced.len()), (0, 1));
        req.pieces[0].rotation = NestPieceRotation::None;
        assert_eq!(solve(&req).placed, 0);
    }

    #[test]
    fn el_sobrante_se_calcula_al_terminar_y_solo_si_se_pide() {
        let mut req = request(vec![piece("a", 50.0, 100.0, 1)], 100.0, 100.0);
        req.spacing = 0.0;
        let none = solve(&req);
        assert!(none.sheets[0].leftovers.is_empty());
        req.leftover_min_mm = 10.0;
        let s = solve(&req);
        let left = &s.sheets[0].leftovers;
        assert_eq!(left.len(), 1, "un retazo");
        // Contorno exterior: 4 esquinas dentro de la hoja y a la derecha de la pieza (con 1 mm).
        let ring = &left[0][0];
        assert!(ring.len() >= 4);
        assert!(
            ring.iter()
                .all(|p| p[0] > 50.5 && p[0] <= 100.001 && p[1] >= -0.001 && p[1] <= 100.001)
        );
    }

    #[test]
    fn los_campos_nuevos_son_opcionales_en_la_peticion() {
        let json = r#"{"pieces":[{"id":"p1","objects":[],"copies":3}],
            "container":{"type":"rect","width":600,"height":400},
            "spacing":1,"margin":0,"rotation":{"mode":"quarter"},
            "allow_mirror":false,"use_holes":false,"gravity":"gravity",
            "time_limit_s":20,"seed":7}"#;
        let r: NestRequest = serde_json::from_str(json).unwrap();
        assert_eq!(r.max_sheets, 1);
        assert!(r.remnants.is_empty());
        assert_eq!(r.leftover_min_mm, 0.0);
        assert_eq!(r.pieces[0].rotation, NestPieceRotation::Inherit);
        let json = r#"{"pieces":[{"id":"p1","objects":[],"copies":3,"rotation":"grain"}],
            "container":{"type":"rect","width":600,"height":400},
            "spacing":1,"margin":0,"rotation":{"mode":"quarter"},
            "allow_mirror":false,"use_holes":false,"gravity":"gravity",
            "time_limit_s":20,"seed":7,"max_sheets":5,
            "remnants":[{"id":"r","paths":[[[0,0],[1,0],[1,1]]]}],"leftover_min_mm":15}"#;
        let r: NestRequest = serde_json::from_str(json).unwrap();
        assert_eq!(r.pieces[0].rotation, NestPieceRotation::Grain);
        assert_eq!(
            (r.max_sheets, r.remnants.len(), r.leftover_min_mm),
            (5, 1, 15.0)
        );
    }

    #[test]
    fn los_argumentos_llegan_como_los_manda_la_interfaz() {
        let json = r#"{"pieces":[{"id":"p1","objects":[],"copies":3}],
            "container":{"type":"rect","width":600,"height":400},
            "spacing":1,"margin":0,"rotation":{"mode":"step","step_deg":15},
            "allow_mirror":true,"use_holes":false,"gravity":"bottom_left",
            "time_limit_s":20,"seed":7}"#;
        let r: NestRequest = serde_json::from_str(json).unwrap();
        assert_eq!(r.pieces[0].copies, 3);
        assert!(matches!(r.rotation.mode, NestRotationMode::Step));
        assert!(matches!(r.gravity, NestGravity::BottomLeft));
        let out = serde_json::to_value(NestStatus::default()).unwrap();
        for k in [
            "running",
            "elapsed_ms",
            "evaluations",
            "utilization",
            "placed",
            "total",
            "placements",
            "sheets",
            "unplaced",
            "timed_out",
            "error",
        ] {
            assert!(out.get(k).is_some(), "falta {k}");
        }
    }
}
