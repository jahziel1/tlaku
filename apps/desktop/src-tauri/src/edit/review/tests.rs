use super::*;
use crate::edit::geometry::{GeometryOp, cleanup};
use crate::edit::shape_ops::{self, ShapeOp, ShapeParams};
use tlaku_core::Point;
use tlaku_doc::{BezPath, ImageObject, LayerSettings, TextObject, VectorObject, affine};

const RED: Rgb = Rgb(255, 0, 0);
const GREEN: Rgb = Rgb(0, 255, 0);
const BLUE: Rgb = Rgb(0, 0, 255);
const YELLOW: Rgb = Rgb(255, 255, 0);
const HI_IMG: Rgb = Rgb(128, 128, 128);
const LO_IMG: Rgb = Rgb(129, 128, 128);
const BED: [f64; 2] = [100.0, 100.0];

fn object(id: &str, layer: Rgb, kind: ObjectKind) -> DesignObject {
    DesignObject {
        id: id.into(),
        layer,
        transform: affine::IDENTITY,
        kind,
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

fn poly(id: &str, layer: Rgb, pts: &[(f64, f64)], closed: bool) -> DesignObject {
    let p = Polyline {
        points: pts.iter().map(|&(x, y)| Point::new(x, y)).collect(),
        closed,
    };
    object(
        id,
        layer,
        ObjectKind::Vector(VectorObject {
            paths: vec![BezPath::from(&p)],
        }),
    )
}

fn rect(id: &str, layer: Rgb, x0: f64, y0: f64, x1: f64, y1: f64) -> DesignObject {
    poly(id, layer, &[(x0, y0), (x1, y0), (x1, y1), (x0, y1)], true)
}

fn image(id: &str, layer: Rgb, px: u32, mm: f64) -> DesignObject {
    object(
        id,
        layer,
        ObjectKind::Image(ImageObject {
            source_path: "x.png".into(),
            width_px: px,
            height_px: px,
            size_mm: [mm, mm],
            mask: Vec::new(),
        }),
    )
}

fn layer(color: Rgb, mode: LayerMode) -> LayerSettings {
    LayerSettings {
        mode,
        interval: 0.1,
        ..LayerSettings::for_color(color)
    }
}

/// Un caso de cada tipo. Todo lo de `RED` cae dentro de la cama salvo los dos de «fuera».
fn synthetic() -> Document {
    let mut silent = layer(GREEN, LayerMode::Line);
    silent.output = false;
    let mut hidden = layer(BLUE, LayerMode::Line);
    hidden.visible = false;
    let text = TextObject {
        text: "Hola".into(),
        font_family: "x".into(),
        bold: false,
        italic: false,
        size_mm: 5.0,
        letter_spacing_mm: 0.0,
        line_spacing: 1.0,
        align: tlaku_doc::TextAlign::Left,
        on_path: None,
        bend_deg: None,
        welded: None,
        outline: Vec::new(),
    };
    let mut objects = vec![
        // Abierto con un hueco de 0,3 mm.
        poly(
            "gap",
            RED,
            &[
                (0.0, 0.0),
                (10.0, 0.0),
                (10.0, 10.0),
                (0.0, 10.0),
                (0.0, 0.3),
            ],
            false,
        ),
        // Dos cuadrados idénticos.
        rect("dup-a", RED, 20.0, 0.0, 30.0, 10.0),
        rect("dup-b", RED, 20.0, 0.0, 30.0, 10.0),
        // Dos rectas cuyos extremos distan 0,05 mm.
        poly("join-a", RED, &[(40.0, 0.0), (50.0, 0.0)], false),
        poly("join-b", RED, &[(50.05, 0.0), (60.0, 0.0)], false),
        // Fragmento de 0,1 mm.
        rect("speck", RED, 70.0, 0.0, 70.1, 0.1),
        // Recta con nodos colineales de sobra.
        poly(
            "nodes",
            RED,
            &(0..=10)
                .map(|i| (f64::from(i) * 2.0, 50.0))
                .collect::<Vec<_>>(),
            false,
        ),
        // Parte dentro y parte fuera; y del todo fuera.
        rect("half", RED, 95.0, 20.0, 105.0, 30.0),
        rect("away", RED, 200.0, 20.0, 210.0, 30.0),
        poly(
            "silent",
            GREEN,
            &[(0.0, 80.0), (10.0, 80.0), (10.0, 90.0)],
            false,
        ),
        rect("hid", BLUE, 40.0, 80.0, 50.0, 90.0),
        object("txt", YELLOW, ObjectKind::Text(text)),
        // 3000 px en 10 mm: 7620 ppp contra los 254 que pide la capa.
        image("img-high", HI_IMG, 3000, 10.0),
        // 100 px en 30 mm: 85 ppp.
        image("img-low", LO_IMG, 100, 30.0),
    ];
    // Que ninguna imagen caiga fuera de la cama.
    objects[12].transform = [1.0, 0.0, 0.0, 1.0, 60.0, 60.0];
    objects[13].transform = [1.0, 0.0, 0.0, 1.0, 10.0, 60.0];
    Document {
        objects,
        layers: vec![
            layer(RED, LayerMode::Line),
            silent,
            hidden,
            layer(YELLOW, LayerMode::Line),
            layer(HI_IMG, LayerMode::Image),
            layer(LO_IMG, LayerMode::Image),
        ],
        ..Document::default()
    }
}

fn find(findings: &[Finding], kind: FindingKind) -> Option<&Finding> {
    findings.iter().find(|f| f.kind == kind)
}

fn has(f: &Finding, id: &str) -> bool {
    f.object_ids.iter().any(|x| x == id)
}

#[test]
fn detecta_un_caso_de_cada_tipo() {
    let doc = synthetic();
    let r = review(&doc, BED, &ReviewOptions::default());
    for kind in [
        FindingKind::NearClosed,
        FindingKind::Overlap,
        FindingKind::Joinable,
        FindingKind::Tiny,
        FindingKind::ExtraNodes,
        FindingKind::OutsideAll,
        FindingKind::OutsidePart,
        FindingKind::Text,
        FindingKind::SilentLayer,
        FindingKind::HiddenLayer,
        FindingKind::ImageDpiHigh,
        FindingKind::ImageDpiLow,
    ] {
        assert!(find(&r, kind).is_some(), "falta {kind:?}: {r:#?}");
    }
    assert_eq!(r.len(), 12, "ni un hallazgo de más: {r:#?}");

    let near = find(&r, FindingKind::NearClosed).unwrap();
    assert_eq!(near.object_ids, ["gap"]);
    assert!((near.value - 0.3).abs() < 1e-6);
    assert_eq!(near.tolerance, Some(0.5));

    let dup = find(&r, FindingKind::Overlap).unwrap();
    assert_eq!(dup.object_ids, ["dup-b"], "se conserva el primero");
    assert!(dup.fix_ids.iter().any(|i| i == "dup-a") && dup.fix_ids.iter().any(|i| i == "dup-b"));

    let join = find(&r, FindingKind::Joinable).unwrap();
    assert_eq!(join.object_ids, ["join-a", "join-b"]);
    assert_eq!(join.count, 1);

    assert_eq!(find(&r, FindingKind::Tiny).unwrap().object_ids, ["speck"]);
    assert_eq!(
        find(&r, FindingKind::ExtraNodes).unwrap().object_ids,
        ["nodes"]
    );
    assert_eq!(
        find(&r, FindingKind::OutsideAll).unwrap().object_ids,
        ["away"]
    );
    assert_eq!(
        find(&r, FindingKind::OutsidePart).unwrap().object_ids,
        ["half"]
    );
    assert_eq!(find(&r, FindingKind::Text).unwrap().object_ids, ["txt"]);
    assert_eq!(
        find(&r, FindingKind::SilentLayer).unwrap().object_ids,
        ["silent"]
    );
    assert_eq!(
        find(&r, FindingKind::HiddenLayer).unwrap().object_ids,
        ["hid"]
    );
    let high = find(&r, FindingKind::ImageDpiHigh).unwrap();
    assert_eq!(high.object_ids, ["img-high"]);
    assert!((high.detail - 254.0).abs() < 1e-6 && high.value > 7000.0);
    let low = find(&r, FindingKind::ImageDpiLow).unwrap();
    assert_eq!(low.object_ids, ["img-low"]);
    assert!(low.value < 100.0);

    // Lo grave va primero.
    let first_non_serious = r
        .iter()
        .position(|f| f.severity != Severity::Serious)
        .unwrap();
    assert!(
        r[first_non_serious..]
            .iter()
            .all(|f| f.severity != Severity::Serious)
    );
    // Las claves son únicas y no cambian entre corridas.
    let keys: BTreeSet<&str> = r.iter().map(|f| f.key.as_str()).collect();
    assert_eq!(keys.len(), r.len());
    assert_eq!(r, review(&doc, BED, &ReviewOptions::default()));
}

#[test]
fn un_diseno_limpio_no_da_hallazgos() {
    let doc = Document {
        objects: vec![
            rect("a", RED, 10.0, 10.0, 30.0, 30.0),
            rect("b", RED, 40.0, 10.0, 60.0, 30.0),
        ],
        layers: vec![layer(RED, LayerMode::Line)],
        ..Document::default()
    };
    assert!(review(&doc, BED, &ReviewOptions::default()).is_empty());
    assert!(review(&Document::default(), BED, &ReviewOptions::default()).is_empty());
}

#[test]
fn ocultos_bloqueados_y_guias_no_cuentan() {
    let mut doc = synthetic();
    for o in &mut doc.objects {
        if o.id == "gap" {
            o.locked = true;
        }
        if o.id == "away" {
            o.hidden = true;
        }
    }
    let guide = LayerSettings {
        output: false,
        ..LayerSettings::for_color(GUIDE)
    };
    doc.layers.push(guide);
    doc.objects
        .push(rect("guide", GUIDE, 300.0, 0.0, 310.0, 10.0));
    let r = review(&doc, BED, &ReviewOptions::default());
    assert!(find(&r, FindingKind::NearClosed).is_none(), "bloqueado");
    assert!(find(&r, FindingKind::OutsideAll).is_none(), "oculto");
    assert!(r.iter().all(|f| !has(f, "guide")), "las guías no salen");
}

/// Aplica un arreglo como lo hace la interfaz: quita los ids que no vuelven y mete los nuevos.
fn apply(doc: &Document, sent: &[DesignObject], result: Vec<DesignObject>) -> Document {
    let sent_ids: BTreeSet<&str> = sent.iter().map(|o| o.id.as_str()).collect();
    let returned: BTreeSet<&str> = result.iter().map(|o| o.id.as_str()).collect();
    let fresh: Vec<DesignObject> = result
        .iter()
        .filter(|o| !sent_ids.contains(o.id.as_str()))
        .cloned()
        .collect();
    let mut objects = Vec::new();
    let mut placed = false;
    for o in &doc.objects {
        if sent_ids.contains(o.id.as_str()) && !returned.contains(o.id.as_str()) {
            if !placed {
                objects.extend(fresh.clone());
                placed = true;
            }
        } else {
            objects.push(o.clone());
        }
    }
    if !placed {
        objects.extend(fresh);
    }
    Document {
        objects,
        ..doc.clone()
    }
}

fn pick(doc: &Document, ids: &[String]) -> Vec<DesignObject> {
    doc.objects
        .iter()
        .filter(|o| ids.contains(&o.id))
        .cloned()
        .collect()
}

#[test]
fn cada_arreglo_hace_desaparecer_su_hallazgo() {
    let doc = synthetic();
    let opts = ReviewOptions::default();
    let before = review(&doc, BED, &opts);

    for (kind, op) in [
        (FindingKind::NearClosed, GeometryOp::Close),
        (FindingKind::Overlap, GeometryOp::Dedupe),
        (FindingKind::Joinable, GeometryOp::Join),
    ] {
        let f = find(&before, kind).unwrap();
        let ids = if f.fix_ids.is_empty() {
            &f.object_ids
        } else {
            &f.fix_ids
        };
        let sent = pick(&doc, ids);
        let result = cleanup(op, &sent, f.tolerance).unwrap();
        let fixed = apply(&doc, &sent, result);
        assert_ne!(fixed, doc, "{kind:?}: el arreglo no cambió nada");
        let after = review(&fixed, BED, &opts);
        assert!(find(&after, kind).is_none(), "{kind:?} sigue: {after:#?}");
        assert_eq!(
            after.len(),
            before.len() - 1,
            "{kind:?} no debe tocar los demás"
        );
    }

    let f = find(&before, FindingKind::ExtraNodes).unwrap();
    let sent = pick(&doc, &f.object_ids);
    let params = ShapeParams {
        tolerance: f.tolerance,
        ..ShapeParams::default()
    };
    let result = shape_ops::run(ShapeOp::Optimize, &sent, &params).unwrap();
    let fixed = apply(&doc, &sent, result.objects);
    assert!(find(&review(&fixed, BED, &opts), FindingKind::ExtraNodes).is_none());
}

#[test]
fn la_tolerancia_manda() {
    let doc = synthetic();
    let strict = ReviewOptions {
        close_mm: Some(0.1),
        join_mm: Some(0.01),
        ..ReviewOptions::default()
    };
    let r = review(&doc, BED, &strict);
    assert!(find(&r, FindingKind::NearClosed).is_none());
    assert!(find(&r, FindingKind::Joinable).is_none());
}

#[test]
fn diez_mil_objetos_son_rapidos() {
    let mut objects = Vec::new();
    for i in 0..10_000 {
        let (col, row) = (i % 100, i / 100);
        let (x, y) = (f64::from(col) * 5.0, f64::from(row) * 5.0);
        // Cada cuarta figura es una recta abierta con un hueco pequeño: trabajo de verdad.
        if i % 4 == 0 {
            objects.push(poly(
                &format!("o{i}"),
                RED,
                &[
                    (x, y),
                    (x + 3.0, y),
                    (x + 3.0, y + 3.0),
                    (x, y + 3.0),
                    (x, y + 0.2),
                ],
                false,
            ));
        } else {
            objects.push(rect(&format!("o{i}"), RED, x, y, x + 3.0, y + 3.0));
        }
    }
    let doc = Document {
        objects,
        layers: vec![layer(RED, LayerMode::Line)],
        ..Document::default()
    };
    let t = std::time::Instant::now();
    let r = review(&doc, [600.0, 600.0], &ReviewOptions::default());
    let secs = t.elapsed().as_secs_f64();
    eprintln!("review de 10 000 objetos: {secs:.3} s");
    assert_eq!(
        find(&r, FindingKind::NearClosed).unwrap().object_ids.len(),
        2500
    );
    // En compilación sin optimizar (pruebas) es varias veces más lento que en la app.
    assert!(secs < 5.0, "tardó {secs:.2} s");
}
