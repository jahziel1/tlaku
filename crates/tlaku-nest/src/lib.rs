//! Nesting: acomoda piezas planas dentro de la cama (un rectángulo) o de un material de forma
//! cualquiera para desperdiciar lo menos posible.
//!
//! # Algoritmo
//! - Cada pieza (contornos cerrados con huecos) se simplifica de forma conservadora, se infla la
//!   mitad de la separación y se descompone en partes convexas ([`convex`]).
//! - Los **polígonos de no-ajuste (NFP)** entre dos piezas con una rotación dada son la unión de
//!   las sumas de Minkowski de sus partes convexas ([`ops`]). El **ajuste interior (IFP)** del
//!   contenedor da dónde puede ir una pieza sin salirse. La región libre para la esquina de una
//!   pieza es el IFP menos los NFP de las ya colocadas ([`engine`]).
//! - La colocación es voraz: gana el vértice libre con mejor puntaje ("gravedad" o "abajo e
//!   izquierda"). Con `use_holes`, los huecos de las piezas colocadas son zonas donde caben otras.
//! - Una búsqueda de recocido simulado sobre el orden y las rotaciones mejora la solución hasta
//!   el tope de tiempo o hasta que se pide parar ([`search`]).
//!
//! # Referencias (solo ideas; no se copió código)
//! Bennell y Oliveira, "The geometry of nesting problems: a tutorial" (2008); SVGnest (MIT) y
//! Deepnest (GPL) como ilustración del uso de NFP con Minkowski y de la búsqueda sobre orden y
//! rotaciones; Hertel y Mehlhorn (1983) para la descomposición convexa.
//!
//! Convenciones: milímetros, Y hacia arriba, rotaciones antihorarias en grados. El contorno que
//! usa el motor contiene a la pieza real, así que las piezas colocadas nunca se solapan y
//! distan al menos `spacing` (salvo una tolerancia numérica de [`TOLERANCE_MM`]).

mod convex;
mod engine;
mod geom;
mod ops;
mod prep;
mod region;
mod search;

pub use engine::Gravity;
pub use prep::TOL_MM as TOLERANCE_MM;

use engine::{Ctl, Engine, Gene};
use search::{Limits, Step, search};
use std::fmt;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};
use tlaku_core::Polyline;

/// Tope de copias en total: más allá el cálculo deja de ser manejable.
pub const MAX_INSTANCES: usize = 5000;

/// Una pieza: sus contornos cerrados en coordenadas de documento (los que están dentro de otros
/// son huecos) y cuántas copias se acomodan. La copia 0 es la pieza original.
#[derive(Debug, Clone)]
pub struct NestPiece {
    pub region: Vec<Polyline>,
    pub copies: u32,
}

#[derive(Debug, Clone)]
pub enum NestContainer {
    /// La cama: de (0, 0) a (ancho, alto).
    Rect { width: f64, height: f64 },
    /// Material irregular: contornos cerrados (con huecos que se evitan).
    Region(Vec<Polyline>),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Rotations {
    /// Solo la orientación original.
    Fixed,
    /// 0, 90, 180 y 270 grados.
    Quarter,
    /// Cada tantos grados (entre 1 y 180).
    Step(f64),
    /// Libre: pasos finos de 5 grados (más lento).
    Free,
}

#[derive(Debug, Clone)]
pub struct NestOptions {
    /// Distancia mínima entre piezas (mm).
    pub spacing: f64,
    /// Distancia mínima entre las piezas y el borde del contenedor (mm).
    pub margin: f64,
    pub rotations: Rotations,
    pub allow_mirror: bool,
    pub use_holes: bool,
    pub gravity: Gravity,
    /// Tope de tiempo de todo el cálculo.
    pub time_limit: Duration,
    pub seed: u64,
    /// Tope de evaluaciones (colocaciones completas). Con él el resultado es reproducible
    /// aunque el tiempo cambie; `None` = solo manda el tiempo.
    pub max_evaluations: Option<u64>,
}

impl Default for NestOptions {
    fn default() -> Self {
        Self {
            spacing: 0.0,
            margin: 0.0,
            rotations: Rotations::Quarter,
            allow_mirror: false,
            use_holes: false,
            gravity: Gravity::Gravity,
            time_limit: Duration::from_secs(20),
            seed: 1,
            max_evaluations: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct NestInput {
    pub pieces: Vec<NestPiece>,
    pub container: NestContainer,
    pub options: NestOptions,
}

/// Dónde queda una copia de una pieza.
#[derive(Debug, Clone, PartialEq)]
pub struct Placement {
    /// Índice en `NestInput::pieces`.
    pub piece: usize,
    pub copy: u32,
    pub rotation_deg: f64,
    pub mirror: bool,
    /// Afín `[a b c d e f]` del documento (`x' = a·x + c·y + e`, `y' = b·x + d·y + f`) que lleva
    /// la pieza original a su lugar: primero el espejo, luego el giro, luego el traslado.
    pub transform: [f64; 6],
}

#[derive(Debug, Clone)]
pub struct Outcome {
    pub placements: Vec<Placement>,
    /// `(pieza, copia)` de las que no cupieron.
    pub unplaced: Vec<(usize, u32)>,
    /// Área neta colocada entre área neta del contenedor, en por ciento.
    pub utilization: f64,
    pub placed_area: f64,
    pub container_area: f64,
    pub evaluations: u64,
    /// Cuenta las mejoras: cambia solo cuando la solución es otra (sirve para no copiar las
    /// transformaciones en cada aviso de progreso).
    pub revision: u64,
    /// Terminó por el tope de tiempo o por una orden de parar (y no por estancamiento).
    pub interrupted: bool,
    pub elapsed: Duration,
}

#[derive(Debug, Clone, PartialEq)]
pub enum NestError {
    NoPieces,
    TooMany(usize),
    /// La pieza (índice) no tiene área: contornos vacíos, degenerados o con puntos no finitos.
    EmptyPiece(usize),
    InvalidContainer,
    InvalidOption(&'static str),
}

impl fmt::Display for NestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NestError::NoPieces => write!(f, "No hay piezas que acomodar."),
            NestError::TooMany(n) => write!(
                f,
                "Son demasiadas piezas ({n}); el máximo es {MAX_INSTANCES} entre originales y copias."
            ),
            NestError::EmptyPiece(i) => write!(
                f,
                "La pieza {} no tiene un contorno cerrado con área.",
                i + 1
            ),
            NestError::InvalidContainer => write!(
                f,
                "El contenedor no tiene un área válida: revisa el tamaño de la cama o el contorno del material."
            ),
            NestError::InvalidOption(what) => write!(f, "Opción inválida: {what}."),
        }
    }
}

impl std::error::Error for NestError {}

fn allowed_angles(r: Rotations) -> Result<Vec<f64>, NestError> {
    match r {
        Rotations::Fixed => Ok(vec![0.0]),
        Rotations::Quarter => Ok(vec![0.0, 90.0, 180.0, 270.0]),
        Rotations::Step(s) => {
            if !s.is_finite() || !(1.0..=180.0).contains(&s) {
                return Err(NestError::InvalidOption(
                    "el paso de rotación debe estar entre 1 y 180 grados",
                ));
            }
            let n = (360.0 / s).floor() as usize;
            Ok((0..n).map(|i| i as f64 * s).collect())
        }
        Rotations::Free => Ok((0..72).map(|i| i as f64 * 5.0).collect()),
    }
}

/// Acomoda las piezas. `stop` interrumpe en cualquier momento (devuelve lo mejor hasta ahora);
/// `progress` se llama tras cada evaluación con la mejor solución y su conteo.
pub fn nest(
    input: &NestInput,
    stop: &AtomicBool,
    progress: &mut dyn FnMut(&Outcome),
) -> Result<Outcome, NestError> {
    let started = Instant::now();
    let o = &input.options;
    if !(o.spacing.is_finite() && o.spacing >= 0.0) {
        return Err(NestError::InvalidOption(
            "la separación no puede ser negativa",
        ));
    }
    if !(o.margin.is_finite() && o.margin >= 0.0) {
        return Err(NestError::InvalidOption("el margen no puede ser negativo"));
    }
    let angles = allowed_angles(o.rotations)?;
    let total: usize = input.pieces.iter().map(|p| p.copies as usize).sum();
    if total == 0 {
        return Err(NestError::NoPieces);
    }
    if total > MAX_INSTANCES {
        return Err(NestError::TooMany(total));
    }

    let container = match &input.container {
        NestContainer::Rect { width, height } => {
            prep::prepare_container(Some((*width, *height)), &[], o.spacing, o.margin)
        }
        NestContainer::Region(paths) => prep::prepare_container(None, paths, o.spacing, o.margin),
    }
    .ok_or(NestError::InvalidContainer)?;

    // Las piezas sin copias no participan, pero se conserva el índice original.
    let mut prepared = Vec::with_capacity(input.pieces.len());
    for (i, p) in input.pieces.iter().enumerate() {
        match prep::prepare_piece(&p.region, o.spacing, o.use_holes) {
            Some(pt) => prepared.push(pt),
            None if p.copies == 0 => prepared.push(prep::empty_piece()),
            None => return Err(NestError::EmptyPiece(i)),
        }
    }
    let container_area = container.net_area;
    let mut engine = Engine::new(
        prepared,
        container,
        angles,
        o.gravity,
        o.use_holes,
        o.allow_mirror,
    );

    // Orden inicial: de mayor a menor área; a igualdad, por índice (estable y reproducible).
    let mut start: Vec<Gene> = Vec::with_capacity(total);
    for (i, p) in input.pieces.iter().enumerate() {
        for c in 0..p.copies {
            start.push(Gene {
                piece: i,
                copy: c,
                rot: None,
                mirror: false,
            });
        }
    }
    start.sort_by(|a, b| {
        let (pa, pb) = (&engine.pieces[a.piece], &engine.pieces[b.piece]);
        pb.inflated_area
            .total_cmp(&pa.inflated_area)
            .then(a.piece.cmp(&b.piece))
            .then(a.copy.cmp(&b.copy))
    });

    let ctl = Ctl {
        stop,
        deadline: started + o.time_limit,
    };
    let limits = Limits {
        max_evaluations: o.max_evaluations,
        started,
        time_limit_s: o.time_limit.as_secs_f64(),
    };

    let mut cached: Option<Outcome> = None;
    let mut revision = 0u64;
    let found = {
        // La conversión a `Outcome` solo se hace cuando hay mejora, para no copiar cientos de
        // transformaciones en cada evaluación.
        let mut observer = |eng: &Engine, step: &Step| {
            if step.improved || cached.is_none() {
                revision += 1;
                let mut o = build_outcome(eng, step.solution, container_area, started);
                o.revision = revision;
                cached = Some(o);
            }
            if let Some(c) = cached.as_mut() {
                c.evaluations = step.evaluations;
                c.elapsed = started.elapsed();
                progress(c);
            }
        };
        search(&mut engine, start, &ctl, &limits, o.seed, &mut observer)
    };
    let mut out = build_outcome(&engine, &found.solution, container_area, started);
    out.evaluations = found.evaluations;
    out.revision = revision;
    out.interrupted = found.interrupted;
    Ok(out)
}

fn build_outcome(
    engine: &Engine,
    sol: &engine::Solution,
    container_area: f64,
    started: Instant,
) -> Outcome {
    let placements = sol
        .placed
        .iter()
        .map(|p| {
            let v = engine.variant(p.variant);
            Placement {
                piece: p.piece,
                copy: p.copy,
                rotation_deg: v.angle,
                mirror: v.mirror,
                transform: engine.transform(p.piece, p.variant, p.pos),
            }
        })
        .collect();
    let mut unplaced = sol.unplaced.clone();
    unplaced.sort_unstable();
    Outcome {
        placements,
        unplaced,
        utilization: if container_area > 0.0 {
            sol.placed_area / container_area * 100.0
        } else {
            0.0
        },
        placed_area: sol.placed_area,
        container_area,
        evaluations: 0,
        revision: 0,
        interrupted: false,
        elapsed: started.elapsed(),
    }
}

#[cfg(test)]
mod tests;
