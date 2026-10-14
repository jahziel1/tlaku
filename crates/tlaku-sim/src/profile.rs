//! Parámetros dinámicos de la máquina que usa la simulación de movimiento.

use std::collections::HashMap;

/// Límites de movimiento de la máquina. Velocidades en mm/min y aceleraciones
/// en mm/s², las mismas unidades que los ajustes `$110`, `$120`, etc. de GRBL.
#[derive(Debug, Clone, PartialEq)]
pub struct MachineProfile {
    /// Velocidad máxima por eje [X, Y] (`$110`, `$111`).
    pub max_rate_mm_min: [f64; 2],
    /// Aceleración máxima por eje [X, Y] (`$120`, `$121`).
    pub accel_mm_s2: [f64; 2],
    /// Velocidad de los movimientos rápidos (`Move::Travel`).
    pub rapid_rate_mm_min: f64,
    /// Desviación de unión (`$11`): cuánto puede "cortar" la esquina el cabezal.
    pub junction_deviation_mm: f64,
    /// Bloques que ve el planificador, contando el que se ejecuta. `None` mira todo el programa;
    /// `Some(15)` imita el búfer de GRBL 1.1 (el `Bf` que informa; el cabezal solo garantiza
    /// poder frenar dentro de esa ventana).
    pub planner_blocks: Option<usize>,
    /// La máquina se detiene al pasar de `Travel` a `Cut` y al revés. `false` en GRBL con modo
    /// láser (`M4`, `$32=1`): `G0` y `G1` seguidos van al planificador sin sincronizar y se
    /// toman como cualquier otra esquina, con la desviación de unión. `true` en el resto
    /// (controladoras que sincronizan o encienden el láser entre movimientos, o de las que no
    /// hay evidencia): es la opción conservadora.
    pub stop_on_cut_change: bool,
}

impl Default for MachineProfile {
    /// Láser de diodo con firmware GRBL de fábrica.
    fn default() -> Self {
        Self {
            max_rate_mm_min: [12000.0, 12000.0],
            accel_mm_s2: [1000.0, 1000.0],
            rapid_rate_mm_min: 12000.0,
            junction_deviation_mm: 0.01,
            planner_blocks: Some(15),
            stop_on_cut_change: false,
        }
    }
}

impl MachineProfile {
    /// Lee `$110`, `$111`, `$120`, `$121` y `$11`; lo que falte usa el valor por
    /// defecto. Los rápidos van a la mayor velocidad de eje, y el límite por
    /// eje los recorta según la dirección.
    pub fn from_grbl_settings(settings: &HashMap<u32, f64>) -> Self {
        let d = Self::default();
        let get = |key: u32, fallback: f64| settings.get(&key).copied().unwrap_or(fallback);
        let max_rate = [
            get(110, d.max_rate_mm_min[0]),
            get(111, d.max_rate_mm_min[1]),
        ];
        Self {
            max_rate_mm_min: max_rate,
            accel_mm_s2: [get(120, d.accel_mm_s2[0]), get(121, d.accel_mm_s2[1])],
            rapid_rate_mm_min: max_rate[0].max(max_rate[1]),
            junction_deviation_mm: get(11, d.junction_deviation_mm),
            planner_blocks: d.planner_blocks,
            stop_on_cut_change: d.stop_on_cut_change,
        }
        .sanitized()
    }

    /// Velocidad máxima (mm/s) al moverse en `direction_rad`: el eje que
    /// primero llega a su límite manda.
    pub fn max_speed_along(&self, direction_rad: f64) -> f64 {
        let (s, c) = direction_rad.sin_cos();
        axis_limit(self.max_rate_mm_min.map(|r| r / 60.0), c, s)
    }

    /// Aceleración efectiva (mm/s²) al moverse en `direction_rad`.
    pub fn accel_along(&self, direction_rad: f64) -> f64 {
        let (s, c) = direction_rad.sin_cos();
        axis_limit(self.accel_mm_s2, c, s)
    }

    /// Reemplaza por el valor por defecto todo lo no finito o no positivo
    /// (la desviación de unión puede ser 0: significa frenar en cada esquina).
    pub(crate) fn sanitized(&self) -> Self {
        let d = Self::default();
        let pos = |v: f64, fallback: f64| {
            if v.is_finite() && v > 0.0 {
                v
            } else {
                fallback
            }
        };
        Self {
            max_rate_mm_min: [0, 1].map(|i| pos(self.max_rate_mm_min[i], d.max_rate_mm_min[i])),
            accel_mm_s2: [0, 1].map(|i| pos(self.accel_mm_s2[i], d.accel_mm_s2[i])),
            rapid_rate_mm_min: pos(self.rapid_rate_mm_min, d.rapid_rate_mm_min),
            junction_deviation_mm: if self.junction_deviation_mm.is_finite()
                && self.junction_deviation_mm >= 0.0
            {
                self.junction_deviation_mm
            } else {
                d.junction_deviation_mm
            },
            planner_blocks: self.planner_blocks,
            stop_on_cut_change: self.stop_on_cut_change,
        }
    }
}

/// Mayor módulo del vector unitario (ux, uy) que respeta el límite de cada eje.
pub(crate) fn axis_limit(limits: [f64; 2], ux: f64, uy: f64) -> f64 {
    let mut best = f64::INFINITY;
    for (limit, u) in limits.into_iter().zip([ux.abs(), uy.abs()]) {
        if u > 1e-12 {
            best = best.min(limit / u);
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::FRAC_PI_4;

    #[test]
    fn grbl_settings_are_read_and_missing_ones_use_defaults() {
        let s = HashMap::from([(110, 3000.0), (111, 6000.0), (120, 200.0), (11, 0.02)]);
        let p = MachineProfile::from_grbl_settings(&s);
        assert_eq!(p.max_rate_mm_min, [3000.0, 6000.0]);
        assert_eq!(
            p.accel_mm_s2,
            [200.0, MachineProfile::default().accel_mm_s2[1]]
        );
        assert_eq!(p.junction_deviation_mm, 0.02);
        assert_eq!(p.rapid_rate_mm_min, 6000.0);
    }

    #[test]
    fn invalid_values_fall_back_to_defaults() {
        let s = HashMap::from([(110, -5.0), (120, f64::NAN), (11, -1.0)]);
        let p = MachineProfile::from_grbl_settings(&s);
        assert_eq!(p, MachineProfile::default());
    }

    #[test]
    fn limits_project_over_direction() {
        let p = MachineProfile {
            accel_mm_s2: [1000.0, 500.0],
            max_rate_mm_min: [6000.0, 6000.0],
            ..Default::default()
        };
        assert!((p.accel_along(0.0) - 1000.0).abs() < 1e-9);
        assert!((p.accel_along(std::f64::consts::FRAC_PI_2) - 500.0).abs() < 1e-9);
        // A 45° el eje Y (más débil) limita: 500 / cos(45°).
        assert!((p.accel_along(FRAC_PI_4) - 500.0 * 2f64.sqrt()).abs() < 1e-9);
        assert!((p.max_speed_along(FRAC_PI_4) - 100.0 * 2f64.sqrt()).abs() < 1e-9);
    }
}
