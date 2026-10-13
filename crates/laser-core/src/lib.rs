//! Modelo de trabajo independiente del hardware: geometría, capas y ajustes de corte.
//!
//! Todas las coordenadas están en milímetros, en el sistema de la máquina
//! (origen abajo a la izquierda, Y hacia arriba).

pub mod optimize;
pub mod toolpath;

use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

impl Point {
    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    pub fn dist(self, other: Point) -> f64 {
        (self.x - other.x).hypot(self.y - other.y)
    }
}

/// Trayectoria de puntos unidos por rectas. Si `closed` es verdadero, el último
/// punto vuelve al primero (el primer punto no se repite al final).
#[derive(Debug, Clone, PartialEq)]
pub struct Polyline {
    pub points: Vec<Point>,
    pub closed: bool,
}

impl Polyline {
    pub fn start(&self) -> Point {
        self.points[0]
    }

    /// Punto donde termina el láser al recorrer la trayectoria.
    pub fn end(&self) -> Point {
        if self.closed {
            self.start()
        } else {
            self.points[self.points.len() - 1]
        }
    }
}

/// Color de capa. Igual que en otros programas de láser, cada color del diseño
/// es una capa con sus propios ajustes de corte.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Rgb(pub u8, pub u8, pub u8);

impl fmt::Display for Rgb {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{:02x}{:02x}{:02x}", self.0, self.1, self.2)
    }
}

impl FromStr for Rgb {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let hex = s.strip_prefix('#').unwrap_or(s);
        if hex.len() != 6 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(format!("color inválido '{s}', usa el formato #rrggbb"));
        }
        let channel = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).unwrap();
        Ok(Rgb(channel(0), channel(2), channel(4)))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CutSettings {
    pub speed_mm_min: f64,
    /// Potencia de 0 a 100 %.
    pub power_pct: f64,
    pub passes: u32,
    pub air_assist: bool,
}

impl Default for CutSettings {
    fn default() -> Self {
        Self {
            speed_mm_min: 1000.0,
            power_pct: 20.0,
            passes: 1,
            air_assist: false,
        }
    }
}

impl CutSettings {
    pub fn validate(&self) -> Result<(), String> {
        if self.speed_mm_min.is_nan() || self.speed_mm_min <= 0.0 {
            return Err(format!("velocidad inválida: {}", self.speed_mm_min));
        }
        if !(0.0..=100.0).contains(&self.power_pct) {
            return Err(format!("potencia fuera de 0-100 %: {}", self.power_pct));
        }
        if self.passes == 0 {
            return Err("las pasadas deben ser al menos 1".into());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Layer {
    pub color: Rgb,
    pub settings: CutSettings,
    pub paths: Vec<Polyline>,
}

/// Trabajo listo para enviar: las capas se ejecutan en orden.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Job {
    pub layers: Vec<Layer>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bounds {
    pub min: Point,
    pub max: Point,
}

impl Job {
    pub fn bounds(&self) -> Option<Bounds> {
        let mut points = self
            .layers
            .iter()
            .flat_map(|l| &l.paths)
            .flat_map(|p| &p.points);
        let first = *points.next()?;
        let mut b = Bounds {
            min: first,
            max: first,
        };
        for p in points {
            b.min.x = b.min.x.min(p.x);
            b.min.y = b.min.y.min(p.y);
            b.max.x = b.max.x.max(p.x);
            b.max.y = b.max.y.max(p.y);
        }
        Some(b)
    }

    /// Reordena las trayectorias de cada capa para reducir movimientos en vacío.
    pub fn optimize(&mut self) {
        let mut pos = Point::new(0.0, 0.0);
        for layer in &mut self.layers {
            layer.paths = optimize::order_paths(std::mem::take(&mut layer.paths), pos);
            if let Some(last) = layer.paths.last() {
                pos = last.end();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rgb_roundtrip() {
        let c: Rgb = "#FF8000".parse().unwrap();
        assert_eq!(c, Rgb(255, 128, 0));
        assert_eq!(c.to_string(), "#ff8000");
        assert!("#12345".parse::<Rgb>().is_err());
    }

    #[test]
    fn settings_validation() {
        assert!(CutSettings::default().validate().is_ok());
        let bad = CutSettings {
            power_pct: 120.0,
            ..Default::default()
        };
        assert!(bad.validate().is_err());
    }
}
