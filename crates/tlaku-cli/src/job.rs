//! Del archivo de diseño a un `ComputedJob`: importa con `tlaku-pipeline`, aplica los ajustes
//! de la línea de comandos a las capas y calcula el trabajo.

use crate::options::{Defaults, ImageParams, LayerOrder, LayerOverride, apply_image, apply_layer};
use std::path::PathBuf;
use tlaku_core::Rgb;
use tlaku_pipeline::doc::affine::translate;
use tlaku_pipeline::doc::{
    DeviceProfile, Document, JobOptions, LayerSettings, ObjectKind, Optimization, Origin,
};
use tlaku_pipeline::{ComputedJob, ImportOptions, InputKind, import_file_with, input_kind};
use tlaku_sim::MachineProfile;

/// Todo lo que el usuario pidió sobre el diseño (sin lo propio de cada subcomando).
#[derive(Debug, Clone)]
pub struct DesignRequest {
    pub input: PathBuf,
    pub defaults: Defaults,
    pub overrides: Vec<(Rgb, LayerOverride)>,
    pub image: ImageParams,
    pub tolerance_mm: f64,
    pub flip_y: bool,
    pub profile: MachineProfile,
    /// Ancho y alto de la cama, en mm.
    pub bed: (f64, f64),
    pub layer_order: LayerOrder,
    /// Ajustes de optimización del orden y de los caminos.
    pub optimization: Optimization,
}

impl DesignRequest {
    /// La máquina descrita por los argumentos; `gcode` completa S máxima, modo y guiones.
    pub fn device(&self) -> DeviceProfile {
        DeviceProfile {
            bed: [self.bed.0, self.bed.1],
            origin: Origin::BottomLeft,
            accel: self.profile.accel_mm_s2,
            max_rate: self.profile.max_rate_mm_min,
            rapid_rate: self.profile.rapid_rate_mm_min,
            ..DeviceProfile::default()
        }
    }
}

/// El documento que describen los argumentos y las advertencias propias de la línea de
/// comandos: `before` va antes de las del motor y `after` después.
struct Prepared {
    doc: Document,
    before: Vec<String>,
    after: Vec<String>,
}

fn prepare(req: &DesignRequest) -> Result<Prepared, String> {
    let kind = input_kind(&req.input)?;
    for (i, (c, _)) in req.overrides.iter().enumerate() {
        if req.overrides[..i].iter().any(|(d, _)| d == c) {
            return Err(format!("la capa {c} aparece más de una vez en --layer"));
        }
    }
    let mut before = Vec::new();
    let mut after = Vec::new();
    if kind != InputKind::Image && !req.image.given.is_empty() {
        before.push(format!(
            "se ignoraron las opciones de imagen ({}) porque la entrada no es una imagen",
            req.image.given.join(", ")
        ));
    }

    let mut imported = import_file_with(
        &req.input,
        &ImportOptions {
            tolerance_mm: req.tolerance_mm,
            flip_y: req.flip_y,
            // El CLI aplana al importar con --tolerance, como siempre.
            keep_curves: false,
        },
    )?;
    let mut layers: Vec<LayerSettings> = Vec::new();
    if kind == InputKind::Image {
        if !req.overrides.is_empty() {
            before.push("se ignoró --layer: una imagen no tiene capas por color".into());
        }
        let (Some((w, h)), Some((x, y))) = (req.image.size, req.image.pos) else {
            return Err(
                "las imágenes requieren --image-size ANCHOxALTO y --image-pos X,Y (en mm; la posición es la esquina inferior izquierda)"
                    .into(),
            );
        };
        for obj in &mut imported.objects {
            if let ObjectKind::Image(img) = &mut obj.kind {
                img.size_mm = [w, h];
            }
            obj.transform = translate(x, y);
        }
        for l in imported.new_layers {
            layers.push(apply_image(l, &req.defaults, &req.image)?);
        }
    } else {
        for l in imported.new_layers {
            let over = req.overrides.iter().find(|(c, _)| *c == l.color);
            layers.push(apply_layer(l, &req.defaults, over.map(|(_, o)| o))?);
        }
        for (c, _) in &req.overrides {
            if !layers.iter().any(|l| l.color == *c) {
                let list: Vec<String> = layers.iter().map(|l| l.color.to_string()).collect();
                after.push(format!(
                    "la capa {c} de --layer no existe en el diseño (capas: {})",
                    list.join(", ")
                ));
            }
        }
    }
    before.extend(imported.warnings);
    Ok(Prepared {
        doc: Document {
            objects: imported.objects,
            layers,
            ..Document::default()
        },
        before,
        after,
    })
}

/// Importa el diseño y arma el `Toolpath` capa por capa, en el orden pedido
/// (`--layer-order`, `order=N`). No verifica la cama: eso lo hace `check_bounds`.
pub fn build(req: &DesignRequest) -> Result<ComputedJob, String> {
    let Prepared { doc, before, after } = prepare(req)?;
    let options = JobOptions {
        layer_order: req.layer_order.into(),
        optimization: req.optimization.clone(),
        ..JobOptions::default()
    };
    let mut job = tlaku_pipeline::build(&doc, &req.device(), &options, None)?;
    let mut warnings = before;
    warnings.append(&mut job.preview.warnings);
    warnings.extend(after);
    job.preview.warnings = warnings;
    Ok(job)
}
