//! Capas `line`: datos previos (`LineCtx`) y `Builder::line_layer`.

use crate::job::LayerReport;
use crate::place::PathMeta;
use crate::start;
use crate::stroke::{self, ShapeCtx, StrokeStats};
use crate::tabs::{self, TabSpec, TabStats};
use crate::zaxis::LayerZ;
use tlaku_core::toolpath::Toolpath;
use tlaku_core::{Job, Layer, Point, Polyline, Rgb};
use tlaku_i18n::tr;
use tlaku_plan::{PlanItem, PlanOptions};

use super::*;

impl LineCtx {
    /// Después del kerf las trayectorias cambian de número y de orden: cada una hereda los datos
    /// de la trayectoria original más cercana a su primer punto (el kerf solo las desplaza `|kerf|`
    /// a cada lado, así que su origen sigue siendo la más próxima). Es aproximado si dos objetos
    /// están a menos de un kerf entre sí. Una trayectoria con inicio o sentido fijados por el
    /// usuario conserva ambos tras la compensación (`start::follow`).
    fn realign(&mut self, after: &mut [Polyline]) {
        let seg_dist = |p: Point, path: &Polyline| -> f64 {
            let pts = &path.points;
            let n = pts.len();
            let count = if path.closed { n } else { n.saturating_sub(1) };
            if n == 1 {
                return p.dist(pts[0]);
            }
            (0..count)
                .map(|i| {
                    let (a, b) = (pts[i], pts[(i + 1) % n]);
                    let (dx, dy) = (b.x - a.x, b.y - a.y);
                    let len2 = dx * dx + dy * dy;
                    let t = if len2 > 0.0 {
                        (((p.x - a.x) * dx + (p.y - a.y) * dy) / len2).clamp(0.0, 1.0)
                    } else {
                        0.0
                    };
                    p.dist(Point::new(a.x + t * dx, a.y + t * dy))
                })
                .fold(f64::INFINITY, f64::min)
        };
        let meta = std::mem::take(&mut self.meta);
        self.meta = after
            .iter_mut()
            .map(|path| {
                let nearest = path.points.first().and_then(|&p| {
                    self.src
                        .iter()
                        .enumerate()
                        .filter(|(_, s)| !s.points.is_empty())
                        .map(|(i, s)| (i, seg_dist(p, s)))
                        .min_by(|a, b| a.1.total_cmp(&b.1))
                });
                let found = nearest.and_then(|(i, _)| meta.get(i).copied().map(|m| (i, m)));
                if let Some((i, m)) = found
                    && m.locked
                {
                    start::follow(&self.src[i], path);
                }
                found.map_or(PathMeta::NONE, |(_, m)| m)
            })
            .collect();
    }
}

impl Builder<'_> {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn line_layer(
        &mut self,
        color: Rgb,
        paths: &[Polyline],
        settings: tlaku_core::CutSettings,
        kerf_mm: f64,
        tabs: Option<TabSpec>,
        z: LayerZ,
    ) {
        let name = color.to_string();
        let mut paths = paths.to_vec();
        if kerf_mm != 0.0 {
            let open = paths.iter().filter(|p| !p.closed).count();
            if open > 0 {
                self.warnings.push(tr!(
                    "pipeline.build_line.open_no_kerf",
                    name = name,
                    n = open
                ));
            }
            paths = tlaku_geom::kerf_compensate(&paths, kerf_mm);
            self.line.realign(&mut paths);
        }
        // Los puentes van después del kerf (se parte el contorno ya compensado). Qué formas los
        // admiten se decide con todo el conjunto, porque "dentro de otra" depende de las demás.
        let eligible = tabs
            .as_ref()
            .map(|t| tlaku_geom::tab_eligible(&paths, t.skip_inner));
        // Los objetos con puentes puestos a mano usan solo esos (ver `tabs::assign_manual`).
        let objects: Vec<usize> = (0..paths.len())
            .map(|i| self.line.meta.get(i).map_or(usize::MAX, |m| m.object))
            .collect();
        let manual = tabs::assign_manual(&paths, &objects, &self.line.manual);
        if tabs.is_none() && manual.iter().any(Option::is_some) {
            self.warnings
                .push(tr!("pipeline.build_line.manual_tabs_off", name = name));
        }
        let items: Vec<PlanItem> = paths
            .into_iter()
            .enumerate()
            .map(|(i, path)| {
                let meta = self.line.meta.get(i).copied().unwrap_or(PathMeta::NONE);
                PlanItem {
                    path,
                    // Número de la capa en el orden de ejecución, el que muestra el resumen.
                    layer_order: self.layers.len() + 1,
                    group: meta.group,
                    priority: meta.priority,
                    locked: meta.locked,
                }
            })
            .collect();
        let plan = tlaku_plan::plan(
            &items,
            &PlanOptions {
                start: self.head,
                ..self.line.plan.clone()
            },
        );
        for w in &plan.warnings {
            self.warnings.push(tr!(
                "pipeline.build_line.plan_warning",
                name = name,
                warning = w
            ));
        }
        if plan.steps.is_empty() {
            self.warnings
                .push(tr!("pipeline.build_line.nothing_to_cut", name = name));
            return;
        }
        // La pieza se planificó como forma cerrada; recién ahora se parte en tramos.
        let mut stats = TabStats::default();
        let mut sstats = StrokeStats::default();
        let mut once = Vec::new();
        // Profundidad par/impar de cada forma (exterior o hueco): la usan entradas, salidas y rampa.
        let depth = self.line.stroke.as_ref().map(|_| {
            tlaku_geom::nesting_depth(&items.iter().map(|it| it.path.clone()).collect::<Vec<_>>())
        });
        for step in &plan.steps {
            let scale = self
                .line
                .meta
                .get(step.item)
                .map_or(1.0, |m| m.power_scale / 100.0);
            let power = stroke::scaled_power(settings.power_pct, scale, &mut sstats);
            let pieces = match (&tabs, &eligible, &manual[step.item]) {
                (Some(spec), _, Some(points)) => {
                    tabs::split_manual(&step.path, points, spec.options.width_mm, &mut stats)
                }
                (Some(spec), Some(ok), None) if ok[step.item] => {
                    tabs::split(&step.path, spec, &mut stats)
                }
                _ => vec![step.path.clone()],
            };
            match &self.line.stroke {
                Some(spec) => {
                    let ctx = ShapeCtx {
                        shape: &step.path,
                        pieces: &pieces,
                        power,
                        tab_power: tabs.as_ref().map_or(0.0, |t| t.gap_power(power)),
                        scale,
                        depth: depth.as_ref().map_or(0, |d| d[step.item]),
                    };
                    stroke::push_shape(spec, &ctx, &mut sstats, &mut once);
                }
                None => tabs::push_shape(
                    &mut once,
                    &pieces,
                    power,
                    tabs.as_ref().map_or(0.0, |t| t.gap_power(power)),
                ),
            }
        }
        self.warnings
            .extend(sstats.warnings(&name, self.line.stroke.as_ref()));
        if let Some(spec) = &tabs {
            if stats.shapes == 0 {
                self.warnings.push(if spec.skip_inner {
                    tr!("pipeline.build_line.tabs_no_shape_inner", name = name)
                } else {
                    tr!("pipeline.build_line.tabs_no_shape", name = name)
                });
            }
            if let Some(w) = stats.warning(&name, spec.options.width_mm) {
                self.warnings.push(w);
            }
            if let Some(w) = tabs::kerf_warning(&name, spec.options.width_mm, kerf_mm) {
                self.warnings.push(w);
            }
            if spec.power > settings.power_pct {
                self.warnings.push(tr!(
                    "pipeline.build_line.tab_power_high",
                    name = name,
                    tab = spec.power,
                    cut = settings.power_pct
                ));
            }
        }
        // Etiqueta y ajustes del bloque salen de `from_line_job`; los movimientos, de aquí.
        let mut block = Toolpath::from_line_job(&Job {
            layers: vec![Layer {
                color,
                settings: settings.clone(),
                paths: Vec::new(),
            }],
        })
        .blocks
        .remove(0);
        block.moves = once;
        let report = LayerReport {
            name,
            color,
            mode: "line",
            count: plan.steps.len(),
            unit: "trayectorias",
            speed_mm_min: settings.speed_mm_min,
            power: format!("{}%", settings.power_pct),
            passes: settings.passes,
            overscan_mm: None,
            plan: Some(plan),
            seconds: 0.0,
            bounds: None,
            tabs: stats.placed,
            z_mm: None,
            pass: None,
            sub: None,
            laser2: false,
        };
        self.push_passes(block, report, settings.passes, z, false);
    }
}
