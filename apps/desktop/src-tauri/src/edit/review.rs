//! Comando `review_design`: «Revisar diseño». Analiza el documento (sin IA, determinista) y
//! devuelve una lista de hallazgos; no cambia nada. Los arreglos los aplica la interfaz con las
//! operaciones que ya existen (`geometry_op` con `join`/`close`/`dedupe`, `shape_op` con
//! `optimize`), cada uno como un solo paso de deshacer.
//!
//! Qué detecta (un hallazgo por tipo y capa, salvo los que no dependen de la capa):
//! - `near_closed`: trazos abiertos con el hueco entre sus extremos de `close_mm` o menos
//!   (misma regla que `close_paths`). Arreglo: cerrar.
//! - `overlap`: tramos repetidos o encimados (misma regla que `dedupe_paths`). Arreglo: quitar
//!   duplicados. `fix_ids` trae todos los objetos de la capa que hay que pasarle a la operación,
//!   porque el que se conserva también participa; `object_ids` solo los que pierden trazo.
//! - `joinable`: extremos de trazos distintos a `join_mm` o menos (misma regla que
//!   `auto_join_tracked`). Arreglo: unir.
//! - `tiny`: objetos cuya caja mide menos de `tiny_mm` en sus dos lados (el láser no los hace
//!   bien). Arreglo: borrarlos o seleccionarlos. Se excluyen del resto de análisis geométrico.
//! - `extra_nodes`: objetos a los que `optimize` (sin suavizar) le quitaría 2 nodos o más.
//!   Arreglo: optimizar.
//! - `outside_all` / `outside_part`: objetos de capas con salida que quedan del todo o solo en
//!   parte fuera de la cama (`[0, w] x [0, h]`, origen abajo a la izquierda). Solo seleccionar.
//! - `text`: textos que siguen siendo texto. Arreglo: convertir a trazos.
//! - `silent_layer`: capas sin salida que tienen objetos (menos la de guías). Arreglo: activar
//!   la salida o seleccionar.
//! - `hidden_layer`: capas ocultas con objetos. Arreglo: mostrar la capa o seleccionar.
//! - `image_dpi_high` / `image_dpi_low`: imágenes en capas de imagen cuya resolución real es
//!   muy superior (más del doble) o muy inferior (menos de la mitad) a la que pide el
//!   intervalo de la capa. Solo aviso.
//!
//! Los objetos ocultos y las capas fuera de salida no entran en el análisis geométrico, y los
//! bloqueados tampoco en los hallazgos que se arreglan editando su trazo (no se pueden tocar).
//! Coste: casi lineal con el número de objetos (las operaciones de `tlaku-geom` usan rejillas).

use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap};
use tlaku_core::{Bounds, Rgb};
use tlaku_doc::{DesignObject, Document, LayerMode, ObjectKind, PathSeg};
use tlaku_geom::{
    OptimizeOptions, Polyline, auto_join_tracked, close_paths, dedupe_paths, node_count,
    optimize_polyline, split_subpaths,
};

/// Error máximo (mm) al aplanar para analizar. Más grueso que el de las operaciones (0,01):
/// aquí solo se miden extremos, cajas y solapes.
const FLATTEN_MM: f64 = 0.02;
/// Color de la capa de guías (`GUIDE_COLOR` de la interfaz): nunca sale a la máquina.
const GUIDE: Rgb = Rgb(0x7e, 0x8a, 0xa0);
/// Holgura (mm) con la que un objeto se considera dentro de la cama.
const BED_SLACK_MM: f64 = 0.01;
/// Mínimo de nodos que `optimize` debe quitar a un objeto para avisar.
const MIN_NODES_SAVED: usize = 2;

/// Umbrales de la revisión; todos opcionales (los de fábrica son los de las operaciones).
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ReviewOptions {
    /// Hueco máximo (mm) que `cerrar` cierra. Por defecto 0,5.
    pub close_mm: Option<f64>,
    /// Distancia máxima (mm) entre extremos que `unir` une. Por defecto 0,1.
    pub join_mm: Option<f64>,
    /// Tolerancia (mm) de `quitar duplicados`. Por defecto 0,01.
    pub dedupe_mm: Option<f64>,
    /// Tamaño (mm) por debajo del cual una figura es un fragmento. Por defecto 0,2.
    pub tiny_mm: Option<f64>,
    /// Tolerancia (mm) de `optimizar`. Por defecto 0,02.
    pub node_mm: Option<f64>,
    /// Múltiplo de la resolución pedida desde el cual una imagen «sobra». Por defecto 2.
    pub dpi_high: Option<f64>,
    /// Fracción de la resolución pedida por debajo de la cual una imagen «falta». Por defecto 0,5.
    pub dpi_low: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FindingKind {
    NearClosed,
    Overlap,
    Joinable,
    Tiny,
    ExtraNodes,
    OutsideAll,
    OutsidePart,
    Text,
    SilentLayer,
    HiddenLayer,
    ImageDpiHigh,
    ImageDpiLow,
}

impl FindingKind {
    fn name(self) -> &'static str {
        match self {
            FindingKind::NearClosed => "near_closed",
            FindingKind::Overlap => "overlap",
            FindingKind::Joinable => "joinable",
            FindingKind::Tiny => "tiny",
            FindingKind::ExtraNodes => "extra_nodes",
            FindingKind::OutsideAll => "outside_all",
            FindingKind::OutsidePart => "outside_part",
            FindingKind::Text => "text",
            FindingKind::SilentLayer => "silent_layer",
            FindingKind::HiddenLayer => "hidden_layer",
            FindingKind::ImageDpiHigh => "image_dpi_high",
            FindingKind::ImageDpiLow => "image_dpi_low",
        }
    }
}

/// Gravedad: `serious` es lo que conviene ver antes de enviar (fuera de la cama, capas sin salida).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Serious,
    Warning,
    Info,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Finding {
    /// Estable para el mismo contenido (tipo, capa y objetos): sirve para «Ignorar».
    pub key: String,
    pub kind: FindingKind,
    pub severity: Severity,
    /// Color `#rrggbb` de la capa, o `None` si el hallazgo no es de una sola capa.
    pub layer: Option<String>,
    /// Objetos afectados (los que se seleccionan).
    pub object_ids: Vec<String>,
    /// Objetos que hay que pasarle a la operación de arreglo; vacío = `object_ids`.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub fix_ids: Vec<String>,
    /// Cuántos casos hay (huecos, uniones, objetos, nodos...), según el tipo.
    pub count: u32,
    /// Número principal del texto: mm quitados, resolución real, nodos que sobran...
    pub value: f64,
    /// Número secundario: resolución pedida, textos sin contorno...
    pub detail: f64,
    /// Tolerancia (mm) con la que se detectó; el arreglo debe usar la misma.
    pub tolerance: Option<f64>,
}

/// Hash FNV-1a de 64 bits, para la clave de un hallazgo.
fn fnv(parts: impl IntoIterator<Item = impl AsRef<str>>) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for p in parts {
        for b in p.as_ref().bytes().chain(std::iter::once(0)) {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
    }
    h
}

fn make(
    kind: FindingKind,
    severity: Severity,
    layer: Option<Rgb>,
    mut object_ids: Vec<String>,
) -> Finding {
    object_ids.sort();
    object_ids.dedup();
    let layer = layer.map(|c| c.to_string());
    let hash = fnv(object_ids.iter());
    Finding {
        key: format!(
            "{}:{}:{hash:016x}",
            kind.name(),
            layer.as_deref().unwrap_or("-")
        ),
        kind,
        severity,
        layer,
        count: object_ids.len() as u32,
        object_ids,
        fix_ids: Vec::new(),
        value: 0.0,
        detail: 0.0,
        tolerance: None,
    }
}

fn positive(v: Option<f64>, default: f64) -> f64 {
    v.filter(|x| x.is_finite() && *x >= 0.0).unwrap_or(default)
}

/// ¿La caja queda del todo fuera, en parte fuera o dentro de la cama?
fn bed_position(b: &Bounds, bed: [f64; 2]) -> Option<FindingKind> {
    // Tocar el borde de la cama (una recta sobre y = 0) es estar dentro.
    let fully = b.max.x < -BED_SLACK_MM
        || b.max.y < -BED_SLACK_MM
        || b.min.x > bed[0] + BED_SLACK_MM
        || b.min.y > bed[1] + BED_SLACK_MM;
    if fully {
        return Some(FindingKind::OutsideAll);
    }
    let partly = b.min.x < -BED_SLACK_MM
        || b.min.y < -BED_SLACK_MM
        || b.max.x > bed[0] + BED_SLACK_MM
        || b.max.y > bed[1] + BED_SLACK_MM;
    partly.then_some(FindingKind::OutsidePart)
}

fn bounds_of_paths(paths: &[Polyline]) -> Option<(f64, f64)> {
    let mut it = paths.iter().flat_map(|p| p.points.iter());
    let first = it.next()?;
    let (mut x0, mut x1, mut y0, mut y1) = (first.x, first.x, first.y, first.y);
    for p in it {
        x0 = x0.min(p.x);
        x1 = x1.max(p.x);
        y0 = y0.min(p.y);
        y1 = y1.max(p.y);
    }
    Some((x1 - x0, y1 - y0))
}

/// Nodos que `optimize` (sin suavizar) le quitaría al objeto, con la misma regla que `shape_op`.
fn nodes_saved(o: &DesignObject, tolerance: f64) -> usize {
    let opts = OptimizeOptions {
        tolerance,
        smooth: false,
        corner_angle_deg: 35.0,
    };
    let mut saved = 0;
    for curve in o.curves() {
        for sub in split_subpaths(&curve) {
            // Un subtrayecto abierto no baja de 2 nodos y uno cerrado no baja de 3: si no tiene
            // margen para quitar `MIN_NODES_SAVED`, `optimize` no puede ganar lo suficiente.
            let before = node_count(&sub);
            let floor = if sub.0.iter().any(|s| matches!(s, PathSeg::Close)) {
                3
            } else {
                2
            };
            if before < floor + MIN_NODES_SAVED {
                continue;
            }
            let Some(flat) = sub.flatten(0.01).into_iter().next() else {
                continue;
            };
            let after = node_count(&optimize_polyline(&flat, &opts));
            if after < before {
                saved += before - after;
            }
        }
    }
    saved
}

/// Resolución real (ppp) de una imagen en sus dos ejes: `(mínima, máxima)`.
fn image_dpi(o: &DesignObject) -> Option<(f64, f64)> {
    let ObjectKind::Image(img) = &o.kind else {
        return None;
    };
    let t = &o.transform;
    let w_mm = img.size_mm[0] * t[0].hypot(t[1]);
    let h_mm = img.size_mm[1] * t[2].hypot(t[3]);
    if !(w_mm > 1e-9 && h_mm > 1e-9) || img.width_px == 0 || img.height_px == 0 {
        return None;
    }
    let dx = f64::from(img.width_px) / (w_mm / 25.4);
    let dy = f64::from(img.height_px) / (h_mm / 25.4);
    Some((dx.min(dy), dx.max(dy)))
}

/// Núcleo puro de `review_design` (sin Tauri), para poder probarlo.
pub fn review(doc: &Document, bed: [f64; 2], opts: &ReviewOptions) -> Vec<Finding> {
    let close_mm = positive(opts.close_mm, 0.5);
    let join_mm = positive(opts.join_mm, 0.1);
    let dedupe_mm = positive(opts.dedupe_mm, 0.01);
    let tiny_mm = positive(opts.tiny_mm, 0.2);
    let node_mm = positive(opts.node_mm, 0.02);
    let dpi_high = positive(opts.dpi_high, 2.0);
    let dpi_low = positive(opts.dpi_low, 0.5);

    // Objetos visibles por capa, en orden de aparición.
    let mut order: Vec<Rgb> = Vec::new();
    let mut by_layer: HashMap<Rgb, Vec<usize>> = HashMap::new();
    for (i, o) in doc.objects.iter().enumerate() {
        if o.hidden {
            continue;
        }
        by_layer
            .entry(o.layer)
            .or_insert_with(|| {
                order.push(o.layer);
                Vec::new()
            })
            .push(i);
    }

    let mut out: Vec<Finding> = Vec::new();
    let ids = |members: &[usize]| -> Vec<String> {
        members.iter().map(|&i| doc.objects[i].id.clone()).collect()
    };

    let mut outside: [Vec<String>; 2] = [Vec::new(), Vec::new()];
    let mut texts: Vec<String> = Vec::new();
    let mut texts_empty = 0u32;

    for &color in &order {
        if color == GUIDE {
            continue;
        }
        let members = &by_layer[&color];
        let layer = doc.layer(color);
        let mode = layer.map_or(LayerMode::Line, |l| l.mode);

        if let Some(l) = layer {
            if !l.output {
                out.push(make(
                    FindingKind::SilentLayer,
                    Severity::Serious,
                    Some(color),
                    ids(members),
                ));
                continue;
            }
            if !l.visible {
                out.push(make(
                    FindingKind::HiddenLayer,
                    Severity::Warning,
                    Some(color),
                    ids(members),
                ));
            }
        }

        // Cama, textos e imágenes: cualquier tipo de objeto.
        let mut dpi_high_ids = Vec::new();
        let mut dpi_low_ids = Vec::new();
        let (mut worst_high, mut worst_low) = (f64::INFINITY, 0.0_f64);
        let wanted_dpi = layer
            .filter(|l| l.mode == LayerMode::Image && l.interval > 1e-9)
            .map(|l| 25.4 / l.interval);
        for &i in members {
            let o = &doc.objects[i];
            if bed[0] > 0.0
                && bed[1] > 0.0
                && let Some(b) = o.bounds_with_copies()
                && let Some(kind) = bed_position(&b, bed)
            {
                outside[usize::from(kind == FindingKind::OutsidePart)].push(o.id.clone());
            }
            if let ObjectKind::Text(t) = &o.kind {
                texts.push(o.id.clone());
                if t.outline.is_empty() {
                    texts_empty += 1;
                }
            }
            if let (Some(want), Some((lo, hi))) = (wanted_dpi, image_dpi(o)) {
                if lo > want * dpi_high {
                    dpi_high_ids.push(o.id.clone());
                    worst_high = worst_high.min(lo);
                } else if hi < want * dpi_low {
                    dpi_low_ids.push(o.id.clone());
                    worst_low = worst_low.max(hi);
                }
            }
        }
        for (kind, found, value) in [
            (FindingKind::ImageDpiHigh, dpi_high_ids, worst_high),
            (FindingKind::ImageDpiLow, dpi_low_ids, worst_low),
        ] {
            if !found.is_empty() {
                let mut f = make(kind, Severity::Info, Some(color), found);
                f.value = value;
                f.detail = wanted_dpi.unwrap_or(0.0);
                out.push(f);
            }
        }

        // Geometría: solo vectores editables en capas de línea o relleno.
        if mode == LayerMode::Image {
            continue;
        }
        let eligible: Vec<usize> = members
            .iter()
            .copied()
            .filter(|&i| {
                let o = &doc.objects[i];
                !o.locked && matches!(o.kind, ObjectKind::Vector(_))
            })
            .collect();
        let flat: Vec<Vec<Polyline>> = eligible
            .iter()
            .map(|&i| doc.objects[i].polylines_with(FLATTEN_MM))
            .collect();

        // Fragmentos diminutos; el resto del análisis los deja fuera.
        let mut tiny_ids = Vec::new();
        let mut live: Vec<usize> = Vec::new(); // posiciones en `eligible`
        for (k, paths) in flat.iter().enumerate() {
            match bounds_of_paths(paths) {
                Some((w, h)) if w < tiny_mm && h < tiny_mm => {
                    tiny_ids.push(doc.objects[eligible[k]].id.clone());
                }
                Some(_) => live.push(k),
                None => {}
            }
        }
        if !tiny_ids.is_empty() {
            let mut f = make(FindingKind::Tiny, Severity::Warning, Some(color), tiny_ids);
            f.value = tiny_mm;
            f.tolerance = Some(tiny_mm);
            out.push(f);
        }

        // Trazos abiertos casi cerrados.
        let mut near_ids = Vec::new();
        let mut near_gaps = 0u32;
        let mut worst_gap = 0.0_f64;
        for &k in &live {
            let closed = close_paths(&flat[k], close_mm);
            let mut hit = false;
            for (before, after) in flat[k].iter().zip(&closed) {
                if after.closed && !before.closed {
                    hit = true;
                    near_gaps += 1;
                    let n = before.points.len();
                    worst_gap = worst_gap.max(before.points[0].dist(before.points[n - 1]));
                }
            }
            if hit {
                near_ids.push(doc.objects[eligible[k]].id.clone());
            }
        }
        if !near_ids.is_empty() {
            let mut f = make(
                FindingKind::NearClosed,
                Severity::Warning,
                Some(color),
                near_ids,
            );
            f.count = near_gaps;
            f.value = worst_gap;
            f.tolerance = Some(close_mm);
            out.push(f);
        }

        // Todas las trayectorias de la capa, con el objeto del que vienen.
        let mut all: Vec<Polyline> = Vec::new();
        let mut owner: Vec<usize> = Vec::new();
        for &k in &live {
            for p in &flat[k] {
                all.push(p.clone());
                owner.push(k);
            }
        }

        // Duplicados y encimados.
        let dd = dedupe_paths(&all, dedupe_mm);
        let touched: BTreeSet<usize> = dd
            .changed
            .iter()
            .enumerate()
            .filter(|(_, c)| **c)
            .map(|(p, _)| owner[p])
            .collect();
        if !touched.is_empty() {
            let mut f = make(
                FindingKind::Overlap,
                Severity::Warning,
                Some(color),
                touched
                    .iter()
                    .map(|&k| doc.objects[eligible[k]].id.clone())
                    .collect(),
            );
            f.fix_ids = live
                .iter()
                .map(|&k| doc.objects[eligible[k]].id.clone())
                .collect();
            f.value = dd.removed_length;
            f.tolerance = Some(dedupe_mm);
            out.push(f);
        }

        // Trazos que deberían unirse (los que se cierran solos ya salen como casi cerrados).
        let mut join_ids = Vec::new();
        let mut joins = 0u32;
        for (_, sources) in auto_join_tracked(&all, join_mm) {
            if sources.len() > 1 {
                joins += 1;
                join_ids.extend(
                    sources
                        .iter()
                        .map(|&s| doc.objects[eligible[owner[s]]].id.clone()),
                );
            }
        }
        if joins > 0 {
            let mut f = make(
                FindingKind::Joinable,
                Severity::Warning,
                Some(color),
                join_ids,
            );
            f.count = joins;
            f.tolerance = Some(join_mm);
            out.push(f);
        }

        // Nodos de sobra.
        let mut node_ids = Vec::new();
        let mut node_total = 0usize;
        for &k in &live {
            let saved = nodes_saved(&doc.objects[eligible[k]], node_mm);
            if saved >= MIN_NODES_SAVED {
                node_total += saved;
                node_ids.push(doc.objects[eligible[k]].id.clone());
            }
        }
        if !node_ids.is_empty() {
            let mut f = make(
                FindingKind::ExtraNodes,
                Severity::Info,
                Some(color),
                node_ids,
            );
            f.value = node_total as f64;
            f.tolerance = Some(node_mm);
            out.push(f);
        }
    }

    for (n, kind) in [FindingKind::OutsideAll, FindingKind::OutsidePart]
        .into_iter()
        .enumerate()
    {
        let found = std::mem::take(&mut outside[n]);
        if !found.is_empty() {
            out.push(make(kind, Severity::Serious, None, found));
        }
    }
    if !texts.is_empty() {
        let severity = if texts_empty > 0 {
            Severity::Warning
        } else {
            Severity::Info
        };
        let mut f = make(FindingKind::Text, severity, None, texts);
        f.detail = f64::from(texts_empty);
        out.push(f);
    }

    // Orden estable: lo grave primero; dentro de cada gravedad, el orden en que se encontró.
    out.sort_by_key(|f| f.severity);
    out
}

/// «Revisar diseño»: lista de hallazgos del documento, `bed` = `[ancho, alto]` de la cama en mm.
/// Solo lee; corre fuera del hilo principal porque con miles de objetos tarda.
#[tauri::command]
pub async fn review_design(
    doc: Document,
    bed: [f64; 2],
    options: Option<ReviewOptions>,
) -> Result<Vec<Finding>, String> {
    tauri::async_runtime::spawn_blocking(move || review(&doc, bed, &options.unwrap_or_default()))
        .await
        .map_err(crate::edit::task_interrupted)
}

#[cfg(test)]
mod tests;
