//! Exportación a DXF AC1018 (AutoCAD 2004) o AC1015 (2000) en mm. Ver [`crate::DxfOptions`] para las decisiones
//! de formato.

use crate::scene::{Scene, subpaths};
use crate::{DxfCurves, DxfOptions, Export};
use dxf::LwPolylineVertex;
use dxf::entities::{Entity, EntityType, LwPolyline, Spline};
use dxf::enums::{AcadVersion, DrawingUnits, Units};
use dxf::tables::Layer;
use dxf::{Color, Drawing, Point};
use tlaku_core::Rgb;
use tlaku_core::bezpath::{BezPath, PathSeg};
use tlaku_doc::Document;
use tlaku_dxf::aci_to_rgb;

pub fn export_dxf(
    doc: &Document,
    selection: Option<&[String]>,
    opts: &DxfOptions,
) -> Result<Export, String> {
    let mut scene = Scene::collect(doc, selection)?;
    let tolerance = if opts.tolerance_mm.is_finite() && opts.tolerance_mm > 0.0 {
        opts.tolerance_mm
    } else {
        0.005
    };
    let mut drawing = Drawing::new();
    // El color verdadero (código 420) solo existe desde AC1018; sin él basta AC1015.
    drawing.header.version = if opts.true_color {
        AcadVersion::R2004
    } else {
        AcadVersion::R2000
    };
    drawing.header.default_drawing_units = Units::Millimeters;
    drawing.header.drawing_units = DrawingUnits::Metric;

    // Una capa DXF por color de capa.
    let mut names: Vec<(Rgb, String)> = Vec::new();
    for color in scene.layer_colors.clone() {
        // El nombre por defecto de una capa es su color (`#ff0000`): no sirve en DXF.
        let base = scene
            .layer_name(color)
            .filter(|n| *n != color.to_string())
            .map_or_else(|| format!("COLOR_{}", hex(color)), layer_name);
        let mut name = base.clone();
        let mut n = 2;
        while names
            .iter()
            .any(|(_, existing)| existing.eq_ignore_ascii_case(&name))
        {
            name = format!("{base}_{n}");
            n += 1;
        }
        drawing.add_layer(Layer {
            name: name.clone(),
            color: Color::from_index(nearest_aci(color)),
            ..Layer::default()
        });
        names.push((color, name));
    }

    let mut skipped_images = 0usize;
    for item in &scene.items {
        if item.image.is_some() {
            skipped_images += 1;
            continue;
        }
        let layer = &names
            .iter()
            .find(|(c, _)| *c == item.obj.layer)
            .map(|(_, n)| n.clone())
            .unwrap_or_default();
        for path in &item.curves {
            for sub in subpaths(path) {
                let Some(kind) = entity_for(&sub, opts.curves, tolerance) else {
                    continue;
                };
                let mut entity = Entity::new(kind);
                entity.common.layer = layer.clone();
                let aci = nearest_aci(item.obj.layer);
                entity.common.color = Color::from_index(aci);
                let c = item.obj.layer;
                if opts.true_color && aci_to_rgb(aci) != c {
                    entity.common.color_24_bit =
                        (i32::from(c.0) << 16) | (i32::from(c.1) << 8) | i32::from(c.2);
                }
                drawing.add_entity(entity);
            }
        }
    }
    if skipped_images > 0 {
        scene.warnings.push(format!(
            "El DXF no lleva imágenes: se omitieron {skipped_images}."
        ));
    }
    let mut bytes = Vec::new();
    drawing
        .save(&mut bytes)
        .map_err(|e| format!("no se pudo escribir el DXF: {e}"))?;
    let b = scene.bounds;
    Ok(Export {
        bytes,
        size_mm: [b.max.x - b.min.x, b.max.y - b.min.y],
        warnings: scene.warnings,
    })
}

/// La entidad que representa un subtrayecto (que empieza con `MoveTo`).
fn entity_for(sub: &BezPath, mode: DxfCurves, tolerance: f64) -> Option<EntityType> {
    let segs = &sub.0;
    let curved = |s: &PathSeg| matches!(s, PathSeg::QuadTo { .. } | PathSeg::CubicTo { .. });
    let all_curves = segs[1..]
        .iter()
        .all(|s| curved(s) || matches!(s, PathSeg::Close));
    if mode == DxfCurves::Splines
        && all_curves
        && segs.iter().any(curved)
        && let Some(spline) = spline_of(segs)
    {
        return Some(EntityType::Spline(spline));
    }
    // Rectas, o curvas mezcladas con rectas: una polilínea, aplanada si hace falta.
    let poly = sub.flatten(tolerance).into_iter().next()?;
    if poly.points.len() < 2 {
        return None;
    }
    let mut lw = LwPolyline {
        vertices: poly
            .points
            .iter()
            .map(|p| LwPolylineVertex {
                x: p.x,
                y: p.y,
                ..LwPolylineVertex::default()
            })
            .collect(),
        ..LwPolyline::default()
    };
    lw.set_is_closed(poly.closed);
    Some(EntityType::LwPolyline(lw))
}

/// SPLINE cúbico (B-spline de grado 3 con nudos repetidos = cadena de Béziers) para un
/// subtrayecto de solo curvas. `None` si no empieza en un punto.
fn spline_of(segs: &[PathSeg]) -> Option<Spline> {
    let PathSeg::MoveTo { to: start } = segs.first()? else {
        return None;
    };
    let mut ctrl = vec![*start];
    let mut cur = *start;
    for seg in &segs[1..] {
        match *seg {
            PathSeg::CubicTo { ctrl1, ctrl2, to } => {
                ctrl.extend([ctrl1, ctrl2, to]);
                cur = to;
            }
            // Una cuadrática se eleva a cúbica sin cambiar la curva.
            PathSeg::QuadTo { ctrl: q, to } => {
                let third = |a: tlaku_core::Point, b: tlaku_core::Point| {
                    tlaku_core::Point::new(
                        a.x + (b.x - a.x) * 2.0 / 3.0,
                        a.y + (b.y - a.y) * 2.0 / 3.0,
                    )
                };
                ctrl.extend([third(cur, q), third(to, q), to]);
                cur = to;
            }
            PathSeg::Close => {
                // Cierra solo si el final ya coincide con el inicio; si no, el hueco es una
                // recta y el subtrayecto no es "solo curvas".
                if cur.dist(*start) > 1e-9 {
                    return None;
                }
            }
            _ => return None,
        }
    }
    let pieces = (ctrl.len() - 1) / 3;
    let mut knots = vec![0.0; 4];
    for k in 1..pieces {
        knots.extend([k as f64; 3]);
    }
    knots.extend([pieces as f64; 4]);
    Some(Spline {
        // Plana (bit 3); sin bit de cerrada: el final coincide con el inicio si hace falta.
        flags: 8,
        degree_of_curve: 3,
        knot_values: knots,
        control_points: ctrl.iter().map(|p| Point::new(p.x, p.y, 0.0)).collect(),
        ..Spline::default()
    })
}

/// Índice ACI (1 a 255) cuyo color RGB está más cerca; el negro exacto es el 7.
pub(crate) fn nearest_aci(c: Rgb) -> u8 {
    if c == Rgb(0, 0, 0) {
        return 7;
    }
    let dist = |a: Rgb| {
        let d = |x: u8, y: u8| (i32::from(x) - i32::from(y)).pow(2);
        d(a.0, c.0) + d(a.1, c.1) + d(a.2, c.2)
    };
    // El 7 (negro aquí) no compite: sería el candidato de cualquier color muy oscuro, y el
    // negro puro ya se resolvió arriba.
    (1..=255u8)
        .filter(|i| *i != 7)
        .min_by_key(|i| dist(aci_to_rgb(*i)))
        .unwrap_or(7)
}

fn hex(c: Rgb) -> String {
    format!("{:02X}{:02X}{:02X}", c.0, c.1, c.2)
}

/// Nombre de capa DXF: solo ASCII sin los caracteres que el formato prohíbe.
fn layer_name(name: &str) -> String {
    let s: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == ' ' || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let s = s.trim().to_string();
    if s.is_empty() { "CAPA".into() } else { s }
}
