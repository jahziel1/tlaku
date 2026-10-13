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
mod join;
mod kerf;
mod nesting;
mod offset;
mod util;

pub use boolean::{BoolOp, boolean, weld};
pub use join::{auto_join, close_paths};
pub use kerf::{kerf_compensate, kerf_compensate_with};
pub use nesting::nesting_depth;
pub use offset::{Join, offset};
pub use util::{
    Orientation, PointLocation, locate_point, orientation, point_in_polygon,
    remove_redundant_points, signed_area,
};

pub use laser_core::{Point, Polyline};

#[cfg(test)]
mod tests;
