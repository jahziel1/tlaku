use super::*;
use std::path::{Path, PathBuf};
use tlaku_core::bezpath::BezPath;
use tlaku_core::{Point, Polyline, Rgb};
use tlaku_doc::affine::{self, IDENTITY};
use tlaku_doc::{
    DesignObject, DeviceProfile, Document, ImageObject, LayerMode, LayerSettings, ObjectKind,
    VectorObject,
};

const RED: Rgb = Rgb(255, 0, 0);
const BLUE: Rgb = Rgb(0, 0, 255);

// ---------------------------------------------------------------------------
// Apoyos
// ---------------------------------------------------------------------------

fn rect_object(id: &str, layer: Rgb, x: f64, y: f64, w: f64, h: f64) -> DesignObject {
    DesignObject {
        id: id.into(),
        layer,
        transform: IDENTITY,
        kind: ObjectKind::Vector(VectorObject::new(vec![BezPath::from(&Polyline {
            points: vec![
                Point::new(x, y),
                Point::new(x + w, y),
                Point::new(x + w, y + h),
                Point::new(x, y + h),
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
    }
}

fn object_open(id: &str, layer: Rgb, pts: &[(f64, f64)]) -> DesignObject {
    let mut o = rect_object(id, layer, 0.0, 0.0, 1.0, 1.0);
    o.kind = ObjectKind::Vector(VectorObject::new(vec![BezPath::from(&Polyline {
        points: pts.iter().map(|&(x, y)| Point::new(x, y)).collect(),
        closed: false,
    })]));
    o
}

fn image_object(
    id: &str,
    path: &Path,
    size: [f64; 2],
    transform: affine::Transform,
) -> DesignObject {
    DesignObject {
        id: id.into(),
        layer: LayerSettings::IMAGE_COLOR,
        transform,
        kind: ObjectKind::Image(ImageObject {
            source_path: path.to_str().unwrap().into(),
            width_px: 16,
            height_px: 16,
            size_mm: size,
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
    }
}

fn line_layer(color: Rgb) -> LayerSettings {
    LayerSettings::for_color(color)
}

fn fill_layer(color: Rgb) -> LayerSettings {
    LayerSettings {
        mode: LayerMode::Fill,
        interval: 1.0,
        ..LayerSettings::for_color(color)
    }
}

fn workdir(test: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("tlaku-pipeline-{}-{test}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// BMP de 24 bits; la fila 0 de `pixel` queda arriba.
fn gray_bmp(width: u32, height: u32, pixel: impl Fn(u32, u32) -> u8) -> Vec<u8> {
    let row_bytes = (width * 3).div_ceil(4) * 4;
    let size = 54 + row_bytes * height;
    let mut bmp = b"BM".to_vec();
    bmp.extend(size.to_le_bytes());
    bmp.extend([0u8; 4]);
    bmp.extend(54u32.to_le_bytes());
    bmp.extend(40u32.to_le_bytes());
    bmp.extend(width.to_le_bytes());
    bmp.extend(height.to_le_bytes());
    bmp.extend(1u16.to_le_bytes());
    bmp.extend(24u16.to_le_bytes());
    bmp.extend([0u8; 24]);
    for y in (0..height).rev() {
        let mut row: Vec<u8> = (0..width).flat_map(|x| [pixel(x, y); 3]).collect();
        row.resize(row_bytes as usize, 0);
        bmp.extend(row);
    }
    bmp
}

/// Degradado horizontal: negro a la izquierda, casi blanco a la derecha.
fn write_gradient(dir: &Path) -> PathBuf {
    let path = dir.join("degradado.bmp");
    std::fs::write(&path, gray_bmp(16, 16, |x, _| (x * 16).min(255) as u8)).unwrap();
    path
}

fn doc(objects: Vec<DesignObject>, layers: Vec<LayerSettings>) -> Document {
    Document {
        objects,
        layers,
        ..Document::default()
    }
}

fn device() -> DeviceProfile {
    DeviceProfile::default()
}

fn approx(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-4, "{a} != {b}");
}

fn point(p: Point, x: f64, y: f64) {
    approx(p.x, x);
    approx(p.y, y);
}

fn three_layers(dir: &Path) -> Document {
    let bmp = write_gradient(dir);
    doc(
        vec![
            rect_object("marco", RED, 10.0, 10.0, 40.0, 30.0),
            rect_object("relleno", BLUE, 60.0, 10.0, 20.0, 10.0),
            image_object("foto", &bmp, [8.0, 8.0], affine::translate(100.0, 10.0)),
        ],
        vec![
            LayerSettings {
                power: 100.0,
                passes: 2,
                ..line_layer(RED)
            },
            fill_layer(BLUE),
            LayerSettings::for_images(),
        ],
    )
}

/// Rectángulo de 20x40 con la esquina inferior izquierda en (10, 20).
fn tall_rect() -> Document {
    doc(
        vec![rect_object("a", RED, 10.0, 20.0, 20.0, 40.0)],
        vec![line_layer(RED)],
    )
}

fn tabbed(layer: LayerSettings) -> LayerSettings {
    let mut l = layer;
    l.tabs.enabled = true;
    l.tabs.width = 2.0;
    l.tabs.count = 4;
    l
}

mod bounds;
mod compute;
mod curves;
mod gcode;
mod i18n;
mod images;
mod import;
mod layers;
mod manual;
mod placement;
mod power_scale;
mod rotary;
mod scan_offset;
mod tabs;
mod zaxis;
