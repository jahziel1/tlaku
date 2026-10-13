use crate::gcode::point_to_machine;
use crate::job::{ComputedJob, LayerReport, block_bounds, check_bounds};
use crate::layers::{
    LayerRole, ResolvedImage, ResolvedLayer, classify, layer_sequence, order_note, resolve_image,
    resolve_layer,
};
use crate::place::{LayerWork, PlacedImage, place};
use crate::preview::preview;
use std::f64::consts::FRAC_PI_2;
use std::path::Path;
use tlaku_core::toolpath::{Move, Toolpath, ToolpathBlock};
use tlaku_core::{Job, Layer, Point, Polyline, Rgb};
use tlaku_doc::{DeviceProfile, Document, JobOptions, LayerMode, Overscan};
use tlaku_plan::{PlanItem, PlanOptions};
use tlaku_raster::{GrayF32, Luma, ProcessOptions, Sampling};
use tlaku_sim::MachineProfile;

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
    Ok(MachineProfile {
        max_rate_mm_min: device.max_rate,
        accel_mm_s2: device.accel,
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
fn repeat_passes(block: &mut ToolpathBlock, passes: u32) {
    if passes > 1 {
        let once = block.moves.clone();
        for _ in 1..passes {
            block.moves.extend_from_slice(&once);
        }
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

struct Builder<'a> {
    profile: &'a MachineProfile,
    toolpath: Toolpath,
    layers: Vec<LayerReport>,
    warnings: Vec<String>,
    head: Point,
}

impl Builder<'_> {
    fn push(&mut self, block: ToolpathBlock, mut report: LayerReport) {
        report.bounds = block_bounds(&block);
        if let Some(last) = block.moves.last() {
            self.head = last.to();
        }
        self.toolpath.blocks.push(block);
        self.layers.push(report);
    }

    fn line_layer(
        &mut self,
        color: Rgb,
        paths: &[Polyline],
        settings: tlaku_core::CutSettings,
        kerf_mm: f64,
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
        }
        let items: Vec<PlanItem> = paths
            .into_iter()
            .map(|path| PlanItem {
                path,
                // Número de la capa en el orden de ejecución, el que muestra el resumen.
                layer_order: self.layers.len() + 1,
                group: None,
                priority: 0,
            })
            .collect();
        let plan = tlaku_plan::plan(
            &items,
            &PlanOptions {
                start: self.head,
                ..Default::default()
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
        let ordered = Job {
            layers: vec![Layer {
                color,
                settings: settings.clone(),
                paths: plan
                    .steps
                    .iter()
                    .map(|s| s.path.clone())
                    .collect::<Vec<Polyline>>(),
            }],
        };
        let block = Toolpath::from_line_job(&ordered).blocks.remove(0);
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
        };
        self.push(block, report);
    }

    fn fill_layer(
        &mut self,
        color: Rgb,
        paths: &[Polyline],
        mut settings: tlaku_fill::FillSettings,
        overscan: Overscan,
        passes: u32,
    ) -> Result<(), String> {
        let name = color.to_string();
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
        let result = tlaku_fill::fill(paths, &settings).map_err(|e| format!("capa {name}: {e}"))?;
        if !result.ignored_open.is_empty() {
            self.warnings.push(format!(
                "capa {name}: {} trayectoria(s) abierta(s) ignorada(s): el relleno solo usa trayectorias cerradas",
                result.ignored_open.len()
            ));
        }
        let mut block = result.block;
        if block.moves.is_empty() {
            self.warnings
                .push(format!("capa {name}: el relleno no produjo ninguna línea"));
            return Ok(());
        }
        let lines = travel_count(&block);
        repeat_passes(&mut block, passes);
        let report = LayerReport {
            name,
            color,
            mode: "fill",
            count: lines,
            unit: "líneas",
            speed_mm_min: settings.speed_mm_min,
            power: format!("{}%", settings.power_pct),
            passes,
            overscan_mm: Some(settings.overscan_mm),
            plan: None,
            seconds: 0.0,
            bounds: None,
        };
        self.push(block, report);
        Ok(())
    }

    fn image(&mut self, color: Rgb, img: &PlacedImage, r: &ResolvedImage) -> Result<(), String> {
        let mut raster = r.raster.clone();
        if r.overscan == Overscan::Auto {
            raster.overscan_mm = tlaku_sim::overscan_mm(raster.speed_mm_min, 0.0, self.profile);
        }
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
        let bitmap =
            tlaku_raster::process(&gray, w, h, &opts).map_err(|e| format!("imagen: {e}"))?;
        let mut block = tlaku_raster::raster_to_block(&bitmap, img.origin, &raster);
        if block.moves.is_empty() {
            self.warnings.push(
                "imagen: no hay ningún punto que grabar (la imagen resultó en blanco)".into(),
            );
            return Ok(());
        }
        let lines = travel_count(&block);
        repeat_passes(&mut block, r.passes);
        let report = LayerReport {
            name: "imagen".into(),
            color,
            mode: "image",
            count: lines,
            unit: "líneas",
            speed_mm_min: raster.speed_mm_min,
            power: format!("{}-{}%", raster.min_power_pct, raster.max_power_pct),
            passes: r.passes,
            overscan_mm: Some(raster.overscan_mm),
            plan: None,
            seconds: 0.0,
            bounds: None,
        };
        self.push(block, report);
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
/// GRBL), y aquí se pasan a las del documento según `device.origin`.
pub fn build(
    doc: &Document,
    device: &DeviceProfile,
    options: &JobOptions,
    current: Option<Point>,
) -> Result<ComputedJob, String> {
    let profile = machine_profile(device)?;
    let mut warnings = Vec::new();
    let to_doc = |p: Point| point_to_machine(p, device.bed, device.origin);
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

    // Se validan todas las capas antes de procesar ninguna.
    let resolved = work
        .iter()
        .map(|l| match l.settings.mode {
            LayerMode::Image => resolve_image(l.settings).map(Resolved::Image),
            _ => resolve_layer(l.settings).map(Resolved::Vector),
        })
        .collect::<Result<Vec<_>, _>>()?;
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
        let pos = match l.settings.order {
            Some(0) => {
                return Err(format!(
                    "capa {}: la posición debe ser un entero desde 1",
                    l.settings.color
                ));
            }
            Some(n) => Some(n as usize),
            None => None,
        };
        if let Some(n) = pos
            && let Some(d) = work[..i]
                .iter()
                .find(|p| p.settings.order == l.settings.order)
        {
            return Err(format!(
                "las capas {} y {} piden la misma posición: order={n}",
                d.settings.color, l.settings.color
            ));
        }
        forced.push(pos);
    }

    let sequence = layer_sequence(&roles, &forced, options.layer_order);
    let n = work.len();
    let mut forced_list = Vec::new();
    for (l, pos) in work.iter().zip(&forced) {
        if let Some(pos) = *pos {
            forced_list.push((l.settings.color, pos));
            if pos > n {
                warnings.push(format!(
                    "capa {}: order={pos} excede el número de capas ({n}); queda al final",
                    l.settings.color
                ));
            }
        }
    }
    forced_list.sort_by_key(|(_, p)| *p);
    let order_note_text = (n > 1).then(|| {
        let free = |i: usize| forced[i].is_none();
        let free_roles: Vec<LayerRole> = (0..n).filter(|&i| free(i)).map(|i| roles[i]).collect();
        let free_cuts: Vec<(Rgb, &'static str)> = sequence
            .iter()
            .filter(|&&i| free(i))
            .filter_map(|&i| classes[i].1.map(|why| (work[i].settings.color, why)))
            .collect();
        order_note(options.layer_order, &free_roles, &free_cuts, &forced_list)
    });

    let mut b = Builder {
        profile: &profile,
        toolpath: Toolpath::default(),
        layers: Vec::new(),
        warnings,
        head: placement.start,
    };
    for i in sequence {
        let layer = &work[i];
        let color = layer.settings.color;
        match &resolved[i] {
            Resolved::Vector(ResolvedLayer::Line { settings, kerf_mm }) => {
                b.line_layer(color, &layer.paths, settings.clone(), *kerf_mm);
            }
            Resolved::Vector(ResolvedLayer::Fill {
                settings,
                overscan,
                passes,
            }) => b.fill_layer(color, &layer.paths, settings.clone(), *overscan, *passes)?,
            Resolved::Image(r) => {
                for img in &layer.images {
                    b.image(color, img, r)?;
                }
            }
        }
    }

    if b.toolpath.blocks.is_empty() {
        return Err(nothing_to_produce(&b.warnings));
    }

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
    check_bounds(&job, device.bed)?;
    Ok(job)
}
