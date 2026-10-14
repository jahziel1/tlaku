//! Archivos sintéticos de tamaño extremo. Deterministas (sin dependencias aleatorias) y de
//! dominio público: los genera esta herramienta.

use std::fmt::Write as _;
use std::fs;
use std::io::{BufWriter, Write};
use std::path::Path;

/// Generador congruencial lineal: lo justo para variar sin traer una dependencia.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> f64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        ((self.0 >> 33) as f64) / f64::from(1u32 << 31)
    }
}

/// Punto `d` de la curva de Hilbert de lado `n` (potencia de 2).
fn hilbert(n: u32, d: u32) -> (u32, u32) {
    let (mut x, mut y, mut t) = (0u32, 0u32, d);
    let mut s = 1u32;
    while s < n {
        let rx = 1 & (t / 2);
        let ry = 1 & (t ^ rx);
        if ry == 0 {
            if rx == 1 {
                x = s - 1 - x;
                y = s - 1 - y;
            }
            std::mem::swap(&mut x, &mut y);
        }
        x += s * rx;
        y += s * ry;
        t /= 4;
        s *= 2;
    }
    (x, y)
}

type Gen = fn(&Path) -> Result<(), String>;

pub const ALL: &[(&str, Gen)] = &[
    ("synth-rects-50k.svg", rects_50k),
    ("synth-path-1m-nodes.svg", path_1m),
    ("synth-text-dense.svg", text_dense),
    ("synth-nested-groups.svg", nested_groups),
    ("synth-circles-20k.svg", circles_20k),
    ("synth-curves-200k.svg", curves_200k),
    ("synth-dxf-lines-100k.dxf", dxf_lines),
    ("synth-dxf-bulge-20k.dxf", dxf_bulge),
    ("synth-dxf-splines-5k.dxf", dxf_splines),
    ("synth-gcode-1m.nc", gcode_1m),
    ("synth-hpgl-500k.plt", hpgl_500k),
    ("synth-image-10000x8000.png", image_big),
    ("synth-image-12000x9000.jpg", image_jpeg),
];

/// Extremos para buscar el límite: 4 M de nodos en un trayecto, 200 000 rectángulos, 1 M de LINE
/// en un DXF y 4,2 M de líneas de G-code.
pub const XL: &[(&str, Gen)] = &[
    ("synth-xl-path-4m-nodes.svg", path_4m),
    ("synth-xl-rects-200k.svg", rects_200k),
    ("synth-xl-dxf-lines-1m.dxf", dxf_lines_1m),
    ("synth-xl-gcode-4m.nc", gcode_5m),
];

pub fn run(dir: &Path, only: Option<&str>, xl: bool) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    for (name, f) in if xl { XL } else { ALL } {
        if only.is_some_and(|o| !name.contains(o)) {
            continue;
        }
        let path = dir.join(name);
        let t = std::time::Instant::now();
        f(&path)?;
        let size = fs::metadata(&path).map_or(0, |m| m.len());
        eprintln!(
            "{name}: {:.1} MB en {:.1} s",
            size as f64 / 1e6,
            t.elapsed().as_secs_f64()
        );
    }
    Ok(())
}

fn out(path: &Path) -> Result<BufWriter<fs::File>, String> {
    fs::File::create(path)
        .map(BufWriter::new)
        .map_err(|e| format!("{}: {e}", path.display()))
}

fn io<T>(r: std::io::Result<T>) -> Result<T, String> {
    r.map_err(|e| e.to_string())
}

const SVG_HEAD: &str = "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"380mm\" height=\"380mm\" viewBox=\"0 0 380 380\">\n";

/// 50 000 rectángulos sueltos en una rejilla de 250 x 200.
fn rects_50k(path: &Path) -> Result<(), String> {
    rects(path, 250, 200, 1.5, 1.8)
}

/// 200 000 rectángulos en una rejilla de 500 x 400.
fn rects_200k(path: &Path) -> Result<(), String> {
    rects(path, 500, 400, 0.75, 0.9)
}

fn rects(path: &Path, cols: u32, rows: u32, dx: f64, dy: f64) -> Result<(), String> {
    let mut w = out(path)?;
    io(w.write_all(SVG_HEAD.as_bytes()))?;
    io(w.write_all(b"<g fill=\"none\" stroke=\"#ff0000\" stroke-width=\"0.05\">\n"))?;
    for j in 0..rows {
        for i in 0..cols {
            io(writeln!(
                w,
                "<rect x=\"{:.2}\" y=\"{:.2}\" width=\"{:.2}\" height=\"{:.2}\"/>",
                2.0 + f64::from(i) * dx,
                2.0 + f64::from(j) * dy,
                dx * 0.7,
                dy * 0.7
            ))?;
        }
    }
    io(w.write_all(b"</g></svg>\n"))
}

/// Un solo trayecto con 1 048 576 nodos (curva de Hilbert de orden 10).
fn path_1m(path: &Path) -> Result<(), String> {
    hilbert_path(path, 1024)
}

/// Un solo trayecto con 4 194 304 nodos (orden 11).
fn path_4m(path: &Path) -> Result<(), String> {
    hilbert_path(path, 2048)
}

fn hilbert_path(path: &Path, n: u32) -> Result<(), String> {
    let mut w = out(path)?;
    io(w.write_all(SVG_HEAD.as_bytes()))?;
    io(w.write_all(b"<path fill=\"none\" stroke=\"#0000ff\" stroke-width=\"0.05\" d=\""))?;
    let cell = 376.0 / f64::from(n);
    let mut s = String::new();
    for d in 0..n * n {
        let (x, y) = hilbert(n, d);
        s.clear();
        let cmd = if d == 0 { 'M' } else { 'L' };
        let _ = write!(
            s,
            "{cmd}{:.3} {:.3}",
            2.0 + f64::from(x) * cell,
            2.0 + f64::from(y) * cell
        );
        io(w.write_all(s.as_bytes()))?;
        if d % 16 == 15 {
            io(w.write_all(b"\n"))?;
        }
    }
    io(w.write_all(b"\"/></svg>\n"))
}

/// 120 líneas de texto de 100 caracteres (12 000 glifos), con tres familias.
fn text_dense(path: &Path) -> Result<(), String> {
    let mut w = out(path)?;
    io(w.write_all(SVG_HEAD.as_bytes()))?;
    let words = [
        "tlaku",
        "laser",
        "corte",
        "grabado",
        "madera",
        "acrilico",
        "cuero",
        "papel",
        "tinta",
        "codice",
        "nahuatl",
        "diseno",
        "potencia",
        "velocidad",
        "capa",
    ];
    let fonts = ["sans-serif", "serif", "monospace"];
    let mut rng = Lcg(7);
    for line in 0..120 {
        let mut text = String::new();
        while text.len() < 98 {
            text.push_str(words[(rng.next() * words.len() as f64) as usize % words.len()]);
            text.push(' ');
        }
        io(writeln!(
            w,
            "<text x=\"4\" y=\"{:.2}\" font-family=\"{}\" font-size=\"2.4\" fill=\"none\" stroke=\"#000\" stroke-width=\"0.03\">{}</text>",
            5.0 + f64::from(line) * 3.0,
            fonts[line as usize % 3],
            text.trim_end()
        ))?;
    }
    io(w.write_all(b"</svg>\n"))
}

/// Grupos anidados 300 niveles, cada uno con su transformación y un clipPath en cada décimo.
fn nested_groups(path: &Path) -> Result<(), String> {
    let mut w = out(path)?;
    io(w.write_all(SVG_HEAD.as_bytes()))?;
    io(w.write_all(b"<defs>\n"))?;
    for i in 0..30 {
        io(writeln!(
            w,
            "<clipPath id=\"c{i}\"><circle cx=\"190\" cy=\"190\" r=\"{}\"/></clipPath>",
            400 - i
        ))?;
    }
    io(w.write_all(b"</defs>\n"))?;
    let depth = 300;
    for d in 0..depth {
        let clip = if d % 10 == 0 {
            format!(" clip-path=\"url(#c{})\"", d / 10)
        } else {
            String::new()
        };
        io(writeln!(
            w,
            "<g transform=\"rotate(0.3 190 190) translate(0.01 0.01) scale(0.9995)\"{clip}>\
             <rect x=\"20\" y=\"20\" width=\"340\" height=\"340\" fill=\"none\" stroke=\"#00aa00\" stroke-width=\"0.1\"/>"
        ))?;
    }
    for _ in 0..depth {
        io(w.write_all(b"</g>"))?;
    }
    io(w.write_all(b"</svg>\n"))
}

/// 20 000 círculos (arcos): muchas curvas pequeñas.
fn circles_20k(path: &Path) -> Result<(), String> {
    let mut w = out(path)?;
    io(w.write_all(SVG_HEAD.as_bytes()))?;
    io(w.write_all(b"<g fill=\"none\" stroke=\"#ff00ff\" stroke-width=\"0.05\">\n"))?;
    for j in 0..100 {
        for i in 0..200 {
            io(writeln!(
                w,
                "<circle cx=\"{:.2}\" cy=\"{:.2}\" r=\"0.8\"/>",
                2.0 + f64::from(i) * 1.88,
                2.0 + f64::from(j) * 3.7
            ))?;
        }
    }
    io(w.write_all(b"</g></svg>\n"))
}

/// 200 000 segmentos Bézier cúbicos encadenados en trayectos de 1 000 (ruido suave).
fn curves_200k(path: &Path) -> Result<(), String> {
    let mut w = out(path)?;
    io(w.write_all(SVG_HEAD.as_bytes()))?;
    let mut rng = Lcg(42);
    for p in 0..200 {
        let (mut x, mut y) = (rng.next() * 370.0 + 5.0, rng.next() * 370.0 + 5.0);
        io(write!(
            w,
            "<path fill=\"none\" stroke=\"#ff8800\" stroke-width=\"0.05\" d=\"M{x:.2} {y:.2}"
        ))?;
        for _ in 0..1000 {
            let step = |v: f64, r: f64| (v + (r - 0.5) * 6.0).clamp(2.0, 378.0);
            let c1 = (step(x, rng.next()), step(y, rng.next()));
            let c2 = (step(x, rng.next()), step(y, rng.next()));
            let e = (step(x, rng.next()), step(y, rng.next()));
            io(write!(
                w,
                " C{:.2} {:.2} {:.2} {:.2} {:.2} {:.2}",
                c1.0, c1.1, c2.0, c2.1, e.0, e.1
            ))?;
            (x, y) = e;
        }
        io(writeln!(w, "\"/>"))?;
        let _ = p;
    }
    io(w.write_all(b"</svg>\n"))
}

fn dxf_head(w: &mut impl Write) -> Result<(), String> {
    io(w.write_all(
        b"0\nSECTION\n2\nHEADER\n9\n$ACADVER\n1\nAC1015\n9\n$INSUNITS\n70\n4\n0\nENDSEC\n0\nSECTION\n2\nENTITIES\n",
    ))
}

fn dxf_tail(w: &mut impl Write) -> Result<(), String> {
    io(w.write_all(b"0\nENDSEC\n0\nEOF\n"))
}

/// 100 000 LINE sueltas.
fn dxf_lines(path: &Path) -> Result<(), String> {
    dxf_n_lines(path, 100_000)
}

/// 1 000 000 LINE sueltas.
fn dxf_lines_1m(path: &Path) -> Result<(), String> {
    dxf_n_lines(path, 1_000_000)
}

fn dxf_n_lines(path: &Path, count: u32) -> Result<(), String> {
    let mut w = out(path)?;
    dxf_head(&mut w)?;
    let mut rng = Lcg(3);
    for i in 0..count {
        let (x, y) = (rng.next() * 360.0, rng.next() * 360.0);
        let (dx, dy) = ((rng.next() - 0.5) * 8.0, (rng.next() - 0.5) * 8.0);
        io(write!(
            w,
            "0\nLINE\n8\nL{}\n62\n{}\n10\n{x:.3}\n20\n{y:.3}\n30\n0.0\n11\n{:.3}\n21\n{:.3}\n31\n0.0\n",
            i % 8,
            1 + i % 7,
            x + dx,
            y + dy
        ))?;
    }
    dxf_tail(&mut w)
}

/// 20 000 LWPOLYLINE de 6 vértices con arcos (bulge).
fn dxf_bulge(path: &Path) -> Result<(), String> {
    let mut w = out(path)?;
    dxf_head(&mut w)?;
    let mut rng = Lcg(5);
    for i in 0..20_000 {
        let (x, y) = (rng.next() * 350.0, rng.next() * 350.0);
        io(write!(w, "0\nLWPOLYLINE\n8\nP{}\n90\n6\n70\n1\n", i % 5))?;
        for k in 0..6 {
            let a = f64::from(k) * std::f64::consts::TAU / 6.0;
            let bulge = if k % 2 == 0 { 0.5 } else { -0.3 };
            io(write!(
                w,
                "10\n{:.3}\n20\n{:.3}\n42\n{bulge}\n",
                x + 8.0 * a.cos(),
                y + 8.0 * a.sin()
            ))?;
        }
    }
    dxf_tail(&mut w)
}

/// 5 000 SPLINE cúbicas con 8 puntos de control.
fn dxf_splines(path: &Path) -> Result<(), String> {
    let mut w = out(path)?;
    dxf_head(&mut w)?;
    let mut rng = Lcg(9);
    let knots = [0.0, 0.0, 0.0, 0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 5.0, 5.0, 5.0];
    for _ in 0..5_000 {
        let (x, y) = (rng.next() * 340.0, rng.next() * 340.0);
        io(write!(
            w,
            "0\nSPLINE\n8\nS\n70\n8\n71\n3\n72\n12\n73\n8\n74\n0\n"
        ))?;
        for k in knots {
            io(write!(w, "40\n{k}\n"))?;
        }
        for k in 0..8 {
            io(write!(
                w,
                "10\n{:.3}\n20\n{:.3}\n30\n0.0\n",
                x + f64::from(k) * 4.0,
                y + (rng.next() - 0.5) * 20.0
            ))?;
        }
    }
    dxf_tail(&mut w)
}

/// G-code de un millón de líneas G1 sobre una curva de Hilbert, con S variable.
fn gcode_1m(path: &Path) -> Result<(), String> {
    hilbert_gcode(path, 1024)
}

/// G-code de 4,2 M de líneas G1 (curva de Hilbert de orden 11).
fn gcode_5m(path: &Path) -> Result<(), String> {
    hilbert_gcode(path, 2048)
}

fn hilbert_gcode(path: &Path, n: u32) -> Result<(), String> {
    let mut w = out(path)?;
    io(w.write_all(b"G21\nG90\nM4 S0\nG0 X2 Y2\nF3000\n"))?;
    let cell = 376.0 / f64::from(n);
    for d in 0..n * n {
        let (x, y) = hilbert(n, d);
        io(writeln!(
            w,
            "G1 X{:.3} Y{:.3} S{}",
            2.0 + f64::from(x) * cell,
            2.0 + f64::from(y) * cell,
            100 + (d / 4096) % 400
        ))?;
    }
    io(w.write_all(b"M5\nG0 X0 Y0\n"))
}

/// HPGL con 500 000 instrucciones PD en trazos de 50 puntos.
fn hpgl_500k(path: &Path) -> Result<(), String> {
    let mut w = out(path)?;
    io(w.write_all(b"IN;SP1;\n"))?;
    let n = 1024u32;
    let mut count = 0u32;
    // 40 mm = 1 600 unidades de 0,025 mm; el lienzo mide ~9 400 unidades.
    let unit = 9400.0 / f64::from(n);
    'outer: loop {
        let mut d = count;
        let (x, y) = hilbert(n, d);
        io(write!(
            w,
            "PU{},{};PD",
            (f64::from(x) * unit) as i64,
            (f64::from(y) * unit) as i64
        ))?;
        for _ in 0..50 {
            d += 1;
            let (x, y) = hilbert(n, d % (n * n));
            io(write!(
                w,
                "{},{},",
                (f64::from(x) * unit) as i64,
                (f64::from(y) * unit) as i64
            ))?;
        }
        io(w.write_all(b"0,0;\n"))?;
        count += 51;
        if count >= 500_000 {
            break 'outer;
        }
    }
    io(w.write_all(b"PU0,0;SP0;\n"))
}

fn noise_image(w: u32, h: u32) -> image::GrayImage {
    let mut rng = Lcg(11);
    let mut img = image::GrayImage::new(w, h);
    for (x, y, p) in img.enumerate_pixels_mut() {
        let base = (f64::from(x) / f64::from(w) * 160.0) + (f64::from(y) / f64::from(h) * 60.0);
        let ring = (((f64::from(x) - 5000.0).hypot(f64::from(y) - 4000.0)) / 40.0).sin() * 25.0;
        p.0[0] = (base + ring + rng.next() * 30.0).clamp(0.0, 255.0) as u8;
    }
    img
}

/// PNG de 10 000 x 8 000 px en grises (80 Mpx).
fn image_big(path: &Path) -> Result<(), String> {
    noise_image(10_000, 8_000)
        .save(path)
        .map_err(|e| e.to_string())
}

/// JPEG de 12 000 x 9 000 px (108 Mpx).
fn image_jpeg(path: &Path) -> Result<(), String> {
    let img = noise_image(12_000, 9_000);
    let f = out(path)?;
    let mut enc = image::codecs::jpeg::JpegEncoder::new_with_quality(f, 80);
    enc.encode_image(&img).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::hilbert;
    use std::collections::HashSet;

    #[test]
    fn hilbert_visits_every_cell_once_with_unit_steps() {
        let n = 16;
        let pts: Vec<_> = (0..n * n).map(|d| hilbert(n, d)).collect();
        assert_eq!(pts.iter().collect::<HashSet<_>>().len(), (n * n) as usize);
        for w in pts.windows(2) {
            let d = w[0].0.abs_diff(w[1].0) + w[0].1.abs_diff(w[1].1);
            assert_eq!(d, 1, "{w:?}");
        }
    }
}
