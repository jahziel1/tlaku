//! Carga de imágenes desde archivo sin materializar la imagen completa.
//!
//! - [`dimensions`] lee solo la cabecera.
//! - [`load_scaled`] decodifica y pasa a gris reduciendo sobre la marcha a la resolución que el
//!   trabajo necesita (ver [`crate::load_for_process`]). PNG se lee fila a fila y TIFF por
//!   franjas o teselas con las bibliotecas `png` y `tiff`, de modo que la memoria no depende del
//!   tamaño de la imagen. Los demás formatos (JPEG, BMP, WebP, GIF, TGA) los decodifica `image`
//!   entera con un límite de memoria generoso ([`DECODE_MAX_ALLOC`]) y se reducen fila a fila.
//! - [`convert_to_png`] escribe un TIFF como PNG también por franjas.

use crate::downsample::Downsampler;
use crate::gray::{GrayF32, Luma};
use image::{DynamicImage, ImageFormat, ImageReader, Limits};
use std::fs::File;
use std::io::{BufReader, BufWriter, Read};
use std::path::Path;
use tlaku_i18n::tr;

/// Memoria máxima, en bytes, que se concede a un decodificador de `image` (lo que ocupa la
/// imagen decodificada). El valor por defecto de la biblioteca (512 MiB) rechazaba imágenes
/// válidas; 2 GiB admite un PNG RGBA de 16 bits de ~250 Mpx o uno de 8 bits de ~500 Mpx.
pub const DECODE_MAX_ALLOC: u64 = 2 * 1024 * 1024 * 1024;

/// Tope de memoria de un trozo (franja o tesela) de TIFF y de sus tablas.
const TIFF_CHUNK_LIMIT: usize = 1 << 30;
const TIFF_TABLE_LIMIT: usize = 64 << 20;

/// Gris cargado y tamaño original `(ancho, alto)` de la imagen.
pub type Scaled = (GrayF32, (usize, usize));

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    Png,
    Tiff,
    Other,
}

/// Reconoce PNG y TIFF por su firma (la extensión puede mentir).
fn sniff(path: &Path) -> Kind {
    let mut head = [0u8; 8];
    let Ok(mut f) = File::open(path) else {
        return Kind::Other;
    };
    if f.read_exact(&mut head).is_err() {
        return Kind::Other;
    }
    if head == [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a] {
        Kind::Png
    } else if matches!(
        &head[..4],
        [b'I', b'I', 42 | 43, 0] | [b'M', b'M', 0, 42 | 43]
    ) {
        Kind::Tiff
    } else {
        Kind::Other
    }
}

fn open(path: &Path) -> Result<BufReader<File>, String> {
    File::open(path)
        .map(BufReader::new)
        .map_err(|e| tr!("raster.load.read_failed", path = path.display(), error = e))
}

fn limits() -> Limits {
    let mut l = Limits::default();
    l.max_alloc = Some(DECODE_MAX_ALLOC);
    l
}

/// Traduce el error de `image` a un mensaje útil.
fn image_error(e: &image::ImageError) -> String {
    let text = e.to_string();
    if matches!(e, image::ImageError::Limits(_)) || text.contains("emory limit") {
        return tr!("raster.load.memory_limit", mib = DECODE_MAX_ALLOC >> 20);
    }
    if text.contains("Tiff") || text.contains("TIFF") {
        return tr!("raster.load.rare_tiff", error = text);
    }
    tr!("raster.load.invalid_image", error = text)
}

fn tiff_error(e: &tiff::TiffError) -> String {
    match e {
        tiff::TiffError::LimitsExceeded => {
            tr!("raster.load.tiff_memory")
        }
        other => tr!("raster.load.tiff_read", error = other),
    }
}

fn png_error(e: &png::DecodingError) -> String {
    tr!("raster.load.invalid_png", error = e)
}

/// Tamaño en píxeles (ancho, alto) leyendo solo la cabecera.
pub fn dimensions(path: &Path) -> Result<(u32, u32), String> {
    match sniff(path) {
        Kind::Tiff => {
            // `image` rechaza ya en la cabecera los TIFF YCbCr; la biblioteca `tiff` no.
            let mut dec = tiff::decoder::Decoder::new(open(path)?).map_err(|e| tiff_error(&e))?;
            return dec.dimensions().map_err(|e| tiff_error(&e));
        }
        Kind::Png => {
            let reader = png::Decoder::new(open(path)?)
                .read_info()
                .map_err(|e| png_error(&e))?;
            let i = reader.info();
            return Ok((i.width, i.height));
        }
        Kind::Other => {}
    }
    let mut last = None;
    for hinted in [true, false] {
        let mut r = ImageReader::new(open(path)?);
        match (hinted, ImageFormat::from_path(path).ok()) {
            (true, Some(f)) => r.set_format(f),
            (true, None) => continue,
            (false, _) => {
                r = r.with_guessed_format().map_err(|e| {
                    tr!("raster.load.read_failed", path = path.display(), error = e)
                })?;
            }
        }
        match r.into_dimensions() {
            Ok(d) => return Ok(d),
            Err(e) => last = Some(e),
        }
    }
    Err(last.map_or_else(|| tr!("raster.load.unknown_format"), |e| image_error(&e)))
}

/// Tamaño en píxeles de una imagen en memoria, leyendo solo su cabecera.
pub fn dimensions_of_bytes(bytes: &[u8]) -> Result<(u32, u32), String> {
    ImageReader::new(std::io::Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|e| tr!("raster.load.invalid_image", error = e))?
        .into_dimensions()
        .map_err(|e| image_error(&e))
}

// --- Filas a gris ---------------------------------------------------------------------------

/// Disposición de las muestras de un píxel.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Fmt {
    Gray,
    GrayA,
    Rgb,
    Rgba,
    /// Y, Cb, Cr de 8 bits (JFIF, rango completo).
    YCbCr,
}

impl Fmt {
    fn samples(self) -> usize {
        match self {
            Fmt::Gray => 1,
            Fmt::GrayA => 2,
            Fmt::Rgb | Fmt::YCbCr => 3,
            Fmt::Rgba => 4,
        }
    }
}

/// Pasa una fila a gris con la misma aritmética que `from_dynamic` (alfa sobre blanco). `get`
/// devuelve la muestra `i` normalizada a 0..1.
fn row_to_gray(fmt: Fmt, out: &mut [f32], weights: [f32; 3], get: impl Fn(usize) -> f32) {
    let [wr, wg, wb] = weights;
    let n = fmt.samples();
    for (x, o) in out.iter_mut().enumerate() {
        let i = x * n;
        let (r, g, b, a) = match fmt {
            Fmt::Gray => {
                let v = get(i);
                (v, v, v, 1.0)
            }
            Fmt::GrayA => {
                let v = get(i);
                (v, v, v, get(i + 1))
            }
            Fmt::Rgb => (get(i), get(i + 1), get(i + 2), 1.0),
            Fmt::Rgba => (get(i), get(i + 1), get(i + 2), get(i + 3)),
            Fmt::YCbCr => {
                let [r, g, b] = ycbcr_to_rgb(get(i), get(i + 1), get(i + 2));
                (r, g, b, 1.0)
            }
        };
        let y = wr * r + wg * g + wb * b;
        *o = (y * a + (1.0 - a)).clamp(0.0, 1.0);
    }
}

/// YCbCr de rango completo a RGB (coeficientes de JFIF / ITU-R BT.601), valores 0..1.
fn ycbcr_to_rgb(y: f32, cb: f32, cr: f32) -> [f32; 3] {
    let (cb, cr) = (cb - 0.5, cr - 0.5);
    [
        (y + 1.402 * cr).clamp(0.0, 1.0),
        (y - 0.344_136 * cb - 0.714_136 * cr).clamp(0.0, 1.0),
        (y + 1.772 * cb).clamp(0.0, 1.0),
    ]
}

/// Gris de la fila `y` de una imagen ya decodificada.
fn dynamic_row(img: &DynamicImage, y: usize, weights: [f32; 3], out: &mut [f32]) {
    let w = img.width() as usize;
    macro_rules! row {
        ($buf:expr, $fmt:expr, $norm:expr) => {{
            let n = $fmt.samples();
            let raw = &$buf.as_raw()[y * w * n..(y + 1) * w * n];
            row_to_gray($fmt, out, weights, |i| $norm(raw[i]))
        }};
    }
    match img {
        DynamicImage::ImageLuma8(b) => row!(b, Fmt::Gray, |v: u8| f32::from(v) / 255.0),
        DynamicImage::ImageLumaA8(b) => row!(b, Fmt::GrayA, |v: u8| f32::from(v) / 255.0),
        DynamicImage::ImageRgb8(b) => row!(b, Fmt::Rgb, |v: u8| f32::from(v) / 255.0),
        DynamicImage::ImageRgba8(b) => row!(b, Fmt::Rgba, |v: u8| f32::from(v) / 255.0),
        DynamicImage::ImageLuma16(b) => row!(b, Fmt::Gray, |v: u16| f32::from(v) / 65535.0),
        DynamicImage::ImageLumaA16(b) => row!(b, Fmt::GrayA, |v: u16| f32::from(v) / 65535.0),
        DynamicImage::ImageRgb16(b) => row!(b, Fmt::Rgb, |v: u16| f32::from(v) / 65535.0),
        DynamicImage::ImageRgba16(b) => row!(b, Fmt::Rgba, |v: u16| f32::from(v) / 65535.0),
        DynamicImage::ImageRgb32F(b) => row!(b, Fmt::Rgb, |v: f32| v),
        DynamicImage::ImageRgba32F(b) => row!(b, Fmt::Rgba, |v: f32| v),
        other => {
            // Variante futura: se convierte la imagen entera (camino lento pero correcto).
            let rgba = other.to_rgba32f();
            let raw = &rgba.as_raw()[y * w * 4..(y + 1) * w * 4];
            row_to_gray(Fmt::Rgba, out, weights, |i| raw[i]);
        }
    }
}

/// Gris de una imagen decodificada, reducida a lo sumo a `max_w` x `max_h`.
pub(crate) fn dynamic_to_gray(
    img: &DynamicImage,
    luma: Luma,
    max_w: usize,
    max_h: usize,
) -> GrayF32 {
    let (w, h) = (img.width() as usize, img.height() as usize);
    let weights = luma.weights();
    let mut ds = Downsampler::new(w, h, max_w, max_h).expect("imagen decodificada sin tamaño cero");
    let mut row = vec![0.0f32; w];
    for y in 0..h {
        dynamic_row(img, y, weights, &mut row);
        ds.add(0, y, &row);
    }
    ds.finish()
}

// --- PNG por filas --------------------------------------------------------------------------

fn load_png(path: &Path, luma: Luma, max_w: usize, max_h: usize) -> Result<Option<Scaled>, String> {
    let mut dec = png::Decoder::new(open(path)?);
    dec.set_transformations(png::Transformations::EXPAND);
    let mut reader = dec.read_info().map_err(|e| png_error(&e))?;
    let info = reader.info();
    if info.interlaced {
        return Ok(None);
    }
    let (w, h) = (info.width as usize, info.height as usize);
    let (color, depth) = reader.output_color_type();
    let fmt = match color {
        png::ColorType::Grayscale => Fmt::Gray,
        png::ColorType::GrayscaleAlpha => Fmt::GrayA,
        png::ColorType::Rgb => Fmt::Rgb,
        png::ColorType::Rgba => Fmt::Rgba,
        png::ColorType::Indexed => return Ok(None),
    };
    let wide = match depth {
        png::BitDepth::Eight => false,
        png::BitDepth::Sixteen => true,
        _ => return Ok(None),
    };
    let weights = luma.weights();
    let mut ds = Downsampler::new(w, h, max_w, max_h)?;
    let mut row = vec![0.0f32; w];
    let mut y = 0;
    while let Some(r) = reader.next_row().map_err(|e| png_error(&e))? {
        if y >= h {
            break;
        }
        let data = r.data();
        if wide {
            row_to_gray(fmt, &mut row, weights, |i| {
                f32::from(u16::from_be_bytes([data[2 * i], data[2 * i + 1]])) / 65535.0
            });
        } else {
            row_to_gray(fmt, &mut row, weights, |i| f32::from(data[i]) / 255.0);
        }
        ds.add(0, y, &row);
        y += 1;
    }
    if y < h {
        return Err(tr!("raster.load.png_truncated", y = y, h = h));
    }
    Ok(Some((ds.finish(), (w, h))))
}

// --- TIFF por franjas o teselas -------------------------------------------------------------

/// Datos de un trozo del TIFF: posición, tamaño y muestras.
struct Chunk {
    x0: usize,
    y0: usize,
    w: usize,
    h: usize,
    data: tiff::decoder::DecodingResult,
}

struct TiffSource {
    dec: tiff::decoder::Decoder<BufReader<File>>,
    width: usize,
    height: usize,
    fmt: Fmt,
    wide: bool,
    count: u32,
    tiles_across: usize,
    chunk_w: usize,
    chunk_h: usize,
    tiled: bool,
}

impl TiffSource {
    /// `Ok(None)` si el TIFF tiene una disposición que este camino no resuelve (planar,
    /// paleta, 32 bits, 1 bit...): el llamador cae en `image`.
    fn open(path: &Path) -> Result<Option<Self>, String> {
        use tiff::ColorType as C;
        let mut limits = tiff::decoder::Limits::default();
        limits.decoding_buffer_size = TIFF_CHUNK_LIMIT;
        limits.intermediate_buffer_size = TIFF_CHUNK_LIMIT;
        limits.ifd_value_size = TIFF_TABLE_LIMIT;
        let mut dec = tiff::decoder::Decoder::new(open(path)?)
            .map_err(|e| tiff_error(&e))?
            .with_limits(limits);
        let (w, h) = dec.dimensions().map_err(|e| tiff_error(&e))?;
        let ct = dec.colortype().map_err(|e| tiff_error(&e))?;
        let (fmt, wide) = match ct {
            C::Gray(8) => (Fmt::Gray, false),
            C::Gray(16) => (Fmt::Gray, true),
            C::GrayA(8) => (Fmt::GrayA, false),
            C::GrayA(16) => (Fmt::GrayA, true),
            C::RGB(8) => (Fmt::Rgb, false),
            C::RGB(16) => (Fmt::Rgb, true),
            C::RGBA(8) => (Fmt::Rgba, false),
            C::RGBA(16) => (Fmt::Rgba, true),
            C::YCbCr(8) => (Fmt::YCbCr, false),
            _ => return Ok(None),
        };
        let planar = dec
            .find_tag_unsigned::<u16>(tiff::tags::Tag::PlanarConfiguration)
            .map_err(|e| tiff_error(&e))?;
        if planar == Some(2) {
            return Ok(None);
        }
        let tiled = dec.get_chunk_type() == tiff::decoder::ChunkType::Tile;
        let count = if tiled {
            dec.tile_count()
        } else {
            dec.strip_count()
        }
        .map_err(|e| tiff_error(&e))?;
        let (cw, ch) = dec.chunk_dimensions();
        let (width, height) = (w as usize, h as usize);
        Ok(Some(Self {
            dec,
            width,
            height,
            fmt,
            wide,
            count,
            tiles_across: width.div_ceil((cw as usize).max(1)),
            chunk_w: cw as usize,
            chunk_h: ch as usize,
            tiled,
        }))
    }

    fn chunk(&mut self, i: u32) -> Result<Chunk, String> {
        let (w, h) = self.dec.chunk_data_dimensions(i);
        let (x0, y0) = if self.tiled {
            let i = i as usize;
            (
                (i % self.tiles_across) * self.chunk_w,
                (i / self.tiles_across) * self.chunk_h,
            )
        } else {
            (0, i as usize * self.chunk_h)
        };
        let data = self.dec.read_chunk(i).map_err(|e| tiff_error(&e))?;
        Ok(Chunk {
            x0,
            y0,
            w: w as usize,
            h: h as usize,
            data,
        })
    }
}

/// Normaliza la muestra `i` del trozo a 0..1.
fn chunk_sample(data: &tiff::decoder::DecodingResult, i: usize) -> f32 {
    use tiff::decoder::DecodingResult as D;
    match data {
        D::U8(v) => f32::from(v[i]) / 255.0,
        D::U16(v) => f32::from(v[i]) / 65535.0,
        _ => 0.0,
    }
}

fn check_chunk(c: &Chunk, fmt: Fmt) -> Result<(), String> {
    use tiff::decoder::DecodingResult as D;
    let len = match &c.data {
        D::U8(v) => v.len(),
        D::U16(v) => v.len(),
        _ => 0,
    };
    if len < c.w * c.h * fmt.samples() {
        return Err(tr!("raster.load.tiff_chunk"));
    }
    Ok(())
}

fn load_tiff(
    path: &Path,
    luma: Luma,
    max_w: usize,
    max_h: usize,
) -> Result<Option<Scaled>, String> {
    let Some(mut src) = TiffSource::open(path)? else {
        return Ok(None);
    };
    let weights = luma.weights();
    let mut ds = Downsampler::new(src.width, src.height, max_w, max_h)?;
    let mut row = vec![0.0f32; src.chunk_w.max(1)];
    let n = src.fmt.samples();
    for i in 0..src.count {
        let c = src.chunk(i)?;
        check_chunk(&c, src.fmt)?;
        if row.len() < c.w {
            row.resize(c.w, 0.0);
        }
        for r in 0..c.h {
            let base = r * c.w * n;
            row_to_gray(src.fmt, &mut row[..c.w], weights, |k| {
                chunk_sample(&c.data, base + k)
            });
            ds.add(c.x0, c.y0 + r, &row[..c.w]);
        }
    }
    let (w, h) = (src.width, src.height);
    Ok(Some((ds.finish(), (w, h))))
}

// --- Carga general --------------------------------------------------------------------------

/// Carga `path` en gris reduciendo a lo sumo a `max_w` x `max_h` píxeles (un eje que ya cabe no
/// se toca; nunca amplía). Devuelve también el tamaño original. El gris es el mismo que daría
/// decodificar toda la imagen y reducirla después con [`crate::resample::resample`].
pub fn load_scaled(path: &Path, luma: Luma, max_w: usize, max_h: usize) -> Result<Scaled, String> {
    let fail = |e: String| format!("{}: {e}", path.display());
    match sniff(path) {
        Kind::Png => {
            if let Some(r) = load_png(path, luma, max_w, max_h).map_err(&fail)? {
                return Ok(r);
            }
        }
        Kind::Tiff => {
            if let Some(r) = load_tiff(path, luma, max_w, max_h).map_err(&fail)? {
                return Ok(r);
            }
        }
        Kind::Other => {}
    }
    let img = decode_with_image(path).map_err(fail)?;
    let size = (img.width() as usize, img.height() as usize);
    Ok((dynamic_to_gray(&img, luma, max_w, max_h), size))
}

/// Decodifica con `image` usando el formato que sugiere la extensión y, si falla (la extensión
/// miente o TGA no tiene firma), el que se reconoce por los bytes.
fn decode_with_image(path: &Path) -> Result<DynamicImage, String> {
    let hint = ImageFormat::from_path(path).ok();
    let mut first = None;
    if let Some(f) = hint {
        let mut r = ImageReader::with_format(open(path)?, f);
        r.limits(limits());
        match r.decode() {
            Ok(img) => return Ok(img),
            Err(e) => first = Some(e),
        }
    }
    let mut r = ImageReader::new(open(path)?)
        .with_guessed_format()
        .map_err(|e| tr!("raster.load.read_failed_nopath", error = e))?;
    r.limits(limits());
    match r.decode() {
        Ok(img) => Ok(img),
        // Si el formato guiado por la extensión ya dio un error específico, ese es el útil.
        Err(e) => Err(image_error(first.as_ref().unwrap_or(&e))),
    }
}

// --- TIFF a PNG por franjas -----------------------------------------------------------------

/// Escribe `src` como PNG en `dst`. Un TIFF de 8 o 16 bits se convierte por franjas (la memoria
/// es una fila de teselas); el resto de formatos se decodifica entero con `image`.
pub fn convert_to_png(src: &Path, dst: &Path) -> Result<(), String> {
    if sniff(src) == Kind::Tiff
        && let Some(r) = tiff_to_png(src, dst)
    {
        return r;
    }
    let img = decode_with_image(src)
        .map_err(|e| tr!("raster.load.read_failed", path = src.display(), error = e))?;
    img.save_with_format(dst, ImageFormat::Png)
        .map_err(|e| tr!("raster.load.write_failed", path = dst.display(), error = e))
}

/// `None` si el TIFF no se puede recorrer por trozos (el llamador usa `image`).
fn tiff_to_png(src: &Path, dst: &Path) -> Option<Result<(), String>> {
    let mut t = TiffSource::open(src).ok().flatten()?;
    Some(tiff_to_png_inner(&mut t, src, dst))
}

fn tiff_to_png_inner(t: &mut TiffSource, src: &Path, dst: &Path) -> Result<(), String> {
    let fail = |e: String| tr!("raster.load.read_failed", path = src.display(), error = e);
    let (color, out_samples) = match t.fmt {
        Fmt::Gray => (png::ColorType::Grayscale, 1),
        Fmt::GrayA => (png::ColorType::GrayscaleAlpha, 2),
        Fmt::Rgb | Fmt::YCbCr => (png::ColorType::Rgb, 3),
        Fmt::Rgba => (png::ColorType::Rgba, 4),
    };
    let bytes = if t.wide { 2 } else { 1 };
    let px = out_samples * bytes;
    let file = File::create(dst)
        .map_err(|e| tr!("raster.load.write_failed", path = dst.display(), error = e))?;
    let mut enc = png::Encoder::new(BufWriter::new(file), t.width as u32, t.height as u32);
    enc.set_color(color);
    enc.set_depth(if t.wide {
        png::BitDepth::Sixteen
    } else {
        png::BitDepth::Eight
    });
    enc.set_compression(png::Compression::Fast);
    let werr =
        |e: png::EncodingError| tr!("raster.load.write_failed", path = dst.display(), error = e);
    let mut writer = enc.write_header().map_err(werr)?;
    let mut stream = writer.stream_writer().map_err(werr)?;

    // Franja de filas completas que se va llenando con los trozos y se vuelca al terminar.
    let stride = t.width * px;
    let mut band: Vec<u8> = Vec::new();
    let mut band_y0 = 0usize;
    let mut band_rows = 0usize;
    let mut next_row = 0usize;
    let n_in = t.fmt.samples();
    let flush = |band: &mut Vec<u8>, rows: usize, stream: &mut dyn std::io::Write| {
        stream
            .write_all(&band[..rows * stride])
            .map_err(|e| tr!("raster.load.write_failed", path = dst.display(), error = e))
    };
    for i in 0..t.count {
        let c = t.chunk(i).map_err(fail)?;
        check_chunk(&c, t.fmt).map_err(fail)?;
        if c.y0 != band_y0 && band_rows > 0 {
            flush(&mut band, band_rows, &mut stream)?;
            next_row = band_y0 + band_rows;
            band_rows = 0;
        }
        if band_rows == 0 {
            band_y0 = c.y0;
            if band_y0 != next_row {
                return Err(fail(tr!("raster.load.tiff_order")));
            }
        }
        band_rows = band_rows.max(c.h);
        if band.len() < band_rows * stride {
            band.resize(band_rows * stride, 0);
        }
        for r in 0..c.h {
            for x in 0..c.w {
                let o = r * stride + (c.x0 + x) * px;
                let s = (r * c.w + x) * n_in;
                write_px(t.fmt, t.wide, &c.data, s, &mut band[o..o + px]);
            }
        }
    }
    if band_rows > 0 {
        flush(&mut band, band_rows, &mut stream)?;
    }
    stream.finish().map_err(werr)?;
    Ok(())
}

/// Un píxel del TIFF en el formato del PNG (big-endian si es de 16 bits).
fn write_px(fmt: Fmt, wide: bool, data: &tiff::decoder::DecodingResult, s: usize, out: &mut [u8]) {
    use tiff::decoder::DecodingResult as D;
    if fmt == Fmt::YCbCr {
        if let D::U8(v) = data {
            let n = |i: usize| f32::from(v[s + i]) / 255.0;
            let rgb = ycbcr_to_rgb(n(0), n(1), n(2));
            for (o, c) in out.iter_mut().zip(rgb) {
                *o = (c * 255.0).round() as u8;
            }
        }
        return;
    }
    match data {
        D::U8(v) => out.copy_from_slice(&v[s..s + out.len()]),
        D::U16(v) if wide => {
            for (k, o) in out.chunks_exact_mut(2).enumerate() {
                o.copy_from_slice(&v[s + k].to_be_bytes());
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resample::resample;
    use image::{Rgb, RgbImage, Rgba, RgbaImage};
    use tiff::encoder::{TiffEncoder, colortype};

    fn dir(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("tlaku-load-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn rgb_noise(w: u32, h: u32) -> RgbImage {
        RgbImage::from_fn(w, h, |x, y| {
            Rgb([
                ((x * 7 + y * 3) % 256) as u8,
                ((x * 13 + y * 5 + 40) % 256) as u8,
                ((x * y + 90) % 256) as u8,
            ])
        })
    }

    fn assert_close(a: &GrayF32, b: &GrayF32, tol: f32) {
        assert_eq!((a.width, a.height), (b.width, b.height));
        for (i, (x, y)) in a.data.iter().zip(&b.data).enumerate() {
            assert!((x - y).abs() <= tol, "píxel {i}: {x} contra {y}");
        }
    }

    /// El gris reducido en la carga coincide con decodificar entero y remuestrear después.
    fn check_against_full(path: &Path, max_w: usize, max_h: usize) {
        let full = crate::gray::load(path, Luma::default()).unwrap();
        let (small, size) = load_scaled(path, Luma::default(), max_w, max_h).unwrap();
        assert_eq!(size, (full.width, full.height));
        let want = resample(&full, full.width.min(max_w), full.height.min(max_h));
        assert_close(&small, &want, 1e-5);
    }

    #[test]
    fn png_by_rows_matches_the_full_decode() {
        let d = dir("png");
        let rgb = d.join("rgb.png");
        rgb_noise(37, 29).save(&rgb).unwrap();
        check_against_full(&rgb, 10, 8);
        check_against_full(&rgb, 37, 29);
        check_against_full(&rgb, 100, 5);
        // RGBA de 16 bits y gris con alfa: el alfa se compone sobre blanco.
        let rgba16 = d.join("rgba16.png");
        image::DynamicImage::ImageRgba16(image::ImageBuffer::from_fn(21, 17, |x, y| {
            image::Rgba([
                (x * 3000) as u16,
                (y * 3500) as u16,
                40000,
                (x * y * 200) as u16,
            ])
        }))
        .save(&rgba16)
        .unwrap();
        check_against_full(&rgba16, 7, 6);
        let ga = d.join("ga.png");
        image::DynamicImage::ImageLumaA8(image::ImageBuffer::from_fn(16, 12, |x, y| {
            image::LumaA([(x * 15) as u8, (y * 20) as u8])
        }))
        .save(&ga)
        .unwrap();
        check_against_full(&ga, 4, 3);
        std::fs::remove_dir_all(d).unwrap();
    }

    #[test]
    fn streaming_gray_equals_the_old_whole_image_conversion() {
        // `from_dynamic` ya no pasa por `to_rgba32f`: debe dar lo mismo que antes.
        let img = RgbaImage::from_fn(9, 7, |x, y| {
            Rgba([(x * 20) as u8, (y * 30) as u8, 77, (x * 28) as u8])
        });
        let dynamic = DynamicImage::ImageRgba8(img);
        for luma in [Luma::Rec709, Luma::Rec601] {
            let new = crate::gray::from_dynamic(&dynamic, luma);
            let [wr, wg, wb] = luma.weights();
            for (p, got) in dynamic.to_rgba32f().pixels().zip(&new.data) {
                let [r, g, b, a] = p.0;
                let want = ((wr * r + wg * g + wb * b) * a + (1.0 - a)).clamp(0.0, 1.0);
                assert_eq!(want, *got);
            }
        }
    }

    #[test]
    fn other_formats_go_through_image() {
        let d = dir("bmp");
        let bmp = d.join("a.bmp");
        rgb_noise(23, 19).save(&bmp).unwrap();
        check_against_full(&bmp, 6, 5);
        assert_eq!(dimensions(&bmp).unwrap(), (23, 19));
        let png = d.join("a.png");
        rgb_noise(5, 4).save(&png).unwrap();
        assert_eq!(dimensions(&png).unwrap(), (5, 4));
        // Extensión que miente: se reconoce por los bytes.
        let liar = d.join("b.jpg");
        std::fs::copy(&png, &liar).unwrap();
        assert_eq!(dimensions(&liar).unwrap(), (5, 4));
        assert_eq!(load_scaled(&liar, Luma::default(), 9, 9).unwrap().1, (5, 4));
        assert!(dimensions(&d.join("no-existe.png")).is_err());
        std::fs::remove_dir_all(d).unwrap();
    }

    fn write_tiff<C: colortype::ColorType<Inner = u8>>(
        path: &Path,
        w: u32,
        h: u32,
        rows_per_strip: u32,
        data: &[u8],
    ) {
        let mut enc = TiffEncoder::new(std::fs::File::create(path).unwrap()).unwrap();
        let mut image = enc.new_image::<C>(w, h).unwrap();
        image.rows_per_strip(rows_per_strip).unwrap();
        let mut done = 0;
        while image.next_strip_sample_count() > 0 {
            let n = image.next_strip_sample_count() as usize;
            image.write_strip(&data[done..done + n]).unwrap();
            done += n;
        }
        image.finish().unwrap();
    }

    #[test]
    fn tiff_by_strips_matches_the_full_decode() {
        let d = dir("tiff");
        let img = rgb_noise(41, 33);
        let path = d.join("a.tif");
        write_tiff::<colortype::RGB8>(&path, 41, 33, 7, img.as_raw());
        check_against_full(&path, 11, 9);
        check_against_full(&path, 41, 33);
        std::fs::remove_dir_all(d).unwrap();
    }

    #[test]
    fn tiff_ycbcr_is_converted_to_rgb() {
        let d = dir("ycbcr");
        let (w, h) = (18u32, 14u32);
        let ycc = RgbImage::from_fn(w, h, |x, y| {
            Rgb([(x * 14) as u8, (100 + x * 5) as u8, (60 + y * 9) as u8])
        });
        let path = d.join("y.tif");
        write_tiff::<colortype::YCbCr8>(&path, w, h, 5, ycc.as_raw());
        // La cabecera se lee aunque `image` rechace este TIFF.
        assert_eq!(dimensions(&path).unwrap(), (w, h));
        let expected = RgbImage::from_fn(w, h, |x, y| {
            let p = ycc.get_pixel(x, y).0;
            let n = |v: u8| f32::from(v) / 255.0;
            let rgb = ycbcr_to_rgb(n(p[0]), n(p[1]), n(p[2]));
            Rgb(rgb.map(|c| (c * 255.0).round() as u8))
        });
        let want = crate::gray::from_dynamic(&DynamicImage::ImageRgb8(expected), Luma::default());
        let (got, size) = load_scaled(&path, Luma::default(), 100, 100).unwrap();
        assert_eq!(size, (w as usize, h as usize));
        assert_close(&got, &want, 0.004);
        // Y la conversión a PNG conserva los colores.
        let png = d.join("y.png");
        convert_to_png(&path, &png).unwrap();
        let back = image::open(&png).unwrap().to_rgb8();
        assert_eq!(back.dimensions(), (w, h));
        for (a, b) in back.pixels().zip(
            RgbImage::from_fn(w, h, |x, y| {
                let p = ycc.get_pixel(x, y).0;
                let n = |v: u8| f32::from(v) / 255.0;
                Rgb(ycbcr_to_rgb(n(p[0]), n(p[1]), n(p[2])).map(|c| (c * 255.0).round() as u8))
            })
            .pixels(),
        ) {
            assert_eq!(a, b);
        }
        std::fs::remove_dir_all(d).unwrap();
    }

    #[test]
    fn tiff_to_png_is_lossless_by_strips() {
        let d = dir("tiff-png");
        let img = rgb_noise(30, 25);
        let tif = d.join("a.tif");
        write_tiff::<colortype::RGB8>(&tif, 30, 25, 4, img.as_raw());
        let png = d.join("a.png");
        convert_to_png(&tif, &png).unwrap();
        assert_eq!(image::open(&png).unwrap().to_rgb8(), img);
        std::fs::remove_dir_all(d).unwrap();
    }

    #[test]
    fn broken_files_report_errors() {
        let d = dir("broken");
        let bad = d.join("x.tif");
        std::fs::write(&bad, b"II*\0\x08\0\0\0no es un tiff").unwrap();
        let e = load_scaled(&bad, Luma::default(), 4, 4).unwrap_err();
        assert!(e.contains("PNG"), "{e}");
        let trunc = d.join("t.png");
        let mut bytes = Vec::new();
        rgb_noise(40, 40)
            .write_to(&mut std::io::Cursor::new(&mut bytes), ImageFormat::Png)
            .unwrap();
        std::fs::write(&trunc, &bytes[..bytes.len() / 2]).unwrap();
        assert!(load_scaled(&trunc, Luma::default(), 4, 4).is_err());
        std::fs::remove_dir_all(d).unwrap();
    }
}
