//! Operaciones geométricas sobre polilíneas: booleanas, soldar, offset, kerf,
//! auto-join y utilidades.
//!
//! Convenciones (todas las funciones):
//! - Coordenadas en milímetros, Y hacia arriba. Una polilínea cerrada no repite
//!   el primer punto al final.
//! - Un conjunto de trayectorias cerradas es una región definida por la regla
//!   par-impar de anidamiento: profundidad 0 = pieza, 1 = agujero, 2 = isla
//!   dentro del agujero, etc. (ver [`nesting_depth`]). Las trayectorias que se
//!   solapan sin contenerse se tratan como piezas distintas y se unen.
//! - Las salidas con regiones (`boolean`, `weld`, `offset`) devuelven los
//!   contornos exteriores en sentido antihorario (área con signo positiva) y
//!   los agujeros en sentido horario, cada exterior seguido de sus agujeros.
//! - Las entradas degeneradas (vacías, con menos de 3 puntos, con NaN o
//!   infinitos, de área cero) se ignoran; ninguna función entra en pánico.

mod boolean;
mod dedupe;
mod join;
mod kerf;
mod nesting;
mod offset;
mod tabs;
mod util;

pub use boolean::{BoolOp, boolean, weld};
pub use dedupe::{DedupeResult, dedupe_paths};
pub use join::{auto_join, auto_join_tracked, close_paths};
pub use kerf::{kerf_compensate, kerf_compensate_with};
pub use nesting::nesting_depth;
pub use offset::{Join, offset};
pub use tabs::{MAX_TABS_PER_SHAPE, TabOptions, TabResult, tab_eligible, tab_segments};
pub use util::{
    Orientation, PointLocation, locate_point, orientation, point_in_polygon,
    remove_redundant_points, signed_area,
};

pub use tlaku_core::{Point, Polyline};

#[cfg(test)]
mod tests;

// Ola 3, diseño avanzado: deformar, contorno envolvente y círculo tangente.
mod hull;
mod tangent;
mod warp;

pub use hull::{concave_hull, convex_hull};
pub use tangent::{Circle, MAX_CIRCLES, tangent_circles};
pub use warp::{Edge, Envelope, Grid, Warp, warp_path};

#[cfg(test)]
mod tests_design;

// Ola 3, formas: recortar, separar, optimizar, redondear esquinas, cortar formas y ranuras.
mod fillet;
mod knife;
mod simplify;
mod slots;
mod split;
mod trim;
mod unfillet;

pub use fillet::{FilletOptions, FilletOutcome, FilletStyle, fillet_path};
pub use knife::{KnifeCut, cut_closed, cut_open};
pub use simplify::{OptimizeOptions, node_count, optimize_polyline};
pub use slots::{SlotAnchor, SlotOptions, SlotReport, resize_slots, slot_widths};
pub use split::{break_groups, split_subpaths};
pub use trim::{TrimResult, nearest_on_paths, trim_at};
pub use unfillet::{Unfillet, unfillet_path};

#[cfg(test)]
mod tests_shapes;
