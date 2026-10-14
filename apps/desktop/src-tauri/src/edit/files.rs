//! Comandos del área `fileio` (ola 1, bloque Formatos): importar con las opciones del diálogo,
//! exportar el diseño a SVG, DXF, PNG, PDF y AI, e imprimir.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};
use tlaku_core::Rgb;
use tlaku_doc::{Document, ImportResult, ObjectKind};
use tlaku_export::{
    DxfOptions, Export, PdfOptions, PngOptions, SvgOptions, export_ai, export_dxf, export_pdf,
    export_png, export_svg,
};
use tlaku_i18n::tr;
use tlaku_pipeline::{ImportExtras, ImportOptions, import_file_extras};

/// Opciones del diálogo de importación (D91). Los valores ausentes toman el comportamiento
/// automático de siempre.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct ImportDialogOptions {
    /// Escala uniforme en porcentaje (100 = sin cambio).
    pub scale_percent: f64,
    /// Resolución (ppp) del SVG sin unidades y de las imágenes.
    pub dpi: Option<f64>,
    /// DXF sin unidades: mm que vale una unidad del dibujo (25.4 = pulgadas).
    pub dxf_unit_mm: Option<f64>,
    /// Une todo lo importado en un grupo.
    pub group: bool,
    /// Capa destino de los vectores, como `#rrggbb`.
    pub target_layer: Option<String>,
    /// PDF y AI: página a importar, desde 1.
    pub pdf_page: u32,
    /// G-code: una capa por nivel de potencia.
    pub gcode_by_power: bool,
    /// G-code: valor de `S` con el que se toman los cortes de potencia 0 (`M3 S0`); `None` = no
    /// se dibujan. Se ofrece cuando el archivo importa vacío por eso.
    pub gcode_default_power: Option<f64>,
}

impl Default for ImportDialogOptions {
    fn default() -> Self {
        Self {
            scale_percent: 100.0,
            dpi: None,
            dxf_unit_mm: None,
            group: false,
            target_layer: None,
            pdf_page: 1,
            gcode_by_power: true,
            gcode_default_power: None,
        }
    }
}

impl ImportDialogOptions {
    fn extras(&self) -> Result<ImportExtras, String> {
        if !self.scale_percent.is_finite() || self.scale_percent <= 0.0 {
            return Err(tr!(
                "desktop.edit.files.bad_scale",
                value = self.scale_percent
            ));
        }
        let invalid = |v: Option<f64>| v.filter(|v| !v.is_finite() || *v <= 0.0);
        if let Some(v) = invalid(self.dpi) {
            return Err(tr!("desktop.edit.files.bad_dpi", value = v));
        }
        if let Some(v) = invalid(self.dxf_unit_mm) {
            return Err(tr!("desktop.edit.files.bad_dxf_unit", value = v));
        }
        if let Some(v) = invalid(self.gcode_default_power) {
            return Err(tr!("desktop.edit.files.bad_gcode_power", value = v));
        }
        let target_layer = match &self.target_layer {
            Some(text) => Some(
                text.parse::<Rgb>()
                    .map_err(|e| tr!("desktop.edit.files.bad_layer_color", text = text, err = e))?,
            ),
            None => None,
        };
        Ok(ImportExtras {
            scale: self.scale_percent / 100.0,
            dpi: self.dpi,
            dxf_unit_mm: self.dxf_unit_mm,
            group: self.group,
            target_layer,
            pdf_page: self.pdf_page.max(1),
            gcode_by_power: self.gcode_by_power,
            gcode_default_power: self.gcode_default_power,
        })
    }
}

/// Importa `path` con las opciones del diálogo. La ruta se vuelve absoluta para que
/// `source_path` de las imágenes sirva después con el protocolo de assets.
pub fn import_with_options(
    path: &str,
    options: &ImportDialogOptions,
) -> Result<ImportResult, String> {
    let abs = std::path::absolute(path).map_err(|e| format!("{path}: {e}"))?;
    import_file_extras(&abs, &ImportOptions::default(), &options.extras()?)
}

/// Qué formato exportar y con qué opciones (las que falten toman su valor por defecto).
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "format", rename_all = "snake_case")]
pub enum ExportRequest {
    Svg(SvgOptions),
    Dxf(DxfOptions),
    Png(PngOptions),
    Pdf(PdfOptions),
    /// `.ai`: PDF 1.5 con una capa por color (mismas opciones que el PDF).
    Ai(PdfOptions),
}

impl ExportRequest {
    fn run(&self, doc: &Document, selection: Option<&[String]>) -> Result<Export, String> {
        match self {
            ExportRequest::Svg(o) => export_svg(doc, selection, o),
            ExportRequest::Dxf(o) => export_dxf(doc, selection, o),
            ExportRequest::Png(o) => export_png(doc, selection, o),
            ExportRequest::Pdf(o) => export_pdf(doc, selection, o),
            ExportRequest::Ai(o) => export_ai(doc, selection, o),
        }
    }
}

/// Lo que se escribió.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ExportSummary {
    pub path: String,
    pub bytes: u64,
    /// Tamaño del dibujo en mm, `[ancho, alto]`.
    pub size_mm: [f64; 2],
    pub warnings: Vec<String>,
}

/// Escribe `bytes` en un archivo temporal junto a `path` y lo renombra.
fn write_bytes_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    use std::io::Write as _;
    let name = path
        .file_name()
        .ok_or_else(|| tr!("desktop.edit.files.not_a_file_path", path = path.display()))?;
    let mut tmp_name = name.to_os_string();
    tmp_name.push(".tmp");
    let tmp = path.with_file_name(tmp_name);
    let write = || -> std::io::Result<()> {
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
        std::fs::rename(&tmp, path)
    };
    write().map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        format!("{}: {e}", path.display())
    })
}

/// Exporta el documento (o solo `selection`) a `path`.
pub fn export_design(
    doc: &Document,
    selection: Option<&[String]>,
    path: &str,
    request: &ExportRequest,
) -> Result<ExportSummary, String> {
    let out = request.run(doc, selection)?;
    write_bytes_atomic(Path::new(path), &out.bytes)?;
    Ok(ExportSummary {
        path: path.to_string(),
        bytes: out.bytes.len() as u64,
        size_mm: out.size_mm,
        warnings: out.warnings,
    })
}

/// Genera el PDF de impresión (a escala 1:1) en la carpeta temporal y devuelve su ruta.
pub fn render_print_pdf(
    doc: &Document,
    selection: Option<&[String]>,
    options: &PdfOptions,
) -> Result<(PathBuf, Export), String> {
    let out = export_pdf(doc, selection, options)?;
    let path = std::env::temp_dir().join(format!("tlaku-impresion-{}.pdf", std::process::id()));
    write_bytes_atomic(&path, &out.bytes)?;
    Ok((path, out))
}

/// Abre un archivo con el visor predeterminado del sistema (desde ahí se imprime).
fn open_with_system(path: &Path) -> Result<(), String> {
    use std::process::Command;
    let mut cmd = if cfg!(target_os = "macos") {
        let mut c = Command::new("open");
        c.arg(path);
        c
    } else if cfg!(target_os = "windows") {
        // `start` es una orden de cmd; el primer argumento entre comillas es el título.
        let mut c = Command::new("cmd");
        c.args(["/C", "start", ""]).arg(path);
        c
    } else {
        let mut c = Command::new("xdg-open");
        c.arg(path);
        c
    };
    cmd.spawn()
        .map(|_| ())
        .map_err(|e| tr!("desktop.edit.files.pdf_viewer_failed", err = e))
}

/// Rutas de las imágenes de un resultado de importación, para permitirlas en el protocolo de assets.
fn image_paths(result: &ImportResult) -> Vec<PathBuf> {
    result
        .objects
        .iter()
        .filter_map(|o| match &o.kind {
            ObjectKind::Image(i) => Some(PathBuf::from(&i.source_path)),
            ObjectKind::Vector(_) | ObjectKind::Text(_) => None,
        })
        .collect()
}

async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(crate::edit::task_interrupted)?
}

#[tauri::command(rename_all = "snake_case")]
pub async fn import_file_with(
    app: AppHandle,
    path: String,
    options: ImportDialogOptions,
) -> Result<ImportResult, String> {
    let result = blocking(move || import_with_options(&path, &options)).await?;
    let scope = app.asset_protocol_scope();
    for p in image_paths(&result) {
        let _ = scope.allow_file(p);
    }
    Ok(result)
}

#[tauri::command(rename_all = "snake_case")]
pub async fn export_design_file(
    doc: Document,
    selection: Option<Vec<String>>,
    path: String,
    request: ExportRequest,
) -> Result<ExportSummary, String> {
    blocking(move || export_design(&doc, selection.as_deref(), &path, &request)).await
}

/// Imprimir: genera el PDF a escala 1:1 y lo abre en el visor del sistema, que tiene el diálogo
/// de impresión (la ventana de la app no imprime a escala real).
#[tauri::command(rename_all = "snake_case")]
pub async fn print_design(
    app: AppHandle,
    doc: Document,
    selection: Option<Vec<String>>,
    options: PdfOptions,
) -> Result<ExportSummary, String> {
    let _ = &app; // El visor lo abre el sistema; la ventana no interviene.
    blocking(move || {
        let (path, out) = render_print_pdf(&doc, selection.as_deref(), &options)?;
        open_with_system(&path)?;
        Ok(ExportSummary {
            path: path.to_string_lossy().into_owned(),
            bytes: out.bytes.len() as u64,
            size_mm: out.size_mm,
            warnings: out.warnings,
        })
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use tlaku_core::Point;
    use tlaku_core::bezpath::{BezPath, PathSeg};
    use tlaku_doc::affine::IDENTITY;
    use tlaku_doc::{DesignObject, LayerSettings, VectorObject};

    fn doc() -> Document {
        let square = BezPath(vec![
            PathSeg::MoveTo {
                to: Point::new(0.0, 0.0),
            },
            PathSeg::LineTo {
                to: Point::new(10.0, 0.0),
            },
            PathSeg::LineTo {
                to: Point::new(10.0, 10.0),
            },
            PathSeg::Close,
        ]);
        let obj = |id: &str| DesignObject {
            id: id.into(),
            layer: Rgb(255, 0, 0),
            transform: IDENTITY,
            kind: ObjectKind::Vector(VectorObject::new(vec![square.clone()])),
            locked: false,
            hidden: false,
            group: None,
            priority: None,
            power_scale: None,
            shape: None,
            start: None,
            manual_tabs: Vec::new(),
            array: None,
        };
        Document {
            objects: vec![obj("a"), obj("b")],
            layers: vec![LayerSettings::for_color(Rgb(255, 0, 0))],
            ..Document::default()
        }
    }

    fn temp(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("tlaku-files-{}-{name}", std::process::id()))
    }

    #[test]
    fn la_solicitud_se_lee_del_json_de_la_interfaz() {
        let r: ExportRequest = serde_json::from_str(r#"{"format":"svg","images":"link"}"#).unwrap();
        let ExportRequest::Svg(o) = r else { panic!() };
        assert_eq!(o.images, tlaku_export::ImageHandling::Link);
        assert_eq!(o.stroke_mm, SvgOptions::default().stroke_mm);
        let r: ExportRequest = serde_json::from_str(r#"{"format":"pdf","page":"a4"}"#).unwrap();
        assert!(matches!(r, ExportRequest::Pdf(o) if o.page == tlaku_export::PageSize::A4));
        let r: ExportRequest = serde_json::from_str(r#"{"format":"ai"}"#).unwrap();
        assert!(matches!(r, ExportRequest::Ai(o) if o.page == tlaku_export::PageSize::Fit));
        assert!(serde_json::from_str::<ExportRequest>(r#"{"format":"cdr"}"#).is_err());
    }

    #[test]
    fn exporta_cada_formato_y_solo_la_seleccion() {
        for (json, ext, magic) in [
            (r#"{"format":"svg"}"#, "svg", b"<?xml".as_slice()),
            (r#"{"format":"dxf"}"#, "dxf", b"  0".as_slice()),
            (r#"{"format":"png","dpi":50}"#, "png", b"\x89PNG".as_slice()),
            (r#"{"format":"pdf"}"#, "pdf", b"%PDF".as_slice()),
            (r#"{"format":"ai"}"#, "ai", b"%PDF-1.5".as_slice()),
        ] {
            let req: ExportRequest = serde_json::from_str(json).unwrap();
            let path = temp(&format!("out.{ext}"));
            let sel = vec!["a".to_string()];
            let r = export_design(&doc(), Some(&sel), path.to_str().unwrap(), &req).unwrap();
            let bytes = std::fs::read(&path).unwrap();
            assert!(
                bytes.starts_with(magic),
                "{ext}: {:?}",
                &bytes[..8.min(bytes.len())]
            );
            assert_eq!(r.bytes as usize, bytes.len());
            assert!(
                (r.size_mm[0] - 10.0).abs() < 1e-6 || ext == "png" || ext == "pdf" || ext == "ai",
                "{ext}"
            );
            std::fs::remove_file(path).unwrap();
        }
        // Una selección que no existe no escribe nada.
        let path = temp("nada.svg");
        let none = vec!["z".to_string()];
        let req = ExportRequest::Svg(SvgOptions::default());
        assert!(export_design(&doc(), Some(&none), path.to_str().unwrap(), &req).is_err());
        assert!(!path.exists());
    }

    #[test]
    fn el_pdf_de_impresion_va_a_la_carpeta_temporal() {
        let (path, out) = render_print_pdf(&doc(), None, &PdfOptions::default()).unwrap();
        assert!(path.starts_with(std::env::temp_dir()));
        assert_eq!(std::fs::read(&path).unwrap(), out.bytes);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn opciones_de_importacion_se_validan() {
        let ok = ImportDialogOptions {
            scale_percent: 50.0,
            target_layer: Some("#00ff00".into()),
            ..Default::default()
        };
        let e = ok.extras().unwrap();
        assert_eq!(e.scale, 0.5);
        assert_eq!(e.target_layer, Some(Rgb(0, 255, 0)));
        for bad in [
            ImportDialogOptions {
                scale_percent: 0.0,
                ..Default::default()
            },
            ImportDialogOptions {
                dpi: Some(-1.0),
                ..Default::default()
            },
            ImportDialogOptions {
                target_layer: Some("verde".into()),
                ..Default::default()
            },
            ImportDialogOptions {
                gcode_default_power: Some(0.0),
                ..Default::default()
            },
        ] {
            assert!(bad.extras().is_err());
        }
        // G-code con M3 S0: vacío sin la opción, con ella se importa.
        let nc = temp("zero.nc");
        std::fs::write(&nc, "G21\nM3 S0\nG0 X0 Y0\nG1 X10 Y0\nG1 X10 Y10\nM5\n").unwrap();
        let e = import_with_options(nc.to_str().unwrap(), &ImportDialogOptions::default());
        assert!(e.unwrap_err().contains("potencia 0"));
        let r = import_with_options(
            nc.to_str().unwrap(),
            &ImportDialogOptions {
                gcode_default_power: Some(500.0),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(r.objects.len(), 1);
        std::fs::remove_file(nc).unwrap();
        // Se importa de verdad: un HPGL escalado al 200 %.
        let plt = temp("a.plt");
        std::fs::write(&plt, "IN;SP1;PU0,0;PD400,0;").unwrap();
        let r = import_with_options(
            plt.to_str().unwrap(),
            &ImportDialogOptions {
                scale_percent: 200.0,
                ..Default::default()
            },
        )
        .unwrap();
        let b = r.objects[0].bounds().unwrap();
        assert!((b.max.x - 20.0).abs() < 1e-6);
        std::fs::remove_file(plt).unwrap();
        // Los valores que faltan en el JSON son los de siempre.
        let d: ImportDialogOptions = serde_json::from_str("{}").unwrap();
        assert_eq!(d.scale_percent, 100.0);
        assert!(d.gcode_by_power && d.gcode_default_power.is_none());
        assert_eq!(d.pdf_page, 1);
    }
}
