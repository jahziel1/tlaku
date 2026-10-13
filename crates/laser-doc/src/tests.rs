use super::*;
use laser_core::{Point, Rgb};
use std::f64::consts::FRAC_PI_2;

fn square(id: &str, layer: Rgb, transform: affine::Transform) -> DesignObject {
    DesignObject {
        id: id.into(),
        layer,
        transform,
        kind: ObjectKind::Vector(VectorObject {
            paths: vec![PathData {
                points: vec![[0.0, 0.0], [10.0, 0.0], [10.0, 5.0], [0.0, 5.0]],
                closed: true,
            }],
        }),
        locked: false,
        hidden: false,
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
        }),
        locked: true,
        hidden: false,
    };
    Document {
        version: FORMAT_VERSION,
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
    assert_eq!(v["version"], 1);
    assert_eq!(v["objects"][0]["layer"], "#ff0000");
    assert_eq!(v["objects"][0]["kind"]["type"], "vector");
    assert_eq!(v["objects"][0]["kind"]["paths"][0]["points"][1][0], 10.0);
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
    v["version"] = 2.into();
    let e = Document::from_json(&v.to_string()).unwrap_err();
    assert!(e.contains("versión de documento no soportada: 2"), "{e}");
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
        Document::from_json(r#"{"version":1,"objects":[]}"#)
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
        kind: ObjectKind::Vector(VectorObject { paths: vec![] }),
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
