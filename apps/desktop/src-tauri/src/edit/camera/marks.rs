//! Marcas grabadas para alinear la cámara con la cama.

use super::*;

// ---------------------------------------------------------------------------

/// Color de la capa de las marcas (azul, que el diseño del usuario rara vez usa).
pub(super) const MARKS_COLOR: Rgb = Rgb(0, 102, 255);
/// Velocidad (mm/min) y potencia (%) por defecto de las marcas: un trazo tenue que no corta.
pub(super) const MARKS_SPEED: f64 = 3000.0;
pub(super) const MARKS_POWER: f64 = 10.0;

pub(super) fn marks_layout(bed: [f64; 2], count: u32) -> Result<MarkLayout, String> {
    MarkLayout::new(bed, count as usize)
}

/// Documento con 4 o 9 cruces en posiciones conocidas de la cama (coordenadas absolutas),
/// en una capa `line` de baja potencia. La marca de la esquina inferior izquierda lleva
/// además un círculo para fijar la orientación en la detección.
pub fn build_marks_document(bed: [f64; 2], count: u32) -> Result<Document, String> {
    let layout = marks_layout(bed, count)?;
    let mut layer = LayerSettings::for_color(MARKS_COLOR);
    layer.name = tr!("desktop.edit.camera.marks_layer");
    layer.mode = LayerMode::Line;
    layer.speed = MARKS_SPEED;
    layer.power = MARKS_POWER;
    layer.passes = 1;
    let mut objects: Vec<DesignObject> = Vec::new();
    for (i, _) in layout.marks.iter().enumerate() {
        let paths: Vec<BezPath> = layout
            .strokes()
            .into_iter()
            .filter(|st| st.mark == i)
            .map(|st| {
                BezPath::from(&Polyline {
                    points: st.points,
                    closed: st.closed,
                })
            })
            .collect();
        objects.push(DesignObject {
            id: format!("mark-{}", i + 1),
            layer: MARKS_COLOR,
            transform: IDENTITY,
            kind: ObjectKind::Vector(VectorObject::new(paths)),
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
    }
    Ok(Document {
        version: DOC_VERSION,
        objects,
        layers: vec![layer],
        device_id: None,
        notes: String::new(),
        guides: Vec::new(),
        variable_text: Default::default(),
    })
}

/// Busca las marcas en una foto ya corregida de lente, las asigna a sus posiciones y ajusta la
/// alineación. `prior_to_bed` es una alineación provisional (imagen a mm) que acelera y afianza
/// la asignación; sin ella, el círculo de la marca de origen fija la orientación. Las
/// coordenadas de píxel que entran y salen tienen el origen en el borde de la imagen (la
/// esquina superior izquierda es `(0, 0)`), como el resto de la alineación.
pub fn run_detect_marks(
    image_b64: &str,
    bed: [f64; 2],
    count: u32,
    prior_to_bed: Option<[[f64; 3]; 3]>,
) -> Result<MarksReport, String> {
    let layout = marks_layout(bed, count)?;
    let img = decode_gray(image_b64)?;
    let prior = match prior_to_bed {
        Some(m) if m.iter().flatten().all(|v| v.is_finite()) => Some(Homography { m }),
        _ => None,
    };
    let opts = DetectOptions {
        prior_to_bed: prior,
        pixel_offset: 0.5,
        ..DetectOptions::default()
    };
    detect_marks(&img, &layout, &opts)
}

#[tauri::command(rename_all = "snake_case")]
pub async fn camera_marks(bed: [f64; 2], count: u32) -> Result<Document, String> {
    tauri::async_runtime::spawn_blocking(move || build_marks_document(bed, count))
        .await
        .map_err(crate::edit::task_interrupted)?
}

#[tauri::command(rename_all = "snake_case")]
pub async fn detect_camera_marks(
    image_png_base64: String,
    bed: [f64; 2],
    count: u32,
    prior_to_bed: Option<[[f64; 3]; 3]>,
) -> Result<MarksReport, String> {
    tauri::async_runtime::spawn_blocking(move || {
        run_detect_marks(&image_png_base64, bed, count, prior_to_bed)
    })
    .await
    .map_err(crate::edit::task_interrupted)?
}
