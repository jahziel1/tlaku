//! Modo Fill: relleno vectorial por barrido (scanline), por regiones (flood) o por contornos
//! concéntricos.
//!
//! Recibe polilíneas cerradas en mm y produce un [`ToolpathBlock`] con líneas paralelas,
//! overscan, serpentina y crosshatch opcionales ([`FillPattern::Scan`]), o con contornos
//! cerrados hacia dentro ([`FillPattern::Offset`], ver el módulo `offset`). Las trayectorias
//! abiertas no se rellenan y se reportan en [`FillResult::ignored_open`].
//!
//! Las líneas de barrido caen en `y = (k + 0.5) * interval` del marco rotado, de modo que
//! una línea nunca coincide con el borde de una caja alineada con el barrido y el resultado
//! es el mismo sin importar cómo se agrupen las formas.

mod cluster;
mod flood;
mod offset;
mod scan;

pub use flood::MAX_SPANS;
pub use offset::{MAX_LEVELS, MAX_POINTS, MAX_RINGS, MAX_WORK};

use cluster::{BBox, components, shapes_interact};
use flood::{FloodParams, flood};
use scan::{Frame, push_edges, scan};
use tlaku_core::toolpath::{Move, PowerMode, ToolpathBlock};
use tlaku_core::{Point, Polyline};

/// Regla para decidir qué zonas quedan dentro cuando las formas se anidan o se cruzan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FillRule {
    /// Par-impar: el área entre contornos anidados se rellena y un solape se deja vacío.
    #[default]
    EvenOdd,
    /// Número de giro distinto de cero: depende del sentido de cada contorno.
    NonZero,
}

/// Patrón del relleno.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FillPattern {
    /// Líneas paralelas a `angle_deg` separadas `interval_mm`.
    #[default]
    Scan,
    /// Contornos concéntricos hacia dentro separados `interval_mm`: el primero a `interval_mm / 2`
    /// del borde, de afuera hacia adentro, hasta que la forma desaparece. Cada contorno es un
    /// trazo cerrado con `Cut` a `power_pct`. Solo usan este patrón `interval_mm`,
    /// `fill_rule`, `speed_mm_min`, `power_pct` y `air_assist`; `angle_deg`, `bidirectional`,
    /// `crosshatch`, `overscan_mm` y `grouping` no tienen efecto (se validan igual). Falla si el
    /// diseño supera los topes [`MAX_RINGS`], [`MAX_POINTS`], [`MAX_WORK`] o [`MAX_LEVELS`].
    Offset,
    /// Relleno por regiones: las mismas líneas que [`FillPattern::Scan`] (misma cobertura) pero
    /// cada región conexa se recorre por separado, en serpentina continua, sin cruzar los
    /// huecos ni saltar entre islas dentro de una línea. Con muchas islas pequeñas (texto)
    /// hace muchos menos viajes largos que el barrido global. Usa `interval_mm`, `angle_deg`,
    /// `bidirectional`, `overscan_mm`, `crosshatch`, `fill_rule` y `grouping`; con
    /// `bidirectional = false` todos los tramos van en el mismo sentido. Falla si hay más de
    /// 20 millones de tramos.
    Flood,
}

/// Cómo se agrupan las formas al barrer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Grouping {
    /// Una sola barrida sobre todas las formas: cada línea cruza todo lo que encuentre.
    #[default]
    All,
    /// Cada forma por separado: un contorno con sus agujeros (y lo que contengan), o formas
    /// que se tocan o se cruzan, se rellena completo antes de pasar a la siguiente, en el
    /// orden de la entrada. Formas disjuntas aunque sus cajas se monten son formas distintas.
    PerShape,
    /// Cada grupo del documento por separado, según los ids de [`fill_with_groups`]: todas las
    /// trayectorias con el mismo id se barren juntas y los grupos van en el orden en que
    /// aparece su primer id. Sin ids (con [`fill`]) se comporta como [`Grouping::PerShape`].
    ByGroup,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FillSettings {
    /// Separación entre líneas de barrido (o entre contornos concéntricos), en mm (mayor que 0).
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
    pub pattern: FillPattern,
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
            pattern: FillPattern::Scan,
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
/// cantidad absurda de líneas. Equivale a [`fill_with_groups`] sin ids de grupo.
pub fn fill(paths: &[Polyline], settings: &FillSettings) -> Result<FillResult, String> {
    fill_with_groups(paths, None, settings)
}

/// Como [`fill`], pero con un id de grupo por trayectoria (`group_ids[i]` es el grupo de
/// `paths[i]`, abiertas incluidas) que usa [`Grouping::ByGroup`]. Falla si `group_ids` no
/// tiene la misma longitud que `paths`. Con otra agrupación los ids se ignoran.
pub fn fill_with_groups(
    paths: &[Polyline],
    group_ids: Option<&[u32]>,
    settings: &FillSettings,
) -> Result<FillResult, String> {
    settings.validate()?;
    if let Some(ids) = group_ids
        && ids.len() != paths.len()
    {
        return Err(format!(
            "se esperaba un id de grupo por trayectoria ({}) y llegaron {}",
            paths.len(),
            ids.len()
        ));
    }
    let ignored_open: Vec<usize> = paths
        .iter()
        .enumerate()
        .filter(|(_, p)| !p.closed)
        .map(|(i, _)| i)
        .collect();

    // Índice original de cada trayectoria cerrada y válida, junto con sus puntos.
    let kept: Vec<usize> = (0..paths.len())
        .filter(|&i| {
            let p = &paths[i];
            p.closed
                && p.points.len() >= 3
                && p.points.iter().all(|q| q.x.is_finite() && q.y.is_finite())
        })
        .collect();
    let polys: Vec<&[Point]> = kept.iter().map(|&i| paths[i].points.as_slice()).collect();

    if settings.pattern == FillPattern::Offset {
        let mut block = ToolpathBlock {
            label: format!(
                "Relleno por contornos intervalo={} potencia={}%",
                settings.interval_mm, settings.power_pct
            ),
            speed_mm_min: settings.speed_mm_min,
            air_assist: settings.air_assist,
            power_mode: PowerMode::Dynamic,
            moves: Vec::new(),
            z_mm: None,
        };
        offset::offset_fill(
            &polys,
            settings.interval_mm,
            settings.fill_rule,
            settings.power_pct,
            &mut block.moves,
        )?;
        return Ok(FillResult {
            block,
            ignored_open,
        });
    }

    let flood_mode = settings.pattern == FillPattern::Flood;
    let mut block = ToolpathBlock {
        label: format!(
            "Relleno{} intervalo={} ángulo={} potencia={}%{}",
            if flood_mode { " por regiones" } else { "" },
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
        z_mm: None,
    };

    let mut angles = vec![settings.angle_deg];
    if settings.crosshatch {
        angles.push(settings.angle_deg + 90.0);
    }
    let frames: Vec<Frame> = angles.iter().map(|&a| Frame::new(a)).collect();

    let per_shape = || {
        components(&bounding_boxes(&polys), |i, j| {
            shapes_interact(polys[i], polys[j])
        })
    };
    let groups: Vec<Vec<usize>> = match (settings.grouping, group_ids) {
        (Grouping::All, _) => vec![(0..polys.len()).collect()],
        (Grouping::PerShape, _) | (Grouping::ByGroup, None) => per_shape(),
        (Grouping::ByGroup, Some(ids)) => {
            let mut slot: std::collections::HashMap<u32, usize> = std::collections::HashMap::new();
            let mut groups: Vec<Vec<usize>> = Vec::new();
            for (k, &orig) in kept.iter().enumerate() {
                let g = *slot.entry(ids[orig]).or_insert_with(|| {
                    groups.push(Vec::new());
                    groups.len() - 1
                });
                groups[g].push(k);
            }
            groups
        }
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
            if flood_mode {
                let params = FloodParams {
                    interval: settings.interval_mm,
                    rule: settings.fill_rule,
                    overscan: settings.overscan_mm,
                    power: settings.power_pct,
                    bidirectional: settings.bidirectional,
                };
                flood(edges, frame, params, &mut block.moves)?;
                continue;
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

/// Cajas envolventes de cada polígono, en coordenadas del diseño.
fn bounding_boxes(polys: &[&[Point]]) -> Vec<BBox> {
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
                b.xmin = b.xmin.min(p.x);
                b.xmax = b.xmax.max(p.x);
                b.ymin = b.ymin.min(p.y);
                b.ymax = b.ymax.max(p.y);
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
