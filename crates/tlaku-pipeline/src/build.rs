use crate::gcode::{dialect_warnings, point_to_machine};
use crate::job::{ComputedJob, LayerReport, block_bounds, check_device_bounds};
use crate::layers::{
    LayerRole, ResolvedImage, ResolvedLayer, SeqUnit, classify, effective_roles, merge_steps,
    order_note, resolve_image, resolve_layer, unit_sequence,
};
use crate::place::{LayerWork, PathMeta, PlacedImage, place};
use crate::preview::preview;
use crate::rotary;
use crate::scan_offset;
use crate::start;
use crate::stroke::{self, ShapeCtx, StrokeSpec, StrokeStats};
use crate::tabs::{self, TabSpec, TabStats};
use crate::zaxis::{self, LayerZ};
use std::collections::HashMap;
use std::f64::consts::FRAC_PI_2;
use std::path::Path;
use tlaku_core::toolpath::{Move, Toolpath, ToolpathBlock};
use tlaku_core::{Job, Layer, Point, Polyline, Rgb};
use tlaku_doc::{
    DeviceProfile, Document, JobOptions, LayerMode, LayerOrder, LayerSettings, Optimization,
    Overscan, ScanOffset,
};
use tlaku_fill::FillPattern;
use tlaku_plan::{PlanItem, PlanOptions, Winding};
use tlaku_raster::{GrayF32, Luma, ProcessOptions, Sampling};
use tlaku_sim::MachineProfile;

/// Parte de una capa fill: las trayectorias de los objetos con una misma escala de potencia y la
/// potencia (%) con que se rellenan.
struct FillPart {
    paths: Vec<Polyline>,
    /// Un id de grupo por trayectoria (`tlaku_fill::fill_with_groups`).
    group_ids: Vec<u32>,
    /// Objetos de los que salen (posición en `Document::objects`), sin repetir.
    objects: Vec<usize>,
    power: f64,
}

impl FillPart {
    fn new(power: f64) -> Self {
        FillPart {
            paths: Vec::new(),
            group_ids: Vec::new(),
            objects: Vec::new(),
            power,
        }
    }
}

/// Máximo de pares de cajas que se comparan al buscar formas superpuestas de escalas distintas.
const OVERLAP_CHECK_LIMIT: usize = 4_000_000;

/// ¿Se superponen (por su caja) trayectorias de partes distintas? Con demasiadas no se comprueba.
fn fill_parts_overlap(parts: &[FillPart]) -> bool {
    type Rect = [f64; 4];
    let boxes: Vec<Vec<Rect>> = parts
        .iter()
        .map(|part| {
            part.paths
                .iter()
                .filter_map(|path| {
                    let first = path.points.first()?;
                    let mut b = [first.x, first.y, first.x, first.y];
                    for q in &path.points {
                        b = [b[0].min(q.x), b[1].min(q.y), b[2].max(q.x), b[3].max(q.y)];
                    }
                    Some(b)
                })
                .collect()
        })
        .collect();
    let mut pairs = 0_usize;
    for (i, a) in boxes.iter().enumerate() {
        for b in &boxes[i + 1..] {
            pairs = pairs.saturating_add(a.len().saturating_mul(b.len()));
        }
    }
    if pairs > OVERLAP_CHECK_LIMIT {
        return false;
    }
    boxes.iter().enumerate().any(|(i, a)| {
        boxes[i + 1..].iter().any(|b| {
            a.iter().any(|x| {
                b.iter()
                    .any(|y| x[0] < y[2] && y[0] < x[2] && x[1] < y[3] && y[1] < x[3])
            })
        })
    })
}

fn machine_profile(device: &DeviceProfile) -> Result<MachineProfile, String> {
    let positive = |name: &str, values: &[f64]| {
        if values.iter().all(|v| v.is_finite() && *v > 0.0) {
            Ok(())
        } else {
            Err(format!(
                "la máquina '{}': {name} debe ser mayor que 0",
                device.name
            ))
        }
    };
    positive("el área de trabajo", &device.bed)?;
    positive("la aceleración", &device.accel)?;
    positive("la velocidad máxima", &device.max_rate)?;
    positive("la velocidad de movimientos rápidos", &[device.rapid_rate])?;
    // Con rotativo el trabajo está en mm de superficie: los límites de Y de la controladora se
    // pasan a superficie para que el tiempo y el overscan sean los reales (ver `rotary`).
    let k = rotary::y_scale(device)?;
    Ok(MachineProfile {
        max_rate_mm_min: [device.max_rate[0], device.max_rate[1] / k],
        accel_mm_s2: [device.accel[0], device.accel[1] / k],
        rapid_rate_mm_min: device.rapid_rate,
        ..MachineProfile::default()
    })
}

fn travel_count(block: &ToolpathBlock) -> usize {
    block
        .moves
        .iter()
        .filter(|m| matches!(m, Move::Travel { .. }))
        .count()
}

/// Repite los movimientos del bloque una vez por pasada.
fn repeat_moves(block: &mut ToolpathBlock, passes: u32) {
    if passes > 1 {
        let once = block.moves.clone();
        for _ in 1..passes {
            block.moves.extend_from_slice(&once);
        }
    }
}

/// Como [`repeat_moves`] y anota las pasadas en la etiqueta.
fn repeat_passes(block: &mut ToolpathBlock, passes: u32) {
    if passes > 1 {
        repeat_moves(block, passes);
        block.label.push_str(&format!(" pasadas={passes}"));
    }
}

fn flipped(gray: &GrayF32, mirror_x: bool, mirror_y: bool) -> GrayF32 {
    if !mirror_x && !mirror_y {
        return gray.clone();
    }
    GrayF32::from_fn(gray.width, gray.height, |x, y| {
        let sx = if mirror_x { gray.width - 1 - x } else { x };
        let sy = if mirror_y { gray.height - 1 - y } else { y };
        gray.get(sx, sy)
    })
}

/// Datos de la capa line que `build` deja listos antes de llamar a `line_layer`.
#[derive(Default)]
struct LineCtx {
    /// Origen (objeto, grupo, prioridad, escala de potencia) de cada trayectoria que recibe
    /// `line_layer`; `meta[step.item]` describe al paso del plan `step`.
    meta: Vec<PathMeta>,
    /// Las trayectorias antes de compensar el kerf (que las reordena y regrupa).
    src: Vec<Polyline>,
    /// Puentes manuales por objeto (posición en `Document::objects`); ver `tabs::assign_manual`.
    manual: HashMap<usize, Vec<Point>>,
    /// Opciones del planificador (la posición de partida la pone `line_layer`).
    plan: PlanOptions,
    /// Ajustes de trazo de la capa (perforación, entradas, pausas...); ver `stroke`.
    stroke: Option<StrokeSpec>,
}

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

struct Builder<'a> {
    profile: &'a MachineProfile,
    toolpath: Toolpath,
    layers: Vec<LayerReport>,
    warnings: Vec<String>,
    head: Point,
    scan_offsets: &'a [ScanOffset],
    scan_initial: f64,
    line: LineCtx,
    /// `(posición, total)` de la operación en curso si su capa tiene sub-capas.
    sub: Option<(u32, u32)>,
    /// Hide backlash: los rellenos e imágenes se graban en un solo sentido.
    unidirectional: bool,
    /// La capa en curso va con el segundo láser.
    laser2: bool,
}

impl Builder<'_> {
    fn push(&mut self, block: ToolpathBlock, mut report: LayerReport) {
        if let Some((n, total)) = self.sub {
            report.sub = Some((n, total));
            report.name = format!("{} · {n}/{total}", report.name);
        }
        report.bounds = block_bounds(&block);
        report.laser2 = self.laser2;
        if let Some(last) = block.moves.last() {
            self.head = last.to();
        }
        self.toolpath.blocks.push(block);
        self.layers.push(report);
    }

    /// Agrega el bloque de una capa con sus `passes` pasadas. `block` trae los movimientos de
    /// una sola pasada. Sin `z_step` (o sin eje Z) las pasadas se repiten dentro del mismo
    /// bloque; con `z_step` cada pasada es su propio bloque con su Z y su propio informe, para
    /// conservar la correspondencia 1:1 entre bloques e informes (ver `zaxis`).
    fn push_passes(
        &mut self,
        mut block: ToolpathBlock,
        mut report: LayerReport,
        passes: u32,
        z: LayerZ,
        label_passes: bool,
    ) {
        if z.splits(passes) {
            for k in 0..passes {
                let mut b = block.clone();
                b.z_mm = z.z_at(k);
                if let Some(zk) = b.z_mm {
                    b.label
                        .push_str(&format!(" pasada {}/{passes} z={zk}", k + 1));
                }
                let mut r = report.clone();
                r.passes = 1;
                r.pass = Some((k + 1, passes));
                r.z_mm = b.z_mm;
                if k > 0 {
                    // El plan y sus explicaciones se muestran una sola vez.
                    r.plan = None;
                }
                self.push(b, r);
            }
            return;
        }
        if label_passes {
            repeat_passes(&mut block, passes);
        } else {
            repeat_moves(&mut block, passes);
        }
        block.z_mm = z.z_at(0);
        report.z_mm = block.z_mm;
        self.push(block, report);
    }

    /// Como [`Self::push_passes`] cuando cada pasada tiene sus propios movimientos (por
    /// ejemplo, con incremento de ángulo): `blocks` trae una pasada por elemento. Se reparte
    /// igual que las pasadas con Z: si `z` pide Z distinta en cada pasada, cada una es su propio
    /// bloque con su informe; si no, se concatenan en un solo bloque.
    fn push_series(&mut self, blocks: Vec<ToolpathBlock>, mut report: LayerReport, z: LayerZ) {
        let passes = blocks.len() as u32;
        if z.splits(passes) {
            for (k, mut b) in (0u32..).zip(blocks) {
                b.z_mm = z.z_at(k);
                if let Some(zk) = b.z_mm {
                    b.label
                        .push_str(&format!(" pasada {}/{passes} z={zk}", k + 1));
                }
                let mut r = report.clone();
                r.passes = 1;
                r.pass = Some((k + 1, passes));
                r.z_mm = b.z_mm;
                r.count = travel_count(&b);
                self.push(b, r);
            }
            return;
        }
        let mut all = blocks.into_iter();
        let Some(mut block) = all.next() else { return };
        for b in all {
            block.moves.extend(b.moves);
        }
        block.label.push_str(&format!(" pasadas={passes}"));
        block.z_mm = z.z_at(0);
        report.z_mm = block.z_mm;
        self.push(block, report);
    }

    #[allow(clippy::too_many_arguments)]
    fn line_layer(
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
                self.warnings.push(format!(
                    "capa {name}: {open} trayectoria(s) abierta(s) no se compensan por kerf"
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
            self.warnings.push(format!(
                "capa {name}: hay objetos con puentes manuales, pero los puentes de la capa están \
                 desactivados (actívalos para que se respeten)"
            ));
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
            self.warnings.push(format!("capa {name}: {w}"));
        }
        if plan.steps.is_empty() {
            self.warnings.push(format!(
                "capa {name}: no quedó ninguna trayectoria que cortar"
            ));
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
                self.warnings.push(format!(
                    "capa {name}: los puentes están activos pero no hay ninguna forma cerrada donde ponerlos{}",
                    if spec.skip_inner {
                        " (las formas dentro de otra se omiten)"
                    } else {
                        ""
                    }
                ));
            }
            if let Some(w) = stats.warning(&name, spec.options.width_mm) {
                self.warnings.push(w);
            }
            if let Some(w) = tabs::kerf_warning(&name, spec.options.width_mm, kerf_mm) {
                self.warnings.push(w);
            }
            if spec.power > settings.power_pct {
                self.warnings.push(format!(
                    "capa {name}: la potencia de puente ({} %) supera la del corte ({} %); los \
                     huecos se marcan con la del corte",
                    spec.power, settings.power_pct
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

    /// Capa fill. Con `angle_increment != 0` y varias pasadas, la pasada `k` barre con el ángulo
    /// `angle + k * angle_increment` (la geometría cambia en cada pasada, así que cada una se
    /// calcula aparte); si no, se calcula una vez y se repite, como siempre.
    ///
    /// `meta` describe cada trayectoria (`LayerWork::meta`): su grupo para el relleno por grupos y
    /// la escala de potencia de su objeto. Si los objetos tienen escalas distintas se rellenan por
    /// separado, una vez por escala, y los movimientos de todas van en un solo bloque.
    #[allow(clippy::too_many_arguments)]
    fn fill_layer(
        &mut self,
        color: Rgb,
        paths: &[Polyline],
        meta: &[PathMeta],
        settings: tlaku_fill::FillSettings,
        overscan: Overscan,
        passes: u32,
        z: LayerZ,
        angle_increment: f64,
    ) -> Result<(), String> {
        let name = color.to_string();
        let parts = self.fill_parts(&name, paths, meta, settings.power_pct);
        if parts.is_empty() {
            return Ok(());
        }
        let power_text = {
            let lo = parts.iter().map(|p| p.power).fold(f64::INFINITY, f64::min);
            let hi = parts.iter().map(|p| p.power).fold(0.0, f64::max);
            if lo == hi {
                format!("{hi}%")
            } else {
                format!("{lo}-{hi}%")
            }
        };
        let mut varied = angle_increment != 0.0 && passes > 1;
        if varied && settings.pattern == FillPattern::Offset {
            self.warnings.push(format!(
                "capa {name}: angle_increment no aplica al relleno por contornos; se ignora"
            ));
            varied = false;
        }
        let mut blocks = Vec::new();
        let mut overscan_mm = 0.0_f64;
        for k in 0..if varied { passes } else { 1 } {
            let mut s = settings.clone();
            if varied {
                s.angle_deg += angle_increment * f64::from(k);
            }
            if self.unidirectional {
                s.bidirectional = false;
            }
            let angle = s.angle_deg;
            let Some((mut block, mm)) = self.fill_block(&name, &parts, s, overscan, k == 0)? else {
                return Ok(());
            };
            if varied {
                block.label.push_str(&format!(" ángulo={angle}"));
            }
            overscan_mm = overscan_mm.max(mm);
            blocks.push(block);
        }
        let lines = blocks.iter().map(travel_count).sum();
        let report = LayerReport {
            name,
            color,
            mode: "fill",
            count: lines,
            unit: "líneas",
            speed_mm_min: settings.speed_mm_min,
            power: power_text,
            passes,
            overscan_mm: Some(overscan_mm),
            plan: None,
            seconds: 0.0,
            bounds: None,
            tabs: 0,
            z_mm: None,
            pass: None,
            sub: None,
            laser2: false,
        };
        if varied {
            self.push_series(blocks, report, z);
        } else {
            let block = blocks.remove(0);
            self.push_passes(block, report, passes, z, true);
        }
        Ok(())
    }

    /// Parte las trayectorias de una capa fill por la escala de potencia de su objeto
    /// (`DesignObject::power_scale`), en orden de aparición, y da a cada parte su potencia (la de
    /// la capa por la escala, limitada a 100 %). Sin objetos con escalas distintas sale una sola
    /// parte con todas las trayectorias. Avisa de lo que se limita, se omite o se superpone.
    fn fill_parts(
        &mut self,
        name: &str,
        paths: &[Polyline],
        meta: &[PathMeta],
        layer_pct: f64,
    ) -> Vec<FillPart> {
        let mut parts: Vec<(f64, FillPart)> = Vec::new();
        for (i, path) in paths.iter().enumerate() {
            let m = meta.get(i).copied().unwrap_or(PathMeta::NONE);
            let at = match parts
                .iter()
                .position(|(s, _)| s.to_bits() == m.power_scale.to_bits())
            {
                Some(at) => at,
                None => {
                    parts.push((
                        m.power_scale,
                        FillPart::new(layer_pct * m.power_scale / 100.0),
                    ));
                    parts.len() - 1
                }
            };
            let part = &mut parts[at].1;
            part.paths.push(path.clone());
            part.group_ids.push(m.fill_group);
            if !part.objects.contains(&m.object) {
                part.objects.push(m.object);
            }
        }
        let mut clamped = 0;
        let mut silent = 0;
        let mut out = Vec::new();
        for (_, mut part) in parts {
            if part.power > 100.0 {
                clamped += part.objects.len();
                part.power = 100.0;
            }
            if part.power <= 0.0 {
                silent += part.objects.len();
                continue;
            }
            out.push(part);
        }
        if clamped > 0 {
            self.warnings.push(format!(
                "capa {name}: la escala de potencia de {clamped} objeto(s) pasa de 100 %: se limita a 100 %"
            ));
        }
        if silent > 0 {
            self.warnings.push(format!(
                "capa {name}: {silent} objeto(s) con potencia 0 (escala de potencia 0 %) no se rellenan"
            ));
        }
        if out.len() > 1 {
            self.warnings.push(format!(
                "capa {name}: los objetos tienen {} escalas de potencia distintas; se rellenan por separado (una vez por escala)",
                out.len()
            ));
            if fill_parts_overlap(&out) {
                self.warnings.push(format!(
                    "capa {name}: hay objetos con escalas de potencia distintas que se superponen; \
                     cada escala se rellena por su cuenta, así que lo común se graba dos veces \
                     (un hueco de otra escala no se respeta)"
                ));
            }
        }
        out
    }

    /// Los movimientos de una pasada de relleno con `settings` tal cual (su ángulo incluido) y el
    /// overscan usado; `None` si no produjo ninguna línea. `first` es la primera pasada de la
    /// capa: los avisos que no dependen del ángulo se dan solo una vez. Cada parte se rellena con
    /// su propia potencia y todas se juntan en un bloque.
    fn fill_block(
        &mut self,
        name: &str,
        parts: &[FillPart],
        mut settings: tlaku_fill::FillSettings,
        overscan: Overscan,
        first: bool,
    ) -> Result<Option<(ToolpathBlock, f64)>, String> {
        if overscan == Overscan::Auto {
            let mut mm = tlaku_sim::overscan_mm(
                settings.speed_mm_min,
                settings.angle_deg.to_radians(),
                self.profile,
            );
            if settings.crosshatch {
                let perp = settings.angle_deg.to_radians() + FRAC_PI_2;
                mm = mm.max(tlaku_sim::overscan_mm(
                    settings.speed_mm_min,
                    perp,
                    self.profile,
                ));
            }
            settings.overscan_mm = mm;
        }
        let mut merged: Option<ToolpathBlock> = None;
        let mut ignored = 0;
        for part in parts {
            let mut s = settings.clone();
            s.power_pct = part.power;
            let (block, open) = self.fill_part_block(name, part, &s)?;
            ignored += open;
            if let Some(block) = block {
                match &mut merged {
                    Some(m) => m.moves.extend(block.moves),
                    None => merged = Some(block),
                }
            }
        }
        if first && ignored > 0 {
            self.warnings.push(format!(
                "capa {name}: {ignored} trayectoria(s) abierta(s) ignorada(s): el relleno solo usa trayectorias cerradas"
            ));
        }
        let Some(block) = merged else {
            if first {
                self.warnings
                    .push(format!("capa {name}: el relleno no produjo ninguna línea"));
            }
            return Ok(None);
        };
        if first
            && matches!(settings.pattern, FillPattern::Scan | FillPattern::Flood)
            && settings.bidirectional
            && let Some(w) = scan_offset::overscan_warning(
                &format!("capa {name}"),
                self.scan_offsets,
                settings.speed_mm_min,
                settings.overscan_mm,
            )
        {
            self.warnings.push(w);
        }
        Ok(Some((block, settings.overscan_mm)))
    }

    /// El relleno de una parte (una escala de potencia) con las compensaciones de barrido, y
    /// cuántas trayectorias abiertas ignoró.
    fn fill_part_block(
        &mut self,
        name: &str,
        part: &FillPart,
        settings: &tlaku_fill::FillSettings,
    ) -> Result<(Option<ToolpathBlock>, usize), String> {
        // Los ids de grupo solo cuentan con `FillGrouping::ByGroup`; con otra agrupación el motor
        // los ignora.
        let result = tlaku_fill::fill_with_groups(&part.paths, Some(&part.group_ids), settings)
            .map_err(|e| format!("capa {name}: {e}"))?;
        let ignored = result.ignored_open.len();
        let mut block = result.block;
        if block.moves.is_empty() {
            return Ok((None, ignored));
        }
        // Solo el barrido (y el relleno por regiones, que son las mismas líneas) tiene líneas de
        // ida y vuelta; el relleno por contornos no.
        if matches!(settings.pattern, FillPattern::Scan | FillPattern::Flood) {
            let mut angles = vec![settings.angle_deg];
            if settings.crosshatch {
                angles.push(settings.angle_deg + 90.0);
            }
            scan_offset::apply(
                &mut block,
                self.scan_offsets,
                self.scan_initial,
                &angles,
                settings.bidirectional,
            );
        }
        Ok((Some(block), ignored))
    }

    fn image(
        &mut self,
        color: Rgb,
        img: &PlacedImage,
        r: &ResolvedImage,
        z: LayerZ,
        angle_increment: f64,
    ) -> Result<(), String> {
        let mut raster = r.raster.clone();
        if self.unidirectional {
            raster.bidirectional = false;
        }
        if r.overscan == Overscan::Auto {
            raster.overscan_mm = tlaku_sim::overscan_mm(raster.speed_mm_min, 0.0, self.profile);
        }
        // Con incremento de ángulo la pasada `k` barre a `angle + k * incremento`: el bitmap es
        // el mismo, pero el recorrido cambia en cada pasada.
        let varied = angle_increment != 0.0 && r.passes > 1;
        let gray = tlaku_raster::gray::load(Path::new(&img.source_path), Luma::default())
            .map_err(|e| format!("{}: {e}", img.source_path))?;
        let gray = flipped(&gray, img.mirror_x, img.mirror_y);
        let opts = ProcessOptions {
            adjust: r.adjust,
            sampling: Sampling::Interval(r.interval_mm),
            mode: r.mode,
            ..Default::default()
        };
        let (w, h) = img.size_mm;
        let mut bitmap =
            tlaku_raster::process(&gray, w, h, &opts).map_err(|e| format!("imagen: {e}"))?;
        // Máscara de recorte: lo que queda fuera de las formas no se graba.
        tlaku_raster::apply_mask(&mut bitmap, img.origin, &img.mask);
        let mut blocks = Vec::new();
        let mut overscan_mm = raster.overscan_mm;
        for k in 0..if varied { r.passes } else { 1 } {
            let mut pass = raster.clone();
            if varied {
                pass.angle_deg += angle_increment * f64::from(k);
                if r.overscan == Overscan::Auto {
                    pass.overscan_mm = tlaku_sim::overscan_mm(
                        pass.speed_mm_min,
                        pass.angle_deg.to_radians(),
                        self.profile,
                    );
                }
                overscan_mm = overscan_mm.max(pass.overscan_mm);
            }
            let mut block = tlaku_raster::raster_to_block(&bitmap, img.origin, &pass);
            if block.moves.is_empty() {
                self.warnings.push(
                    "imagen: no hay ningún punto que grabar (la imagen resultó en blanco)".into(),
                );
                return Ok(());
            }
            scan_offset::apply(
                &mut block,
                self.scan_offsets,
                self.scan_initial,
                &[pass.angle_deg],
                pass.bidirectional,
            );
            if k == 0
                && pass.bidirectional
                && let Some(w) = scan_offset::overscan_warning(
                    "imagen",
                    self.scan_offsets,
                    pass.speed_mm_min,
                    pass.overscan_mm,
                )
            {
                self.warnings.push(w);
            }
            blocks.push(block);
        }
        let lines = blocks.iter().map(travel_count).sum();
        let report = LayerReport {
            name: "imagen".into(),
            color,
            mode: "image",
            count: lines,
            unit: "líneas",
            speed_mm_min: raster.speed_mm_min,
            power: format!("{}-{}%", raster.min_power_pct, raster.max_power_pct),
            passes: r.passes,
            overscan_mm: Some(overscan_mm),
            plan: None,
            seconds: 0.0,
            bounds: None,
            tabs: 0,
            z_mm: None,
            pass: None,
            sub: None,
            laser2: false,
        };
        if varied {
            self.push_series(blocks, report, z);
        } else {
            let block = blocks.remove(0);
            self.push_passes(block, report, r.passes, z, true);
        }
        Ok(())
    }
}

/// Ajustes ya validados de una capa con contenido.
enum Resolved {
    Vector(ResolvedLayer),
    Image(ResolvedImage),
}

/// Calcula el trabajo sin verificar los límites de la cama (útil para `plan` o para mostrar
/// la vista previa de un diseño que no cabe). `current` es la posición de trabajo de la
/// cabeza; hace falta con `start_from = current_position`.
///
/// `current` y `options.user_origin` vienen en coordenadas de la máquina (las que muestra
/// GRBL), y aquí se pasan a las del documento según `device.origin` (y, con rotativo, de mm
/// de la controladora a mm de superficie).
pub fn build(
    doc: &Document,
    device: &DeviceProfile,
    options: &JobOptions,
    current: Option<Point>,
) -> Result<ComputedJob, String> {
    let profile = machine_profile(device)?;
    let y_scale = rotary::y_scale(device)?;
    let mut warnings = Vec::new();
    // Inversa de lo que hace `gcode`: primero se deshace la escala del rotativo y luego el origen.
    let to_doc =
        |p: Point| point_to_machine(Point::new(p.x, p.y / y_scale), device.bed, device.origin);
    let options = &JobOptions {
        user_origin: options
            .user_origin
            .map(|[x, y]| to_doc(Point::new(x, y)))
            .map(|p| [p.x, p.y]),
        ..options.clone()
    };
    let placement = place(doc, options, current.map(to_doc), &mut warnings)?;
    let work: &[LayerWork] = &placement.layers;
    if work.is_empty() {
        return Err(nothing_to_produce(&warnings));
    }
    let optimization = &options.optimization;
    if !(optimization.overlap_tolerance.is_finite() && optimization.overlap_tolerance >= 0.0) {
        return Err(format!(
            "optimización: overlap_tolerance inválida ({}); debe ser 0 mm o más",
            optimization.overlap_tolerance
        ));
    }
    let plan_options = plan_options(optimization);
    if optimization.hide_backlash {
        warnings.push(
            "hide_backlash activo: las formas cerradas se cortan siempre en sentido antihorario y \
             los rellenos e imágenes se graban en un solo sentido; el trabajo es más lento"
                .into(),
        );
    }

    // Se validan todas las capas antes de procesar ninguna.
    let resolved = work
        .iter()
        .map(|l| match l.settings.mode {
            LayerMode::Image => resolve_image(l.settings).map(Resolved::Image),
            _ => resolve_layer(l.settings).map(Resolved::Vector),
        })
        .collect::<Result<Vec<_>, _>>()?;
    // Puentes (solo capas line) y Z de cada capa.
    let mut tab_specs = Vec::with_capacity(work.len());
    let mut stroke_specs = Vec::with_capacity(work.len());
    let mut zs = Vec::with_capacity(work.len());
    for l in work {
        if !l.settings.angle_increment.is_finite() {
            return Err(format!(
                "capa {}: angle_increment inválido: {}",
                l.color, l.settings.angle_increment
            ));
        }
        tab_specs.push(match l.settings.mode {
            LayerMode::Line => tabs::resolve(l.settings)?,
            _ => None,
        });
        stroke_specs.push(match l.settings.mode {
            LayerMode::Line => stroke::resolve(l.settings)?,
            _ => None,
        });
        zs.push(zaxis::resolve(device, l.settings, &mut warnings)?);
    }
    let classes: Vec<_> = resolved
        .iter()
        .map(|r| match r {
            Resolved::Vector(v) => classify(v),
            Resolved::Image(_) => (LayerRole::Engrave, None),
        })
        .collect();
    let roles: Vec<LayerRole> = classes.iter().map(|(r, _)| *r).collect();
    let mut forced = Vec::with_capacity(work.len());
    for (i, l) in work.iter().enumerate() {
        // Las sub-capas siguen a su capa: su `order` se ignora.
        let pos = match l.settings.order {
            _ if l.sub.is_some() => None,
            Some(0) => {
                return Err(format!(
                    "capa {}: la posición debe ser un entero desde 1",
                    l.color
                ));
            }
            Some(n) => Some(n as usize),
            None => None,
        };
        if let Some(n) = pos
            && let Some(d) = work[..i]
                .iter()
                .find(|p| p.sub.is_none() && p.settings.order == l.settings.order)
        {
            return Err(format!(
                "las capas {} y {} piden la misma posición: order={n}",
                d.color, l.color
            ));
        }
        forced.push(pos);
    }

    let units: Vec<SeqUnit> = work
        .iter()
        .enumerate()
        .map(|(i, l)| SeqUnit {
            parent: l.parent,
            sub: l.sub.is_some(),
            role: roles[i],
            forced: forced[i],
        })
        .collect();
    let (sequence, movable) = unit_sequence(&units, options.layer_order);
    let effective = effective_roles(&units);
    let n = movable.len();
    let mut forced_list = Vec::new();
    for (l, pos) in work.iter().zip(&forced) {
        if let Some(pos) = *pos {
            forced_list.push((l.color, pos));
            if pos > n {
                warnings.push(format!(
                    "capa {}: order={pos} excede el número de capas ({n}); queda al final",
                    l.color
                ));
            }
        }
    }
    forced_list.sort_by_key(|(_, p)| *p);
    // Con `by_layer = false` se juntan las capas de corte consecutivas que hacen exactamente lo
    // mismo para planificarlas como una sola.
    let settings_list: Vec<&LayerSettings> = work.iter().map(|l| l.settings).collect();
    let steps: Vec<Vec<usize>> = if optimization.by_layer {
        sequence.iter().map(|&i| vec![i]).collect()
    } else {
        merge_steps(&sequence, &settings_list, &forced)
    };
    let merged: Vec<Vec<Rgb>> = steps
        .iter()
        .filter(|s| s.len() > 1)
        .map(|s| s.iter().map(|&i| work[i].color).collect())
        .collect();
    let has_subs = work.iter().any(|l| l.sub_total > 0);
    let order_note_text = {
        let order = (n > 1).then(|| {
            let free = |i: usize| forced[i].is_none() && movable.contains(&i);
            let free_roles: Vec<LayerRole> = (0..work.len())
                .filter(|&i| free(i))
                .map(|i| effective[i])
                .collect();
            let free_cuts: Vec<(Rgb, &'static str)> = sequence
                .iter()
                .filter(|&&i| free(i))
                .filter_map(|&i| classes[i].1.map(|why| (work[i].color, why)))
                .collect();
            let mut note = order_note(options.layer_order, &free_roles, &free_cuts, &forced_list);
            if has_subs && options.layer_order == LayerOrder::Auto {
                note.push_str(
                    "; las sub-capas se ordenan por su propio rol, nunca antes que su capa",
                );
            }
            note
        });
        let opt = (*optimization != Optimization::default())
            .then(|| optimization_note(optimization, &plan_options, &merged));
        match (order, opt) {
            (Some(o), Some(p)) => Some(format!("{o}; {p}")),
            (o, p) => o.or(p),
        }
    };

    let mut b = Builder {
        profile: &profile,
        toolpath: Toolpath::default(),
        layers: Vec::new(),
        warnings,
        head: placement.start,
        scan_offsets: &device.scan_offsets,
        scan_initial: device.scan_initial_offset,
        line: LineCtx::default(),
        sub: None,
        unidirectional: optimization.hide_backlash,
        laser2: false,
    };
    for step in &steps {
        let i = step[0];
        let layer = &work[i];
        let color = layer.color;
        b.sub = layer.position();
        b.laser2 = layer.settings.laser2;
        match &resolved[i] {
            Resolved::Vector(ResolvedLayer::Line { settings, kerf_mm }) => {
                let mut paths: Vec<Polyline> = Vec::new();
                let mut meta = Vec::new();
                let mut manual: HashMap<usize, Vec<Point>> = HashMap::new();
                for &k in step {
                    paths.extend(work[k].paths.iter().cloned());
                    meta.extend(work[k].meta.iter().copied());
                    for (o, pts) in &work[k].manual_tabs {
                        manual.entry(*o).or_insert_with(|| pts.clone());
                    }
                }
                if optimization.remove_overlaps {
                    if *kerf_mm != 0.0 {
                        b.warnings.push(format!(
                            "capa {color}: remove_overlaps no se aplica con kerf (las formas \
                             abiertas que deja no se pueden compensar); se corta como está"
                        ));
                    } else {
                        let d = tlaku_geom::dedupe_paths(&paths, optimization.overlap_tolerance);
                        if d.removed_length > 0.0 {
                            let cut = d.changed.iter().filter(|&&c| c).count();
                            b.warnings.push(format!(
                                "capa {color}: remove_overlaps quitó {:.2} mm de trazo repetido \
                                 en {cut} trayectoria(s)",
                                d.removed_length
                            ));
                        }
                        meta = d.origin.iter().map(|&o| meta[o]).collect();
                        paths = d.paths;
                    }
                }
                b.line = LineCtx {
                    src: if *kerf_mm != 0.0 {
                        paths.clone()
                    } else {
                        Vec::new()
                    },
                    meta,
                    manual,
                    plan: plan_options.clone(),
                    stroke: stroke_specs[i].clone(),
                };
                b.line_layer(
                    color,
                    &paths,
                    settings.clone(),
                    *kerf_mm,
                    tab_specs[i],
                    zs[i],
                );
            }
            Resolved::Vector(ResolvedLayer::Fill {
                settings,
                overscan,
                passes,
            }) => b.fill_layer(
                color,
                &layer.paths,
                &layer.meta,
                settings.clone(),
                *overscan,
                *passes,
                zs[i],
                layer.settings.angle_increment,
            )?,
            Resolved::Image(r) => {
                for img in &layer.images {
                    b.image(color, img, r, zs[i], layer.settings.angle_increment)?;
                }
            }
        }
    }

    if b.toolpath.blocks.is_empty() {
        return Err(nothing_to_produce(&b.warnings));
    }

    // Con rotativo no hay límite de Y, pero el diseño no debe dar más de una vuelta y la Y de la
    // controladora no debe pasar del área si los límites por software están activos.
    if device.rotary.enabled
        && let Some(area) = b
            .layers
            .iter()
            .filter_map(|l| l.bounds)
            .reduce(tlaku_doc::union)
    {
        if let Some(circumference) = rotary::circumference_mm(device) {
            let height = area.max.y - area.min.y;
            if height > circumference + 1e-6 {
                b.warnings.push(format!(
                    "el diseño mide {height:.2} mm en Y y la circunferencia de la pieza es \
                     {circumference:.2} mm (diámetro {} mm): los extremos se encimarán",
                    device.rotary.object_diameter
                ));
            }
        }
        if let Some(w) = rotary::soft_limit_warning(device, area) {
            b.warnings.push(w);
        }
    }
    let notes = dialect_warnings(&b.toolpath, &b.layers, device);
    b.warnings.extend(notes);

    let estimate = tlaku_sim::estimate_from(placement.start, &b.toolpath, &profile);
    for (layer, e) in b.layers.iter_mut().zip(&estimate.blocks) {
        layer.seconds = e.seconds;
    }
    let move_times = tlaku_sim::move_times(placement.start, &b.toolpath, &profile);
    let preview = preview(
        &b.toolpath,
        &b.layers,
        estimate.total_s,
        move_times,
        order_note_text,
        b.warnings,
    );
    Ok(ComputedJob {
        toolpath: b.toolpath,
        layers: b.layers,
        preview,
    })
}

/// Opciones del planificador que salen de los ajustes de optimización del trabajo.
/// `hide_backlash` fija el sentido de giro de las cerradas y deja las abiertas como vienen.
fn plan_options(o: &Optimization) -> PlanOptions {
    PlanOptions {
        order_by_layer: o.by_layer,
        order_by_group: o.by_group,
        order_by_priority: o.by_priority,
        inner_first: o.inner_first,
        reduce_travel: o.reduce_travel,
        choose_start_point: o.best_start,
        choose_direction: o.choose_direction && !o.hide_backlash,
        closed_winding: o.hide_backlash.then_some(Winding::Ccw),
        ..PlanOptions::default()
    }
}

/// Frase para la vista previa con las opciones de optimización activas (se da solo cuando no
/// son las de por defecto). `merged` lista las capas que `by_layer = false` juntó.
fn optimization_note(o: &Optimization, plan: &PlanOptions, merged: &[Vec<Rgb>]) -> String {
    let mut parts = tlaku_plan::describe(plan);
    if !o.by_layer {
        parts.push(if merged.is_empty() {
            "sin orden por capa: no hay capas de corte consecutivas con los mismos ajustes que \
             mezclar (las que tienen ajustes distintos no se mezclan)"
                .to_string()
        } else {
            let list: Vec<String> = merged
                .iter()
                .map(|colors| {
                    colors
                        .iter()
                        .map(Rgb::to_string)
                        .collect::<Vec<_>>()
                        .join(" + ")
                })
                .collect();
            format!(
                "sin orden por capa: se mezclaron las capas con los mismos ajustes ({})",
                list.join("; ")
            )
        });
    }
    if o.remove_overlaps {
        parts.push(format!(
            "tramos repetidos quitados dentro de cada capa de corte (tolerancia {} mm)",
            o.overlap_tolerance
        ));
    }
    if o.hide_backlash {
        parts.push("hide_backlash: recorrido en un solo sentido, más lento".to_string());
    }
    format!("optimización: {}", parts.join("; "))
}

fn nothing_to_produce(warnings: &[String]) -> String {
    format!(
        "no hay nada que producir: {}",
        if warnings.is_empty() {
            "el diseño está vacío (o ningún objeto tiene salida)".to_string()
        } else {
            warnings.join("; ")
        }
    )
}

/// Calcula el trabajo y verifica que quepa en la cama de `device`.
pub fn compute(
    doc: &Document,
    device: &DeviceProfile,
    options: &JobOptions,
) -> Result<ComputedJob, String> {
    compute_at(doc, device, options, None)
}

/// Como [`compute`], con la posición actual de la cabeza (para `start_from = current_position`).
pub fn compute_at(
    doc: &Document,
    device: &DeviceProfile,
    options: &JobOptions,
    current: Option<Point>,
) -> Result<ComputedJob, String> {
    let job = build(doc, device, options, current)?;
    check_device_bounds(&job, device)?;
    Ok(job)
}
