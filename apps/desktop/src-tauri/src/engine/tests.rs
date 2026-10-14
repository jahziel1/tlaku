use super::*;
use tlaku_core::bezpath::BezPath;
use tlaku_core::{Point, Polyline, Rgb};
use tlaku_doc::affine::IDENTITY;
use tlaku_doc::{DesignObject, LayerSettings, ObjectKind, VectorObject};

fn temp(test: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("tlaku-desktop-{}-{test}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn square_doc() -> Document {
    let red = Rgb(255, 0, 0);
    Document {
        objects: vec![DesignObject {
            id: "a".into(),
            layer: red,
            transform: IDENTITY,
            kind: ObjectKind::Vector(VectorObject::new(vec![BezPath::from(&Polyline {
                points: vec![
                    Point::new(10.0, 10.0),
                    Point::new(30.0, 10.0),
                    Point::new(30.0, 30.0),
                    Point::new(10.0, 30.0),
                ],
                closed: true,
            })])),
            locked: false,
            hidden: false,
            group: None,
            priority: None,
            power_scale: None,
            shape: None,
            start: None,
            manual_tabs: Vec::new(),
            array: None,
        }],
        layers: vec![LayerSettings::for_color(red)],
        ..Document::default()
    }
}

#[test]
fn compute_job_returns_a_preview_with_move_times() {
    let p = compute_job(
        &square_doc(),
        &DeviceProfile::default(),
        &JobOptions::default(),
        None,
    )
    .unwrap();
    assert!(!p.moves.is_empty());
    assert_eq!(p.move_times.len() * 4, p.moves.len());
    assert!((p.move_times.last().unwrap() - p.total_s).abs() < 1e-9);
}

#[test]
fn current_position_needs_the_head_and_uses_it() {
    let opts = JobOptions {
        start_from: tlaku_doc::StartFrom::CurrentPosition,
        job_origin: 6,
        ..JobOptions::default()
    };
    let d = DeviceProfile::default();
    assert!(compute_job(&square_doc(), &d, &opts, None).is_err());
    let p = compute_job(&square_doc(), &d, &opts, Some([100.0, 50.0])).unwrap();
    assert_eq!(p.bounds[0], [100.0, 50.0]);
}

#[test]
fn a_design_outside_the_bed_is_an_error_with_an_explanation() {
    let d = DeviceProfile {
        bed: [20.0, 20.0],
        ..DeviceProfile::default()
    };
    let e = compute_job(&square_doc(), &d, &JobOptions::default(), None).unwrap_err();
    assert!(e.contains("se sale del área de trabajo"), "{e}");
}

#[test]
fn export_writes_the_gcode_file() {
    let dir = temp("export");
    let path = dir.join("pieza.gcode");
    let r = export_gcode(
        &square_doc(),
        &DeviceProfile::default(),
        &JobOptions::default(),
        path.to_str().unwrap(),
        None,
    )
    .unwrap();
    let text = fs::read_to_string(&path).unwrap();
    assert!(text.contains("G21") && text.contains("G1"), "{text}");
    assert_eq!(
        r.lines as usize,
        text.lines().filter(|l| !l.trim().is_empty()).count()
    );
    assert!(r.estimate_s > 0.0);
    // Sin archivos temporales sobrantes.
    assert_eq!(fs::read_dir(&dir).unwrap().count(), 1);
}

#[test]
fn job_and_frame_lines_are_ready_to_stream() {
    let d = DeviceProfile::default();
    let lines = job_lines(&square_doc(), &d, &JobOptions::default(), None).unwrap();
    assert!(lines.iter().any(|l| l.starts_with("G1")));
    let frame = frame_lines(
        &square_doc(),
        &d,
        &JobOptions::default(),
        3000.0,
        None,
        false,
    )
    .unwrap();
    assert!(frame.contains(&"G0 X10 Y10".to_string()), "{frame:?}");
    assert!(frame.iter().all(|l| !l.contains("S100")));
    assert!(frame_lines(&square_doc(), &d, &JobOptions::default(), 0.0, None, false).is_err());
    // Una línea de inicio que no cabe en el búfer se rechaza antes de enviar.
    let mut small = d.clone();
    small.rx_buffer = 6;
    assert!(job_lines(&square_doc(), &small, &JobOptions::default(), None).is_err());
}

#[test]
fn projects_roundtrip_and_reject_bad_files() {
    let dir = temp("project");
    let path = dir.join("a.tlaku");
    let doc = square_doc();
    save_project(path.to_str().unwrap(), &doc).unwrap();
    assert_eq!(load_project(path.to_str().unwrap()).unwrap(), doc);
    fs::write(&path, r#"{"version": 99}"#).unwrap();
    assert!(
        load_project(path.to_str().unwrap())
            .unwrap_err()
            .contains("versión")
    );
    assert!(load_project(dir.join("no.tlaku").to_str().unwrap()).is_err());
    assert!(save_project(dir.join("no/existe/a.tlaku").to_str().unwrap(), &doc).is_err());
}

#[test]
fn version_1_projects_are_migrated_on_load() {
    let dir = temp("v1");
    let path = dir.join("viejo.tlaku");
    fs::write(
        &path,
        r##"{"version":1,"objects":[{"id":"a","layer":"#ff0000","transform":[1,0,0,1,0,0],
        "locked":false,"hidden":false,"kind":{"type":"vector","paths":[
        {"points":[[10,10],[30,10],[30,30],[10,30]],"closed":true}]}}],
        "layers":[],"device_id":null}"##,
    )
    .unwrap();
    let doc = load_project(path.to_str().unwrap()).unwrap();
    assert_eq!(doc.version, 2);
    assert_eq!(doc.objects, square_doc().objects);
}

#[test]
fn import_makes_the_path_absolute_and_reports_unsupported_files() {
    let dir = temp("import");
    let svg = dir.join("c.svg");
    fs::write(
        &svg,
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="20mm" height="10mm"><rect x="0" y="0" width="20" height="10" stroke="#ff0000" fill="none"/></svg>"##,
    )
    .unwrap();
    let r = import(svg.to_str().unwrap()).unwrap();
    assert!(!r.objects.is_empty() && !r.new_layers.is_empty());
    let e = import(dir.join("x.txt").to_str().unwrap()).unwrap_err();
    assert!(e.contains("no soportado"), "{e}");
}

#[test]
fn devices_default_save_replace_and_delete() {
    let dir = temp("devices").join("config");
    // Sin archivo: el perfil de fábrica, y todavía no se escribe nada.
    let list = list_devices(&dir).unwrap();
    assert_eq!(list, vec![DeviceProfile::default()]);
    assert!(!devices_file(&dir).exists());

    let mut a = DeviceProfile {
        id: "taller".into(),
        name: "Taller".into(),
        ..DeviceProfile::default()
    };
    save_device(&dir, a.clone()).unwrap();
    // La primera vez conserva el de fábrica que ya se mostraba y añade el nuevo.
    assert_eq!(
        list_devices(&dir).unwrap(),
        vec![DeviceProfile::default(), a.clone()]
    );
    delete_device(&dir, &DeviceProfile::default().id).unwrap();
    assert_eq!(list_devices(&dir).unwrap(), vec![a.clone()]);

    a.name = "Taller 2".into();
    save_device(&dir, a.clone()).unwrap();
    let b = DeviceProfile {
        id: "otra".into(),
        ..DeviceProfile::default()
    };
    save_device(&dir, b.clone()).unwrap();
    assert_eq!(list_devices(&dir).unwrap(), vec![a, b.clone()]);

    delete_device(&dir, "taller").unwrap();
    assert_eq!(list_devices(&dir).unwrap(), vec![b]);
    assert!(
        delete_device(&dir, "taller")
            .unwrap_err()
            .contains("No existe")
    );
}

#[test]
fn invalid_profiles_are_rejected_and_a_corrupt_file_is_explained() {
    let dir = temp("devices-bad");
    let d = DeviceProfile {
        bed: [0.0, 100.0],
        ..DeviceProfile::default()
    };
    assert!(
        save_device(&dir, d)
            .unwrap_err()
            .contains("área de trabajo")
    );
    let d = DeviceProfile {
        id: " ".into(),
        ..DeviceProfile::default()
    };
    assert!(save_device(&dir, d).is_err());
    fs::write(devices_file(&dir), "no es json").unwrap();
    assert!(list_devices(&dir).unwrap_err().contains("dañado"));
}

#[test]
fn image_paths_lists_only_images() {
    let mut doc = square_doc();
    assert!(image_paths(&doc).is_empty());
    doc.objects.push(DesignObject {
        id: "img".into(),
        layer: LayerSettings::IMAGE_COLOR,
        transform: IDENTITY,
        kind: ObjectKind::Image(tlaku_doc::ImageObject {
            source_path: "/tmp/foto.png".into(),
            width_px: 4,
            height_px: 4,
            size_mm: [10.0, 10.0],
            mask: Vec::new(),
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
    });
    assert_eq!(image_paths(&doc), vec![PathBuf::from("/tmp/foto.png")]);
}
