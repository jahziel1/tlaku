//! Del documento a un `ComputedJob`: `build` recorre las capas en orden y delega en `Builder`.

use crate::galvo;
use crate::gcode::{dialect_warnings, machine_to_doc};
use crate::job::{ComputedJob, LayerReport, check_device_bounds};
use crate::layers::{
    LayerRole, ResolvedImage, ResolvedLayer, SeqUnit, classify, effective_roles, merge_steps,
    order_note, resolve_image, resolve_layer, unit_sequence,
};
use crate::place::{LayerWork, PathMeta, place};
use crate::preview::preview;
use crate::rotary;
use crate::slice3d;
use crate::stroke::{self, StrokeSpec};
use crate::tabs::{self};
use crate::zaxis::{self};
use std::collections::HashMap;
use tlaku_core::toolpath::Toolpath;
use tlaku_core::{Point, Polyline, Rgb};
use tlaku_doc::{
    DeviceProfile, Document, JobOptions, LayerMode, LayerOrder, LayerSettings, Optimization,
    ScanOffset,
};
use tlaku_i18n::tr;
use tlaku_plan::PlanOptions;
use tlaku_sim::MachineProfile;

mod builder;
mod fill;
mod helpers;
mod image;
mod line;
mod plan;

use helpers::*;
use plan::*;

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
    /// Desplazamiento de U de la capa en curso (`LayerSettings::u_offset`).
    u_offset_mm: f64,
    /// Parámetros de galvo de la capa en curso (solo máquinas galvo).
    galvo: Option<galvo::LayerCtx>,
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
    let to_doc = |p: Point| machine_to_doc(Point::new(p.x, p.y / y_scale), device);
    let options = &JobOptions {
        user_origin: options
            .user_origin
            .map(|[x, y]| to_doc(Point::new(x, y)))
            .map(|p| [p.x, p.y]),
        ..options.clone()
    };
    let mut placement = place(doc, options, current.map(to_doc), &mut warnings)?;
    // Print & Cut: la transformación del registro se aplica al trabajo ya colocado.
    crate::registration::apply(&mut placement, options.transform.as_ref())?;
    let work: &[LayerWork] = &placement.layers;
    if work.is_empty() {
        return Err(nothing_to_produce(doc, &warnings));
    }
    let optimization = &options.optimization;
    if !(optimization.overlap_tolerance.is_finite() && optimization.overlap_tolerance >= 0.0) {
        return Err(tr!(
            "pipeline.build_mod.bad_overlap_tolerance",
            value = optimization.overlap_tolerance
        ));
    }
    let plan_options = plan_options(optimization);
    if optimization.hide_backlash {
        warnings.push(tr!("pipeline.build_mod.hide_backlash"));
    }

    // Se validan todas las capas antes de procesar ninguna.
    let mut resolved = work
        .iter()
        .map(|l| match l.settings.mode {
            LayerMode::Image => resolve_image(l.settings).map(Resolved::Image),
            _ => resolve_layer(l.settings).map(Resolved::Vector),
        })
        .collect::<Result<Vec<_>, _>>()?;
    // Galvo: rellenos e imágenes sin overscan (los espejos casi no tienen inercia), salvo que el
    // perfil pida uno (`overscan_mm`): entonces la capa que no fija el suyo usa el del perfil.
    let galvo_job = galvo::JobCtx::new(device)?;
    if let Some(gj) = &galvo_job {
        let os = gj.overscan_mm();
        for r in &mut resolved {
            match r {
                Resolved::Vector(v) => galvo::overscan_layer(v, os),
                Resolved::Image(i) => galvo::overscan_image(i, os),
            }
        }
    }
    // Puentes (solo capas line) y Z de cada capa.
    let mut tab_specs = Vec::with_capacity(work.len());
    let mut stroke_specs = Vec::with_capacity(work.len());
    let mut zs = Vec::with_capacity(work.len());
    for l in work {
        if !l.settings.angle_increment.is_finite() {
            return Err(tr!(
                "pipeline.build_mod.bad_angle_increment",
                layer = l.color,
                value = l.settings.angle_increment
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
                return Err(tr!("pipeline.build_mod.bad_position", layer = l.color));
            }
            Some(n) => Some(n as usize),
            None => None,
        };
        if let Some(n) = pos
            && let Some(d) = work[..i]
                .iter()
                .find(|p| p.sub.is_none() && p.settings.order == l.settings.order)
        {
            return Err(tr!(
                "pipeline.build_mod.same_position",
                a = d.color,
                b = l.color,
                n = n
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
                warnings.push(tr!(
                    "pipeline.build_mod.position_too_high",
                    layer = l.color,
                    pos = pos,
                    n = n
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
                note.push_str("; ");
                note.push_str(&tr!("pipeline.build_mod.sub_layers_order"));
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
        u_offset_mm: 0.0,
        galvo: None,
    };
    for step in &steps {
        let i = step[0];
        let layer = &work[i];
        let color = layer.color;
        b.sub = layer.position();
        b.laser2 = layer.settings.laser2;
        b.u_offset_mm = layer.settings.u_offset;
        b.galvo = galvo::layer_ctx(galvo_job.as_ref(), layer.settings, &mut b.warnings)?;
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
                        b.warnings
                            .push(tr!("pipeline.build_mod.overlaps_with_kerf", layer = color));
                    } else {
                        let d = tlaku_geom::dedupe_paths(&paths, optimization.overlap_tolerance);
                        if d.removed_length > 0.0 {
                            let cut = d.changed.iter().filter(|&&c| c).count();
                            b.warnings.push(tr!(
                                "pipeline.build_mod.overlaps_removed",
                                layer = color,
                                mm = format!("{:.2}", d.removed_length),
                                n = cut
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
                // Con el grabado 3D la imagen sale en varios cortes (uno solo si está apagado).
                let slices = slice3d::expand(r, &layer.settings.galvo.slice3d)?;
                for img in &layer.images {
                    let (first, warned) = (b.toolpath.blocks.len(), b.warnings.len());
                    for (k, slice) in slices.iter().enumerate() {
                        let z = slice3d::z_for(zs[i], k, slices.len());
                        let at = b.layers.len();
                        b.image(color, img, slice, z, layer.settings.angle_increment)?;
                        slice3d::name_slice(&mut b.layers[at..], k, slices.len());
                    }
                    slice3d::merge_blank_warnings(&mut b.warnings, warned, slices.len());
                    if slices.len() > 1 {
                        slice3d::fix_pauses(&mut b.toolpath.blocks[first..]);
                    }
                }
            }
        }
    }

    if b.toolpath.blocks.is_empty() {
        return Err(nothing_to_produce(doc, &b.warnings));
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
                b.warnings.push(tr!(
                    "pipeline.build_mod.rotary_overlap",
                    height = format!("{height:.2}"),
                    circumference = format!("{circumference:.2}"),
                    diameter = device.rotary.object_diameter
                ));
            }
        }
        if let Some(w) = rotary::soft_limit_warning(device, area) {
            b.warnings.push(w);
        }
    }
    let notes = dialect_warnings(&b.toolpath, &b.layers, device);
    b.warnings.extend(notes);
    b.warnings
        .extend(crate::layers::speed_warnings(work, device));
    if device.kind == tlaku_doc::DeviceKind::Ruida {
        b.warnings
            .extend(crate::ruida::layer_warnings(&b.toolpath, &b.layers, device));
    }
    if galvo_job.is_some() {
        let notes = galvo::field_warnings(&b.layers, device);
        b.warnings.extend(notes);
    }

    // Una sola planificación para el estimado y los tiempos por movimiento.
    let (mut estimate, mut move_times) =
        tlaku_sim::estimate_with_times(placement.start, &b.toolpath, &profile, true);
    if galvo_job.is_some() {
        // Repeticiones y pausas de cada bloque; los tiempos por movimiento se reescalan.
        let k = galvo::adjust_estimate(&mut estimate, &b.toolpath.blocks);
        move_times.iter_mut().for_each(|t| *t *= k);
    }
    for (layer, e) in b.layers.iter_mut().zip(&estimate.blocks) {
        layer.seconds = e.seconds;
    }
    // El G-code termina con `M5` y `G0 X0 Y0`: ese regreso también tarda. Solo suma al total; ni
    // las capas (`layer.seconds`) ni `move_times` lo incluyen, para que la reproducción de la
    // vista previa y el reparto por capa sigan siendo los del trabajo en sí.
    let return_s = crate::gcode::return_leg(&b.toolpath, device).map_or(0.0, |(from, to)| {
        tlaku_sim::rapid_leg_seconds(from, to, &profile)
    });
    estimate.total_s += return_s;
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
