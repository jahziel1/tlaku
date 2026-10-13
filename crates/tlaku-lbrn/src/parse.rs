//! Del XML de LightBurn al modelo de [`crate::Project`].

use crate::pathdata::{Prims, build_path, parse_prims, parse_verts};
use crate::{
    BitmapShape, IDENTITY, Layer, Project, Shape, ShapeKind, TextShape, Transform, pathdata,
};
use base64::Engine;
use roxmltree::Node;
use std::collections::{BTreeMap, HashMap};
use tlaku_core::Point;
use tlaku_core::bezpath::BezPath;

/// Profundidad máxima de grupos anidados (protege contra archivos hostiles).
const MAX_DEPTH: usize = 64;

/// Datos de geometría reutilizados entre formas: LightBurn guarda la lista de vértices o de
/// primitivas una sola vez y las demás formas la citan con `VertID`/`PrimID`.
#[derive(Default)]
struct Ctx {
    verts: HashMap<String, Vec<pathdata::Vert>>,
    prims: HashMap<String, Prims>,
    warnings: Vec<String>,
    /// Avisos que se cuentan en vez de repetirse (clave = mensaje).
    counted: BTreeMap<String, usize>,
}

impl Ctx {
    fn count(&mut self, msg: impl Into<String>) {
        *self.counted.entry(msg.into()).or_insert(0) += 1;
    }
}

fn attr_f64(n: Node, name: &str) -> Option<f64> {
    n.attribute(name)?
        .trim()
        .parse::<f64>()
        .ok()
        .filter(|v| v.is_finite())
}

fn child<'a, 'i>(n: Node<'a, 'i>, name: &str) -> Option<Node<'a, 'i>> {
    n.children()
        .find(|c| c.is_element() && c.has_tag_name(name))
}

fn xform(n: Node, ctx: &mut Ctx) -> Transform {
    let Some(text) = child(n, "XForm").and_then(|x| x.text()) else {
        return IDENTITY;
    };
    let vals: Vec<f64> = text
        .split_whitespace()
        .filter_map(|v| v.parse::<f64>().ok())
        .collect();
    match <[f64; 6]>::try_from(vals) {
        Ok(t) if t.iter().all(|v| v.is_finite()) => t,
        _ => {
            ctx.count("Una forma trae un XForm inválido; se usa la identidad");
            IDENTITY
        }
    }
}

/// Lee un proyecto. `data` es el XML completo (con o sin BOM UTF-8).
pub fn parse(data: &[u8]) -> Result<Project, String> {
    let data = data.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(data);
    let text = std::str::from_utf8(data)
        .map_err(|_| "no es texto UTF-8: un proyecto de LightBurn es XML".to_string())?;
    let doc = roxmltree::Document::parse(text).map_err(|e| {
        format!("no es un XML válido de LightBurn ({e}); ¿el archivo está dañado o comprimido?")
    })?;
    let root = doc.root_element();
    if !root.has_tag_name("LightBurnProject") {
        return Err(format!(
            "no es un proyecto de LightBurn: el elemento raíz es <{}>",
            root.tag_name().name()
        ));
    }

    let mut ctx = Ctx::default();
    let mut layers: Vec<Layer> = Vec::new();
    let mut shapes = Vec::new();
    for node in root.children().filter(Node::is_element) {
        match node.tag_name().name() {
            "CutSetting" | "CutSetting_Img" => {
                if let Some(layer) = parse_layer(node) {
                    // Un índice repetido es una sub-capa de la primera (misma capa, otra operación).
                    if let Some(main) = layers.iter_mut().find(|l| l.index == layer.index) {
                        main.subs.push(layer);
                    } else {
                        layers.push(layer);
                    }
                } else {
                    ctx.count("Una capa (CutSetting) sin número de índice se omite");
                }
            }
            "Shape" => {
                if let Some(shape) = parse_shape(node, &mut ctx, 0) {
                    shapes.push(shape);
                }
            }
            _ => {}
        }
    }

    let mut warnings = ctx.warnings;
    for (msg, n) in ctx.counted {
        warnings.push(if n > 1 {
            format!("{msg} ({n} veces)")
        } else {
            msg
        });
    }
    Ok(Project {
        app_version: root.attribute("AppVersion").unwrap_or_default().to_string(),
        format_version: root
            .attribute("FormatVersion")
            .and_then(|v| v.trim().parse().ok()),
        mirror_x: is_true(root.attribute("MirrorX")),
        mirror_y: is_true(root.attribute("MirrorY")),
        layers,
        shapes,
        warnings,
    })
}

fn is_true(v: Option<&str>) -> bool {
    v.is_some_and(|v| v.eq_ignore_ascii_case("true") || v == "1")
}

fn parse_layer(n: Node) -> Option<Layer> {
    let mut params = HashMap::new();
    for c in n.children().filter(Node::is_element) {
        if let Some(v) = c.attribute("Value") {
            params
                .entry(c.tag_name().name().to_string())
                .or_insert_with(|| v.to_string());
        }
    }
    let index: i32 = params.get("index")?.trim().parse().ok()?;
    let name = params
        .get("name")
        .filter(|s| !s.trim().is_empty())
        .cloned()
        .unwrap_or_else(|| format!("C{index:02}"));
    let mut kind = n.attribute("type").unwrap_or("Cut").to_string();
    if n.has_tag_name("CutSetting_Img") && !kind.eq_ignore_ascii_case("image") {
        kind = "Image".into();
    }
    Some(Layer {
        index,
        kind,
        name,
        params,
        subs: Vec::new(),
    })
}

fn parse_shape(n: Node, ctx: &mut Ctx, depth: usize) -> Option<Shape> {
    let ty = n.attribute("Type").unwrap_or_default();
    let cut_index = n.attribute("CutIndex").and_then(|v| v.trim().parse().ok());
    let xf = xform(n, ctx);
    let locked = is_true(n.attribute("Locked"));
    let kind = match ty {
        "Rect" => {
            let (Some(w), Some(h)) = (attr_f64(n, "W"), attr_f64(n, "H")) else {
                ctx.count("Un Rect sin W/H válidos se omite");
                return None;
            };
            ShapeKind::Rect {
                w,
                h,
                corner_radius: attr_f64(n, "Cr").unwrap_or(0.0),
            }
        }
        "Ellipse" => {
            let (Some(rx), Some(ry)) = (attr_f64(n, "Rx"), attr_f64(n, "Ry")) else {
                ctx.count("Una Ellipse sin Rx/Ry válidos se omite");
                return None;
            };
            ShapeKind::Ellipse { rx, ry }
        }
        "Path" | "Polygon" => match parse_path(n, ctx) {
            Some(path) => ShapeKind::Path(path),
            None if ty == "Polygon" => ShapeKind::Unknown(ty.to_string()),
            None => return None,
        },
        "Group" => {
            if depth >= MAX_DEPTH {
                ctx.count("Se omitieron grupos anidados a demasiada profundidad");
                return None;
            }
            let kids = child(n, "Children")
                .map(|c| {
                    c.children()
                        .filter(|k| k.is_element() && k.has_tag_name("Shape"))
                        .filter_map(|k| parse_shape(k, ctx, depth + 1))
                        .collect()
                })
                .unwrap_or_default();
            ShapeKind::Group(kids)
        }
        "Text" => ShapeKind::Text(parse_text(n, ctx)),
        "Bitmap" => match parse_bitmap(n, ctx) {
            Some(b) => ShapeKind::Bitmap(b),
            None => return None,
        },
        other => ShapeKind::Unknown(other.to_string()),
    };
    Some(Shape {
        cut_index,
        xform: xf,
        locked,
        kind,
    })
}

/// Vértices y primitivas de un `Path` (o de un `BackupPath`), con sus dos formas de escribirse:
/// `VertList`/`PrimList` (texto compacto) y los elementos `<V vx= vy= .../>` y `<P T= p0= p1=/>`
/// de las versiones viejas. Devuelve `None` si no hay geometría que dibujar.
fn parse_path(n: Node, ctx: &mut Ctx) -> Option<BezPath> {
    let vert_id = n.attribute("VertID").map(str::to_string);
    let prim_id = n.attribute("PrimID").map(str::to_string);

    // Vértices: texto propio, elementos viejos o los citados por VertID.
    let verts = if let Some(t) = child(n, "VertList")
        .and_then(|x| x.text())
        .filter(|t| !t.trim().is_empty())
    {
        let (v, bad) = parse_verts(t);
        if bad > 0 {
            ctx.count("Se omitieron vértices ilegibles en una trayectoria");
        }
        Some(v)
    } else if n.children().any(|c| c.is_element() && c.has_tag_name("V")) {
        Some(legacy_verts(n))
    } else {
        vert_id.as_ref().and_then(|id| ctx.verts.get(id).cloned())
    };
    if let (Some(id), Some(v)) = (&vert_id, &verts) {
        ctx.verts.entry(id.clone()).or_insert_with(|| v.clone());
    }

    let prims = if let Some(t) = child(n, "PrimList")
        .and_then(|x| x.text())
        .filter(|t| !t.trim().is_empty())
    {
        let (p, unknown) = parse_prims(t);
        for c in unknown {
            ctx.count(format!(
                "Primitiva de trayectoria '{c}' desconocida: se omite"
            ));
        }
        Some(p)
    } else if n.children().any(|c| c.is_element() && c.has_tag_name("P")) {
        Some(legacy_prims(n, ctx))
    } else {
        prim_id.as_ref().and_then(|id| ctx.prims.get(id).cloned())
    };
    if let (Some(id), Some(p)) = (&prim_id, &prims) {
        ctx.prims.entry(id.clone()).or_insert_with(|| p.clone());
    }

    let (verts, prims) = match (verts, prims) {
        (Some(v), Some(p)) if !v.is_empty() => (v, p),
        _ => {
            ctx.count("Se omitieron trayectorias sin vértices o sin primitivas");
            return None;
        }
    };
    let built = build_path(&verts, &prims);
    if built.degraded > 0 {
        ctx.count("Curvas Bézier sin manejadores se importaron como rectas");
    }
    if built.invalid > 0 {
        ctx.count("Se omitieron segmentos que citan un vértice inexistente");
    }
    if built.path.0.is_empty() {
        ctx.count("Se omitieron trayectorias sin segmentos");
        return None;
    }
    Some(built.path)
}

fn legacy_verts(n: Node) -> Vec<pathdata::Vert> {
    n.children()
        .filter(|c| c.is_element() && c.has_tag_name("V"))
        .filter_map(|v| {
            let p = Point::new(attr_f64(v, "vx")?, attr_f64(v, "vy")?);
            let pair = |ax: &str, ay: &str| match (attr_f64(v, ax), attr_f64(v, ay)) {
                (Some(x), Some(y)) => Some(Point::new(x, y)),
                _ => None,
            };
            Some(pathdata::Vert {
                p,
                c0: pair("c0x", "c0y"),
                c1: pair("c1x", "c1y"),
            })
        })
        .collect()
}

fn legacy_prims(n: Node, ctx: &mut Ctx) -> Prims {
    use pathdata::{Prim, PrimKind};
    let mut out = Vec::new();
    for p in n
        .children()
        .filter(|c| c.is_element() && c.has_tag_name("P"))
    {
        let kind = match p.attribute("T") {
            Some("L") => PrimKind::Line,
            Some("B") => PrimKind::Bezier,
            other => {
                ctx.count(format!(
                    "Primitiva de trayectoria '{}' desconocida: se omite",
                    other.unwrap_or("?")
                ));
                continue;
            }
        };
        let idx = |name: &str| {
            p.attribute(name)
                .and_then(|v| v.trim().parse::<usize>().ok())
        };
        if let (Some(a), Some(b)) = (idx("p0"), idx("p1")) {
            out.push(Prim { kind, a, b });
        }
    }
    Prims::List(out)
}

/// Familia, negrita y cursiva de la cadena `Font` (formato de `QFont::toString` de Qt:
/// `familia,pt,px,pista,peso,estilo,...`; peso 50 normal y 75 negrita en Qt5, 400 y 700 en Qt6).
fn parse_font(font: &str) -> (String, bool, bool) {
    let mut fields = font.split(',');
    let family = fields.next().unwrap_or_default().trim().to_string();
    let rest: Vec<&str> = fields.collect();
    let weight: f64 = rest
        .get(3)
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(50.0);
    let style: i32 = rest.get(4).and_then(|v| v.trim().parse().ok()).unwrap_or(0);
    let bold = if weight <= 99.0 {
        weight >= 63.0
    } else {
        weight >= 600.0
    };
    (family, bold, style == 1 || style == 2)
}

fn parse_text(n: Node, ctx: &mut Ctx) -> TextShape {
    let font = n.attribute("Font").unwrap_or_default().to_string();
    let (family, bold, italic) = parse_font(&font);
    let outline = if n.attribute("HasBackupPath") == Some("1") || child(n, "BackupPath").is_some() {
        child(n, "BackupPath").and_then(|b| {
            // El contorno guardado puede citar vértices o primitivas de otra forma por ID.
            parse_path(b, ctx)
        })
    } else {
        None
    };
    TextShape {
        text: n.attribute("Str").unwrap_or_default().to_string(),
        font,
        family,
        bold,
        italic,
        height: attr_f64(n, "H").filter(|h| *h > 0.0).unwrap_or(10.0),
        letter_spacing: attr_f64(n, "LS").unwrap_or(0.0),
        line_spacing: attr_f64(n, "LnS").unwrap_or(0.0),
        align_h: attr_f64(n, "Ah").map_or(0, |v| v.clamp(0.0, 2.0) as u8),
        align_v: attr_f64(n, "Av").map_or(0, |v| v.clamp(0.0, 2.0) as u8),
        outline,
        variable: n.attribute("VariableOffset").is_some() || n.attribute("Eval").is_some(),
    }
}

fn parse_bitmap(n: Node, ctx: &mut Ctx) -> Option<BitmapShape> {
    let (Some(w), Some(h)) = (attr_f64(n, "W"), attr_f64(n, "H")) else {
        ctx.count("Un Bitmap sin W/H válidos se omite");
        return None;
    };
    let raw: String = n
        .attribute("Data")
        .unwrap_or_default()
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    if raw.is_empty() {
        ctx.count("Un Bitmap sin imagen incrustada se omite");
        return None;
    }
    let Ok(data) = base64::engine::general_purpose::STANDARD.decode(raw.as_bytes()) else {
        ctx.count("Un Bitmap con la imagen incrustada ilegible (base64 inválido) se omite");
        return None;
    };
    Some(BitmapShape {
        w,
        h,
        data,
        file: n
            .attribute("File")
            .map(str::to_string)
            .filter(|f| !f.is_empty()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ShapeKind;

    fn shapes(xml: &str) -> Project {
        parse(xml.as_bytes()).expect("XML válido")
    }

    #[test]
    fn accepts_bom_and_reads_header() {
        let mut data = b"\xEF\xBB\xBF".to_vec();
        data.extend_from_slice(
            br#"<LightBurnProject AppVersion="1.4.03" FormatVersion="1" MirrorX="True"/>"#,
        );
        let p = parse(&data).unwrap();
        assert_eq!(p.app_version, "1.4.03");
        assert_eq!(p.format_version, Some(1));
        assert!(p.mirror_x && !p.mirror_y);
    }

    #[test]
    fn missing_xform_is_identity_and_bad_one_is_reported() {
        let p = shapes(
            r#"<LightBurnProject>
                 <Shape Type="Rect" W="1" H="1"/>
                 <Shape Type="Rect" W="1" H="1"><XForm>1 2 3</XForm></Shape>
               </LightBurnProject>"#,
        );
        assert_eq!(p.shapes[0].xform, IDENTITY);
        assert_eq!(p.shapes[1].xform, IDENTITY);
        assert!(p.warnings.iter().any(|w| w.contains("XForm inválido")));
    }

    #[test]
    fn backup_path_can_cite_primitives_by_id() {
        let p = shapes(
            r#"<LightBurnProject>
                 <Shape Type="Path" CutIndex="0" VertID="1" PrimID="2">
                   <VertList>V0 0V1 0V1 1</VertList><PrimList>L0 1L1 2L2 0</PrimList>
                 </Shape>
                 <Shape Type="Text" Str="x" H="5" HasBackupPath="1">
                   <BackupPath Type="Path" PrimID="2"><VertList>V0 0V2 0V2 2</VertList><PrimList></PrimList></BackupPath>
                 </Shape>
               </LightBurnProject>"#,
        );
        let ShapeKind::Text(t) = &p.shapes[1].kind else {
            panic!("se esperaba texto");
        };
        let b = t.outline.as_ref().unwrap().bounds().unwrap();
        assert_eq!((b.max.x, b.max.y), (2.0, 2.0));
    }

    #[test]
    fn bad_base64_bitmap_and_deep_groups_are_omitted() {
        let mut xml =
            String::from(r#"<LightBurnProject><Shape Type="Bitmap" W="1" H="1" Data="!!!"/>"#);
        for _ in 0..70 {
            xml.push_str(r#"<Shape Type="Group"><Children>"#);
        }
        for _ in 0..70 {
            xml.push_str("</Children></Shape>");
        }
        xml.push_str("</LightBurnProject>");
        let p = shapes(&xml);
        assert!(p.warnings.iter().any(|w| w.contains("base64")));
        assert!(p.warnings.iter().any(|w| w.contains("profundidad")));
    }

    #[test]
    fn duplicate_layer_index_becomes_a_sub_layer() {
        let p = shapes(
            r#"<LightBurnProject>
                 <CutSetting type="Cut"><index Value="1"/><name Value="A"/></CutSetting>
                 <CutSetting type="Scan"><index Value="1"/><name Value="B"/></CutSetting>
               </LightBurnProject>"#,
        );
        assert_eq!(p.layers.len(), 1);
        assert_eq!(p.layers[0].name, "A");
        assert_eq!(p.layers[0].subs.len(), 1);
        assert_eq!(p.layers[0].subs[0].name, "B");
        assert_eq!(p.layers[0].subs[0].kind, "Scan");
    }

    #[test]
    fn font_string_weight_and_style() {
        assert_eq!(
            parse_font("Arial,-1,100,5,50,0,0,0,0,0"),
            ("Arial".into(), false, false)
        );
        assert_eq!(
            parse_font("Arial,-1,100,5,75,1,0,0,0,0"),
            ("Arial".into(), true, true)
        );
        assert_eq!(
            parse_font("Foo,-1,100,5,700,0"),
            ("Foo".into(), true, false)
        );
    }
}
