//! Comandos del área `trace` (fase 3): vectorizado de imágenes sobre `tlaku-trace`.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tlaku_core::Rgb;
use tlaku_doc::{Document, ImageObject, LayerMode, LayerSettings, ObjectKind, union};
use tlaku_export::{PngOptions, export_png};
use tlaku_i18n::tr;
use tlaku_trace::{TraceResult, TraceSettings};

/// Traza el archivo de imagen `path` en contornos de `size_mm` (ancho, alto). Las curvas salen en
/// coordenadas locales de la imagen (mm, origen abajo a la izquierda, Y hacia arriba), así que
/// basta darles el `transform` del objeto imagen para que coincidan con ella.
pub fn run_trace(
    path: &str,
    settings: &TraceSettings,
    size_mm: [f64; 2],
) -> Result<TraceResult, String> {
    if path.trim().is_empty() {
        return Err(tr!("desktop.edit.imaging.no_path"));
    }
    tlaku_trace::trace_path(path, settings, size_mm)
}

// La interfaz manda los argumentos en snake_case (`size_mm`), como el resto del contrato.
#[tauri::command(rename_all = "snake_case")]
pub async fn trace_image(
    path: String,
    settings: TraceSettings,
    size_mm: [f64; 2],
) -> Result<TraceResult, String> {
    // El trazado es CPU puro y puede tardar segundos: fuera del hilo principal.
    tauri::async_runtime::spawn_blocking(move || run_trace(&path, &settings, size_mm))
        .await
        .map_err(crate::edit::task_interrupted)?
}

// ---------------------------------------------------------------------------------------------
// Convertir a mapa de bits (D82) y guardar la imagen procesada
// ---------------------------------------------------------------------------------------------

/// Opciones de "Convertir a mapa de bits".
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct BitmapOptions {
    /// Puntos por pulgada del PNG.
    pub dpi: f64,
    /// Rellena las formas cerradas; si no, solo dibuja el contorno.
    pub filled: bool,
    /// Grosor del contorno en mm (con `filled` apagado).
    pub stroke_mm: f64,
    /// Negativo: las formas en blanco sobre fondo negro.
    pub invert: bool,
}

impl Default for BitmapOptions {
    fn default() -> Self {
        Self {
            dpi: 300.0,
            filled: true,
            stroke_mm: 0.3,
            invert: false,
        }
    }
}

/// El PNG escrito y dónde queda en el diseño.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BitmapResult {
    pub path: String,
    pub width_px: u32,
    pub height_px: u32,
    /// Tamaño de la imagen en mm (ancho, alto), con los píxeles a la resolución pedida.
    pub size_mm: [f64; 2],
    /// Esquina inferior izquierda de la imagen en el documento (mm).
    pub origin_mm: [f64; 2],
    /// El proyecto no estaba guardado y el PNG quedó en la carpeta temporal.
    pub in_temp_dir: bool,
}

/// Primer nombre libre `<stem>-bitmap-<n>.png` en `dir`.
fn free_png_name(dir: &Path, stem: &str) -> PathBuf {
    let stem: String = stem
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || matches!(c, '-' | '_' | ' ') {
                c
            } else {
                '_'
            }
        })
        .collect();
    let stem = if stem.trim().is_empty() {
        "tlaku".to_string()
    } else {
        stem
    };
    (1..)
        .map(|n| dir.join(format!("{stem}-bitmap-{n}.png")))
        .find(|p| !p.exists())
        .expect("siempre hay un número libre")
}

/// Rasteriza las formas de `selection` (vectores y texto con contorno) a un PNG en `dir` (o en la
/// carpeta temporal si no hay) y dice dónde quedó. El dibujo sale en negro sobre blanco, sin margen,
/// para grabarlo como imagen.
pub fn vectors_to_bitmap_file(
    doc: &Document,
    selection: &[String],
    dir: Option<&Path>,
    stem: &str,
    opts: &BitmapOptions,
) -> Result<BitmapResult, String> {
    if !(opts.dpi.is_finite() && (10.0..=2400.0).contains(&opts.dpi)) {
        return Err(tr!("desktop.edit.trace.bad_dpi", value = opts.dpi));
    }
    if !(opts.stroke_mm.is_finite() && opts.stroke_mm > 0.0) {
        return Err(tr!("desktop.edit.trace.bad_stroke", value = opts.stroke_mm));
    }
    let black = Rgb(0, 0, 0);
    let mut tmp = doc.clone();
    tmp.objects = doc
        .objects
        .iter()
        .filter(|o| selection.contains(&o.id) && !o.hidden)
        .filter(|o| match &o.kind {
            ObjectKind::Vector(_) => true,
            ObjectKind::Text(t) => !t.outline.is_empty(),
            ObjectKind::Image(_) => false,
        })
        .cloned()
        .map(|mut o| {
            o.layer = black;
            o
        })
        .collect();
    let bounds = tmp
        .objects
        .iter()
        .filter_map(|o| o.bounds())
        .reduce(union)
        .ok_or_else(|| tr!("desktop.edit.trace.no_vectors"))?;
    tmp.layers = vec![LayerSettings {
        mode: if opts.filled {
            LayerMode::Fill
        } else {
            LayerMode::Line
        },
        visible: true,
        ..LayerSettings::for_color(black)
    }];
    // Sin relleno, la mitad del trazo sale de la caja del dibujo: se deja ese margen.
    let margin = if opts.filled {
        0.0
    } else {
        opts.stroke_mm / 2.0
    };
    let png_opts = PngOptions {
        dpi: opts.dpi,
        margin_mm: margin,
        transparent: false,
        stroke_mm: opts.stroke_mm,
        fill_layers: opts.filled,
    };
    let out = export_png(&tmp, None, &png_opts)?;
    let bytes = if opts.invert {
        let mut img = image::load_from_memory(&out.bytes)
            .map_err(|e| tr!("desktop.edit.trace.read_png_failed", err = e))?
            .to_rgba8();
        for p in img.pixels_mut() {
            p.0[0] = 255 - p.0[0];
            p.0[1] = 255 - p.0[1];
            p.0[2] = 255 - p.0[2];
        }
        let mut cur = std::io::Cursor::new(Vec::new());
        img.write_to(&mut cur, image::ImageFormat::Png)
            .map_err(|e| tr!("desktop.edit.trace.encode_png_failed", err = e))?;
        cur.into_inner()
    } else {
        out.bytes
    };
    let (width_px, height_px) = {
        let img = image::load_from_memory(&bytes)
            .map_err(|e| tr!("desktop.edit.trace.read_png_failed", err = e))?;
        (img.width(), img.height())
    };
    let px_mm = 25.4 / opts.dpi;
    let size_mm = [f64::from(width_px) * px_mm, f64::from(height_px) * px_mm];
    // El PNG cuelga de la esquina superior izquierda de la página: la parte que sobra por redondear
    // al píxel queda por debajo.
    let top = bounds.max.y + margin;
    let origin_mm = [bounds.min.x - margin, top - size_mm[1]];

    let (folder, in_temp_dir) = match dir {
        Some(d) if d.is_dir() => (d.to_path_buf(), false),
        _ => (std::env::temp_dir().join("tlaku-bitmaps"), true),
    };
    std::fs::create_dir_all(&folder).map_err(|e| format!("{}: {e}", folder.display()))?;
    let path = free_png_name(&folder, stem);
    std::fs::write(&path, &bytes).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(BitmapResult {
        path: path.to_string_lossy().into_owned(),
        width_px,
        height_px,
        size_mm,
        origin_mm,
        in_temp_dir,
    })
}

#[tauri::command(rename_all = "snake_case")]
pub async fn vectors_to_bitmap(
    doc: Document,
    selection: Vec<String>,
    dir: Option<String>,
    stem: String,
    options: BitmapOptions,
) -> Result<BitmapResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        vectors_to_bitmap_file(
            &doc,
            &selection,
            dir.as_deref().map(Path::new),
            &stem,
            &options,
        )
    })
    .await
    .map_err(crate::edit::task_interrupted)?
}

/// Lo que se escribió con "Guardar imagen procesada".
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ProcessedSummary {
    pub path: String,
    pub bytes: u64,
    pub width_px: u32,
    pub height_px: u32,
}

/// Guarda como PNG lo que se grabaría de `image` con los ajustes de imagen de `layer`.
pub fn save_processed(
    image: &ImageObject,
    layer: &LayerSettings,
    path: &str,
) -> Result<ProcessedSummary, String> {
    if path.trim().is_empty() {
        return Err(tr!("desktop.edit.trace.missing_path"));
    }
    let (png, [width_px, height_px]) = tlaku_pipeline::processed_png(
        Path::new(&image.source_path),
        image.size_mm,
        layer,
        &image.mask,
    )?;
    std::fs::write(path, &png).map_err(|e| format!("{path}: {e}"))?;
    Ok(ProcessedSummary {
        path: path.to_string(),
        bytes: png.len() as u64,
        width_px,
        height_px,
    })
}

#[tauri::command(rename_all = "snake_case")]
pub async fn save_processed_image(
    image: ImageObject,
    layer: LayerSettings,
    path: String,
) -> Result<ProcessedSummary, String> {
    tauri::async_runtime::spawn_blocking(move || save_processed(&image, &layer, &path))
        .await
        .map_err(crate::edit::task_interrupted)?
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{GrayImage, Luma};
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU32, Ordering};
    use tlaku_core::bezpath::PathSeg;

    static COUNTER: AtomicU32 = AtomicU32::new(0);

    /// PNG temporal que se borra al salir del ámbito.
    struct TempPng(PathBuf);

    impl TempPng {
        /// 100x60 px, fondo blanco y un rectángulo negro de 40x20 en (10, 10).
        fn rect() -> Self {
            let n = COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir()
                .join(format!("tlaku-trace-test-{}-{n}.png", std::process::id()));
            let img = GrayImage::from_fn(100, 60, |x, y| {
                Luma([if (10..50).contains(&x) && (10..30).contains(&y) {
                    0
                } else {
                    255
                }])
            });
            img.save(&path)
                .expect("no se pudo escribir el PNG de prueba");
            Self(path)
        }

        fn path(&self) -> &str {
            self.0.to_str().unwrap()
        }
    }

    impl Drop for TempPng {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }

    fn bounds(result: &TraceResult) -> (f64, f64, f64, f64) {
        let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
        for path in &result.paths {
            for seg in &path.0 {
                let to = match seg {
                    PathSeg::MoveTo { to } | PathSeg::LineTo { to } => to,
                    PathSeg::QuadTo { to, .. } | PathSeg::CubicTo { to, .. } => to,
                    PathSeg::Close => continue,
                };
                x0 = x0.min(to.x);
                y0 = y0.min(to.y);
                x1 = x1.max(to.x);
                y1 = y1.max(to.y);
            }
        }
        (x0, y0, x1, y1)
    }

    #[test]
    fn traza_un_rectangulo_en_mm_con_y_hacia_arriba() {
        let png = TempPng::rect();
        // 100x60 px en 50x30 mm: 0,5 mm por píxel.
        let settings = TraceSettings {
            mode: tlaku_trace::TraceMode::Polygon,
            ..TraceSettings::default()
        };
        let result = run_trace(png.path(), &settings, [50.0, 30.0]).unwrap();
        assert_eq!(result.paths.len(), 1);
        let (x0, y0, x1, y1) = bounds(&result);
        // Rectángulo en x 10..50 px y y 10..30 px (hacia abajo): 5..25 mm en X y 15..25 mm en Y.
        assert!(
            (x0 - 5.0).abs() < 0.6 && (x1 - 25.0).abs() < 0.6,
            "{x0} {x1}"
        );
        assert!(
            (y0 - 15.0).abs() < 0.6 && (y1 - 25.0).abs() < 0.6,
            "{y0} {y1}"
        );
    }

    #[test]
    fn invertir_traza_el_fondo() {
        let png = TempPng::rect();
        let settings = TraceSettings {
            invert: true,
            ..TraceSettings::default()
        };
        let result = run_trace(png.path(), &settings, [50.0, 30.0]).unwrap();
        let (x0, y0, x1, y1) = bounds(&result);
        // El fondo ocupa toda la imagen.
        assert!(
            x0 < 0.6 && y0 < 0.6 && x1 > 49.4 && y1 > 29.4,
            "{x0} {y0} {x1} {y1}"
        );
    }

    #[test]
    fn umbral_cero_no_traza_nada_y_avisa() {
        let png = TempPng::rect();
        let settings = TraceSettings {
            threshold: 0.0,
            ..TraceSettings::default()
        };
        let result = run_trace(png.path(), &settings, [50.0, 30.0]).unwrap();
        assert!(result.paths.is_empty());
        assert!(!result.warnings.is_empty());
    }

    #[test]
    fn errores_en_espanol() {
        let png = TempPng::rect();
        let defaults = TraceSettings::default();
        let err = run_trace("/no/existe/imagen.png", &defaults, [10.0, 10.0]).unwrap_err();
        assert!(err.contains("No se pudo leer"), "{err}");
        assert!(run_trace("", &defaults, [10.0, 10.0]).is_err());
        let bad = TraceSettings {
            threshold: 2.0,
            ..TraceSettings::default()
        };
        assert!(run_trace(png.path(), &bad, [10.0, 10.0]).is_err());
        assert!(run_trace(png.path(), &defaults, [0.0, 10.0]).is_err());
    }

    #[test]
    fn los_argumentos_llegan_como_los_manda_la_interfaz() {
        // Mismo JSON que `traceImage`: settings con todos los campos en snake_case.
        let json = r#"{"threshold":0.4,"invert":true,"blur_px":1.5,"speckle_px":3,
            "corner_threshold_deg":45,"smoothing":0.5,"mode":"polygon"}"#;
        let s: TraceSettings = serde_json::from_str(json).unwrap();
        assert!(s.invert && s.mode == tlaku_trace::TraceMode::Polygon);
        let out = serde_json::to_value(TraceResult::default()).unwrap();
        assert!(out["paths"].is_array() && out["warnings"].is_array());
    }

    /// Documento con un cuadrado de 20x10 mm en (5, 7) en la capa roja.
    fn doc_with_rect() -> Document {
        let obj: tlaku_doc::DesignObject = serde_json::from_value(serde_json::json!({
            "id": "r1",
            "layer": "#ff0000",
            "transform": [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
            "kind": { "type": "vector", "paths": [[
                { "type": "move_to", "to": [5.0, 7.0] },
                { "type": "line_to", "to": [25.0, 7.0] },
                { "type": "line_to", "to": [25.0, 17.0] },
                { "type": "line_to", "to": [5.0, 17.0] },
                { "type": "close" }
            ]] },
            "locked": false,
            "hidden": false,
            "group": null
        }))
        .unwrap();
        Document {
            objects: vec![obj],
            layers: vec![LayerSettings::for_color(Rgb(255, 0, 0))],
            ..Document::default()
        }
    }

    fn scratch_dir(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("tlaku-bitmap-test-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn convierte_vectores_en_un_png_que_ocupa_su_caja() {
        let dir = scratch_dir("caja");
        let opts = BitmapOptions {
            dpi: 254.0, // 10 puntos por mm
            ..BitmapOptions::default()
        };
        let r = vectors_to_bitmap_file(
            &doc_with_rect(),
            &["r1".into()],
            Some(&dir),
            "proyecto",
            &opts,
        )
        .unwrap();
        assert!(!r.in_temp_dir);
        assert!(r.path.ends_with("proyecto-bitmap-1.png"), "{}", r.path);
        assert_eq!((r.width_px, r.height_px), (200, 100));
        assert!((r.size_mm[0] - 20.0).abs() < 0.2 && (r.size_mm[1] - 10.0).abs() < 0.2);
        assert!((r.origin_mm[0] - 5.0).abs() < 0.2 && (r.origin_mm[1] - 7.0).abs() < 0.2);
        // Relleno negro sobre blanco.
        let img = image::open(&r.path).unwrap().to_luma8();
        assert_eq!(img.get_pixel(100, 50).0[0], 0);
        // Un segundo archivo no pisa al primero.
        let r2 = vectors_to_bitmap_file(
            &doc_with_rect(),
            &["r1".into()],
            Some(&dir),
            "proyecto",
            &opts,
        )
        .unwrap();
        assert!(r2.path.ends_with("proyecto-bitmap-2.png"), "{}", r2.path);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn negativo_y_contorno() {
        let dir = scratch_dir("negativo");
        let opts = BitmapOptions {
            dpi: 254.0,
            invert: true,
            ..BitmapOptions::default()
        };
        let r = vectors_to_bitmap_file(&doc_with_rect(), &["r1".into()], Some(&dir), "n", &opts)
            .unwrap();
        let img = image::open(&r.path).unwrap().to_luma8();
        assert_eq!(img.get_pixel(100, 50).0[0], 255);
        // Solo contorno: el centro queda blanco y el borde, negro.
        let opts = BitmapOptions {
            dpi: 254.0,
            filled: false,
            stroke_mm: 0.5,
            ..BitmapOptions::default()
        };
        let r = vectors_to_bitmap_file(&doc_with_rect(), &["r1".into()], Some(&dir), "c", &opts)
            .unwrap();
        let img = image::open(&r.path).unwrap().to_luma8();
        assert_eq!(img.get_pixel(100, 50).0[0], 255);
        assert!(img.get_pixel(100, 2).0[0] < 128);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn sin_carpeta_va_a_la_temporal_y_los_errores_hablan_en_espanol() {
        let opts = BitmapOptions::default();
        let r =
            vectors_to_bitmap_file(&doc_with_rect(), &["r1".into()], None, "sin guardar", &opts)
                .unwrap();
        assert!(r.in_temp_dir);
        let _ = std::fs::remove_file(&r.path);
        let e = vectors_to_bitmap_file(&doc_with_rect(), &["nada".into()], None, "x", &opts)
            .unwrap_err();
        assert!(e.contains("no tiene formas"), "{e}");
        let bad = BitmapOptions {
            dpi: 0.0,
            ..BitmapOptions::default()
        };
        assert!(vectors_to_bitmap_file(&doc_with_rect(), &["r1".into()], None, "x", &bad).is_err());
    }

    #[test]
    fn guarda_la_imagen_procesada() {
        let png = TempPng::rect();
        let dir = scratch_dir("procesada");
        let image = ImageObject {
            source_path: png.path().to_string(),
            width_px: 100,
            height_px: 60,
            size_mm: [10.0, 6.0],
            mask: Vec::new(),
        };
        let mut layer = LayerSettings::for_images();
        layer.interval = 0.1;
        let out = dir.join("salida.png");
        let r = save_processed(&image, &layer, out.to_str().unwrap()).unwrap();
        assert_eq!((r.width_px, r.height_px), (100, 60));
        assert!(r.bytes > 0 && out.exists());
        assert!(save_processed(&image, &layer, "").is_err());
        let _ = std::fs::remove_dir_all(dir);
    }
}
