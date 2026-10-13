//! Modo Fill: relleno vectorial por barrido (scanline).
//!
//! Recibe polilíneas cerradas en mm y produce un [`ToolpathBlock`] con líneas paralelas,
//! overscan, serpentina y crosshatch opcionales. Las trayectorias abiertas no se rellenan y
//! se reportan en [`FillResult::ignored_open`].
//!
//! Las líneas de barrido caen en `y = (k + 0.5) * interval` del marco rotado, de modo que
//! una línea nunca coincide con el borde de una caja alineada con el barrido y el resultado
//! es el mismo sin importar cómo se agrupen las formas.

mod cluster;
mod scan;

use cluster::{BBox, components};
use laser_core::toolpath::{Move, PowerMode, ToolpathBlock};
use laser_core::{Point, Polyline};
use scan::{Frame, push_edges, scan};

/// Regla para decidir qué zonas quedan dentro cuando las formas se anidan o se cruzan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FillRule {
    /// Par-impar: el área entre contornos anidados se rellena y un solape se deja vacío.
    #[default]
    EvenOdd,
    /// Número de giro distinto de cero: depende del sentido de cada contorno.
    NonZero,
}

/// Cómo se agrupan las formas al barrer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Grouping {
    /// Una sola barrida sobre todas las formas: cada línea cruza todo lo que encuentre.
    #[default]
    All,
    /// Cada componente (contorno con sus agujeros, o formas que se solapan) por separado,
    /// para no recorrer los espacios vacíos entre formas lejanas.
    PerShape,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FillSettings {
    /// Separación entre líneas de barrido, en mm (mayor que 0).
    pub interval_mm: f64,
    /// 0 = horizontal, 90 = vertical, en sentido antihorario.
    pub angle_deg: f64,
    /// Alterna el sentido de las líneas consecutivas (serpentina).
    pub bidirectional: bool,
    /// Recorrido con potencia 0 antes y después de cada línea, en mm.
    pub overscan_mm: f64,
    /// Segunda pasada a `angle_deg + 90`.
    pub crosshatch: bool,
    pub fill_rule: FillRule,
    pub grouping: Grouping,
    pub speed_mm_min: f64,
    /// Potencia de 0 a 100 %.
    pub power_pct: f64,
    pub air_assist: bool,
}

impl Default for FillSettings {
    fn default() -> Self {
        Self {
            interval_mm: 0.1,
            angle_deg: 0.0,
            bidirectional: true,
            overscan_mm: 0.0,
            crosshatch: false,
            fill_rule: FillRule::EvenOdd,
            grouping: Grouping::All,
            speed_mm_min: 1000.0,
            power_pct: 20.0,
            air_assist: false,
        }
    }
}

impl FillSettings {
    pub fn validate(&self) -> Result<(), String> {
        if !self.interval_mm.is_finite() || self.interval_mm <= 0.0 {
            return Err(format!("intervalo inválido: {}", self.interval_mm));
        }
        if !self.angle_deg.is_finite() {
            return Err(format!("ángulo inválido: {}", self.angle_deg));
        }
        if !self.overscan_mm.is_finite() || self.overscan_mm < 0.0 {
            return Err(format!("overscan inválido: {}", self.overscan_mm));
        }
        if !self.speed_mm_min.is_finite() || self.speed_mm_min <= 0.0 {
            return Err(format!("velocidad inválida: {}", self.speed_mm_min));
        }
        if !(0.0..=100.0).contains(&self.power_pct) {
            return Err(format!("potencia fuera de 0-100 %: {}", self.power_pct));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct FillResult {
    pub block: ToolpathBlock,
    /// Índices (dentro de la entrada) de las trayectorias abiertas, que no se rellenan.
    pub ignored_open: Vec<usize>,
}

/// Rellena las trayectorias cerradas de `paths`.
///
/// Las cerradas con menos de 3 puntos, o con coordenadas no finitas, no aportan área y se
/// omiten sin error. Falla solo si los ajustes son inválidos o el intervalo produciría una
/// cantidad absurda de líneas.
pub fn fill(paths: &[Polyline], settings: &FillSettings) -> Result<FillResult, String> {
    settings.validate()?;

    let ignored_open: Vec<usize> = paths
        .iter()
        .enumerate()
        .filter(|(_, p)| !p.closed)
        .map(|(i, _)| i)
        .collect();

    let polys: Vec<&[Point]> = paths
        .iter()
        .filter(|p| p.closed && p.points.len() >= 3)
        .filter(|p| p.points.iter().all(|q| q.x.is_finite() && q.y.is_finite()))
        .map(|p| p.points.as_slice())
        .collect();

    let mut block = ToolpathBlock {
        label: format!(
            "Relleno intervalo={} ángulo={} potencia={}%{}",
            settings.interval_mm,
            settings.angle_deg,
            settings.power_pct,
            if settings.crosshatch {
                " crosshatch"
            } else {
                ""
            }
        ),
        speed_mm_min: settings.speed_mm_min,
        air_assist: settings.air_assist,
        power_mode: PowerMode::Dynamic,
        moves: Vec::new(),
    };

    let mut angles = vec![settings.angle_deg];
    if settings.crosshatch {
        angles.push(settings.angle_deg + 90.0);
    }
    let frames: Vec<Frame> = angles.iter().map(|&a| Frame::new(a)).collect();

    let groups = match settings.grouping {
        Grouping::All => vec![(0..polys.len()).collect()],
        Grouping::PerShape => components(&bounding_boxes(&polys, frames[0])),
    };

    for group in groups.iter().filter(|g: &&Vec<usize>| !g.is_empty()) {
        for &frame in &frames {
            let mut edges = Vec::new();
            let mut local = Vec::new();
            for &i in group {
                local.clear();
                local.extend(polys[i].iter().map(|&p| frame.to_local(p)));
                if local.iter().all(|p| p.x.is_finite() && p.y.is_finite()) {
                    push_edges(&mut edges, &local);
                }
            }
            let mut writer = LineWriter::new(settings, frame, &mut block.moves);
            scan(
                edges,
                settings.interval_mm,
                settings.fill_rule,
                |y, spans| writer.line(y, spans),
            )?;
        }
    }

    Ok(FillResult {
        block,
        ignored_open,
    })
}

fn bounding_boxes(polys: &[&[Point]], frame: Frame) -> Vec<BBox> {
    polys
        .iter()
        .map(|pts| {
            let mut b = BBox {
                xmin: f64::INFINITY,
                xmax: f64::NEG_INFINITY,
                ymin: f64::INFINITY,
                ymax: f64::NEG_INFINITY,
            };
            for &p in *pts {
                let q = frame.to_local(p);
                b.xmin = b.xmin.min(q.x);
                b.xmax = b.xmax.max(q.x);
                b.ymin = b.ymin.min(q.y);
                b.ymax = b.ymax.max(q.y);
            }
            b
        })
        .collect()
}

/// Traduce los tramos de cada línea a movimientos, alternando el sentido si hace falta.
struct LineWriter<'a> {
    overscan: f64,
    power: f64,
    bidirectional: bool,
    frame: Frame,
    emitted: usize,
    moves: &'a mut Vec<Move>,
}

impl<'a> LineWriter<'a> {
    fn new(settings: &FillSettings, frame: Frame, moves: &'a mut Vec<Move>) -> Self {
        Self {
            overscan: settings.overscan_mm,
            power: settings.power_pct,
            bidirectional: settings.bidirectional,
            frame,
            emitted: 0,
            moves,
        }
    }

    fn line(&mut self, y: f64, spans: &[(f64, f64)]) {
        let reverse = self.bidirectional && self.emitted % 2 == 1;
        self.emitted += 1;
        let sign = if reverse { -1.0 } else { 1.0 };
        let (frame, os, power) = (self.frame, self.overscan, self.power);
        let at = |x: f64| frame.to_world(x, y);

        // Cada tramo orientado como (desde, hasta) en el sentido de recorrido.
        let oriented = |&(a, b): &(f64, f64)| if reverse { (b, a) } else { (a, b) };
        let first = oriented(if reverse {
            &spans[spans.len() - 1]
        } else {
            &spans[0]
        });
        let last = oriented(if reverse {
            &spans[0]
        } else {
            &spans[spans.len() - 1]
        });

        self.moves.push(Move::Travel {
            to: at(first.0 - sign * os),
        });
        if os > 0.0 {
            self.moves.push(Move::Cut {
                to: at(first.0),
                power_pct: 0.0,
            });
        }
        for i in 0..spans.len() {
            let idx = if reverse { spans.len() - 1 - i } else { i };
            let (from, to) = oriented(&spans[idx]);
            if i > 0 {
                self.moves.push(Move::Cut {
                    to: at(from),
                    power_pct: 0.0,
                });
            }
            self.moves.push(Move::Cut {
                to: at(to),
                power_pct: power,
            });
        }
        if os > 0.0 {
            self.moves.push(Move::Cut {
                to: at(last.1 + sign * os),
                power_pct: 0.0,
            });
        }
    }
}
