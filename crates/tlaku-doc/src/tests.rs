use super::*;
use std::f64::consts::FRAC_PI_2;
use tlaku_core::bezpath::{BezPath, PathSeg};
use tlaku_core::{Point, Polyline, Rgb};

fn rect_path(w: f64, h: f64) -> BezPath {
    BezPath::from(&Polyline {
        points: vec![
            Point::new(0.0, 0.0),
            Point::new(w, 0.0),
            Point::new(w, h),
            Point::new(0.0, h),
        ],
        closed: true,
    })
}

fn square(id: &str, layer: Rgb, transform: affine::Transform) -> DesignObject {
    DesignObject {
        id: id.into(),
        layer,
        transform,
        kind: ObjectKind::Vector(VectorObject::new(vec![rect_path(10.0, 5.0)])),
        locked: false,
        hidden: false,
        group: None,
        priority: None,
        power_scale: None,
        shape: None,
        start: None,
        manual_tabs: Vec::new(),
        array: None,
    }
}

fn sample() -> Document {
    let red = Rgb(255, 0, 0);
    let image = DesignObject {
        id: "img".into(),
        layer: LayerSettings::IMAGE_COLOR,
        transform: affine::translate(3.0, 4.0),
        kind: ObjectKind::Image(ImageObject {
            source_path: "/tmp/foto.png".into(),
            width_px: 100,
            height_px: 50,
            size_mm: [20.0, 10.0],
            mask: Vec::new(),
        }),
        locked: true,
        hidden: false,
        group: Some("g1".into()),
        priority: None,
        power_scale: None,
        shape: None,
        start: None,
        manual_tabs: Vec::new(),
        array: None,
    };
    Document {
        version: DOC_VERSION,
        objects: vec![square("a", red, affine::IDENTITY), image],
        layers: vec![
            LayerSettings {
                mode: LayerMode::Fill,
                overscan: Overscan::Mm(1.5),
                order: Some(2),
                ..LayerSettings::for_color(red)
            },
            LayerSettings::for_images(),
        ],
        device_id: Some("falcon".into()),
        notes: String::new(),
        guides: Vec::new(),
        variable_text: Default::default(),
    }
}

#[test]
fn json_roundtrip_keeps_everything() {
    let doc = sample();
    let text = doc.to_json().unwrap();
    assert_eq!(Document::from_json(&text).unwrap(), doc);
}

#[test]
fn json_uses_the_contract_shape() {
    let v: serde_json::Value = serde_json::from_str(&sample().to_json().unwrap()).unwrap();
    assert_eq!(v["version"], 2);
    assert_eq!(v["objects"][0]["layer"], "#ff0000");
    assert_eq!(v["objects"][0]["kind"]["type"], "vector");
    assert_eq!(v["objects"][0]["kind"]["paths"][0][0]["type"], "move_to");
    assert_eq!(v["objects"][0]["kind"]["paths"][0][1]["to"][0], 10.0);
    assert_eq!(v["objects"][0]["kind"]["paths"][0][4]["type"], "close");
    assert!(v["objects"][0]["group"].is_null());
    assert_eq!(v["objects"][1]["group"], "g1");
    assert_eq!(v["objects"][1]["kind"]["type"], "image");
    assert_eq!(v["objects"][1]["kind"]["size_mm"][0], 20.0);
    assert_eq!(v["layers"][0]["mode"], "fill");
    assert_eq!(v["layers"][0]["overscan"], 1.5);
    assert_eq!(v["layers"][1]["overscan"], "auto");
    assert_eq!(v["layers"][1]["image"]["mode"], "stucki");
    assert_eq!(v["layers"][0]["order"], 2);
    assert!(v["layers"][1]["order"].is_null());
    assert_eq!(v["device_id"], "falcon");
}

#[test]
fn unknown_or_missing_version_is_rejected() {
    let mut v: serde_json::Value = serde_json::from_str(&sample().to_json().unwrap()).unwrap();
    v["version"] = 3.into();
    let e = Document::from_json(&v.to_string()).unwrap_err();
    assert!(e.contains("versión de documento no soportada: 3"), "{e}");
    v["version"] = 0.into();
    assert!(Document::from_json(&v.to_string()).is_err());
    v.as_object_mut().unwrap().remove("version");
    assert!(
        Document::from_json(&v.to_string())
            .unwrap_err()
            .contains("falta")
    );
    assert!(
        Document::from_json("{")
            .unwrap_err()
            .contains("documento inválido")
    );
    assert!(
        Document::from_json(r#"{"version":2,"objects":[]}"#)
            .unwrap_err()
            .contains("layers")
    );
}

#[test]
fn overscan_accepts_auto_and_numbers_only() {
    assert_eq!(
        serde_json::from_str::<Overscan>("\"auto\"").unwrap(),
        Overscan::Auto
    );
    assert_eq!(
        serde_json::from_str::<Overscan>("2").unwrap(),
        Overscan::Mm(2.0)
    );
    assert!(serde_json::from_str::<Overscan>("\"mucho\"").is_err());
    assert!(serde_json::from_str::<Overscan>("null").is_err());
}

#[test]
fn rgb_serializes_as_hex_text() {
    assert_eq!(
        serde_json::to_string(&Rgb(255, 128, 0)).unwrap(),
        "\"#ff8000\""
    );
    assert_eq!(
        serde_json::from_str::<Rgb>("\"#FF8000\"").unwrap(),
        Rgb(255, 128, 0)
    );
    assert!(serde_json::from_str::<Rgb>("\"rojo\"").is_err());
}

fn close(a: Point, x: f64, y: f64) {
    assert!(
        (a.x - x).abs() < 1e-9 && (a.y - y).abs() < 1e-9,
        "{a:?} != ({x}, {y})"
    );
}

#[test]
fn transform_is_applied_to_every_point() {
    let t = affine::compose(&affine::translate(100.0, 50.0), &affine::scale(2.0, 3.0));
    let obj = square("a", Rgb(0, 0, 0), t);
    let p = &obj.polylines()[0];
    assert!(p.closed);
    close(p.points[0], 100.0, 50.0);
    close(p.points[1], 120.0, 50.0);
    close(p.points[2], 120.0, 65.0);
    close(p.points[3], 100.0, 65.0);
}

#[test]
fn rotation_is_counter_clockwise_with_y_up() {
    let obj = square("a", Rgb(0, 0, 0), affine::rotate(FRAC_PI_2));
    let p = &obj.polylines()[0];
    // (10, 0) -> (0, 10)
    close(p.points[1], 0.0, 10.0);
    // (0, 5) -> (-5, 0)
    close(p.points[3], -5.0, 0.0);
}

#[test]
fn compose_applies_inner_first() {
    let t = affine::compose(&affine::translate(1.0, 0.0), &affine::scale(2.0, 2.0));
    assert_eq!(affine::apply(&t, [3.0, 4.0]), [7.0, 8.0]);
    let id = affine::compose(&affine::IDENTITY, &affine::IDENTITY);
    assert!(affine::is_identity(&id));
}

#[test]
fn image_polyline_is_its_placed_box() {
    let doc = sample();
    let img = doc.object("img").unwrap();
    let b = img.bounds().unwrap();
    close(b.min, 3.0, 4.0);
    close(b.max, 23.0, 14.0);
    // Reflejada en X: la caja ocupa el mismo ancho hacia la izquierda del punto de anclaje.
    let mut flipped = img.clone();
    flipped.transform = affine::compose(&affine::translate(30.0, 0.0), &affine::scale(-1.0, 1.0));
    let b = flipped.bounds().unwrap();
    close(b.min, 10.0, 0.0);
    close(b.max, 30.0, 10.0);
}

#[test]
fn bounds_of_object_and_document() {
    let doc = sample();
    let b = doc.object("a").unwrap().bounds().unwrap();
    close(b.min, 0.0, 0.0);
    close(b.max, 10.0, 5.0);
    let all = doc.bounds().unwrap();
    close(all.min, 0.0, 0.0);
    close(all.max, 23.0, 14.0);
    assert!(Document::default().bounds().is_none());
    let empty = DesignObject {
        kind: ObjectKind::Vector(VectorObject::new(vec![])),
        ..square("e", Rgb(0, 0, 0), affine::IDENTITY)
    };
    assert!(empty.bounds().is_none());
}

#[test]
fn defaults_match_the_engine_defaults() {
    let l = LayerSettings::for_color(Rgb(255, 0, 0));
    assert_eq!(l.name, "#ff0000");
    assert_eq!(l.mode, LayerMode::Line);
    assert!(l.output && l.visible);
    assert_eq!((l.speed, l.power, l.passes), (1000.0, 20.0, 1));
    assert_eq!(l.overscan, Overscan::Auto);
    assert_eq!(LayerSettings::for_images().speed, 3000.0);
    assert_eq!(LayerSettings::for_images().mode, LayerMode::Image);
    assert_eq!(LayerSettings::for_images().name, "Imagen");
    let en = tlaku_i18n::with_locale(tlaku_i18n::Locale::En, || LayerSettings::for_images().name);
    assert_eq!(en, "Image");
    let d = DeviceProfile::default();
    assert_eq!(d.bed, [400.0, 415.0]);
    assert_eq!(d.kind, DeviceKind::Grbl);
    assert_eq!(d.origin, Origin::BottomLeft);
    let j = JobOptions::default();
    assert_eq!(j.start_from, StartFrom::Absolute);
    assert!(j.selection.is_none());
}

#[test]
fn device_and_options_roundtrip() {
    let d = DeviceProfile::default();
    let v: serde_json::Value = serde_json::to_value(&d).unwrap();
    assert_eq!(v["kind"], "grbl");
    assert_eq!(v["origin"], "bottom_left");
    assert_eq!(v["power_mode"], "dynamic");
    assert_eq!(serde_json::from_value::<DeviceProfile>(v).unwrap(), d);
    let o = JobOptions {
        start_from: StartFrom::CurrentPosition,
        user_origin: Some([1.0, 2.0]),
        selection: Some(vec!["a".into()]),
        ..Default::default()
    };
    let v = serde_json::to_value(&o).unwrap();
    assert_eq!(v["start_from"], "current_position");
    assert_eq!(v["layer_order"], "auto");
    assert_eq!(serde_json::from_value::<JobOptions>(v).unwrap(), o);
}

#[test]
fn preview_and_import_results_serialize() {
    let preview = JobPreview {
        layers: vec![PreviewLayer {
            color: Rgb(0, 0, 255),
            mode: "fill".into(),
            seconds: 1.5,
            summary: "3 líneas".into(),
            travel_saved_pct: None,
            explanations: vec![],
        }],
        moves: vec![0.0, 1.0, 2.0, 0.0, 1.0, 3.0, 4.0, 50.0],
        move_times: vec![0.5, 2.0],
        total_s: 2.0,
        bounds: [[0.0, 0.0], [10.0, 5.0]],
        order_note: None,
        warnings: vec!["aviso".into()],
    };
    let v = serde_json::to_value(&preview).unwrap();
    assert_eq!(v["layers"][0]["color"], "#0000ff");
    assert!(v["layers"][0]["travel_saved_pct"].is_null());
    assert_eq!(v["moves"].as_array().unwrap().len(), 8);
    assert_eq!(v["move_times"].as_array().unwrap().len(), 2);
    assert_eq!(serde_json::from_value::<JobPreview>(v).unwrap(), preview);
    let imported = ImportResult {
        objects: sample().objects,
        new_layers: sample().layers,
        warnings: vec![],
    };
    let text = serde_json::to_string(&imported).unwrap();
    assert_eq!(
        serde_json::from_str::<ImportResult>(&text).unwrap(),
        imported
    );
}

#[test]
fn version_1_documents_are_migrated() {
    let v1 = r##"{
        "version": 1,
        "objects": [
            {"id":"a","layer":"#ff0000","transform":[1,0,0,1,0,0],"locked":false,"hidden":false,
             "kind":{"type":"vector","paths":[
                {"points":[[0,0],[10,0],[10,5]],"closed":true},
                {"points":[[1,1],[2,2]],"closed":false},
                {"points":[],"closed":false}]}},
            {"id":"i","layer":"#808080","transform":[1,0,0,1,0,0],"locked":false,"hidden":true,
             "kind":{"type":"image","source_path":"/x.png","width_px":2,"height_px":2,"size_mm":[1,1]}}
        ],
        "layers": [],
        "device_id": null
    }"##;
    let doc = Document::from_json(v1).unwrap();
    assert_eq!(doc.version, DOC_VERSION);
    assert!(doc.objects.iter().all(|o| o.group.is_none()));
    let ObjectKind::Vector(v) = &doc.objects[0].kind else {
        panic!("se esperaba un vector");
    };
    assert_eq!(v.paths[0], triangle());
    assert_eq!(v.paths[1].0.len(), 2);
    assert!(matches!(v.paths[1].0[1], PathSeg::LineTo { .. }));
    assert!(v.paths[2].0.is_empty());
    // Sin pérdida: aplanar da las mismas polilíneas que había.
    let lines = doc.objects[0].polylines();
    assert_eq!(lines[0].points.len(), 3);
    assert!(lines[0].closed && !lines[1].closed);
    // Y se vuelve a guardar ya como versión 2.
    let again = Document::from_json(&doc.to_json().unwrap()).unwrap();
    assert_eq!(again, doc);
}

/// El triángulo (0,0) (10,0) (10,5) cerrado, como lo escribe la migración.
fn triangle() -> BezPath {
    BezPath::from(&Polyline {
        points: vec![
            Point::new(0.0, 0.0),
            Point::new(10.0, 0.0),
            Point::new(10.0, 5.0),
        ],
        closed: true,
    })
}

fn circle_object(transform: affine::Transform) -> DesignObject {
    let k = 0.552_284_749_8 * 10.0;
    let p = Point::new;
    let seg = |a, b, c| PathSeg::CubicTo {
        ctrl1: a,
        ctrl2: b,
        to: c,
    };
    DesignObject {
        kind: ObjectKind::Vector(VectorObject::new(vec![BezPath(vec![
            PathSeg::MoveTo { to: p(10.0, 0.0) },
            seg(p(10.0, k), p(k, 10.0), p(0.0, 10.0)),
            seg(p(-k, 10.0), p(-10.0, k), p(-10.0, 0.0)),
            seg(p(-10.0, -k), p(-k, -10.0), p(0.0, -10.0)),
            seg(p(k, -10.0), p(10.0, -k), p(10.0, 0.0)),
            PathSeg::Close,
        ])])),
        ..square("c", Rgb(0, 0, 0), transform)
    }
}

#[test]
fn curve_bounds_are_exact_and_follow_the_transform() {
    let c = circle_object(affine::compose(
        &affine::translate(50.0, 20.0),
        &affine::scale(2.0, 1.0),
    ));
    let b = c.bounds().unwrap();
    close(b.min, 30.0, 10.0);
    close(b.max, 70.0, 30.0);
    // Aplanada: la tolerancia se respeta en mm reales (después de escalar).
    let poly = &c.polylines()[0];
    assert!(poly.closed && poly.points.len() > 16);
    let (cx, cy) = (50.0, 20.0);
    for q in &poly.points {
        let r = ((q.x - cx) / 20.0).hypot((q.y - cy) / 10.0);
        assert!((r - 1.0).abs() < 0.01, "{q:?}");
    }
}

#[test]
fn text_object_roundtrips_and_uses_its_outline() {
    let text = DesignObject {
        id: "t".into(),
        layer: Rgb(0, 0, 255),
        transform: affine::translate(5.0, 5.0),
        kind: ObjectKind::Text(TextObject {
            text: "Hola\nmundo".into(),
            font_family: "Arial".into(),
            bold: true,
            italic: false,
            size_mm: 10.0,
            letter_spacing_mm: 0.5,
            line_spacing: 1.2,
            align: TextAlign::Center,
            on_path: Some(TextOnPath {
                object_id: "a".into(),
                offset_mm: 2.0,
                flip: true,
            }),
            bend_deg: None,
            welded: None,
            features: None,
            variations: None,
            uppercase: None,
            valign: None,
            word_spacing_mm: None,
            outline: vec![rect_path(4.0, 6.0)],
        }),
        locked: false,
        hidden: false,
        group: None,
        priority: None,
        power_scale: None,
        shape: None,
        start: None,
        manual_tabs: Vec::new(),
        array: None,
    };
    let v = serde_json::to_value(&text).unwrap();
    assert_eq!(v["kind"]["type"], "text");
    assert_eq!(v["kind"]["align"], "center");
    assert_eq!(v["kind"]["on_path"]["object_id"], "a");
    assert_eq!(v["kind"]["outline"][0][0]["type"], "move_to");
    assert_eq!(serde_json::from_value::<DesignObject>(v).unwrap(), text);
    let b = text.bounds().unwrap();
    close(b.min, 5.0, 5.0);
    close(b.max, 9.0, 11.0);
    // Sin contorno generado no hay geometría.
    let mut empty = text.clone();
    if let ObjectKind::Text(t) = &mut empty.kind {
        t.outline.clear();
    }
    assert!(empty.polylines().is_empty() && empty.bounds().is_none());
}

fn near(a: [f64; 2], b: [f64; 2]) {
    assert!(
        (a[0] - b[0]).abs() < 1e-9 && (a[1] - b[1]).abs() < 1e-9,
        "{a:?} != {b:?}"
    );
}

fn with_array(array: VirtualArray) -> DesignObject {
    DesignObject {
        array: Some(array),
        ..square("a", Rgb(255, 0, 0), affine::translate(5.0, 5.0))
    }
}

#[test]
fn without_array_there_are_no_copies() {
    let o = square("a", Rgb(255, 0, 0), affine::IDENTITY);
    assert!(o.virtual_copies().unwrap().is_empty());
    assert_eq!(o.bounds_with_copies(), o.bounds());
}

#[test]
fn grid_steps_from_edge_to_edge() {
    // Caja de 10 x 5 mm: paso en X = 10 + 2, paso en Y = 5 + 4.
    let o = with_array(VirtualArray::Grid {
        cols: 3,
        rows: 2,
        dx: 2.0,
        dy: 4.0,
    });
    let t = o.virtual_copies().unwrap();
    // 3 x 2 = 6 piezas: 5 copias, la original no se cuenta.
    assert_eq!(t.len(), 5);
    near(affine::apply(&t[0], [0.0, 0.0]), [12.0, 0.0]);
    near(affine::apply(&t[1], [0.0, 0.0]), [24.0, 0.0]);
    near(affine::apply(&t[2], [0.0, 0.0]), [0.0, 9.0]);
    near(affine::apply(&t[4], [0.0, 0.0]), [24.0, 9.0]);
}

#[test]
fn circular_full_turn_does_not_repeat_the_original() {
    let o = with_array(VirtualArray::Circular {
        count: 4,
        center: [0.0, 0.0],
        sweep_deg: 360.0,
        rotate: true,
    });
    let t = o.virtual_copies().unwrap();
    assert_eq!(t.len(), 3);
    // Cuartos de vuelta antihorarios alrededor del origen.
    near(affine::apply(&t[0], [10.0, 0.0]), [0.0, 10.0]);
    near(affine::apply(&t[1], [10.0, 0.0]), [-10.0, 0.0]);
    near(affine::apply(&t[2], [10.0, 0.0]), [0.0, -10.0]);
}

#[test]
fn circular_arc_includes_both_ends_and_can_keep_orientation() {
    let o = with_array(VirtualArray::Circular {
        count: 3,
        center: [0.0, 0.0],
        sweep_deg: 180.0,
        rotate: false,
    });
    let t = o.virtual_copies().unwrap();
    assert_eq!(t.len(), 2);
    // El centro de la caja (10, 7.5) recorre media vuelta; la orientación no cambia.
    near(affine::apply(&t[1], [10.0, 7.5]), [-10.0, -7.5]);
    for m in &t {
        assert_eq!(&m[..4], &[1.0, 0.0, 0.0, 1.0]);
    }
}

#[test]
fn invalid_arrays_are_reported() {
    let mut o = with_array(VirtualArray::Grid {
        cols: 0,
        rows: 2,
        dx: 0.0,
        dy: 0.0,
    });
    assert!(o.virtual_copies().unwrap_err().contains("al menos 1"));
    o.array = Some(VirtualArray::Grid {
        cols: 100,
        rows: 100,
        dx: 0.0,
        dy: 0.0,
    });
    assert!(o.virtual_copies().unwrap_err().contains("más de"));
    o.array = Some(VirtualArray::Circular {
        count: 3,
        center: [f64::NAN, 0.0],
        sweep_deg: 360.0,
        rotate: true,
    });
    assert!(o.virtual_copies().is_err());
    // Un arreglo inválido no cambia la caja.
    assert_eq!(o.bounds_with_copies(), o.bounds());
}

#[test]
fn bounds_include_the_virtual_copies() {
    // Objeto de 10 x 5 mm con la esquina en (5, 5).
    let grid = with_array(VirtualArray::Grid {
        cols: 3,
        rows: 2,
        dx: 2.0,
        dy: 4.0,
    });
    let b = grid.bounds_with_copies().unwrap();
    close(b.min, 5.0, 5.0);
    close(b.max, 5.0 + 10.0 + 2.0 * 12.0, 5.0 + 5.0 + 9.0);
    // La caja propia no cambia.
    close(grid.bounds().unwrap().max, 15.0, 10.0);
    let doc = Document {
        objects: vec![grid],
        ..Document::default()
    };
    close(doc.bounds().unwrap().max, 39.0, 19.0);

    // Circular con giro: el cuarto de vuelta lleva la caja a x negativa.
    let circ = with_array(VirtualArray::Circular {
        count: 4,
        center: [0.0, 0.0],
        sweep_deg: 360.0,
        rotate: true,
    });
    let b = circ.bounds_with_copies().unwrap();
    close(b.min, -15.0, -15.0);
    close(b.max, 15.0, 15.0);
}

#[test]
fn layer_u_offset_defaults_to_zero_and_roundtrips() {
    // Un archivo anterior a la ola 6E no trae `u_offset`: sale 0.
    let mut v: serde_json::Value =
        serde_json::to_value(LayerSettings::for_color(Rgb(255, 0, 0))).unwrap();
    v.as_object_mut().unwrap().remove("u_offset");
    let old: LayerSettings = serde_json::from_value(v).unwrap();
    assert_eq!(old.u_offset, 0.0);
    let layer = LayerSettings {
        u_offset: -3.5,
        ..LayerSettings::for_color(Rgb(0, 0, 255))
    };
    let back: LayerSettings =
        serde_json::from_str(&serde_json::to_string(&layer).unwrap()).unwrap();
    assert_eq!(back.u_offset, -3.5);
}
