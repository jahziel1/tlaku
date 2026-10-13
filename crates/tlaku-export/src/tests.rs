use super::*;
use tlaku_core::bezpath::{BezPath, PathSeg};
use tlaku_core::{Point, Rgb};
use tlaku_doc::affine::{self, IDENTITY};
use tlaku_doc::{
    DesignObject, Document, ImageObject, LayerMode, LayerSettings, ObjectKind, TextAlign,
    TextObject, VectorObject,
};

fn p(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn rect(x: f64, y: f64, w: f64, h: f64) -> BezPath {
    BezPath(vec![
        PathSeg::MoveTo { to: p(x, y) },
        PathSeg::LineTo { to: p(x + w, y) },
        PathSeg::LineTo {
            to: p(x + w, y + h),
        },
        PathSeg::LineTo { to: p(x, y + h) },
        PathSeg::Close,
    ])
}

/// Círculo de cuatro cúbicas.
fn circle(cx: f64, cy: f64, r: f64) -> BezPath {
    let k = 0.552_284_749_8 * r;
    BezPath(vec![
        PathSeg::MoveTo { to: p(cx + r, cy) },
        PathSeg::CubicTo {
            ctrl1: p(cx + r, cy + k),
            ctrl2: p(cx + k, cy + r),
            to: p(cx, cy + r),
        },
        PathSeg::CubicTo {
            ctrl1: p(cx - k, cy + r),
            ctrl2: p(cx - r, cy + k),
            to: p(cx - r, cy),
        },
        PathSeg::CubicTo {
            ctrl1: p(cx - r, cy - k),
            ctrl2: p(cx - k, cy - r),
            to: p(cx, cy - r),
        },
        PathSeg::CubicTo {
            ctrl1: p(cx + k, cy - r),
            ctrl2: p(cx + r, cy - k),
            to: p(cx + r, cy),
        },
        PathSeg::Close,
    ])
}

fn vector(id: &str, color: Rgb, paths: Vec<BezPath>) -> DesignObject {
    DesignObject {
        id: id.into(),
        layer: color,
        transform: IDENTITY,
        kind: ObjectKind::Vector(VectorObject { paths }),
        locked: false,
        hidden: false,
        group: None,
    }
}

const RED: Rgb = Rgb(255, 0, 0);
const BLUE: Rgb = Rgb(0, 0, 255);

/// Un círculo rojo (r 10, centro 30,25), un rectángulo azul y un trazo mixto (recta y curva).
fn sample() -> Document {
    let mixed = BezPath(vec![
        PathSeg::MoveTo { to: p(60.0, 5.0) },
        PathSeg::LineTo { to: p(80.0, 5.0) },
        PathSeg::QuadTo {
            ctrl: p(90.0, 15.0),
            to: p(80.0, 25.0),
        },
        PathSeg::Close,
    ]);
    let mut moved = vector("rect", BLUE, vec![rect(50.0, 30.0, 20.0, 10.0)]);
    // El transform se aplica: el rectángulo queda desplazado 5 mm a la derecha.
    moved.transform = affine::translate(5.0, 0.0);
    Document {
        objects: vec![
            vector("circulo", RED, vec![circle(30.0, 25.0, 10.0)]),
            moved,
            vector("mixto", BLUE, vec![mixed]),
        ],
        layers: vec![
            LayerSettings::for_color(RED),
            LayerSettings::for_color(BLUE),
        ],
        ..Document::default()
    }
}

/// Puntos de todos los trayectos de una capa, aplanados con tolerancia fina.
fn flat_points(paths: &[BezPath]) -> Vec<Point> {
    paths
        .iter()
        .flat_map(|b| b.flatten(0.001))
        .flat_map(|pl| pl.points)
        .collect()
}

fn bounds_of(paths: &[BezPath]) -> (f64, f64, f64, f64) {
    let pts = flat_points(paths);
    let min = |f: fn(&Point) -> f64| pts.iter().map(f).fold(f64::MAX, f64::min);
    let max = |f: fn(&Point) -> f64| pts.iter().map(f).fold(f64::MIN, f64::max);
    (min(|p| p.x), min(|p| p.y), max(|p| p.x), max(|p| p.y))
}

fn doc_layer_paths(doc: &Document, color: Rgb) -> Vec<BezPath> {
    doc.objects
        .iter()
        .filter(|o| o.layer == color)
        .flat_map(|o| o.curves())
        .collect()
}

fn assert_close(a: (f64, f64, f64, f64), b: (f64, f64, f64, f64), tol: f64) {
    for (x, y) in [(a.0, b.0), (a.1, b.1), (a.2, b.2), (a.3, b.3)] {
        assert!((x - y).abs() < tol, "{a:?} != {b:?}");
    }
}

fn text(s: &Export) -> String {
    String::from_utf8(s.bytes.clone()).unwrap()
}

// ---------------------------------------------------------------------------
// SVG
// ---------------------------------------------------------------------------

#[test]
fn svg_tiene_mm_reales_y_curvas() {
    let doc = sample();
    let svg = export_svg(&doc, None, &SvgOptions::default()).unwrap();
    let s = text(&svg);
    // Caja del diseño: x 20..90 (la curva cuadrática llega a x 85), y 5..40 -> 65 x 35 mm como mucho.
    assert!(s.contains("mm\" height=\""), "{s}");
    assert!(s.contains("viewBox=\"0 0 "));
    assert!(s.contains(" C"), "las curvas Bézier se conservan: {s}");
    assert!(s.contains("Q"));
    assert!(s.contains("id=\"capa-ff0000\""));
    assert!(s.contains("stroke=\"#0000ff\""));
    assert!(svg.size_mm[0] > 60.0 && svg.size_mm[1] > 30.0);
}

#[test]
fn svg_ida_y_vuelta_con_tlaku_svg() {
    let doc = sample();
    let svg = export_svg(
        &doc,
        None,
        &SvgOptions {
            keep_position: true,
            ..SvgOptions::default()
        },
    )
    .unwrap();
    let imported = tlaku_svg::import_svg(&svg.bytes, 0.001).unwrap();
    assert!(imported.warnings.is_empty(), "{:?}", imported.warnings);
    // Las dimensiones salen de width/height en mm.
    assert!((imported.width_mm - svg.size_mm[0]).abs() < 1e-3);
    assert!((imported.height_mm - svg.size_mm[1]).abs() < 1e-3);
    let layers = imported.into_curves(true);
    assert_eq!(layers.len(), 2);
    for (color, paths) in &layers {
        let expected = doc_layer_paths(&doc, *color);
        assert_close(bounds_of(paths), bounds_of(&expected), 2e-3);
        assert_eq!(
            paths.iter().map(|p| p.0.len()).sum::<usize>(),
            expected.iter().map(|p| p.0.len()).sum::<usize>(),
            "misma cantidad de segmentos en {color}"
        );
    }
    // El círculo sigue siendo de cúbicas (no se aplanó).
    let red = &layers.iter().find(|(c, _)| *c == RED).unwrap().1;
    assert!(
        red[0]
            .0
            .iter()
            .any(|s| matches!(s, PathSeg::CubicTo { .. }))
    );
}

#[test]
fn svg_ajustado_al_diseno_desplaza_el_origen() {
    let doc = sample();
    let svg = export_svg(&doc, None, &SvgOptions::default()).unwrap();
    let imported = tlaku_svg::import_svg(&svg.bytes, 0.001).unwrap();
    let layers = imported.into_curves(true);
    let (mut x0, mut y0) = (f64::MAX, f64::MAX);
    for (_, paths) in &layers {
        let b = bounds_of(paths);
        x0 = x0.min(b.0);
        y0 = y0.min(b.1);
    }
    // Sin keep_position el diseño empieza en la esquina de la página.
    assert!(x0.abs() < 2e-3 && y0.abs() < 2e-3, "{x0} {y0}");
}

#[test]
fn svg_seleccion_grupos_y_ocultos() {
    let mut doc = sample();
    doc.objects[0].group = Some("g1".into());
    doc.objects[2].group = Some("g1".into());
    doc.objects[1].hidden = true;
    let all = export_svg(&doc, None, &SvgOptions::default()).unwrap();
    let s = text(&all);
    assert!(s.contains("id=\"grupo-g1\""));
    assert!(!s.contains("id=\"rect\""), "los ocultos no salen");
    assert!(all.warnings.iter().any(|w| w.contains("oculto")));
    // Solo la selección.
    let sel = vec!["circulo".to_string()];
    let one = export_svg(&doc, Some(&sel), &SvgOptions::default()).unwrap();
    let s = text(&one);
    assert!(s.contains("id=\"circulo\"") && !s.contains("id=\"mixto\""));
    assert!((one.size_mm[0] - 20.0).abs() < 1e-6 && (one.size_mm[1] - 20.0).abs() < 1e-6);
    // Selección vacía: error.
    let none = vec!["nada".to_string()];
    assert!(export_svg(&doc, Some(&none), &SvgOptions::default()).is_err());
}

fn text_object(outline: Vec<BezPath>) -> DesignObject {
    DesignObject {
        id: "txt".into(),
        layer: RED,
        transform: IDENTITY,
        kind: ObjectKind::Text(TextObject {
            text: "Hola".into(),
            font_family: "Sans".into(),
            bold: false,
            italic: false,
            size_mm: 10.0,
            letter_spacing_mm: 0.0,
            line_spacing: 1.0,
            align: TextAlign::Left,
            on_path: None,
            bend_deg: None,
            welded: None,
            outline,
        }),
        locked: false,
        hidden: false,
        group: None,
    }
}

#[test]
fn texto_sale_como_trayectos_y_sin_contorno_avisa() {
    let mut doc = sample();
    doc.objects
        .push(text_object(vec![rect(0.0, 0.0, 5.0, 8.0)]));
    let svg = export_svg(&doc, None, &SvgOptions::default()).unwrap();
    assert!(text(&svg).contains("id=\"txt\""));
    doc.objects.last_mut().unwrap().kind = text_object(vec![]).kind;
    let svg = export_svg(&doc, None, &SvgOptions::default()).unwrap();
    assert!(!text(&svg).contains("id=\"txt\""));
    assert!(svg.warnings.iter().any(|w| w.contains("Hola")));
}

/// PNG pequeño de 2x2 en disco.
fn temp_png(name: &str) -> String {
    let path = std::env::temp_dir().join(format!("tlaku-export-{}-{name}.png", std::process::id()));
    let img = image::RgbaImage::from_fn(2, 2, |x, y| {
        if (x + y) % 2 == 0 {
            image::Rgba([0, 0, 0, 255])
        } else {
            image::Rgba([255, 255, 255, 255])
        }
    });
    img.save(&path).unwrap();
    path.to_string_lossy().into_owned()
}

fn with_image(path: &str) -> Document {
    let mut doc = sample();
    doc.objects.push(DesignObject {
        id: "foto".into(),
        layer: LayerSettings::IMAGE_COLOR,
        // Rotada 90° y desplazada para comprobar la matriz.
        transform: affine::compose(
            &affine::translate(100.0, 10.0),
            &affine::rotate(std::f64::consts::FRAC_PI_2),
        ),
        kind: ObjectKind::Image(ImageObject {
            source_path: path.into(),
            width_px: 2,
            height_px: 2,
            size_mm: [20.0, 10.0],
        }),
        locked: false,
        hidden: false,
        group: None,
    });
    doc.layers.push(LayerSettings::for_images());
    doc
}

#[test]
fn svg_imagenes_incrustadas_enlazadas_u_omitidas() {
    let path = temp_png("svg");
    let doc = with_image(&path);
    let embed = export_svg(&doc, None, &SvgOptions::default()).unwrap();
    let s = text(&embed);
    assert!(s.contains("xlink:href=\"data:image/png;base64,"), "{s}");
    assert!(s.contains("transform=\"matrix("));
    let link = export_svg(
        &doc,
        None,
        &SvgOptions {
            images: ImageHandling::Link,
            ..SvgOptions::default()
        },
    )
    .unwrap();
    assert!(text(&link).contains(&format!("xlink:href=\"{path}\"")));
    let skip = export_svg(
        &doc,
        None,
        &SvgOptions {
            images: ImageHandling::Skip,
            ..SvgOptions::default()
        },
    )
    .unwrap();
    assert!(!text(&skip).contains("<image"));
    // Un SVG con imagen sigue siendo legible (el importador la omite con un aviso).
    assert!(tlaku_svg::import_svg(&embed.bytes, 0.01).is_ok());
    std::fs::remove_file(path).unwrap();
}

// ---------------------------------------------------------------------------
// DXF
// ---------------------------------------------------------------------------

fn load_dxf(bytes: &[u8]) -> dxf::Drawing {
    dxf::Drawing::load(&mut &bytes[..]).unwrap()
}

#[test]
fn dxf_ida_y_vuelta_con_splines_y_polilineas() {
    let doc = sample();
    let out = export_dxf(&doc, None, &DxfOptions::default()).unwrap();
    let s = text(&out);
    // Con color verdadero hace falta AC1018; sin él, AC1015.
    assert!(s.contains("AC1018"));
    let plain = export_dxf(
        &doc,
        None,
        &DxfOptions {
            true_color: false,
            ..DxfOptions::default()
        },
    )
    .unwrap();
    assert!(text(&plain).contains("AC1015"));
    assert!(tlaku_dxf::import_dxf(&plain.bytes, 0.01).is_ok());
    let drawing = load_dxf(&out.bytes);
    let (mut splines, mut polys) = (0, 0);
    for e in drawing.entities() {
        match &e.specific {
            dxf::entities::EntityType::Spline(sp) => {
                splines += 1;
                assert_eq!(sp.degree_of_curve, 3);
                assert_eq!(sp.knot_values.len(), sp.control_points.len() + 4);
            }
            dxf::entities::EntityType::LwPolyline(_) => polys += 1,
            other => panic!("entidad inesperada: {other:?}"),
        }
    }
    // El círculo es un SPLINE; el rectángulo y el trazo mixto, polilíneas.
    assert_eq!((splines, polys), (1, 2));

    let imported = tlaku_dxf::import_dxf(&out.bytes, 0.001).unwrap();
    assert!(
        imported
            .warnings
            .iter()
            .all(|w| !w.contains("no declara unidades")),
        "{:?}",
        imported.warnings
    );
    for color in [RED, BLUE] {
        let got: Vec<BezPath> = imported
            .paths
            .iter()
            .filter(|(c, _)| *c == color)
            .map(|(_, pl)| BezPath::from(pl))
            .collect();
        assert!(!got.is_empty(), "falta {color}");
        assert_close(
            bounds_of(&got),
            bounds_of(&doc_layer_paths(&doc, color)),
            6e-3,
        );
    }
}

#[test]
fn dxf_aplanado_color_aci_y_verdadero() {
    let mut doc = sample();
    // Un naranja que no está en la tabla ACI.
    let orange = Rgb(250, 120, 10);
    doc.objects
        .push(vector("naranja", orange, vec![rect(0.0, 0.0, 4.0, 4.0)]));
    let out = export_dxf(
        &doc,
        None,
        &DxfOptions {
            curves: DxfCurves::Flatten,
            ..DxfOptions::default()
        },
    )
    .unwrap();
    let drawing = load_dxf(&out.bytes);
    assert!(
        drawing
            .entities()
            .all(|e| matches!(e.specific, dxf::entities::EntityType::LwPolyline(_)))
    );
    // El color exacto sobrevive por color verdadero y la entidad lleva un ACI cercano.
    let e = drawing
        .entities()
        .find(|e| e.common.color_24_bit != 0)
        .expect("color verdadero");
    assert_eq!(e.common.color_24_bit, (250 << 16) | (120 << 8) | 10);
    assert!(e.common.color.index().is_some());
    let imported = tlaku_dxf::import_dxf(&out.bytes, 0.001).unwrap();
    assert!(imported.paths.iter().any(|(c, _)| *c == orange));
    // Sin color verdadero solo queda el ACI.
    let aci = export_dxf(
        &doc,
        None,
        &DxfOptions {
            true_color: false,
            ..DxfOptions::default()
        },
    )
    .unwrap();
    assert!(
        load_dxf(&aci.bytes)
            .entities()
            .all(|e| e.common.color_24_bit == 0)
    );
    // Capas: una por color, con el ACI cercano.
    assert_eq!(
        load_dxf(&out.bytes)
            .layers()
            .filter(|l| l.name != "0")
            .count(),
        3
    );
}

#[test]
fn dxf_aci_mas_cercano() {
    use crate::dxf_out::nearest_aci;
    assert_eq!(nearest_aci(Rgb(255, 0, 0)), 1);
    assert_eq!(nearest_aci(Rgb(0, 0, 255)), 5);
    assert_eq!(nearest_aci(Rgb(0, 0, 0)), 7);
    assert_eq!(nearest_aci(Rgb(255, 255, 255)), 255);
    assert_eq!(nearest_aci(Rgb(250, 250, 5)), 2);
}

#[test]
fn dxf_omite_imagenes_con_aviso() {
    let path = temp_png("dxf");
    let out = export_dxf(&with_image(&path), None, &DxfOptions::default()).unwrap();
    assert!(out.warnings.iter().any(|w| w.contains("imágenes")));
    std::fs::remove_file(path).unwrap();
}

// ---------------------------------------------------------------------------
// PNG
// ---------------------------------------------------------------------------

#[test]
fn png_a_la_resolucion_pedida() {
    let doc = Document {
        objects: vec![vector("r", RED, vec![rect(0.0, 0.0, 10.0, 5.0)])],
        layers: vec![LayerSettings::for_color(RED)],
        ..Document::default()
    };
    // 254 ppp = 10 píxeles por mm; sin margen la imagen mide 100 x 50.
    let out = export_png(
        &doc,
        None,
        &PngOptions {
            dpi: 254.0,
            margin_mm: 0.0,
            ..PngOptions::default()
        },
    )
    .unwrap();
    let img = image::load_from_memory(&out.bytes).unwrap().to_rgba8();
    assert_eq!((img.width(), img.height()), (100, 50));
    // Borde izquierdo del rectángulo: rojo; interior (sin relleno): blanco.
    let red = img.get_pixel(0, 25).0;
    assert!(red[0] > 200 && red[1] < 80, "{red:?}");
    assert_eq!(img.get_pixel(50, 25).0, [255, 255, 255, 255]);
    // Resolución física guardada: 10 px/mm = 10000 px/m.
    let idx = out.bytes.windows(4).position(|w| w == b"pHYs").unwrap();
    let ppm = u32::from_be_bytes(out.bytes[idx + 4..idx + 8].try_into().unwrap());
    assert_eq!(ppm, 10_000);
    // Fondo transparente.
    let t = export_png(
        &doc,
        None,
        &PngOptions {
            dpi: 254.0,
            margin_mm: 0.0,
            transparent: true,
            ..PngOptions::default()
        },
    )
    .unwrap();
    let img = image::load_from_memory(&t.bytes).unwrap().to_rgba8();
    assert_eq!(img.get_pixel(50, 25).0[3], 0);
    // Resolución absurda: error claro.
    let e = export_png(
        &doc,
        None,
        &PngOptions {
            dpi: 1_000_000.0,
            ..PngOptions::default()
        },
    )
    .unwrap_err();
    assert!(e.contains("demasiado grande"), "{e}");
}

#[test]
fn png_rellena_capas_de_relleno_y_dibuja_imagenes() {
    let mut doc = Document {
        objects: vec![vector("r", RED, vec![rect(0.0, 0.0, 10.0, 10.0)])],
        layers: vec![LayerSettings::for_color(RED)],
        ..Document::default()
    };
    doc.layers[0].mode = LayerMode::Fill;
    let out = export_png(
        &doc,
        None,
        &PngOptions {
            dpi: 254.0,
            margin_mm: 0.0,
            ..PngOptions::default()
        },
    )
    .unwrap();
    let img = image::load_from_memory(&out.bytes).unwrap().to_rgba8();
    assert_eq!(img.get_pixel(50, 50).0, [255, 0, 0, 255]);

    // Una imagen de 2x2 (negro arriba a la izquierda) de 10 x 10 mm.
    let path = temp_png("png");
    let doc = Document {
        objects: vec![DesignObject {
            id: "foto".into(),
            layer: LayerSettings::IMAGE_COLOR,
            transform: IDENTITY,
            kind: ObjectKind::Image(ImageObject {
                source_path: path.clone(),
                width_px: 2,
                height_px: 2,
                size_mm: [10.0, 10.0],
            }),
            locked: false,
            hidden: false,
            group: None,
        }],
        layers: vec![LayerSettings::for_images()],
        ..Document::default()
    };
    let out = export_png(
        &doc,
        None,
        &PngOptions {
            dpi: 254.0,
            margin_mm: 0.0,
            ..PngOptions::default()
        },
    )
    .unwrap();
    assert!(out.warnings.is_empty(), "{:?}", out.warnings);
    let img = image::load_from_memory(&out.bytes).unwrap().to_rgba8();
    // Arriba a la izquierda negro, arriba a la derecha blanco (la fila 0 del PNG va arriba).
    assert!(
        img.get_pixel(25, 25).0[0] < 40,
        "{:?}",
        img.get_pixel(25, 25)
    );
    assert!(img.get_pixel(75, 25).0[0] > 215);
    assert!(img.get_pixel(25, 75).0[0] > 215);
    assert!(img.get_pixel(75, 75).0[0] < 40);
    std::fs::remove_file(path).unwrap();
}

// ---------------------------------------------------------------------------
// PDF
// ---------------------------------------------------------------------------

fn parse_pdf(bytes: &[u8]) -> (lopdf::Document, Vec<lopdf::content::Operation>, [f64; 4]) {
    let pdf = lopdf::Document::load_mem(bytes).unwrap();
    let page = *pdf.get_pages().values().next().unwrap();
    let ops = lopdf::content::Content::decode(&pdf.get_page_content(page))
        .unwrap()
        .operations;
    let media: Vec<f64> = pdf
        .get_dictionary(page)
        .unwrap()
        .get(b"MediaBox")
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .map(|o| f64::from(o.as_float().unwrap()))
        .collect();
    (pdf, ops, [media[0], media[1], media[2], media[3]])
}

#[test]
fn pdf_vectorial_a_escala_1_a_1() {
    let doc = sample();
    let out = export_pdf(
        &doc,
        None,
        &PdfOptions {
            margin_mm: 0.0,
            ..PdfOptions::default()
        },
    )
    .unwrap();
    assert!(out.bytes.starts_with(b"%PDF-1.4"));
    let (pdf, ops, media) = parse_pdf(&out.bytes);
    assert_eq!(pdf.get_pages().len(), 1);
    // La hoja mide lo que el diseño, en puntos.
    let k = 72.0 / 25.4;
    assert!((media[2] - out.size_mm[0] * k).abs() < 0.01);
    assert!((media[3] - out.size_mm[1] * k).abs() < 0.01);
    let count = |name: &str| ops.iter().filter(|o| o.operator == name).count();
    // 3 objetos con un trayecto cada uno -> 3 trazos; el círculo tiene 4 cúbicas y el trazo
    // mixto una cuadrática elevada a cúbica.
    assert_eq!(count("S"), 3);
    assert_eq!(count("c"), 5);
    assert_eq!(count("RG"), 2);
    // Los puntos están en pt con el origen en la esquina del diseño: el rectángulo azul
    // empieza en x = 55 (50 + 5 del transform), y = 30 - 5 (y mínimo del diseño).
    let first_move = ops
        .iter()
        .filter(|o| o.operator == "m")
        .map(|o| {
            (
                f64::from(o.operands[0].as_float().unwrap()),
                f64::from(o.operands[1].as_float().unwrap()),
            )
        })
        .collect::<Vec<_>>();
    // Mínimos del diseño: x = 20 (círculo), y = 5 (trazo mixto).
    assert!(
        first_move
            .iter()
            .any(|(x, y)| (x - (55.0 - 20.0) * k).abs() < 0.01
                && (y - (30.0 - 5.0) * k).abs() < 0.01)
    );
}

#[test]
fn pdf_hoja_a4_orientacion_y_desborde() {
    let doc = sample();
    let out = export_pdf(
        &doc,
        None,
        &PdfOptions {
            page: PageSize::A4,
            ..PdfOptions::default()
        },
    )
    .unwrap();
    let (_, _, media) = parse_pdf(&out.bytes);
    let k = 72.0 / 25.4;
    // El diseño es más ancho que alto: hoja apaisada.
    assert!((media[2] - 297.0 * k).abs() < 0.01 && (media[3] - 210.0 * k).abs() < 0.01);
    assert!(out.warnings.is_empty());
    // Un diseño de 400 mm no cabe en A4 a 1:1: se ajusta con aviso.
    let big = Document {
        objects: vec![vector("g", RED, vec![rect(0.0, 0.0, 400.0, 100.0)])],
        layers: vec![LayerSettings::for_color(RED)],
        ..Document::default()
    };
    let out = export_pdf(
        &big,
        None,
        &PdfOptions {
            page: PageSize::A4,
            margin_mm: 0.0,
            ..PdfOptions::default()
        },
    )
    .unwrap();
    assert!(out.warnings.iter().any(|w| w.contains("no cabe")));
    assert!((out.size_mm[0] - 400.0).abs() < 1e-6);
}

#[test]
fn pdf_con_imagen_incrustada() {
    let path = temp_png("pdf");
    let doc = with_image(&path);
    let out = export_pdf(&doc, None, &PdfOptions::default()).unwrap();
    let (pdf, ops, _) = parse_pdf(&out.bytes);
    assert!(ops.iter().any(|o| o.operator == "Do"));
    let has_image = pdf.objects.values().any(|o| {
        o.as_stream().is_ok_and(|s| {
            matches!(
                s.dict.get(b"Subtype").and_then(|v| v.as_name()),
                Ok(b"Image")
            )
        })
    });
    assert!(has_image);
    // Sin imágenes.
    let out = export_pdf(
        &doc,
        None,
        &PdfOptions {
            images: false,
            ..PdfOptions::default()
        },
    )
    .unwrap();
    let (_, ops, _) = parse_pdf(&out.bytes);
    assert!(!ops.iter().any(|o| o.operator == "Do"));
    std::fs::remove_file(path).unwrap();
}

#[test]
fn documento_vacio_es_error() {
    let doc = Document::default();
    assert!(export_svg(&doc, None, &SvgOptions::default()).is_err());
    assert!(export_dxf(&doc, None, &DxfOptions::default()).is_err());
    assert!(export_png(&doc, None, &PngOptions::default()).is_err());
    assert!(export_pdf(&doc, None, &PdfOptions::default()).is_err());
}

#[test]
fn pdf_ida_y_vuelta_con_el_importador_de_pdf() {
    let doc = sample();
    let out = export_pdf(
        &doc,
        None,
        &PdfOptions {
            margin_mm: 0.0,
            ..PdfOptions::default()
        },
    )
    .unwrap();
    let path = std::env::temp_dir().join(format!("tlaku-export-{}-ida.pdf", std::process::id()));
    std::fs::write(&path, &out.bytes).unwrap();
    let imported = tlaku_pipeline::import_file(&path).unwrap();
    std::fs::remove_file(&path).unwrap();
    assert!(imported.warnings.is_empty(), "{:?}", imported.warnings);
    let b = doc.bounds().unwrap();
    for color in [RED, BLUE] {
        let got: Vec<BezPath> = imported
            .objects
            .iter()
            .filter(|o| o.layer == color)
            .flat_map(|o| o.curves())
            .collect();
        let want = bounds_of(&doc_layer_paths(&doc, color));
        // La página empieza en la esquina inferior izquierda del diseño.
        let want = (
            want.0 - b.min.x,
            want.1 - b.min.y,
            want.2 - b.min.x,
            want.3 - b.min.y,
        );
        // Los números del PDF llevan 4 decimales en puntos.
        assert_close(bounds_of(&got), want, 2e-3);
    }
}
