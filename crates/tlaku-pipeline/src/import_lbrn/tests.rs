use super::import;
use crate::import::{ImportOptions, import_file, import_file_with};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use tlaku_core::Rgb;
use tlaku_core::bezpath::PathSeg;
use tlaku_doc::affine::translate;
use tlaku_doc::{
    DesignObject, FillStyle, ImageMode, ImportResult, LayerMode, LayerSettings, ObjectKind,
    Overscan,
};

const GUIDE: Rgb = Rgb(0x7e, 0x8a, 0xa0);

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/lbrn")
        .join(name)
}

/// Copia del fixture en una carpeta temporal propia (la importación de imágenes escribe junto al
/// proyecto); se borra al terminar.
struct Temp {
    dir: PathBuf,
    path: PathBuf,
}

impl Temp {
    fn new(name: &str) -> Self {
        static N: AtomicUsize = AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!(
            "tlaku-lbrn-test-{}-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed),
            name.replace('.', "_")
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(name);
        std::fs::copy(fixture(name), &path).unwrap();
        Self { dir, path }
    }
}

impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn load(name: &str) -> ImportResult {
    let t = Temp::new(name);
    import_file(&t.path).unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-6
}

fn assert_bounds(o: &DesignObject, x0: f64, y0: f64, x1: f64, y1: f64) {
    let b = o.bounds().expect("el objeto dibuja algo");
    assert!(
        close(b.min.x, x0) && close(b.min.y, y0) && close(b.max.x, x1) && close(b.max.y, y1),
        "caja ({}, {})-({}, {}), esperada ({x0}, {y0})-({x1}, {y1})",
        b.min.x,
        b.min.y,
        b.max.x,
        b.max.y
    );
}

fn layer<'a>(r: &'a ImportResult, name: &str) -> &'a LayerSettings {
    r.new_layers
        .iter()
        .find(|l| l.name == name)
        .unwrap_or_else(|| panic!("no hay la capa «{name}»"))
}

fn has_warning(r: &ImportResult, needle: &str) -> bool {
    r.warnings.iter().any(|w| w.contains(needle))
}

#[test]
fn rect_and_rounded_rect() {
    let r = load("rect.lbrn2");
    assert_eq!(r.objects.len(), 2);
    assert!(r.warnings.is_empty(), "{:?}", r.warnings);
    // Rect plano de 40 x 20 centrado en (50, 30): mismas coordenadas de cama.
    let plain = &r.objects[0];
    assert_eq!(plain.transform, translate(50.0, 30.0));
    assert_bounds(plain, 30.0, 20.0, 70.0, 40.0);
    assert!(!plain.locked);
    // Con radio de esquina: curvas Bézier y misma caja; viene bloqueado.
    let rounded = &r.objects[1];
    assert_bounds(rounded, 85.0, 65.0, 115.0, 95.0);
    assert!(rounded.locked);
    let ObjectKind::Vector(v) = &rounded.kind else {
        panic!("se esperaba un vector");
    };
    let cubics = v.paths[0]
        .0
        .iter()
        .filter(|s| matches!(s, PathSeg::CubicTo { .. }))
        .count();
    assert_eq!(cubics, 4);
    // Cada objeto en el color de su capa.
    assert_eq!(plain.layer, Rgb(0, 0, 0));
    assert_eq!(rounded.layer, Rgb(0x2f, 0x7b, 0xff));
    assert_eq!(r.new_layers.len(), 2);
    assert_eq!(layer(&r, "C01").speed, 1200.0);
    assert_eq!(layer(&r, "C01").power, 60.0);
}

#[test]
fn ellipse_and_rotated_ellipse() {
    let r = load("ellipse.lbrn2");
    assert_eq!(r.objects.len(), 2);
    assert_bounds(&r.objects[0], 50.0, 35.0, 70.0, 45.0);
    // Girada 90 grados: los radios se intercambian.
    assert_bounds(&r.objects[1], 55.0, 90.0, 65.0, 110.0);
    assert_eq!(r.objects[0].layer, Rgb(0xff, 0x3b, 0x4a));
}

#[test]
fn path_with_bezier_shared_geometry_and_open_polyline() {
    let r = load("path_curves.lbrn2");
    assert_eq!(r.objects.len(), 3, "{:?}", r.warnings);
    assert!(r.warnings.is_empty(), "{:?}", r.warnings);
    let first = &r.objects[0];
    let ObjectKind::Vector(v) = &first.kind else {
        panic!("se esperaba un vector");
    };
    let segs = &v.paths[0].0;
    assert!(matches!(segs[0], PathSeg::MoveTo { .. }));
    assert!(matches!(segs[2], PathSeg::CubicTo { .. }));
    assert!(matches!(segs.last(), Some(PathSeg::Close)));
    // La cúbica (20,0) (30,5) (30,15) (20,20) llega a x = 27.5 (extremo exacto, no el control).
    assert_bounds(first, 10.0, 10.0, 37.5, 30.0);
    // La segunda cita los mismos vértices y primitivas por VertID/PrimID.
    assert_bounds(&r.objects[1], 60.0, 10.0, 87.5, 30.0);
    // `LineOpen`: polilínea abierta por los tres vértices.
    let open = &r.objects[2];
    assert_bounds(open, 0.0, 50.0, 10.0, 60.0);
    let lines = open.polylines();
    assert_eq!(lines.len(), 1);
    assert!(!lines[0].closed);
}

#[test]
fn transformed_nested_groups() {
    let r = load("group_transformed.lbrn2");
    assert_eq!(r.objects.len(), 3);
    // Rect de 10 x 4 en (10, 0), dentro de un grupo girado 90 grados y movido a (100, 0).
    assert_bounds(&r.objects[0], 98.0, 5.0, 102.0, 15.0);
    // Elipse de radio 1 en (3, 0) dentro de un subgrupo con escala 2: centro (100, 6), radio 2.
    assert_bounds(&r.objects[1], 98.0, 4.0, 102.0, 8.0);
    // Los dos objetos comparten grupo (el más externo); el grupo de un solo objeto no agrupa.
    let g = r.objects[0].group.clone().expect("grupo");
    assert_eq!(r.objects[1].group.as_deref(), Some(g.as_str()));
    assert_eq!(r.objects[2].group, None);
    assert_bounds(&r.objects[2], 4.0, 4.0, 6.0, 6.0);
}

#[test]
fn text_with_saved_outline_and_without() {
    let r = load("text.lbrn2");
    assert_eq!(r.objects.len(), 2);
    let ObjectKind::Text(t) = &r.objects[0].kind else {
        panic!("se esperaba un texto");
    };
    assert_eq!(t.text, "Hi");
    assert_eq!(t.font_family, "Arial");
    assert!(!t.bold && !t.italic);
    assert_eq!(t.size_mm, 10.0);
    assert_eq!(t.align, tlaku_doc::TextAlign::Left);
    assert_eq!(t.outline.len(), 1);
    // El contorno guardado queda donde estaba en la cama: la caja de LightBurn era x 21..23,
    // y 30..37 (ancla en (20, 30), vértices de y 0 a 7).
    assert_bounds(&r.objects[0], 21.0, 30.0, 23.0, 37.0);

    let ObjectKind::Text(t2) = &r.objects[1].kind else {
        panic!("se esperaba un texto");
    };
    assert_eq!(t2.text, "Sin contorno");
    assert!(t2.bold && t2.italic, "Font traía peso 75 y estilo 1");
    assert_eq!(t2.size_mm, 6.0);
    assert_eq!(t2.align, tlaku_doc::TextAlign::Center);
    // Con las fuentes del sistema se genera el contorno; sin ellas se avisa.
    assert_eq!(
        t2.outline.is_empty(),
        has_warning(&r, "no trae su contorno")
    );
}

#[test]
fn embedded_bitmap_is_extracted_next_to_the_project() {
    let t = Temp::new("image.lbrn2");
    let r = import_file(&t.path).unwrap();
    assert_eq!(r.objects.len(), 1, "{:?}", r.warnings);
    let o = &r.objects[0];
    let ObjectKind::Image(img) = &o.kind else {
        panic!("se esperaba una imagen");
    };
    assert_eq!((img.width_px, img.height_px), (4, 2));
    assert_eq!(img.size_mm, [20.0, 10.0]);
    // La caja local de Tlaku va de (0,0) a (w,h); LightBurn la centraba: se compensa.
    assert_eq!(o.transform, translate(40.0, 35.0));
    assert_bounds(o, 40.0, 35.0, 60.0, 45.0);
    // Archivo extraído en `<proyecto>_imagenes/`.
    let file = Path::new(&img.source_path);
    assert_eq!(file.parent().unwrap(), t.dir.join("image_imagenes"));
    assert!(file.extension().is_some_and(|e| e == "png"));
    let bytes = std::fs::read(file).unwrap();
    assert_eq!(&bytes[..4], b"\x89PNG");
    // Reimportar reutiliza el mismo archivo.
    let again = import_file(&t.path).unwrap();
    let ObjectKind::Image(img2) = &again.objects[0].kind else {
        panic!()
    };
    assert_eq!(img2.source_path, img.source_path);
    // Capa de imagen con sus ajustes.
    let l = &r.new_layers[0];
    assert_eq!(l.mode, LayerMode::Image);
    assert_eq!((l.power, l.min_power, l.speed), (35.0, 5.0, 3000.0));
    assert_eq!(l.image.mode, ImageMode::Threshold);
    assert!(close(l.interval, 0.1));
    assert_eq!(l.overscan, Overscan::Mm(1.0));
    assert_eq!(o.layer, l.color);
}

#[test]
fn bitmap_falls_back_to_temp_dir_when_project_dir_is_read_only() {
    // El "directorio" del proyecto es un archivo: no se puede crear la carpeta de imágenes.
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("Cargo.toml")
        .join("image.lbrn2");
    let data = std::fs::read(fixture("image.lbrn2")).unwrap();
    let r = import(&path, &data, &ImportOptions::default()).unwrap();
    let ObjectKind::Image(img) = &r.objects[0].kind else {
        panic!("se esperaba una imagen");
    };
    assert!(Path::new(&img.source_path).starts_with(std::env::temp_dir()));
    assert!(has_warning(&r, "No se pudo escribir junto al proyecto"));
    let _ = std::fs::remove_dir_all(Path::new(&img.source_path).parent().unwrap());
}

#[test]
fn three_layers_with_different_settings() {
    let r = load("three_layers.lbrn2");
    assert_eq!(r.objects.len(), 4);
    // Orden de ejecución de LightBurn (prioridad 0, 1, 2) y la capa de herramienta al final.
    let names: Vec<_> = r.new_layers.iter().map(|l| l.name.as_str()).collect();
    assert_eq!(names, ["Grabado", "Marcas", "Corte", "T1"]);

    let cut = layer(&r, "Corte");
    assert_eq!(cut.mode, LayerMode::Line);
    assert_eq!(cut.speed, 600.0); // 10 mm/s
    assert_eq!((cut.power, cut.min_power), (80.0, 10.0));
    assert_eq!(cut.passes, 2);
    assert!(close(cut.kerf, 0.1));
    assert!(cut.air);
    assert_eq!((cut.z_offset, cut.z_step), (1.0, 0.5));
    assert!(cut.output && cut.visible);
    assert_eq!(cut.order, Some(3));
    assert!(cut.tabs.enabled && close(cut.tabs.width, 0.8));
    assert_eq!((cut.tabs.count, cut.tabs.spacing), (6, 0.0));
    assert!(cut.tabs.skip_inner);
    // El perforado ya se traduce (sin los valores del archivo, con los de fábrica).
    assert!(cut.perforation.enabled);
    assert!(!has_warning(&r, "perforado"));

    let fill = layer(&r, "Grabado");
    assert_eq!(fill.mode, LayerMode::Fill);
    assert_eq!(fill.fill_style, FillStyle::Scan);
    assert_eq!(fill.speed, 6000.0); // 100 mm/s
    assert_eq!(fill.power, 25.0);
    assert!(close(fill.interval, 0.15));
    assert_eq!(fill.angle, 45.0);
    assert!(fill.crosshatch && !fill.bidirectional && !fill.air);
    // 2.5 % de 100 mm/s.
    assert_eq!(fill.overscan, Overscan::Mm(2.5));
    assert_eq!(fill.order, Some(1));

    let marks = layer(&r, "Marcas");
    assert!(!marks.output && !marks.visible);
    assert_eq!(marks.order, Some(2));
    assert_eq!(marks.speed, 1800.0);

    // T1 es una guía: no sale a la máquina y no entra en el orden.
    let tool = layer(&r, "T1");
    assert!(!tool.output);
    assert_eq!(tool.color, GUIDE);
    assert_eq!(tool.order, None);
    assert_eq!(r.objects[3].layer, GUIDE);

    // Cada capa con un color distinto y cada objeto en el de su capa.
    let colors: std::collections::HashSet<_> = r.new_layers.iter().map(|l| l.color).collect();
    assert_eq!(colors.len(), 4);
    assert_eq!(r.objects[0].layer, cut.color);
    assert_eq!(r.objects[1].layer, fill.color);
    assert_eq!(r.objects[2].layer, marks.color);
}

#[test]
fn advanced_cut_settings_map_to_tlaku_without_warnings() {
    let r = load("advanced_cut.lbrn2");
    for needle in [
        "no tiene equivalente",
        "perforado",
        "sobrecorte",
        "rampa",
        "modo de puntos",
        "inundado",
        "cut-through",
        "retardos",
    ] {
        assert!(
            !has_warning(&r, needle),
            "aviso inesperado: {needle}: {:?}",
            r.warnings
        );
    }

    let perf = layer(&r, "Perforado");
    assert!(perf.perforation.enabled);
    assert_eq!(
        (perf.perforation.cut_mm, perf.perforation.skip_mm),
        (2.0, 0.5)
    );
    assert_eq!(perf.overcut, 1.5);
    assert_eq!((perf.start_delay_ms, perf.end_delay_ms), (30.0, 10.0));
    assert!(!perf.cut_through.enabled && !perf.dot_mode.enabled && !perf.ramp.enabled);

    let dots = layer(&r, "Puntos");
    assert!(dots.dot_mode.enabled);
    assert_eq!(
        (dots.dot_mode.dwell_ms, dots.dot_mode.spacing_mm),
        (4.0, 0.8)
    );
    assert!(!dots.perforation.enabled);

    // El corte pasante toma su tiempo de la pausa de inicio, que deja de ser pausa.
    let ramp = layer(&r, "Rampa y pasante");
    assert!(ramp.ramp.enabled && ramp.ramp.outer_only);
    assert_eq!(ramp.ramp.length_mm, 3.0);
    assert!(ramp.cut_through.enabled);
    assert_eq!((ramp.cut_through.power, ramp.cut_through.ms), (70.0, 120.0));
    assert_eq!(ramp.start_delay_ms, 0.0);

    let flood = layer(&r, "Inundado");
    assert_eq!(flood.mode, LayerMode::Fill);
    assert_eq!(flood.fill_style, FillStyle::Flood);
}

#[test]
fn repeated_index_becomes_sub_layers() {
    let r = load("advanced_cut.lbrn2");
    // Las dos capas con índice 4 son una sola con una sub-capa.
    let main = layer(&r, "Relleno y contorno");
    assert_eq!(main.mode, LayerMode::Fill);
    assert_eq!(main.sub_layers.len(), 1);
    let sub = &main.sub_layers[0];
    assert_eq!(sub.mode, LayerMode::Line);
    assert_eq!((sub.speed, sub.power, sub.passes), (720.0, 55.0, 2));
    assert_eq!(sub.color, main.color);
    assert!(r.new_layers.iter().all(|l| l.name != "Contorno"));
    assert!(!has_warning(&r, "repetida"));
}

#[test]
fn legacy_lbrn_with_vertex_and_primitive_elements() {
    let r = load("legacy.lbrn");
    assert_eq!(r.objects.len(), 1, "{:?}", r.warnings);
    // Sin `FormatVersion` (lbrn viejo) no hay aviso de versión.
    assert!(r.warnings.is_empty(), "{:?}", r.warnings);
    let o = &r.objects[0];
    let ObjectKind::Vector(v) = &o.kind else {
        panic!("se esperaba un vector");
    };
    assert!(matches!(v.paths[0].0.last(), Some(PathSeg::Close)));
    let b = o.bounds().unwrap();
    assert!(close(b.min.x, 5.0) && close(b.min.y, 5.0) && close(b.max.y, 15.0));
    // La curva se sale del triángulo hacia la derecha.
    assert!(b.max.x > 15.0, "{}", b.max.x);
    assert_eq!(r.new_layers[0].speed, 900.0);
}

#[test]
fn unknown_version_and_unsupported_shapes_are_reported() {
    let r = load("unknown_version.lbrn2");
    assert_eq!(r.objects.len(), 1);
    assert!(has_warning(&r, "FormatVersion 7"), "{:?}", r.warnings);
    assert!(has_warning(&r, "9.9.99"));
    assert!(
        has_warning(&r, "2 formas de tipo «Spline»"),
        "{:?}",
        r.warnings
    );
    assert!(
        has_warning(&r, "1 forma de tipo «Polygon»"),
        "{:?}",
        r.warnings
    );
}

#[test]
fn shape_on_undefined_layer_gets_a_default_one() {
    let xml = br#"<LightBurnProject FormatVersion="1">
        <Shape Type="Rect" CutIndex="3" W="10" H="10" Cr="0"><XForm>1 0 0 1 0 0</XForm></Shape>
    </LightBurnProject>"#;
    let r = import(Path::new("x.lbrn2"), xml, &ImportOptions::default()).unwrap();
    assert_eq!(r.new_layers.len(), 1);
    assert_eq!(r.new_layers[0].name, "C03");
    assert!(has_warning(&r, "La capa C03 no está definida"));
}

#[test]
fn flat_import_bakes_the_transform() {
    let t = Temp::new("rect.lbrn2");
    let opts = ImportOptions {
        keep_curves: false,
        ..ImportOptions::default()
    };
    let r = import_file_with(&t.path, &opts).unwrap();
    assert_eq!(r.objects[1].transform, tlaku_doc::affine::IDENTITY);
    assert_bounds(&r.objects[1], 85.0, 65.0, 115.0, 95.0);
}

#[test]
fn invalid_files_are_rejected() {
    let opts = ImportOptions::default();
    let p = Path::new("x.lbrn2");
    let e = import(p, b"no es xml", &opts).unwrap_err();
    assert!(e.contains("x.lbrn2") && e.contains("XML"), "{e}");
    let e = import(p, b"<svg></svg>", &opts).unwrap_err();
    assert!(e.contains("no es un proyecto de LightBurn"), "{e}");
    let e = import(p, b"<LightBurnProject/>", &opts).unwrap_err();
    assert!(e.contains("no contiene formas ni capas"), "{e}");
    // Solo capas: se importan con un aviso.
    let xml = br#"<LightBurnProject><CutSetting type="Cut"><index Value="0"/></CutSetting></LightBurnProject>"#;
    let r = import(p, xml, &opts).unwrap();
    assert!(r.objects.is_empty() && r.new_layers.len() == 1);
    assert!(has_warning(&r, "solo se importaron las capas"));
}

#[test]
fn dispatched_from_import_file_by_extension() {
    let t = Temp::new("legacy.lbrn");
    assert!(import_file(&t.path).is_ok());
}
