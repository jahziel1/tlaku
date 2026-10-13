use crate::gcode::point_to_machine;
use crate::job::{ComputedJob, LayerReport, block_bounds, check_device_bounds};
use crate::layers::{
    LayerRole, ResolvedImage, ResolvedLayer, classify, layer_sequence, order_note, resolve_image,
    resolve_layer,
};
use crate::place::{LayerWork, PlacedImage, place};
use crate::preview::preview;
use crate::rotary;
use crate::scan_offset;
use crate::tabs::{self, TabSpec, TabStats};
use crate::zaxis::{self, LayerZ};
use std::f64::consts::FRAC_PI_2;
use std::path::Path;
use tlaku_core::toolpath::{Move, Toolpath, ToolpathBlock};
use tlaku_core::{Job, Layer, Point, Polyline, Rgb};
use tlaku_doc::{DeviceProfile, Document, JobOptions, LayerMode, Overscan, ScanOffset};
use tlaku_fill::FillPattern;
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

struct Builder<'a> {
    profile: &'a MachineProfile,
    toolpath: Toolpath,
    layers: Vec<LayerReport>,
    warnings: Vec<String>,
    head: Point,
    scan_offsets: &'a [ScanOffset],
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
        }
        // Los puentes van después del kerf (se parte el contorno ya compensado). Qué formas los
        // admiten se decide con todo el conjunto, porque "dentro de otra" depende de las demás.
        let eligible = tabs
            .as_ref()
            .map(|t| tlaku_geom::tab_eligible(&paths, t.skip_inner));
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
        // La pieza se planificó como forma cerrada; recién ahora se parte en tramos.
        let mut stats = TabStats::default();
        let mut once = Vec::new();
        for step in &plan.steps {
            match (&tabs, &eligible) {
                (Some(spec), Some(ok)) if ok[step.item] => {
                    let pieces = tabs::split(&step.path, spec, &mut stats);
                    tabs::push_shape(&mut once, &pieces, settings.power_pct);
                }
                _ => tabs::push_shape(
                    &mut once,
                    std::slice::from_ref(&step.path),
                    settings.power_pct,
                ),
            }
        }
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
        };
        self.push_passes(block, report, settings.passes, z, false);
    }

    fn fill_layer(
        &mut self,
        color: Rgb,
        paths: &[Polyline],
        mut settings: tlaku_fill::FillSettings,
        overscan: Overscan,
        passes: u32,
        z: LayerZ,
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
        // Solo el barrido tiene líneas de ida y vuelta; el relleno por contornos no.
        if settings.bidirectional && settings.pattern == FillPattern::Scan {
            let mut angles = vec![settings.angle_deg];
            if settings.crosshatch {
                angles.push(settings.angle_deg + 90.0);
            }
            scan_offset::apply(&mut block, self.scan_offsets, &angles);
        }
        let lines = travel_count(&block);
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
            tabs: 0,
            z_mm: None,
            pass: None,
        };
        self.push_passes(block, report, passes, z, true);
        Ok(())
    }

    fn image(
        &mut self,
        color: Rgb,
        img: &PlacedImage,
        r: &ResolvedImage,
        z: LayerZ,
    ) -> Result<(), String> {
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
        if raster.bidirectional {
            scan_offset::apply(&mut block, self.scan_offsets, &[raster.angle_deg]);
        }
        let lines = travel_count(&block);
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
            tabs: 0,
            z_mm: None,
            pass: None,
        };
        self.push_passes(block, report, r.passes, z, true);
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
    let mut zs = Vec::with_capacity(work.len());
    for l in work {
        tab_specs.push(match l.settings.mode {
            LayerMode::Line => tabs::resolve(l.settings)?,
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
        scan_offsets: &device.scan_offsets,
    };
    for i in sequence {
        let layer = &work[i];
        let color = layer.settings.color;
        match &resolved[i] {
            Resolved::Vector(ResolvedLayer::Line { settings, kerf_mm }) => {
                b.line_layer(
                    color,
                    &layer.paths,
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
                settings.clone(),
                *overscan,
                *passes,
                zs[i],
            )?,
            Resolved::Image(r) => {
                for img in &layer.images {
                    b.image(color, img, r, zs[i])?;
                }
            }
        }
    }

    if b.toolpath.blocks.is_empty() {
        return Err(nothing_to_produce(&b.warnings));
    }

    // Con rotativo no hay límite de Y, pero el diseño no debe dar más de una vuelta.
    if let Some(circumference) = rotary::circumference_mm(device)
        && let Some(area) = b
            .layers
            .iter()
            .filter_map(|l| l.bounds)
            .reduce(tlaku_doc::union)
    {
        let height = area.max.y - area.min.y;
        if height > circumference + 1e-6 {
            b.warnings.push(format!(
                "el diseño mide {height:.2} mm en Y y la circunferencia de la pieza es \
                 {circumference:.2} mm (diámetro {} mm): los extremos se encimarán",
                device.rotary.object_diameter
            ));
        }
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
    check_device_bounds(&job, device)?;
    Ok(job)
}
