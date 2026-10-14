//! Imagen de la cama: composición del mosaico de fotos de la cámara del cabezal.

use super::*;

// ---------------------------------------------------------------------------

/// Una foto ya corregida de lente con su posición en la cama.
#[derive(Debug, Clone, Deserialize)]
pub struct BedTile {
    pub image_png_base64: String,
    /// Píxel (origen en el borde de la imagen) -> mm de la cama, por filas.
    pub to_bed: [[f64; 3]; 3],
}

/// Imagen compuesta y dónde queda en el documento.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BedImage {
    /// Archivo PNG escrito; vacío si no se pidió guardar.
    pub path: String,
    pub width_px: u32,
    pub height_px: u32,
    /// Tamaño de la imagen en la cama (mm).
    pub size_mm: [f64; 2],
    /// Esquina inferior izquierda de la imagen en el documento (mm).
    pub origin_mm: [f64; 2],
    /// El proyecto no estaba guardado y el PNG quedó en la carpeta temporal.
    pub in_temp_dir: bool,
    /// Fracción de la imagen cubierta por alguna foto (0..1).
    pub coverage: f64,
    /// Píxeles por mm de la imagen.
    pub px_per_mm: f64,
    /// Vista previa en PNG (base64), reducida a un ancho máximo de 1600 px.
    pub preview_png_base64: String,
}

/// Ancho máximo de la vista previa que viaja a la interfaz.
pub(super) const PREVIEW_MAX_WIDTH: u32 = 1600;
/// Resolución por defecto y máxima de la imagen de la cama (px por mm).
pub(super) const DEFAULT_PX_PER_MM: f64 = 4.0;
pub(super) const MAX_PX_PER_MM: f64 = 20.0;

pub(super) fn decode_rgb(b64: &str) -> Result<image::RgbImage, String> {
    let raw = b64.split_once(',').map_or(b64, |(_, rest)| rest).trim();
    let bytes = STANDARD
        .decode(raw)
        .map_err(|e| format!("La imagen no es base64 válido: {e}"))?;
    Ok(image::load_from_memory(&bytes)
        .map_err(|e| format!("No se pudo decodificar la imagen: {e}"))?
        .to_rgb8())
}

pub(super) fn encode_png(img: &image::RgbImage) -> Result<Vec<u8>, String> {
    let mut cur = std::io::Cursor::new(Vec::new());
    img.write_to(&mut cur, image::ImageFormat::Png)
        .map_err(|e| format!("No se pudo codificar el PNG: {e}"))?;
    Ok(cur.into_inner())
}

/// Primer nombre libre `<stem>-camara-<n>.png` en `dir`.
pub(super) fn free_camera_name(dir: &Path, stem: &str) -> PathBuf {
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
        .map(|n| dir.join(format!("{stem}-camara-{n}.png")))
        .find(|p| !p.exists())
        .expect("siempre hay un número libre")
}

/// Opciones de [`run_compose_bed`].
#[derive(Debug, Clone)]
pub struct ComposeArgs {
    pub bed: [f64; 2],
    /// Resolución; si es `None` se usa la de la foto (con tope).
    pub px_per_mm: Option<f64>,
    pub grayscale: bool,
    pub feather_px: f64,
    pub crop_px: f64,
    /// Rectángulo (mm) `[x0, y0, x1, y1]`; si es `None`, lo que cubren las fotos dentro de la cama.
    pub rect: Option<[f64; 4]>,
    pub dir: Option<PathBuf>,
    pub stem: String,
    /// Escribir el PNG (si no, solo se devuelve la vista previa).
    pub save: bool,
}

/// Compone las fotos en una imagen alineada con la cama y, si se pide, la guarda como PNG.
pub fn run_compose_bed(tiles: &[BedTile], args: &ComposeArgs) -> Result<BedImage, String> {
    if tiles.is_empty() {
        return Err("No hay ninguna foto que componer.".into());
    }
    if tiles.len() > 400 {
        return Err("Son demasiadas fotos (máximo 400).".into());
    }
    if !(args.bed[0] > 0.0 && args.bed[1] > 0.0) {
        return Err("El tamaño de la cama no es válido.".into());
    }
    let images = tiles
        .iter()
        .enumerate()
        .map(|(i, t)| decode_rgb(&t.image_png_base64).map_err(|e| format!("Foto {}: {e}", i + 1)))
        .collect::<Result<Vec<_>, _>>()?;
    for (i, t) in tiles.iter().enumerate() {
        if t.to_bed.iter().flatten().any(|v| !v.is_finite()) {
            return Err(format!("La posición de la foto {} no es válida.", i + 1));
        }
    }
    let refs: Vec<Tile> = images
        .iter()
        .zip(tiles)
        .map(|(image, t)| Tile {
            image,
            to_bed: Homography { m: t.to_bed },
        })
        .collect();
    let rect = match args.rect {
        Some(r) => r,
        None => tlaku_camera::covered_rect(&refs, args.bed)
            .ok_or("Ninguna foto cae dentro de la cama: revisa la alineación de la cámara.")?,
    };
    let native = refs
        .iter()
        .map(tlaku_camera::tile_scale_px_per_mm)
        .fold(0.0_f64, f64::max);
    let ppm = args
        .px_per_mm
        .unwrap_or_else(|| native.clamp(1.0, DEFAULT_PX_PER_MM.max(native.min(8.0))))
        .clamp(0.5, MAX_PX_PER_MM);
    let mut opts = MosaicOptions::new(rect, ppm);
    opts.feather_px = args.feather_px.max(1.0);
    opts.crop_px = args.crop_px.max(0.0);
    let mut mosaic = tlaku_camera::compose(&refs, &opts)?;
    if args.grayscale {
        tlaku_camera::to_gray(&mut mosaic.image);
    }
    let (width_px, height_px) = mosaic.image.dimensions();
    let preview = if width_px > PREVIEW_MAX_WIDTH {
        let h = (u64::from(height_px) * u64::from(PREVIEW_MAX_WIDTH) / u64::from(width_px)).max(1);
        image::imageops::resize(
            &mosaic.image,
            PREVIEW_MAX_WIDTH,
            h as u32,
            image::imageops::FilterType::Triangle,
        )
    } else {
        mosaic.image.clone()
    };
    let preview_png_base64 = STANDARD.encode(encode_png(&preview)?);
    let (path, in_temp_dir) = if args.save {
        let (folder, temp) = match &args.dir {
            Some(d) if d.is_dir() => (d.clone(), false),
            _ => (std::env::temp_dir().join("tlaku-camera"), true),
        };
        fs::create_dir_all(&folder).map_err(|e| format!("{}: {e}", folder.display()))?;
        let path = free_camera_name(&folder, &args.stem);
        fs::write(&path, encode_png(&mosaic.image)?)
            .map_err(|e| format!("{}: {e}", path.display()))?;
        (path.to_string_lossy().into_owned(), temp)
    } else {
        (String::new(), false)
    };
    Ok(BedImage {
        path,
        width_px,
        height_px,
        size_mm: mosaic.size_mm,
        origin_mm: mosaic.origin_mm,
        in_temp_dir,
        coverage: mosaic.coverage,
        px_per_mm: mosaic.px_per_mm,
        preview_png_base64,
    })
}

#[tauri::command(rename_all = "snake_case")]
#[allow(clippy::too_many_arguments)]
pub async fn compose_bed_image(
    tiles: Vec<BedTile>,
    bed: [f64; 2],
    px_per_mm: Option<f64>,
    grayscale: bool,
    feather_px: Option<f64>,
    crop_px: Option<f64>,
    rect: Option<[f64; 4]>,
    dir: Option<String>,
    stem: String,
    save: bool,
) -> Result<BedImage, String> {
    tauri::async_runtime::spawn_blocking(move || {
        run_compose_bed(
            &tiles,
            &ComposeArgs {
                bed,
                px_per_mm,
                grayscale,
                feather_px: feather_px.unwrap_or(24.0),
                crop_px: crop_px.unwrap_or(0.0),
                rect,
                dir: dir.map(PathBuf::from),
                stem,
                save,
            },
        )
    })
    .await
    .map_err(blocking_error)?
}
