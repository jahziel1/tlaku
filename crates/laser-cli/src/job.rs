//! Del archivo de diseño a un `Toolpath`: importar, resolver capas, planificar, rellenar,
//! rasterizar, estimar el tiempo y verificar los límites de la cama.

use crate::options::{
    Defaults, ImageParams, LayerOrder, LayerOverride, Overscan, ResolvedLayer, resolve_image,
    resolve_layer,
};
use laser_core::toolpath::{Move, Toolpath, ToolpathBlock};
use laser_core::{Bounds, Job, Layer, Point, Polyline, Rgb};
use laser_plan::{Plan, PlanItem, PlanOptions};
use laser_raster::{Luma, ProcessOptions, Sampling};
use laser_sim::MachineProfile;
use std::f64::consts::FRAC_PI_2;
use std::path::{Path, PathBuf};

/// Margen (mm) para no rechazar un diseño que toca el borde por error de redondeo.
const BED_EPS_MM: f64 = 1e-6;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputKind {
    Svg,
    Dxf,
    Image,
}

pub fn input_kind(path: &Path) -> Result<InputKind, String> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    match ext.as_str() {
        "svg" => Ok(InputKind::Svg),
        "dxf" => Ok(InputKind::Dxf),
        "png" | "jpg" | "jpeg" | "bmp" => Ok(InputKind::Image),
        _ => Err(format!(
            "{}: formato de entrada no soportado; usa .svg, .dxf, .png, .jpg, .jpeg o .bmp",
            path.display()
        )),
    }
}

/// Todo lo que el usuario pidió sobre el diseño (sin lo propio de cada subcomando).
#[derive(Debug, Clone)]
pub struct DesignRequest {
    pub input: PathBuf,
    pub defaults: Defaults,
    pub overrides: Vec<(Rgb, LayerOverride)>,
    pub image: ImageParams,
    pub tolerance_mm: f64,
    pub flip_y: bool,
    pub profile: MachineProfile,
    pub layer_order: LayerOrder,
}

/// Función de una capa en el orden automático. El orden de las variantes es el de ejecución.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LayerRole {
    /// Relleno o imagen.
    Engrave,
    /// Línea que no corta la pieza (marcado, trazos).
    Line,
    /// Línea que separa la pieza del material: debe ir al final.
    Cut,
}

/// Potencia (%) desde la que una línea con varias pasadas se considera corte.
const CUT_POWER_PCT: f64 = 80.0;

const KERF_REASON: &str = "kerf";
const POWER_REASON: &str = "potencia ≥ 80 % con varias pasadas";

/// Clasifica una capa ya resuelta; para los cortes devuelve además el motivo.
fn classify(layer: &ResolvedLayer) -> (LayerRole, Option<&'static str>) {
    match layer {
        ResolvedLayer::Fill { .. } => (LayerRole::Engrave, None),
        ResolvedLayer::Line { settings, kerf_mm } => {
            if *kerf_mm != 0.0 {
                (LayerRole::Cut, Some(KERF_REASON))
            } else if settings.power_pct >= CUT_POWER_PCT && settings.passes > 1 {
                (LayerRole::Cut, Some(POWER_REASON))
            } else {
                (LayerRole::Line, None)
            }
        }
    }
}

/// Índices de las capas en el orden de ejecución. Las capas con posición explícita
/// (`forced`, desde 1) se colocan después de ordenar las demás; una posición mayor que el
/// número de capas equivale a "al final". El orden automático es estable: dentro de cada
/// grupo se conserva el del archivo.
pub fn layer_sequence(
    roles: &[LayerRole],
    forced: &[Option<usize>],
    mode: LayerOrder,
) -> Vec<usize> {
    let mut seq: Vec<usize> = (0..roles.len()).filter(|&i| forced[i].is_none()).collect();
    if mode == LayerOrder::Auto {
        seq.sort_by_key(|&i| roles[i]);
    }
    let mut pinned: Vec<usize> = (0..roles.len()).filter(|&i| forced[i].is_some()).collect();
    pinned.sort_by_key(|&i| forced[i]);
    for i in pinned {
        let at = forced[i].map_or(0, |n| n - 1).min(seq.len());
        seq.insert(at, i);
    }
    seq
}

/// Frase que explica el orden elegido, para mostrarla en pantalla. `roles` y `cuts` son solo
/// de las capas sin posición explícita (`forced` lista las que sí la tienen).
fn order_note(
    mode: LayerOrder,
    roles: &[LayerRole],
    cuts: &[(Rgb, &'static str)],
    forced: &[(Rgb, usize)],
) -> String {
    let mut note = match mode {
        LayerOrder::File => "orden del archivo (--layer-order file)".to_string(),
        LayerOrder::Auto => {
            let names = [
                (LayerRole::Engrave, "grabados"),
                (LayerRole::Line, "líneas"),
                (LayerRole::Cut, "cortes"),
            ];
            let present: Vec<&str> = names
                .iter()
                .filter(|(r, _)| roles.contains(r))
                .map(|(_, n)| *n)
                .collect();
            if roles.is_empty() {
                "orden fijado con order=".to_string()
            } else if present.len() < 2 {
                let why = if roles.len() > 1 {
                    " (todas las capas son del mismo tipo)"
                } else {
                    ""
                };
                format!("orden automático: se conserva el orden del archivo{why}")
            } else {
                let last = present.len() - 1;
                let parts: Vec<String> = present
                    .iter()
                    .enumerate()
                    .map(|(i, n)| match i {
                        0 => format!("{n} primero"),
                        i if i == last => format!("{n} al final"),
                        _ => format!("{n} después"),
                    })
                    .collect();
                format!("orden automático: {}", parts.join(", "))
            }
        }
    };
    if mode == LayerOrder::Auto && !cuts.is_empty() {
        let list: Vec<String> = cuts
            .iter()
            .map(|(c, why)| format!("{c} por {why}"))
            .collect();
        note.push_str(&format!(" (corte: {})", list.join("; ")));
    }
    if !forced.is_empty() {
        let list: Vec<String> = forced
            .iter()
            .map(|(c, n)| format!("{c} en la posición {n}"))
            .collect();
        let sep = if roles.is_empty() {
            ": "
        } else {
            "; order= explícito: "
        };
        note.push_str(&format!("{sep}{}", list.join(", ")));
    }
    note
}

/// Resumen y detalle de una capa del trabajo.
#[derive(Debug, Clone)]
pub struct LayerReport {
    /// `#rrggbb`, o `imagen`.
    pub name: String,
    /// `line`, `fill` o `image`.
    pub mode: &'static str,
    pub count: usize,
    /// Qué se cuenta en `count`: `trayectorias` o `líneas`.
    pub unit: &'static str,
    pub speed_mm_min: f64,
    pub power: String,
    pub passes: u32,
    pub overscan_mm: Option<f64>,
    /// Plan de corte (solo capas line).
    pub plan: Option<Plan>,
    pub seconds: f64,
    /// Caja de todos los movimientos, incluido el overscan.
    pub bounds: Option<Bounds>,
}

pub struct Built {
    pub toolpath: Toolpath,
    /// Una entrada por bloque del `toolpath`, en el mismo orden.
    pub layers: Vec<LayerReport>,
    pub warnings: Vec<String>,
    pub total_seconds: f64,
    /// Por qué las capas quedaron en este orden (`None` si solo hay una).
    pub order_note: Option<String>,
}

impl Built {
    /// Caja envolvente de todos los movimientos, incluido el overscan.
    pub fn bounds(&self) -> Option<Bounds> {
        self.layers.iter().filter_map(|l| l.bounds).reduce(union)
    }
}

fn union(a: Bounds, b: Bounds) -> Bounds {
    Bounds {
        min: Point::new(a.min.x.min(b.min.x), a.min.y.min(b.min.y)),
        max: Point::new(a.max.x.max(b.max.x), a.max.y.max(b.max.y)),
    }
}

pub fn block_bounds(block: &ToolpathBlock) -> Option<Bounds> {
    let mut it = block.moves.iter().map(Move::to);
    let first = it.next()?;
    let mut b = Bounds {
        min: first,
        max: first,
    };
    for p in it {
        b.min.x = b.min.x.min(p.x);
        b.min.y = b.min.y.min(p.y);
        b.max.x = b.max.x.max(p.x);
        b.max.y = b.max.y.max(p.y);
    }
    Some(b)
}

fn read_file(path: &Path) -> Result<Vec<u8>, String> {
    std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))
}

/// Importa un SVG o DXF y devuelve el trabajo (una capa por color) y las advertencias.
fn import_vector(req: &DesignRequest, kind: InputKind) -> Result<(Job, Vec<String>), String> {
    let data = read_file(&req.input)?;
    let (job, warnings) = match kind {
        InputKind::Svg => {
            let doc = laser_svg::import_svg(&data, req.tolerance_mm)
                .map_err(|e| format!("{}: SVG inválido: {e}", req.input.display()))?;
            let warnings = doc.warnings.clone();
            (
                doc.into_job(|_| laser_core::CutSettings::default(), req.flip_y),
                warnings,
            )
        }
        InputKind::Dxf => {
            let doc = laser_dxf::import_dxf(&data, req.tolerance_mm)
                .map_err(|e| format!("{}: {e}", req.input.display()))?;
            let warnings = doc.warnings.clone();
            (
                doc.into_job(|_| laser_core::CutSettings::default()),
                warnings,
            )
        }
        InputKind::Image => unreachable!("las imágenes no pasan por aquí"),
    };
    if job.layers.is_empty() {
        return Err(format!(
            "{}: el archivo no contiene trayectorias",
            req.input.display()
        ));
    }
    Ok((job, warnings))
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

struct Builder<'a> {
    req: &'a DesignRequest,
    toolpath: Toolpath,
    layers: Vec<LayerReport>,
    warnings: Vec<String>,
    head: Point,
    order_note: Option<String>,
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

    fn line_layer(&mut self, layer: &Layer, settings: laser_core::CutSettings, kerf_mm: f64) {
        let name = layer.color.to_string();
        let mut paths = layer.paths.clone();
        if kerf_mm != 0.0 {
            let open = paths.iter().filter(|p| !p.closed).count();
            if open > 0 {
                self.warnings.push(format!(
                    "capa {name}: {open} trayectoria(s) abierta(s) no se compensan por kerf"
                ));
            }
            paths = laser_geom::kerf_compensate(&paths, kerf_mm);
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
        let plan = laser_plan::plan(
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
                color: layer.color,
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
        layer: &Layer,
        mut settings: laser_fill::FillSettings,
        overscan: Overscan,
        passes: u32,
    ) -> Result<(), String> {
        let name = layer.color.to_string();
        if overscan == Overscan::Auto {
            let mut mm = laser_sim::overscan_mm(
                settings.speed_mm_min,
                settings.angle_deg.to_radians(),
                &self.req.profile,
            );
            if settings.crosshatch {
                let perp = settings.angle_deg.to_radians() + FRAC_PI_2;
                mm = mm.max(laser_sim::overscan_mm(
                    settings.speed_mm_min,
                    perp,
                    &self.req.profile,
                ));
            }
            settings.overscan_mm = mm;
        }
        let result =
            laser_fill::fill(&layer.paths, &settings).map_err(|e| format!("capa {name}: {e}"))?;
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

    fn image_layer(&mut self) -> Result<(), String> {
        let req = self.req;
        let img = &req.image;
        let (Some((w, h)), Some((x, y))) = (img.size, img.pos) else {
            return Err(
                "las imágenes requieren --image-size ANCHOxALTO y --image-pos X,Y (en mm; la posición es la esquina inferior izquierda)"
                    .into(),
            );
        };
        let r = resolve_image(&req.defaults, img)?;
        let mut raster = r.raster.clone();
        if r.overscan == Overscan::Auto {
            raster.overscan_mm = laser_sim::overscan_mm(raster.speed_mm_min, 0.0, &req.profile);
        }
        let gray = laser_raster::gray::load(&req.input, Luma::default())
            .map_err(|e| format!("{}: {e}", req.input.display()))?;
        let opts = ProcessOptions {
            adjust: r.adjust,
            sampling: Sampling::Interval(r.interval_mm),
            mode: r.mode,
            ..Default::default()
        };
        let bitmap =
            laser_raster::process(&gray, w, h, &opts).map_err(|e| format!("imagen: {e}"))?;
        let mut block = laser_raster::raster_to_block(&bitmap, Point::new(x, y), &raster);
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

/// Importa el diseño y arma el `Toolpath` capa por capa, en el orden pedido
/// (`--layer-order`, `order=N`).
pub fn build(req: &DesignRequest) -> Result<Built, String> {
    let kind = input_kind(&req.input)?;
    for (i, (c, _)) in req.overrides.iter().enumerate() {
        if req.overrides[..i].iter().any(|(d, _)| d == c) {
            return Err(format!("la capa {c} aparece más de una vez en --layer"));
        }
    }
    for (i, (c, o)) in req.overrides.iter().enumerate() {
        let Some(n) = o.order else { continue };
        if let Some((d, _)) = req.overrides[..i].iter().find(|(_, p)| p.order == Some(n)) {
            return Err(format!(
                "las capas {d} y {c} piden la misma posición: order={n}"
            ));
        }
    }
    let mut b = Builder {
        req,
        toolpath: Toolpath::default(),
        layers: Vec::new(),
        warnings: Vec::new(),
        head: Point::new(0.0, 0.0),
        order_note: None,
    };
    if kind != InputKind::Image && !req.image.given.is_empty() {
        b.warnings.push(format!(
            "se ignoraron las opciones de imagen ({}) porque la entrada no es una imagen",
            req.image.given.join(", ")
        ));
    }

    if kind == InputKind::Image {
        if !req.overrides.is_empty() {
            b.warnings
                .push("se ignoró --layer: una imagen no tiene capas por color".into());
        }
        b.image_layer()?;
    } else {
        let (job, import_warnings) = import_vector(req, kind)?;
        b.warnings.extend(import_warnings);
        let known: Vec<Rgb> = job.layers.iter().map(|l| l.color).collect();
        // Se validan todas las capas antes de procesar ninguna.
        let resolved = job
            .layers
            .iter()
            .map(|l| {
                let over = req.overrides.iter().find(|(c, _)| *c == l.color);
                resolve_layer(l.color, &req.defaults, over.map(|(_, o)| o))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let classes: Vec<_> = resolved.iter().map(classify).collect();
        let roles: Vec<LayerRole> = classes.iter().map(|(r, _)| *r).collect();
        let forced: Vec<Option<usize>> = job
            .layers
            .iter()
            .map(|l| {
                req.overrides
                    .iter()
                    .find(|(c, _)| *c == l.color)
                    .and_then(|(_, o)| o.order)
            })
            .collect();
        let sequence = layer_sequence(&roles, &forced, req.layer_order);
        let n = job.layers.len();
        let mut forced_list = Vec::new();
        for (layer, pos) in job.layers.iter().zip(&forced) {
            if let Some(pos) = *pos {
                forced_list.push((layer.color, pos));
                if pos > n {
                    b.warnings.push(format!(
                        "capa {}: order={pos} excede el número de capas ({n}); queda al final",
                        layer.color
                    ));
                }
            }
        }
        forced_list.sort_by_key(|(_, p)| *p);
        if n > 1 {
            let free = |i: usize| forced[i].is_none();
            let free_roles: Vec<LayerRole> =
                (0..n).filter(|&i| free(i)).map(|i| roles[i]).collect();
            let free_cuts: Vec<(Rgb, &'static str)> = sequence
                .iter()
                .filter(|&&i| free(i))
                .filter_map(|&i| classes[i].1.map(|why| (job.layers[i].color, why)))
                .collect();
            b.order_note = Some(order_note(
                req.layer_order,
                &free_roles,
                &free_cuts,
                &forced_list,
            ));
        }
        for i in sequence {
            let layer = &job.layers[i];
            match resolved[i].clone() {
                ResolvedLayer::Line { settings, kerf_mm } => {
                    b.line_layer(layer, settings, kerf_mm);
                }
                ResolvedLayer::Fill {
                    settings,
                    overscan,
                    passes,
                } => b.fill_layer(layer, settings, overscan, passes)?,
            }
        }
        for (c, _) in &req.overrides {
            if !known.contains(c) {
                let list: Vec<String> = known.iter().map(Rgb::to_string).collect();
                b.warnings.push(format!(
                    "la capa {c} de --layer no existe en el diseño (capas: {})",
                    list.join(", ")
                ));
            }
        }
    }

    if b.toolpath.blocks.is_empty() {
        return Err(format!(
            "no hay nada que producir con {}: {}",
            req.input.display(),
            if b.warnings.is_empty() {
                "el diseño está vacío".to_string()
            } else {
                b.warnings.join("; ")
            }
        ));
    }

    let estimate = laser_sim::estimate(&b.toolpath, &req.profile);
    for (layer, e) in b.layers.iter_mut().zip(&estimate.blocks) {
        layer.seconds = e.seconds;
    }
    Ok(Built {
        toolpath: b.toolpath,
        layers: b.layers,
        warnings: b.warnings,
        total_seconds: estimate.total_s,
        order_note: b.order_note,
    })
}

/// Verifica que TODOS los movimientos (incluido el overscan) caigan dentro de la cama
/// `ancho x alto`, con el origen en la esquina inferior izquierda.
pub fn check_bounds(built: &Built, bed: (f64, f64)) -> Result<(), String> {
    let mut problems = Vec::new();
    for (layer, block) in built.layers.iter().zip(&built.toolpath.blocks) {
        let bad_point = block
            .moves
            .iter()
            .map(Move::to)
            .any(|p| !(p.x.is_finite() && p.y.is_finite()));
        if bad_point {
            problems.push(format!(
                "la capa {} tiene coordenadas que no son números",
                layer.name
            ));
            continue;
        }
        let Some(b) = layer.bounds else { continue };
        if b.min.x < -BED_EPS_MM
            || b.min.y < -BED_EPS_MM
            || b.max.x > bed.0 + BED_EPS_MM
            || b.max.y > bed.1 + BED_EPS_MM
        {
            let os = match layer.overscan_mm {
                Some(mm) if mm > 0.0 => format!(" (incluye {mm:.2} mm de overscan)"),
                _ => String::new(),
            };
            problems.push(format!(
                "la capa {} ({}) se sale del área de trabajo de {}x{} mm: ocupa X de {:.2} a {:.2} mm, \
                 Y de {:.2} a {:.2} mm{os}",
                layer.name, layer.mode, bed.0, bed.1, b.min.x, b.max.x, b.min.y, b.max.y
            ));
        }
    }
    if problems.is_empty() {
        Ok(())
    } else {
        problems.push(
            "mueve el diseño dentro del área (o ajusta --bed si tu máquina es más grande)".into(),
        );
        Err(problems.join("\n"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use laser_core::toolpath::PowerMode;

    #[test]
    fn input_kinds() {
        assert_eq!(input_kind(Path::new("a.SVG")), Ok(InputKind::Svg));
        assert_eq!(input_kind(Path::new("a.dxf")), Ok(InputKind::Dxf));
        assert_eq!(input_kind(Path::new("a.JPEG")), Ok(InputKind::Image));
        assert!(
            input_kind(Path::new("a.pdf"))
                .unwrap_err()
                .contains("no soportado")
        );
        assert!(input_kind(Path::new("sin_extension")).is_err());
    }

    use LayerRole::{Cut, Engrave, Line};

    #[test]
    fn auto_order_is_stable_and_cuts_go_last() {
        let roles = [Cut, Line, Engrave, Line, Engrave, Cut];
        let none = [None; 6];
        assert_eq!(
            layer_sequence(&roles, &none, LayerOrder::Auto),
            [2, 4, 1, 3, 0, 5]
        );
        assert_eq!(
            layer_sequence(&roles, &none, LayerOrder::File),
            [0, 1, 2, 3, 4, 5]
        );
    }

    #[test]
    fn explicit_order_wins_over_auto_and_clamps() {
        let roles = [Cut, Engrave, Line];
        // La capa de corte pide ir primera.
        let forced = [Some(1), None, None];
        assert_eq!(layer_sequence(&roles, &forced, LayerOrder::Auto), [0, 1, 2]);
        // El grabado pide la posición 3 (y la línea la 1): se respetan ambas.
        let forced = [None, Some(3), Some(1)];
        assert_eq!(layer_sequence(&roles, &forced, LayerOrder::Auto), [2, 0, 1]);
        // Fuera de rango: al final.
        let forced = [None, Some(9), None];
        assert_eq!(layer_sequence(&roles, &forced, LayerOrder::File), [0, 2, 1]);
    }

    #[test]
    fn classification_rules() {
        use laser_core::CutSettings;
        let line = |power_pct: f64, passes: u32, kerf_mm: f64| ResolvedLayer::Line {
            settings: CutSettings {
                power_pct,
                passes,
                ..Default::default()
            },
            kerf_mm,
        };
        assert_eq!(classify(&line(100.0, 1, 0.0)).0, Line);
        assert_eq!(classify(&line(50.0, 5, 0.0)).0, Line);
        assert_eq!(classify(&line(80.0, 2, 0.0)), (Cut, Some(POWER_REASON)));
        assert_eq!(classify(&line(10.0, 1, 0.1)), (Cut, Some(KERF_REASON)));
        assert_eq!(classify(&line(10.0, 1, -0.1)).0, Cut);
    }

    #[test]
    fn order_note_explains_the_groups() {
        let red = Rgb(255, 0, 0);
        let note = order_note(
            LayerOrder::Auto,
            &[Engrave, Cut],
            &[(red, KERF_REASON)],
            &[],
        );
        assert_eq!(
            note,
            "orden automático: grabados primero, cortes al final (corte: #ff0000 por kerf)"
        );
        let note = order_note(LayerOrder::Auto, &[Engrave, Line, Cut], &[], &[]);
        assert_eq!(
            note,
            "orden automático: grabados primero, líneas después, cortes al final"
        );
        let note = order_note(LayerOrder::Auto, &[Line, Line], &[], &[]);
        assert!(note.contains("mismo tipo"), "{note}");
        let note = order_note(LayerOrder::Auto, &[], &[], &[(red, 1)]);
        assert!(note.starts_with("orden fijado con order="), "{note}");
        let note = order_note(LayerOrder::File, &[Cut, Engrave], &[], &[(red, 2)]);
        assert!(
            note.starts_with("orden del archivo") && note.contains("#ff0000 en la posición 2"),
            "{note}"
        );
    }

    #[test]
    fn bounds_check_covers_every_move() {
        let block = ToolpathBlock {
            label: "x".into(),
            speed_mm_min: 1000.0,
            air_assist: false,
            power_mode: PowerMode::Dynamic,
            moves: vec![
                Move::Travel {
                    to: Point::new(-0.5, 5.0),
                },
                Move::Cut {
                    to: Point::new(10.0, 5.0),
                    power_pct: 0.0,
                },
            ],
        };
        let layer = LayerReport {
            name: "#ff0000".into(),
            mode: "fill",
            count: 1,
            unit: "líneas",
            speed_mm_min: 1000.0,
            power: "10%".into(),
            passes: 1,
            overscan_mm: Some(2.0),
            plan: None,
            seconds: 0.0,
            bounds: block_bounds(&block),
        };
        let built = Built {
            toolpath: Toolpath {
                blocks: vec![block],
            },
            layers: vec![layer],
            warnings: vec![],
            total_seconds: 0.0,
            order_note: None,
        };
        let e = check_bounds(&built, (100.0, 100.0)).unwrap_err();
        assert!(e.contains("#ff0000") && e.contains("overscan"), "{e}");
    }
}
