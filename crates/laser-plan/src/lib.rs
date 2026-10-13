//! Planificador del orden de corte, explicable.
//!
//! Cada paso del plan trae un [`Reason`] que dice qué regla lo decidió. Las reglas no se
//! pisan en silencio: forman una jerarquía fija y unas restricciones de precedencia.
//!
//! # Reglas y precedencia (de mayor a menor)
//!
//! 1. **Niveles** `capa > grupo > prioridad` (solo los activados en [`PlanOptions`]). Los
//!    elementos con la misma clave forman un *cubo*; los cubos se cortan completos y en
//!    orden ascendente de clave (sin grupo va antes que cualquier grupo; menor prioridad
//!    primero). Ninguna otra opción cruza la frontera de un cubo.
//! 2. **Interior antes que contenedor** (`inner_first`), dentro de cada cubo. Una figura
//!    cerrada contiene a otra cerrada o abierta si su caja envolvente y varios vértices de
//!    muestra caen dentro del polígono y su área es estrictamente mayor (si una figura cae
//!    en dos contenedores solapados que no se contienen, solo cuenta el de menor área). Es una
//!    restricción dura: ni el recorrido corto ni la mejora 2-opt la rompen. Si el contenedor
//!    y el interior quedan en cubos distintos y el contenedor sale antes, gana el nivel y se
//!    avisa en [`Plan::warnings`] y en el [`Reason`] del contenedor.
//! 3. **Recorrido corto** (`reduce_travel`): vecino más cercano entre los elementos
//!    "listos" (sin interiores pendientes) y mejora 2-opt/Or-opt que solo acepta cambios que
//!    respetan 1 y 2. Sin `reduce_travel` se conserva el orden original (salvo lo que
//!    imponen 1 y 2) y no hay mejora.
//! 4. **Punto de inicio** (cerradas) y **sentido** (abiertas): solo cambian por dónde se
//!    entra, nunca el orden entre figuras.
//!
//! El resultado es determinista: mismas entradas, mismo plan.

mod contain;
mod explain;
mod geom;
mod grid;
mod improve;
mod planner;
mod sched;
mod shapes;

use std::fmt;

use laser_core::{Point, Polyline};

pub use planner::plan;

/// Una trayectoria por planificar con las claves de orden de su objeto.
#[derive(Debug, Clone, PartialEq)]
pub struct PlanItem {
    pub path: Polyline,
    pub layer_order: usize,
    pub group: Option<usize>,
    /// Menor valor se corta antes.
    pub priority: i32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlanOptions {
    pub order_by_layer: bool,
    pub order_by_group: bool,
    pub order_by_priority: bool,
    pub inner_first: bool,
    /// Vecino más cercano y mejora de recorrido. Apagado: orden original.
    pub reduce_travel: bool,
    /// Las cerradas pueden empezar en cualquier vértice.
    pub choose_start_point: bool,
    /// Las abiertas pueden recorrerse al revés.
    pub choose_direction: bool,
    /// Pasadas de 2-opt + Or-opt (0 desactiva la mejora y el ajuste fino de inicios).
    pub improve_passes: usize,
    /// Posición del cabezal antes del primer corte.
    pub start: Point,
}

impl Default for PlanOptions {
    /// Capa y prioridad activas (como el planificador por defecto de otros programas),
    /// interiores primero y recorrido corto.
    fn default() -> Self {
        Self {
            order_by_layer: true,
            order_by_group: false,
            order_by_priority: true,
            inner_first: true,
            reduce_travel: true,
            choose_start_point: true,
            choose_direction: true,
            improve_passes: 3,
            start: Point::new(0.0, 0.0),
        }
    }
}

/// Nivel jerárquico de orden.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    Layer,
    Group,
    Priority,
}

impl fmt::Display for Level {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Level::Layer => "capa",
            Level::Group => "grupo",
            Level::Priority => "prioridad",
        })
    }
}

/// Figura más cercana que el nivel o la restricción de interiores obligó a saltarse al
/// elegir la primera de un cubo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Blocked {
    /// Figura que estaba más cerca pero debía esperar a sus interiores.
    pub item: usize,
    /// `true`: la figura elegida está dentro de `item` (por eso va antes). `false`: `item`
    /// espera a otros interiores y la elegida no tiene relación con ella.
    pub contains_this: bool,
}

/// Por qué un paso va donde va. Los índices `usize` son posiciones en la entrada.
#[derive(Debug, Clone, PartialEq)]
pub enum Reason {
    /// Primera figura de una capa nueva. `blocked`: figura más cercana que se saltó por
    /// "interiores primero" (se dice siempre, también en el primer paso de un cubo).
    FirstOfLayer {
        layer: usize,
        blocked: Option<Blocked>,
    },
    /// Primera figura de un grupo nuevo (`None`: figuras sin grupo). `blocked` como arriba.
    FirstOfGroup {
        group: Option<usize>,
        blocked: Option<Blocked>,
    },
    /// Primera figura de una prioridad nueva. `blocked` como arriba.
    FirstOfPriority {
        priority: i32,
        blocked: Option<Blocked>,
    },
    /// Va antes que `container`, que la contiene y estaba más cerca.
    InnerFirst { container: usize },
    /// La figura más cercana (`distance_mm` hasta su punto más próximo) entre las permitidas.
    /// `blocked`: la figura aún más cercana que debía esperar a sus interiores (la más
    /// cercana de las bloqueadas; si la elegida está dentro de alguna, sale `InnerFirst`).
    Nearest {
        distance_mm: f64,
        blocked: Option<usize>,
    },
    /// Sigue el orden original (recorrido corto desactivado). `blocked`: figura que iba antes
    /// en el original pero debe esperar a sus interiores.
    InputOrder { blocked: Option<usize> },
    /// La mejora (2-opt/Or-opt o el ajuste de inicio y sentido) la dejó aquí porque acorta el
    /// recorrido total, aunque la figura no sea la más cercana.
    Improved,
    /// Sale antes que su interior `inner` porque el nivel `level` tiene precedencia sobre
    /// "interiores primero".
    LevelOverridesInner { inner: usize, level: Level },
}

/// Texto de "primera figura de un cubo": sin bloqueo explica el nivel; con bloqueo dice
/// además qué figura más cercana se saltó y por qué.
fn first_of(
    f: &mut fmt::Formatter<'_>,
    bucket: fmt::Arguments<'_>,
    rule: &str,
    blocked: &Option<Blocked>,
) -> fmt::Result {
    match blocked {
        None => write!(f, "primera figura de {bucket}: {rule}"),
        Some(Blocked {
            item,
            contains_this: true,
        }) => write!(
            f,
            "primera figura de {bucket}; va antes que la figura #{item} (la más cercana) \
             porque está dentro de ella y los interiores se cortan primero"
        ),
        Some(Blocked {
            item,
            contains_this: false,
        }) => write!(
            f,
            "primera figura de {bucket}; la figura #{item} estaba más cerca \
             pero debe esperar a sus interiores"
        ),
    }
}

impl fmt::Display for Reason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Reason::FirstOfLayer { layer, blocked } => first_of(
                f,
                format_args!("capa {layer}"),
                "las capas se cortan en su orden",
                blocked,
            ),
            Reason::FirstOfGroup {
                group: Some(g),
                blocked,
            } => first_of(
                f,
                format_args!("grupo {g}"),
                "los grupos se cortan en su orden dentro de la capa",
                blocked,
            ),
            Reason::FirstOfGroup {
                group: None,
                blocked,
            } => first_of(
                f,
                format_args!("figuras sin grupo"),
                "las figuras sin grupo van antes que los grupos",
                blocked,
            ),
            Reason::FirstOfPriority { priority, blocked } => first_of(
                f,
                format_args!("prioridad {priority}"),
                "la prioridad menor se corta antes",
                blocked,
            ),
            Reason::InnerFirst { container } => write!(
                f,
                "va antes que la figura #{container}, que la contiene: interiores primero"
            ),
            Reason::Nearest {
                distance_mm,
                blocked: None,
            } => write!(
                f,
                "la más cercana ({distance_mm:.2} mm) entre las que podían cortarse ahora"
            ),
            Reason::Nearest {
                distance_mm,
                blocked: Some(b),
            } => write!(
                f,
                "la más cercana ({distance_mm:.2} mm) entre las que podían cortarse ahora; \
                 la figura #{b} estaba más cerca pero debe esperar a sus interiores"
            ),
            Reason::InputOrder { blocked: None } => {
                write!(
                    f,
                    "sigue el orden original (reducir recorrido está apagado)"
                )
            }
            Reason::InputOrder { blocked: Some(b) } => write!(
                f,
                "sigue el orden original; la figura #{b} iba antes pero debe esperar a sus interiores"
            ),
            Reason::Improved => write!(
                f,
                "la mejora de recorrido (2-opt/Or-opt, inicio y sentido) la dejó aquí porque \
                 acorta el total, aunque no es la figura más cercana"
            ),
            Reason::LevelOverridesInner { inner, level } => write!(
                f,
                "sale antes que su interior #{inner} porque el orden por {level} \
                 tiene precedencia sobre interiores primero"
            ),
        }
    }
}

/// Aviso sobre una regla que no pudo cumplirse o una entrada ignorada.
#[derive(Debug, Clone, PartialEq)]
pub enum Warning {
    /// Trayectoria vacía o con coordenadas no finitas; no aparece en el plan.
    SkippedInvalid { item: usize },
    /// `container` se corta antes que su interior `inner` porque `level` manda.
    InnerOverriddenByLevel {
        inner: usize,
        container: usize,
        level: Level,
    },
}

impl fmt::Display for Warning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Warning::SkippedInvalid { item } => write!(
                f,
                "la figura #{item} está vacía o tiene coordenadas inválidas y se omitió"
            ),
            Warning::InnerOverriddenByLevel {
                inner,
                container,
                level,
            } => write!(
                f,
                "la figura #{container} se corta antes que su interior #{inner}: \
                 el orden por {level} tiene precedencia sobre interiores primero"
            ),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlanStep {
    /// Posición de la figura en la entrada.
    pub item: usize,
    /// Trayectoria ya rotada (cerradas) o invertida (abiertas).
    pub path: Polyline,
    pub reason: Reason,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Plan {
    pub steps: Vec<PlanStep>,
    /// Movimientos en vacío del plan, desde `PlanOptions::start`.
    pub travel_mm: f64,
    /// Referencia para mostrar el ahorro: movimientos en vacío del orden original *hecho
    /// válido*, es decir, el orden de entrada reordenado lo mínimo para respetar niveles e
    /// interiores primero (topológico estable), sin rotar el inicio de las cerradas ni invertir
    /// el sentido de las abiertas, y partiendo de `PlanOptions::start`. Así el ahorro mide solo
    /// lo que aporta la optimización, y `travel_mm <= baseline_travel_mm` siempre.
    pub baseline_travel_mm: f64,
    pub warnings: Vec<Warning>,
}

impl Plan {
    /// Milímetros ahorrados frente a la referencia válida (nunca negativo).
    pub fn saved_mm(&self) -> f64 {
        self.baseline_travel_mm - self.travel_mm
    }

    /// Ahorro en porcentaje del recorrido original (0 si no había recorrido).
    pub fn saved_pct(&self) -> f64 {
        if self.baseline_travel_mm > 0.0 {
            100.0 * self.saved_mm() / self.baseline_travel_mm
        } else {
            0.0
        }
    }
}
