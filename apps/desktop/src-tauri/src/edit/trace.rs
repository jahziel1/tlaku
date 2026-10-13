//! Comandos del área `trace` (fase 3): vectorizado de imágenes sobre `tlaku-trace`.

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
        return Err("La imagen no tiene una ruta de archivo".into());
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
        .map_err(|e| format!("La tarea se interrumpió: {e}"))?
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
}
