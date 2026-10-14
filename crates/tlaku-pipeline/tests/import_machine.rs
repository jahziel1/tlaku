//! Importar los formatos de máquina que Tlaku también escribe (`.rd` de Ruida): ida y vuelta con
//! archivos generados por el propio Tlaku, de principio a fin (diseño, `compute`, `.rd` en disco,
//! `import_file`, `compute` otra vez).

use tlaku_core::bezpath::BezPath;
use tlaku_core::toolpath::Move;
use tlaku_core::{Point, Polyline, Rgb};
use tlaku_doc::affine::IDENTITY;
use tlaku_doc::{
    DesignObject, DeviceProfile, Document, JobOptions, LayerSettings, ObjectKind, Origin,
    VectorObject,
};
use tlaku_pipeline::{compute, export_rd_for_device, import_file};
use tlaku_ruida::rd::from_swizzled;
use tlaku_ruida::swizzle::Magic;

const RED: Rgb = Rgb(255, 0, 0);
const BLUE: Rgb = Rgb(0, 0, 255);

fn shape(id: &str, layer: Rgb, closed: bool, pts: &[(f64, f64)]) -> DesignObject {
    DesignObject {
        id: id.into(),
        layer,
        transform: IDENTITY,
        kind: ObjectKind::Vector(VectorObject::new(vec![BezPath::from(&Polyline {
            points: pts.iter().map(|&(x, y)| Point::new(x, y)).collect(),
            closed,
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

fn layer(color: Rgb, speed: f64, power: f64) -> LayerSettings {
    LayerSettings {
        speed,
        power,
        ..LayerSettings::for_color(color)
    }
}

/// Longitud (mm) de los cortes con potencia del trabajo, por bloque.
fn cut_lengths(job: &tlaku_pipeline::ComputedJob) -> Vec<f64> {
    job.toolpath
        .blocks
        .iter()
        .map(|b| {
            let mut pos = Point::new(0.0, 0.0);
            let mut len = 0.0;
            for m in &b.moves {
                if let Move::Cut { to, power_pct } = *m
                    && power_pct > 0.0
                {
                    len += pos.dist(to);
                }
                pos = m.to();
            }
            len
        })
        .collect()
}

#[test]
fn a_ruida_file_written_by_tlaku_comes_back_with_the_same_cuts() {
    let device = DeviceProfile {
        origin: Origin::TopLeft,
        ..DeviceProfile::default()
    };
    let doc = Document {
        objects: vec![
            shape(
                "a",
                RED,
                true,
                &[(20.0, 30.0), (70.0, 30.0), (70.0, 60.0), (20.0, 60.0)],
            ),
            shape(
                "b",
                BLUE,
                false,
                &[(100.0, 20.0), (140.0, 55.0), (180.0, 20.0)],
            ),
        ],
        layers: vec![layer(RED, 900.0, 55.0), layer(BLUE, 2400.0, 30.0)],
        ..Document::default()
    };
    let job = compute(&doc, &device, &JobOptions::default()).unwrap();
    let plain = export_rd_for_device(&job, &device).unwrap();
    let path = std::env::temp_dir().join(format!("tlaku-import-rd-{}.rd", std::process::id()));
    std::fs::write(&path, Magic::default().swizzle_all(&plain)).unwrap();
    // El archivo en disco está ofuscado: el flujo en claro no se puede leer sin la clave.
    assert_eq!(
        from_swizzled(&std::fs::read(&path).unwrap(), Magic::default()),
        plain
    );

    let imported = import_file(&path).unwrap();
    std::fs::remove_file(&path).unwrap();
    assert_eq!(imported.objects.len(), 2);
    assert_eq!(imported.new_layers.len(), 2);
    let again = Document {
        objects: imported.objects,
        layers: imported.new_layers,
        ..Document::default()
    };
    let job2 = compute(&again, &device, &JobOptions::default()).unwrap();
    let (a, b) = (cut_lengths(&job), cut_lengths(&job2));
    assert_eq!(a.len(), b.len());
    for (x, y) in a.iter().zip(&b) {
        assert!((x - y).abs() < 0.05, "{a:?} contra {b:?}");
    }
    // Los ajustes de cada capa sobreviven.
    let speeds: Vec<f64> = job2
        .toolpath
        .blocks
        .iter()
        .map(|b| b.speed_mm_min)
        .collect();
    assert!(
        (speeds[0] - 900.0).abs() < 0.1 && (speeds[1] - 2400.0).abs() < 0.1,
        "{speeds:?}"
    );
    for (l, want) in again.layers.iter().zip([55.0, 30.0]) {
        assert!((l.power - want).abs() < 0.01, "{}", l.power);
    }
    // Y la geometría ocupa lo mismo y en el mismo sitio.
    let (b1, b2) = (doc.bounds().unwrap(), again.bounds().unwrap());
    for (p, q) in [(b1.min, b2.min), (b1.max, b2.max)] {
        assert!(p.dist(q) < 0.01, "{p:?} contra {q:?}");
    }
}
