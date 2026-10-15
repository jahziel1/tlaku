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
mod leftover;
mod search;
mod sheets;
mod solve;

pub use engine::Gravity;
pub use leftover::{LeftoverOptions, leftover};
pub use prep::TOL_MM as TOLERANCE_MM;
pub use sheets::{SheetOutcome, SheetsInput, SheetsOutcome, nest_sheets};

use std::fmt;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};
use tlaku_core::Polyline;
use tlaku_i18n::tr;

/// Tope de copias en total: más allá el cálculo deja de ser manejable. Es el mismo que usa la
/// interfaz (`MAX_NEST_INSTANCES` en `apps/desktop/src/features/nest/request.ts`; una prueba de
/// la interfaz comprueba que coinciden) y que el de las copias virtuales.
pub const MAX_INSTANCES: usize = 2000;

/// Una pieza: sus contornos cerrados en coordenadas de documento (los que están dentro de otros
/// son huecos) y cuántas copias se acomodan. La copia 0 es la pieza original.
#[derive(Debug, Clone)]
pub struct NestPiece {
    pub region: Vec<Polyline>,
    pub copies: u32,
    /// Giros permitidos a esta pieza (la veta o «sin giro»).
    pub rotation: PieceRotation,
}

/// Giros permitidos a una pieza concreta. Sirve para la veta de la madera (solo 0° y 180°) y
/// para las piezas que no deben girar.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum PieceRotation {
    /// Los de [`NestOptions::rotations`].
    #[default]
    Inherit,
    /// Solo la orientación original.
    Fixed,
    /// Solo estos ángulos (grados antihorarios; se normalizan a `[0, 360)`).
    Only(Vec<f64>),
}

impl PieceRotation {
    /// Veta horizontal: la pieza puede estar derecha o volteada 180°, nunca de lado.
    pub fn grain() -> Self {
        PieceRotation::Only(vec![0.0, 180.0])
    }
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
    InvalidOption(BadOption),
}

/// Qué opción de [`NestOptions`] es inválida (para el mensaje de [`NestError::InvalidOption`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BadOption {
    RotationStep,
    /// Un ángulo de pieza no es finito.
    PieceAngle,
    /// `max_sheets` es cero.
    MaxSheets,
    NegativeSpacing,
    NegativeMargin,
}

impl fmt::Display for NestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NestError::NoPieces => f.write_str(&tr!("nest.lib.no_pieces")),
            NestError::TooMany(n) => {
                f.write_str(&tr!("nest.lib.too_many", n = n, max = MAX_INSTANCES))
            }
            NestError::EmptyPiece(i) => f.write_str(&tr!("nest.lib.empty_piece", n = i + 1)),
            NestError::InvalidContainer => f.write_str(&tr!("nest.lib.invalid_container")),
            NestError::InvalidOption(BadOption::RotationStep) => {
                f.write_str(&tr!("nest.lib.option_rotation_step"))
            }
            NestError::InvalidOption(BadOption::PieceAngle) => {
                f.write_str(&tr!("nest.lib.option_piece_angle"))
            }
            NestError::InvalidOption(BadOption::MaxSheets) => {
                f.write_str(&tr!("nest.lib.option_max_sheets"))
            }
            NestError::InvalidOption(BadOption::NegativeSpacing) => {
                f.write_str(&tr!("nest.lib.option_spacing"))
            }
            NestError::InvalidOption(BadOption::NegativeMargin) => {
                f.write_str(&tr!("nest.lib.option_margin"))
            }
        }
    }
}

impl std::error::Error for NestError {}

/// Acomoda las piezas en un solo contenedor. `stop` interrumpe en cualquier momento (devuelve lo
/// mejor hasta ahora); `progress` se llama tras cada evaluación con la mejor solución y su
/// conteo. Para repartir en varias hojas, véase [`nest_sheets`].
pub fn nest(
    input: &NestInput,
    stop: &AtomicBool,
    progress: &mut dyn FnMut(&Outcome),
) -> Result<Outcome, NestError> {
    let started = Instant::now();
    let o = &input.options;
    check_options(o)?;
    let prepared = solve::prepare(&input.pieces, o)?;
    let genes = solve::all_genes(&input.pieces);
    solve::solve_sheet(
        &prepared,
        &genes,
        &input.container,
        o,
        stop,
        started,
        o.time_limit,
        progress,
    )
}

/// Revisa las opciones comunes a [`nest`] y [`nest_sheets`].
pub(crate) fn check_options(o: &NestOptions) -> Result<(), NestError> {
    if !(o.spacing.is_finite() && o.spacing >= 0.0) {
        return Err(NestError::InvalidOption(BadOption::NegativeSpacing));
    }
    if !(o.margin.is_finite() && o.margin >= 0.0) {
        return Err(NestError::InvalidOption(BadOption::NegativeMargin));
    }
    Ok(())
}

#[cfg(test)]
mod tests;
