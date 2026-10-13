//! Importación de DXF: convierte las entidades geométricas en polilíneas en
//! milímetros (Y hacia arriba, igual que DXF), con el color ACI o true color
//! de cada entidad.
//!
//! Lo que no se puede importar nunca se pierde en silencio: cada omisión
//! genera una advertencia en [`DxfDocument::warnings`].

mod aci;
mod geom;

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::f64::consts::TAU;

use dxf::entities::{Entity, EntityType, Insert, LwPolyline, Polyline as DxfPolyline, Spline};
use dxf::enums::Units;
use dxf::{Block, Drawing};
use tlaku_core::{Bounds, CutSettings, Job, Layer, Point, Polyline, Rgb};

pub use aci::{DEFAULT_COLOR, aci_to_rgb};
use geom::{Affine, Nurbs, P2};

/// Profundidad máxima de bloques anidados (protege contra ciclos y bombas).
const MAX_DEPTH: usize = 32;
/// Máximo de copias de un arreglo de INSERT.
const MAX_ARRAY: i64 = 100_000;
/// Distancia (mm) bajo la cual dos puntos consecutivos se consideran el mismo.
const EPS_MM: f64 = 1e-6;

/// Resultado de la importación. Las coordenadas están en mm.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct DxfDocument {
    pub paths: Vec<(Rgb, Polyline)>,
    pub warnings: Vec<String>,
}

impl DxfDocument {
    /// Crea un trabajo con una capa por color, en el orden en que aparecen.
    pub fn into_job(self, settings_for: impl Fn(Rgb) -> CutSettings) -> Job {
        let mut job = Job::default();
        for (color, polyline) in self.paths {
            match job.layers.iter_mut().find(|l| l.color == color) {
                Some(layer) => layer.paths.push(polyline),
                None => job.layers.push(Layer {
                    color,
                    settings: settings_for(color),
                    paths: vec![polyline],
                }),
            }
        }
        job
    }

    /// Caja envolvente de todo lo importado, útil para pedir al usuario que
    /// confirme que el tamaño es razonable.
    pub fn bounds(&self) -> Option<Bounds> {
        let mut points = self.paths.iter().flat_map(|(_, p)| &p.points);
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
}

/// Milímetros por unidad de dibujo; `None` si el archivo no declara unidades.
fn unit_to_mm(units: Units) -> Option<f64> {
    Some(match units {
        Units::Unitless => return None,
        Units::Inches => 25.4,
        Units::Feet => 304.8,
        Units::Miles => 1_609_344.0,
        Units::Millimeters => 1.0,
        Units::Centimeters => 10.0,
        Units::Meters => 1_000.0,
        Units::Kilometers => 1_000_000.0,
        Units::Microinches => 25.4e-6,
        Units::Mils => 0.0254,
        Units::Yards => 914.4,
        Units::Angstroms => 1e-7,
        Units::Nanometers => 1e-6,
        Units::Microns => 1e-3,
        Units::Decimeters => 100.0,
        Units::Decameters => 10_000.0,
        Units::Hectometers => 100_000.0,
        Units::Gigameters => 1e12,
        Units::AstronomicalUnits => 1.495_978_707e14,
        Units::LightYears => 9.460_730_472_580_8e18,
        Units::Parsecs => 3.085_677_581_491_367e19,
        // Unidades topográficas de EE. UU.: 1 ft = 1200/3937 m.
        Units::USSurveyFeet => 304.800_609_601_219_2,
        Units::USSurveyInch => 25.400_050_800_101_6,
        Units::USSurveyYard => 914.401_828_803_657_6,
        Units::USSurveyMile => 1_609_347.218_694_437_4,
    })
}

/// `tolerance_mm` es el error máximo permitido al aproximar curvas con rectas.
pub fn import_dxf(bytes: &[u8], tolerance_mm: f64) -> Result<DxfDocument, String> {
    if !tolerance_mm.is_finite() || tolerance_mm <= 0.0 {
        return Err(format!(
            "tolerancia inválida: {tolerance_mm} (debe ser un número mayor que 0)"
        ));
    }
    if bytes.trim_ascii().is_empty() {
        return Err("el archivo DXF está vacío".to_string());
    }
    let drawing = Drawing::load(&mut &bytes[..]).map_err(|e| format!("DXF inválido: {e}"))?;

    let mut warnings = Vec::new();
    let unit = match unit_to_mm(drawing.header.default_drawing_units) {
        Some(scale) => scale,
        None => {
            warnings.push(
                "El archivo no declara unidades ($INSUNITS ausente o 0): se supone milímetros. \
                 Si el tamaño sale unas 25,4 veces menor de lo esperado, el diseño está en pulgadas."
                    .to_string(),
            );
            1.0
        }
    };

    let mut layers = HashMap::new();
    for l in drawing.layers() {
        let info = LayerInfo {
            color: l.color.index(),
            on: l.is_layer_on,
        };
        layers.insert(l.name.clone(), info);
        layers.insert(l.name.to_lowercase(), info);
    }
    let blocks = drawing
        .blocks()
        .map(|b| (b.name.to_lowercase(), b))
        .collect();

    let hatches = count_hatches(bytes);
    let mut importer = Importer {
        tol: tolerance_mm,
        layers,
        blocks,
        hatch_in_block: hatches.in_blocks,
        out: Vec::new(),
        stats: Stats::default(),
        stack: Vec::new(),
    };
    importer.stats.hatch += hatches.in_entities;

    let root = Ctx {
        matrix: Affine::scale(unit, unit),
        block_color: None,
        layer0: None,
        depth: 0,
    };
    for e in drawing.entities() {
        if e.common.is_in_paper_space {
            importer.stats.paper_space += 1;
            continue;
        }
        importer.visit(e, &root);
    }

    warnings.extend(importer.stats.into_warnings());
    if importer.out.is_empty() {
        warnings.push("El archivo no contiene geometría importable.".to_string());
    }
    Ok(DxfDocument {
        paths: importer.out,
        warnings,
    })
}

#[derive(Clone, Copy)]
struct LayerInfo {
    color: Option<u8>,
    on: bool,
}

/// Contexto heredado al entrar en un bloque.
struct Ctx<'a> {
    /// De coordenadas del bloque a mm (incluye la escala de unidades).
    matrix: Affine,
    /// Color que toman las entidades BYBLOCK.
    block_color: Option<Rgb>,
    /// Capa que heredan las entidades de la capa "0" dentro de un bloque.
    layer0: Option<&'a str>,
    depth: usize,
}

#[derive(Default)]
struct Stats {
    text: usize,
    dimensions: usize,
    hatch: usize,
    meshes: usize,
    paper_space: usize,
    hidden: usize,
    layer_off: usize,
    invalid: usize,
    spline_fit_only: usize,
    other: BTreeMap<&'static str, usize>,
    missing_blocks: BTreeSet<String>,
    cyclic_blocks: BTreeSet<String>,
    too_deep: bool,
    array_too_big: usize,
}

impl Stats {
    fn into_warnings(self) -> Vec<String> {
        let mut w = Vec::new();
        let mut skipped = |n: usize, what: &str| {
            if n > 0 {
                let (verb, noun) = if n == 1 {
                    ("Se omitió", "entidad")
                } else {
                    ("Se omitieron", "entidades")
                };
                w.push(format!("{verb} {n} {noun} {what}."));
            }
        };
        skipped(
            self.text,
            "de texto (TEXT/MTEXT); conviértelo a contornos antes de exportar",
        );
        skipped(self.dimensions, "DIMENSION (cotas)");
        skipped(self.hatch, "HATCH (rellenos)");
        skipped(self.meshes, "de malla (POLYLINE 3D de superficie)");
        skipped(self.paper_space, "del espacio papel");
        skipped(self.hidden, "invisibles");
        skipped(self.layer_off, "en capas apagadas");
        skipped(
            self.invalid,
            "con coordenadas no válidas o geometría degenerada",
        );
        if !self.other.is_empty() {
            let total: usize = self.other.values().sum();
            let detail: Vec<String> = self
                .other
                .iter()
                .map(|(k, n)| format!("{k}: {n}"))
                .collect();
            skipped(
                total,
                &format!("de tipos no soportados ({})", detail.join(", ")),
            );
        }
        if self.spline_fit_only > 0 {
            w.push(format!(
                "{} SPLINE sin puntos de control se aproximaron uniendo sus puntos de ajuste con rectas.",
                self.spline_fit_only
            ));
        }
        for name in &self.missing_blocks {
            w.push(format!(
                "El bloque '{name}' no existe: su INSERT se omitió."
            ));
        }
        for name in &self.cyclic_blocks {
            w.push(format!(
                "El bloque '{name}' se contiene a sí mismo: se cortó el ciclo."
            ));
        }
        if self.too_deep {
            w.push(format!(
                "Bloques anidados a más de {MAX_DEPTH} niveles: se omitió el resto."
            ));
        }
        if self.array_too_big > 0 {
            w.push(format!(
                "{} INSERT con arreglos de más de {MAX_ARRAY} copias se omitieron.",
                self.array_too_big
            ));
        }
        w
    }
}

struct HatchCount {
    in_entities: usize,
    in_blocks: HashMap<String, usize>,
}

/// La biblioteca `dxf` descarta en silencio las entidades que no conoce
/// (HATCH entre ellas), así que se cuentan leyendo el texto crudo: por
/// sección ENTITIES y por bloque, para sumar solo las que se dibujarían.
/// En DXF binario no se cuenta.
fn count_hatches(bytes: &[u8]) -> HatchCount {
    let mut result = HatchCount {
        in_entities: 0,
        in_blocks: HashMap::new(),
    };
    if bytes.starts_with(b"AutoCAD Binary DXF") {
        return result;
    }
    #[derive(PartialEq)]
    enum Section {
        Other,
        Entities,
        Blocks,
    }
    let mut section = Section::Other;
    let mut block: Option<String> = None;
    let mut expect_section_name = false;
    let mut expect_block_name = false;

    let mut lines = bytes.split(|&b| b == b'\n').map(|l| l.trim_ascii());
    while let (Some(code), Some(value)) = (lines.next(), lines.next()) {
        match code {
            b"0" => {
                expect_section_name = value == b"SECTION";
                expect_block_name = value == b"BLOCK";
                match value {
                    b"ENDBLK" => block = None,
                    b"HATCH" => match (&section, &block) {
                        (Section::Entities, _) => result.in_entities += 1,
                        (Section::Blocks, Some(name)) => {
                            *result.in_blocks.entry(name.clone()).or_default() += 1
                        }
                        _ => {}
                    },
                    _ => {}
                }
            }
            b"2" if expect_section_name => {
                expect_section_name = false;
                section = match value {
                    b"ENTITIES" => Section::Entities,
                    b"BLOCKS" => Section::Blocks,
                    _ => Section::Other,
                };
            }
            b"2" if expect_block_name => {
                expect_block_name = false;
                block = Some(String::from_utf8_lossy(value).to_lowercase());
            }
            _ => {}
        }
    }
    result
}

struct Importer<'a> {
    tol: f64,
    layers: HashMap<String, LayerInfo>,
    blocks: HashMap<String, &'a Block>,
    hatch_in_block: HashMap<String, usize>,
    out: Vec<(Rgb, Polyline)>,
    stats: Stats,
    /// Bloques que se están expandiendo, para detectar ciclos.
    stack: Vec<String>,
}

impl<'a> Importer<'a> {
    fn layer_info(&self, name: &str) -> Option<LayerInfo> {
        self.layers
            .get(name)
            .or_else(|| self.layers.get(&name.to_lowercase()))
            .copied()
    }

    fn entity_color(&self, e: &Entity, layer: &str, ctx: &Ctx) -> Rgb {
        let true_color = e.common.color_24_bit;
        if true_color != 0 {
            return Rgb(
                (true_color >> 16) as u8,
                (true_color >> 8) as u8,
                true_color as u8,
            );
        }
        let color = &e.common.color;
        if color.is_by_block() {
            return ctx.block_color.unwrap_or(DEFAULT_COLOR);
        }
        let index = color
            .index()
            .or_else(|| self.layer_info(layer).and_then(|l| l.color));
        index.map_or(DEFAULT_COLOR, aci_to_rgb)
    }

    fn visit(&mut self, e: &'a Entity, ctx: &Ctx<'a>) {
        if !e.common.is_visible {
            self.stats.hidden += 1;
            return;
        }
        let layer: &'a str = match ctx.layer0 {
            Some(parent) if e.common.layer == "0" => parent,
            _ => &e.common.layer,
        };
        if self.layer_info(layer).is_some_and(|l| !l.on) {
            self.stats.layer_off += 1;
            return;
        }
        let color = self.entity_color(e, layer, ctx);

        match &e.specific {
            EntityType::Line(l) => {
                let pts = vec![(l.p1.x, l.p1.y), (l.p2.x, l.p2.y)];
                self.emit(pts, false, &ctx.matrix, color);
            }
            EntityType::LwPolyline(p) => self.lw_polyline(p, ctx, color),
            EntityType::Polyline(p) => self.polyline(p, ctx, color),
            EntityType::Circle(c) => {
                let m = ctx
                    .matrix
                    .mul(&geom::ocs(c.normal.x, c.normal.y, c.normal.z));
                if let Some(tol) = self.local_tol(&m) {
                    let pts = geom::circle_points((c.center.x, c.center.y), c.radius.abs(), tol);
                    self.emit(pts, true, &m, color);
                }
            }
            EntityType::Arc(a) => {
                let m = ctx
                    .matrix
                    .mul(&geom::ocs(a.normal.x, a.normal.y, a.normal.z));
                if let Some(tol) = self.local_tol(&m) {
                    let start = a.start_angle.to_radians();
                    let mut sweep = (a.end_angle - a.start_angle).to_radians();
                    sweep = sweep.rem_euclid(TAU);
                    if sweep < 1e-12 {
                        sweep = TAU;
                    }
                    let mut pts = Vec::new();
                    geom::arc_points(
                        (a.center.x, a.center.y),
                        a.radius.abs(),
                        start,
                        sweep,
                        tol,
                        &mut pts,
                    );
                    self.emit(pts, false, &m, color);
                }
            }
            EntityType::Ellipse(el) => {
                let Some(tol) = self.local_tol(&ctx.matrix) else {
                    return;
                };
                let n = unit3(el.normal.x, el.normal.y, el.normal.z);
                let major = [el.major_axis.x, el.major_axis.y, el.major_axis.z];
                let c = geom::cross(n, major);
                let ratio = el.minor_axis_ratio;
                let minor = (c[0] * ratio, c[1] * ratio);
                let mut sweep = (el.end_parameter - el.start_parameter).rem_euclid(TAU);
                if sweep < 1e-12 {
                    sweep = TAU;
                }
                let pts = geom::ellipse_points(
                    (el.center.x, el.center.y),
                    (major[0], major[1]),
                    minor,
                    el.start_parameter,
                    sweep,
                    tol,
                );
                self.emit(pts, (sweep - TAU).abs() < 1e-9, &ctx.matrix, color);
            }
            EntityType::Spline(s) => self.spline(s, ctx, color),
            EntityType::Insert(ins) => self.insert(ins, layer, color, ctx),
            EntityType::Text(_) | EntityType::MText(_) | EntityType::RText(_) => {
                self.stats.text += 1
            }
            // Los atributos de un INSERT se ignoran junto con sus textos.
            EntityType::Attribute(_) | EntityType::AttributeDefinition(_) => self.stats.text += 1,
            EntityType::RotatedDimension(_)
            | EntityType::RadialDimension(_)
            | EntityType::DiameterDimension(_)
            | EntityType::AngularThreePointDimension(_)
            | EntityType::OrdinateDimension(_) => self.stats.dimensions += 1,
            other => *self.stats.other.entry(kind_name(other)).or_default() += 1,
        }
    }

    /// Tolerancia en coordenadas locales para que el error final sea `tol`.
    fn local_tol(&mut self, m: &Affine) -> Option<f64> {
        let scale = m.max_scale();
        if scale.is_finite() && scale > 1e-12 {
            Some(self.tol / scale)
        } else {
            self.stats.invalid += 1;
            None
        }
    }

    fn lw_polyline(&mut self, p: &LwPolyline, ctx: &Ctx, color: Rgb) {
        let m = ctx.matrix.mul(&geom::ocs(
            p.extrusion_direction.x,
            p.extrusion_direction.y,
            p.extrusion_direction.z,
        ));
        let Some(tol) = self.local_tol(&m) else {
            return;
        };
        let verts: Vec<(P2, f64)> = p.vertices.iter().map(|v| ((v.x, v.y), v.bulge)).collect();
        let closed = p.flags & 1 != 0;
        self.emit(bulge_chain(&verts, closed, tol), closed, &m, color);
    }

    fn polyline(&mut self, p: &DxfPolyline, ctx: &Ctx, color: Rgb) {
        if p.flags & (16 | 64) != 0 {
            self.stats.meshes += 1;
            return;
        }
        // En una polilínea 3D los vértices ya están en coordenadas globales.
        let m = if p.flags & 8 != 0 {
            ctx.matrix
        } else {
            ctx.matrix
                .mul(&geom::ocs(p.normal.x, p.normal.y, p.normal.z))
        };
        let Some(tol) = self.local_tol(&m) else {
            return;
        };
        // Los vértices con bandera 16 son puntos de control de un ajuste de
        // curva, no parte del trazado.
        let verts: Vec<(P2, f64)> = p
            .vertices()
            .filter(|v| v.flags & 16 == 0)
            .map(|v| ((v.location.x, v.location.y), v.bulge))
            .collect();
        let closed = p.flags & 1 != 0;
        self.emit(bulge_chain(&verts, closed, tol), closed, &m, color);
    }

    fn spline(&mut self, s: &Spline, ctx: &Ctx, color: Rgb) {
        let Some(tol) = self.local_tol(&ctx.matrix) else {
            return;
        };
        let closed = s.flags & 1 != 0;
        let degree = usize::try_from(s.degree_of_curve).unwrap_or(0);
        let ctrl: Vec<P2> = s.control_points.iter().map(|p| (p.x, p.y)).collect();

        if (1..=15).contains(&degree) && ctrl.len() > degree {
            let n = ctrl.len();
            let expected = n + degree + 1;
            let knots = if s.knot_values.len() == expected
                && s.knot_values.windows(2).all(|w| w[0] <= w[1])
            {
                s.knot_values.clone()
            } else {
                geom::clamped_uniform_knots(n, degree)
            };
            let weights = if s.weight_values.len() == n && s.weight_values.iter().all(|&w| w > 0.0)
            {
                s.weight_values.clone()
            } else {
                vec![1.0; n]
            };
            let pts = if degree == 1 {
                ctrl
            } else {
                Nurbs {
                    degree,
                    knots: &knots,
                    points: &ctrl,
                    weights: &weights,
                }
                .flatten(tol)
            };
            self.emit(pts, closed, &ctx.matrix, color);
        } else {
            let fit: Vec<P2> = s.fit_points.iter().map(|p| (p.x, p.y)).collect();
            if fit.len() >= 2 {
                self.stats.spline_fit_only += 1;
                self.emit(fit, closed, &ctx.matrix, color);
            } else {
                self.stats.invalid += 1;
            }
        }
    }

    fn insert(&mut self, ins: &'a Insert, layer: &'a str, color: Rgb, ctx: &Ctx<'a>) {
        let key = ins.name.to_lowercase();
        let Some(&block) = self.blocks.get(&key) else {
            self.stats.missing_blocks.insert(ins.name.clone());
            return;
        };
        if self.stack.contains(&key) {
            self.stats.cyclic_blocks.insert(ins.name.clone());
            return;
        }
        if ctx.depth >= MAX_DEPTH {
            self.stats.too_deep = true;
            return;
        }
        let cols = i64::from(ins.column_count.max(1));
        let rows = i64::from(ins.row_count.max(1));
        if cols * rows > MAX_ARRAY {
            self.stats.array_too_big += 1;
            return;
        }

        let to_world = ctx
            .matrix
            .mul(&geom::ocs(
                ins.extrusion_direction.x,
                ins.extrusion_direction.y,
                ins.extrusion_direction.z,
            ))
            .mul(&Affine::translate(ins.location.x, ins.location.y))
            .mul(&Affine::rotate(ins.rotation.to_radians()));
        let block_frame = Affine::scale(ins.x_scale_factor, ins.y_scale_factor)
            .mul(&Affine::translate(-block.base_point.x, -block.base_point.y));
        let hatches = self.hatch_in_block.get(&key).copied().unwrap_or(0);

        self.stack.push(key);
        for row in 0..rows {
            for col in 0..cols {
                // El espaciado del arreglo va en el marco girado del INSERT, sin escalar.
                let offset = Affine::translate(
                    col as f64 * ins.column_spacing,
                    row as f64 * ins.row_spacing,
                );
                let inner = Ctx {
                    matrix: to_world.mul(&offset).mul(&block_frame),
                    block_color: Some(color),
                    layer0: Some(layer),
                    depth: ctx.depth + 1,
                };
                self.stats.hatch += hatches;
                for e in &block.entities {
                    self.visit(e, &inner);
                }
            }
        }
        self.stack.pop();
    }

    /// Transforma a mm, limpia y guarda la trayectoria.
    fn emit(&mut self, pts: Vec<P2>, closed: bool, m: &Affine, color: Rgb) {
        let mut pts: Vec<Point> = pts
            .into_iter()
            .map(|p| {
                let (x, y) = m.apply(p);
                Point::new(x, y)
            })
            .collect();
        if pts.iter().any(|p| !p.x.is_finite() || !p.y.is_finite()) {
            self.stats.invalid += 1;
            return;
        }
        pts.dedup_by(|a, b| a.dist(*b) < EPS_MM);
        let mut closed = closed;
        // Una trayectoria que vuelve a su inicio es cerrada aunque no tenga la bandera.
        if pts.len() > 2 && pts[0].dist(pts[pts.len() - 1]) < EPS_MM {
            pts.pop();
            closed = true;
        }
        if pts.len() < 3 {
            closed = false;
        }
        if pts.len() < 2 {
            self.stats.invalid += 1;
            return;
        }
        self.out.push((
            color,
            Polyline {
                points: pts,
                closed,
            },
        ));
    }
}

/// Une los vértices de una polilínea con rectas o arcos (bulge). Con `closed`
/// agrega el tramo de cierre; el último punto puede coincidir con el primero.
fn bulge_chain(verts: &[(P2, f64)], closed: bool, tol: f64) -> Vec<P2> {
    let mut pts = Vec::with_capacity(verts.len());
    let Some(&(first, _)) = verts.first() else {
        return pts;
    };
    pts.push(first);
    for pair in verts.windows(2) {
        geom::bulge_points(pair[0].0, pair[1].0, pair[0].1, tol, &mut pts);
    }
    if closed && verts.len() > 1 {
        let (last, bulge) = verts[verts.len() - 1];
        geom::bulge_points(last, first, bulge, tol, &mut pts);
    }
    pts
}

fn unit3(x: f64, y: f64, z: f64) -> [f64; 3] {
    let len = (x * x + y * y + z * z).sqrt();
    if len < 1e-12 || !len.is_finite() {
        [0.0, 0.0, 1.0]
    } else {
        [x / len, y / len, z / len]
    }
}

fn kind_name(e: &EntityType) -> &'static str {
    match e {
        EntityType::Face3D(_) => "3DFACE",
        EntityType::Solid3D(_) => "3DSOLID",
        EntityType::Body(_) => "BODY",
        EntityType::Region(_) => "REGION",
        EntityType::Image(_) => "IMAGE",
        EntityType::Leader(_) => "LEADER",
        EntityType::MLine(_) => "MLINE",
        EntityType::ModelPoint(_) => "POINT",
        EntityType::Ray(_) => "RAY",
        EntityType::XLine(_) => "XLINE",
        EntityType::Solid(_) => "SOLID",
        EntityType::Trace(_) => "TRACE",
        EntityType::Shape(_) => "SHAPE",
        EntityType::Helix(_) => "HELIX",
        EntityType::Wipeout(_) => "WIPEOUT",
        EntityType::Tolerance(_) => "TOLERANCE",
        EntityType::Section(_) => "SECTION",
        EntityType::DgnUnderlay(_) | EntityType::DwfUnderlay(_) | EntityType::PdfUnderlay(_) => {
            "UNDERLAY"
        }
        _ => "otros",
    }
}

#[cfg(test)]
mod tests;
