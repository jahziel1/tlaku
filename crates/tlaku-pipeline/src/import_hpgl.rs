//! Importación de HPGL (`.plt`, `.hpgl`, `.hpg`), el lenguaje de plotters y cortadoras de vinilo.
//!
//! Referencia pública: HP-GL/2 y las guías de programación de Roland (DPX/CAMM-GL), Graphtec y
//! GCC, todas con el mismo núcleo de comandos. La unidad del plotter es 0,025 mm (40 por mm).
//!
//! Se leen `IN`/`DF`, `PU`, `PD`, `PA`, `PR`, `SP` (cada pluma es una capa), `PC` (color de
//! pluma), `IP` y `SC` (escala de usuario anisótropa), `CI`, `AA`/`AR`, `EA`/`ER`/`RA`/`RR` y `PE`
//! (polilínea codificada de HP-GL/2). Las secuencias de escape de PCL/PJL y de Graphtec que
//! envuelven al HPGL se saltan. Lo que se ignora avisa, salvo los comandos de estilo (grosor,
//! tipo de línea, velocidad) que no cambian la geometría.

use crate::import::{ImportExtras, ImportOptions, arc_to_cubics, from_curves};
use std::path::Path;
use tlaku_core::bezpath::{BezPath, PathSeg};
use tlaku_core::{Point, Rgb};
use tlaku_doc::ImportResult;
use tlaku_i18n::tr;

/// Unidades del plotter por milímetro.
const UNITS_PER_MM: f64 = 40.0;
/// Distancia (unidades del plotter) bajo la cual el último punto cierra el trazo.
const CLOSE_EPS: f64 = 0.5;

/// Trayectorias del HPGL, una capa por pluma.
pub(crate) fn import(
    path: &Path,
    data: &[u8],
    _opts: &ImportOptions,
    _extras: &ImportExtras,
) -> Result<ImportResult, String> {
    let mut parser = Parser::new();
    parser.run(&clean(data));
    let name = path.display();
    let (layers, mut warnings) = parser.finish();
    if layers.is_empty() {
        return Err(tr!("pipeline.import_hpgl.no_strokes", name = name));
    }
    warnings.insert(0, tr!("pipeline.import_hpgl.assumed_units"));
    Ok(from_curves(layers, warnings))
}

/// Quita de los datos las secuencias de escape de PCL/PJL/DCI y las líneas `@PJL`, y pasa el
/// resto a texto (los bytes que no son ASCII solo aparecen en rótulos y en `PE` de 8 bits).
fn clean(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len());
    let mut i = 0;
    while i < data.len() {
        let b = data[i];
        if b == 0x1b {
            i += 1;
            match data.get(i) {
                // ESC % ... letra: cambio de lenguaje de PCL.
                Some(b'%') | Some(b'*') | Some(b'&') | Some(b'(') | Some(b')') => {
                    i += 1;
                    while i < data.len() && !data[i].is_ascii_uppercase() {
                        i += 1;
                    }
                    i += 1;
                }
                // ESC . x ... : instrucciones de dispositivo (DCI) hasta el ':'.
                Some(b'.') => {
                    while i < data.len() && data[i] != b':' && data[i] != b'\n' {
                        i += 1;
                    }
                    i += 1;
                }
                _ => i += 1,
            }
            continue;
        }
        if data[i..].starts_with(b"@PJL") {
            while i < data.len() && data[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        out.push(b);
        i += 1;
    }
    out
}

/// Números de un texto de parámetros: separados por coma o espacio, o pegados por su signo.
fn numbers(text: &str) -> Vec<f64> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let flush = |cur: &mut String, out: &mut Vec<f64>| {
        if !cur.is_empty() {
            if let Ok(v) = cur.parse::<f64>() {
                out.push(v);
            }
            cur.clear();
        }
    };
    for c in text.chars() {
        match c {
            '0'..='9' | '.' => cur.push(c),
            '-' | '+' => {
                flush(&mut cur, &mut out);
                cur.push(c);
            }
            _ => flush(&mut cur, &mut out),
        }
    }
    flush(&mut cur, &mut out);
    out
}

/// Colores de las primeras plumas (los de HP-GL/2); las demás salen de un tono repartido.
fn default_pen_color(pen: u32) -> Rgb {
    const BASE: [Rgb; 8] = [
        Rgb(0, 0, 0),
        Rgb(0, 0, 0),
        Rgb(255, 0, 0),
        Rgb(0, 160, 0),
        Rgb(230, 190, 0),
        Rgb(0, 0, 255),
        Rgb(255, 0, 255),
        Rgb(0, 190, 220),
    ];
    if let Some(c) = BASE.get(pen as usize) {
        return *c;
    }
    // Ángulo áureo: tonos bien separados aunque haya muchas plumas.
    let hue = (f64::from(pen) * 137.508) % 360.0;
    let (s, v) = (0.85, 0.8);
    let c = v * s;
    let x = c * (1.0 - ((hue / 60.0) % 2.0 - 1.0).abs());
    let m = v - c;
    let (r, g, b) = match (hue / 60.0) as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let q = |v: f64| ((v + m) * 255.0).round() as u8;
    Rgb(q(r), q(g), q(b))
}

/// Escala de usuario: `IP` (puntos P1 y P2 en unidades del plotter) más `SC` (la ventana de
/// usuario que se les asigna).
#[derive(Clone, Copy)]
struct Scaling {
    p1: (f64, f64),
    p2: (f64, f64),
    /// `xmin, xmax, ymin, ymax`.
    user: [f64; 4],
}

struct Parser {
    pen_down: bool,
    absolute: bool,
    pen: u32,
    pos: Point,
    ip: Option<[f64; 4]>,
    sc_user: Option<[f64; 4]>,
    scaling: Option<Scaling>,
    pen_colors: Vec<(u32, Rgb)>,
    /// Subtrayecto en curso, en unidades del plotter.
    segs: Vec<PathSeg>,
    /// Trazos terminados por color.
    layers: Vec<(Rgb, Vec<BezPath>)>,
    unknown: Vec<String>,
    labels: usize,
    isotropic: bool,
    scale_without_ip: bool,
}

impl Parser {
    fn new() -> Self {
        Self {
            pen_down: false,
            absolute: true,
            pen: 1,
            pos: Point::new(0.0, 0.0),
            ip: None,
            sc_user: None,
            scaling: None,
            pen_colors: Vec::new(),
            segs: Vec::new(),
            layers: Vec::new(),
            unknown: Vec::new(),
            labels: 0,
            isotropic: false,
            scale_without_ip: false,
        }
    }

    fn run(&mut self, data: &[u8]) {
        let mut i = 0;
        let n = data.len();
        while i < n {
            // Separadores entre instrucciones.
            while i < n && (data[i].is_ascii_whitespace() || data[i] == b';') {
                i += 1;
            }
            if i >= n {
                break;
            }
            // Los comandos de Roland empiezan con '!' (por ejemplo `!MC1`): se saltan hasta ';'.
            if data[i] == b'!' {
                while i < n && data[i] != b';' && data[i] != b'\n' {
                    i += 1;
                }
                continue;
            }
            if i + 1 >= n || !data[i].is_ascii_alphabetic() || !data[i + 1].is_ascii_alphabetic() {
                // Basura entre instrucciones: se descarta hasta el siguiente separador.
                while i < n && data[i] != b';' && data[i] != b'\n' {
                    i += 1;
                }
                continue;
            }
            let mnemonic = [
                data[i].to_ascii_uppercase(),
                data[i + 1].to_ascii_uppercase(),
            ];
            i += 2;
            let start = i;
            match &mnemonic {
                // El rótulo termina en ETX (0x03), no en ';'.
                b"LB" => {
                    while i < n && data[i] != 0x03 {
                        i += 1;
                    }
                    self.labels += 1;
                    i += 1;
                    continue;
                }
                // La polilínea codificada puede traer cualquier carácter menos ';'.
                b"PE" => {
                    while i < n && data[i] != b';' {
                        i += 1;
                    }
                    self.polyline_encoded(&data[start..i]);
                    continue;
                }
                _ => {}
            }
            while i < n && data[i] != b';' && data[i] != b'\n' && data[i] != b'\r' {
                // Un mnemónico pegado al anterior sin ';' (`PU0,0PD1,1`) empieza donde hay dos
                // letras seguidas tras un número.
                if data[i].is_ascii_alphabetic()
                    && data.get(i + 1).is_some_and(u8::is_ascii_alphabetic)
                    && i > start
                    && (data[i - 1].is_ascii_digit() || data[i - 1] == b'.')
                {
                    break;
                }
                i += 1;
            }
            let params = String::from_utf8_lossy(&data[start..i]).into_owned();
            self.command(mnemonic, &numbers(&params));
        }
        self.flush();
    }

    fn command(&mut self, mnemonic: [u8; 2], p: &[f64]) {
        match &mnemonic {
            b"IN" | b"DF" => {
                self.flush();
                self.pen_down = false;
                self.absolute = true;
                self.scaling = None;
                self.ip = None;
            }
            b"SP" => {
                self.flush();
                let pen = p.first().copied().unwrap_or(0.0).max(0.0) as u32;
                // La pluma 0 es "guardar la pluma": las siguientes líneas siguen con la 1.
                self.pen = if pen == 0 { 1 } else { pen };
            }
            b"PC" => {
                if p.len() >= 4 {
                    let q = |v: f64| v.clamp(0.0, 255.0).round() as u8;
                    let color = Rgb(q(p[1]), q(p[2]), q(p[3]));
                    let pen = p[0].max(0.0) as u32;
                    self.pen_colors.retain(|(n, _)| *n != pen);
                    self.pen_colors.push((pen, color));
                }
            }
            b"IP" => {
                if p.len() >= 4 {
                    self.ip = Some([p[0], p[1], p[2], p[3]]);
                } else {
                    self.ip = None;
                }
                self.rebuild_scaling();
            }
            b"SC" => {
                if p.len() >= 4 && p[1] != p[0] && p[3] != p[2] {
                    self.sc_user = Some([p[0], p[1], p[2], p[3]]);
                    self.isotropic = p.get(4).is_some_and(|t| *t != 0.0);
                } else {
                    self.sc_user = None;
                }
                self.rebuild_scaling();
            }
            b"PU" => {
                self.flush();
                self.pen_down = false;
                self.move_to_all(p);
            }
            b"PD" => {
                self.pen_down = true;
                self.move_to_all(p);
            }
            b"PA" => {
                self.absolute = true;
                self.move_to_all(p);
            }
            b"PR" => {
                self.absolute = false;
                self.move_to_all(p);
            }
            b"CI" => {
                if let Some(&r) = p.first() {
                    self.circle(r);
                }
            }
            b"AA" | b"AR" => {
                if p.len() >= 3 {
                    let rel = &mnemonic == b"AR";
                    self.arc(p[0], p[1], p[2], rel);
                }
            }
            b"EA" | b"ER" | b"RA" | b"RR" => {
                if p.len() >= 2 {
                    let rel = mnemonic[1] == b'R';
                    self.rectangle(p[0], p[1], rel);
                }
            }
            // Estilo y configuración que no cambian la geometría.
            b"PW" | b"LT" | b"VS" | b"FS" | b"PG" | b"NR" | b"SI" | b"SR" | b"DI" | b"DR"
            | b"LO" | b"CS" | b"SS" | b"CA" | b"IW" | b"PS" | b"AF" | b"AH" | b"RO" | b"PM"
            | b"EP" | b"FP" | b"FT" | b"CR" | b"NP" | b"TL" | b"XT" | b"YT" | b"SM" | b"SA"
            | b"DT" | b"DV" | b"OP" | b"OI" | b"OA" | b"OC" | b"OD" | b"OE" | b"OF" | b"OH"
            | b"OO" | b"OS" | b"OW" | b"BP" | b"ST" | b"SL" | b"MC" | b"BL" | b"EC" | b"ES"
            | b"GM" | b"GP" | b"IM" | b"IV" | b"KY" | b"MG" | b"MT" | b"OL" | b"PP" | b"QL"
            | b"RF" | b"RP" | b"SG" | b"SV" | b"UL" | b"WG" | b"FR" | b"FI" | b"FN" | b"WU" => {}
            _ => {
                let name = String::from_utf8_lossy(&mnemonic).into_owned();
                if !self.unknown.contains(&name) {
                    self.unknown.push(name);
                }
            }
        }
    }

    fn rebuild_scaling(&mut self) {
        self.scaling = None;
        if let (Some(ip), Some(user)) = (self.ip, self.sc_user) {
            self.scaling = Some(Scaling {
                p1: (ip[0], ip[1]),
                p2: (ip[2], ip[3]),
                user,
            });
        } else if self.sc_user.is_some() {
            self.scale_without_ip = true;
        }
    }

    /// De unidades de usuario a unidades del plotter, un punto absoluto.
    fn to_plotter(&self, x: f64, y: f64) -> Point {
        match self.scaling {
            Some(s) => Point::new(
                s.p1.0 + (x - s.user[0]) / (s.user[1] - s.user[0]) * (s.p2.0 - s.p1.0),
                s.p1.1 + (y - s.user[2]) / (s.user[3] - s.user[2]) * (s.p2.1 - s.p1.1),
            ),
            None => Point::new(x, y),
        }
    }

    /// Un desplazamiento en unidades de usuario a unidades del plotter.
    fn delta_to_plotter(&self, dx: f64, dy: f64) -> (f64, f64) {
        match self.scaling {
            Some(s) => (
                dx / (s.user[1] - s.user[0]) * (s.p2.0 - s.p1.0),
                dy / (s.user[3] - s.user[2]) * (s.p2.1 - s.p1.1),
            ),
            None => (dx, dy),
        }
    }

    /// Recorre una lista de pares de coordenadas con el modo (absoluto o relativo) y la pluma
    /// actuales.
    fn move_to_all(&mut self, p: &[f64]) {
        for pair in p.as_chunks::<2>().0 {
            let target = if self.absolute {
                self.to_plotter(pair[0], pair[1])
            } else {
                let (dx, dy) = self.delta_to_plotter(pair[0], pair[1]);
                Point::new(self.pos.x + dx, self.pos.y + dy)
            };
            self.go_to(target);
        }
    }

    fn go_to(&mut self, target: Point) {
        if !(target.x.is_finite() && target.y.is_finite()) {
            return;
        }
        if self.pen_down {
            if self.segs.is_empty() {
                self.segs.push(PathSeg::MoveTo { to: self.pos });
            }
            self.segs.push(PathSeg::LineTo { to: target });
        } else {
            self.flush();
        }
        self.pos = target;
    }

    fn circle(&mut self, r: f64) {
        self.flush();
        let (rx, _) = self.delta_to_plotter(r.abs(), 0.0);
        let c = self.pos;
        if rx <= 0.0 {
            return;
        }
        let start = Point::new(c.x + rx, c.y);
        self.segs.push(PathSeg::MoveTo { to: start });
        self.segs.extend(arc_to_cubics(
            (c.x, c.y),
            rx,
            0.0,
            std::f64::consts::TAU,
            start,
        ));
        self.segs.push(PathSeg::Close);
        self.flush();
    }

    /// Arco desde la posición actual alrededor de `(cx, cy)` (absoluto, o relativo a la posición
    /// actual con `AR`) un barrido de `sweep` grados (positivo = antihorario).
    fn arc(&mut self, cx: f64, cy: f64, sweep_deg: f64, rel: bool) {
        let center = if rel {
            let (dx, dy) = self.delta_to_plotter(cx, cy);
            Point::new(self.pos.x + dx, self.pos.y + dy)
        } else {
            self.to_plotter(cx, cy)
        };
        let r = self.pos.dist(center);
        if r < 1e-9 || sweep_deg == 0.0 {
            return;
        }
        let a0 = (self.pos.y - center.y).atan2(self.pos.x - center.x);
        let sweep = sweep_deg.to_radians();
        let end = Point::new(
            center.x + r * (a0 + sweep).cos(),
            center.y + r * (a0 + sweep).sin(),
        );
        // El arco se dibuja con la pluma abajo, tal como lo definen AA y AR.
        if self.segs.is_empty() {
            self.segs.push(PathSeg::MoveTo { to: self.pos });
        }
        self.segs
            .extend(arc_to_cubics((center.x, center.y), r, a0, sweep, end));
        self.pos = end;
        if !self.pen_down {
            self.flush();
        }
    }

    /// Contorno del rectángulo cuya esquina opuesta es `(x, y)`.
    fn rectangle(&mut self, x: f64, y: f64, rel: bool) {
        self.flush();
        let a = self.pos;
        let b = if rel {
            let (dx, dy) = self.delta_to_plotter(x, y);
            Point::new(a.x + dx, a.y + dy)
        } else {
            self.to_plotter(x, y)
        };
        self.segs.push(PathSeg::MoveTo { to: a });
        for to in [Point::new(b.x, a.y), b, Point::new(a.x, b.y)] {
            self.segs.push(PathSeg::LineTo { to });
        }
        self.segs.push(PathSeg::Close);
        self.flush();
    }

    /// `PE`: polilínea codificada. `flags` y números en base 32 (`7`) o base 64.
    fn polyline_encoded(&mut self, data: &[u8]) {
        let mut base32 = false;
        let mut frac_bits = 0u32;
        let mut absolute = false;
        let mut pen_up_next = false;
        let mut i = 0;
        // Lee un número codificado desde `i`; `None` al terminar los datos.
        fn read(data: &[u8], i: &mut usize, base32: bool) -> Option<f64> {
            let (cont_max, term_min, bits) = if base32 { (94, 95, 5) } else { (126, 191, 6) };
            let mut acc: i64 = 0;
            let mut shift = 0u32;
            while *i < data.len() {
                let c = i64::from(data[*i]);
                if matches!(c, 9 | 10 | 13 | 32) {
                    *i += 1;
                    continue;
                }
                *i += 1;
                if (63..=cont_max).contains(&c) {
                    acc |= (c - 63) << shift;
                    shift += bits;
                } else if (term_min..term_min + (1 << bits)).contains(&c) {
                    acc |= (c - term_min) << shift;
                    let v = acc >> 1;
                    return Some(if acc & 1 == 1 { -(v as f64) } else { v as f64 });
                } else {
                    return None;
                }
                if shift > 60 {
                    return None;
                }
            }
            None
        }
        while i < data.len() {
            match data[i] {
                b' ' | b'\n' | b'\r' | b'\t' => i += 1,
                b'7' => {
                    base32 = true;
                    i += 1;
                }
                b'<' => {
                    pen_up_next = true;
                    i += 1;
                }
                b'=' => {
                    absolute = true;
                    i += 1;
                }
                b'>' => {
                    i += 1;
                    match read(data, &mut i, base32) {
                        Some(f) => frac_bits = f.max(0.0) as u32,
                        None => break,
                    }
                }
                b':' => {
                    i += 1;
                    match read(data, &mut i, base32) {
                        Some(pen) => {
                            self.flush();
                            self.pen = (pen.max(0.0) as u32).max(1);
                        }
                        None => break,
                    }
                }
                _ => {
                    let (Some(x), Some(y)) =
                        (read(data, &mut i, base32), read(data, &mut i, base32))
                    else {
                        break;
                    };
                    let k = f64::from(1u32 << frac_bits.min(30));
                    let (x, y) = (x / k, y / k);
                    let target = if absolute {
                        absolute = false;
                        Point::new(x, y)
                    } else {
                        Point::new(self.pos.x + x, self.pos.y + y)
                    };
                    if pen_up_next {
                        self.flush();
                        self.pos = target;
                        pen_up_next = false;
                    } else {
                        self.pen_down = true;
                        self.go_to(target);
                    }
                }
            }
        }
        self.pen_down = false;
        self.flush();
    }

    /// Termina el subtrayecto en curso y lo guarda en la capa de la pluma.
    fn flush(&mut self) {
        let mut segs = std::mem::take(&mut self.segs);
        if segs.len() < 2 {
            return;
        }
        // Un trazo que vuelve a su inicio es una forma cerrada.
        let first = match segs[0] {
            PathSeg::MoveTo { to } => to,
            _ => return,
        };
        let last = match segs.last() {
            Some(PathSeg::LineTo { to })
            | Some(PathSeg::CubicTo { to, .. })
            | Some(PathSeg::QuadTo { to, .. }) => Some(*to),
            _ => None,
        };
        if let Some(last) = last
            && segs.len() > 2
            && first.dist(last) < CLOSE_EPS
        {
            if matches!(segs.last(), Some(PathSeg::LineTo { .. })) {
                segs.pop();
            }
            segs.push(PathSeg::Close);
        }
        // Un trazo de un solo punto (PD sin recorrido) no dibuja nada.
        let draws = segs.iter().any(|s| match s {
            PathSeg::LineTo { to } => to.dist(first) > 1e-9,
            PathSeg::Close => false,
            _ => true,
        });
        if !draws {
            return;
        }
        let color = self.color_of(self.pen);
        let path = BezPath(segs);
        match self.layers.iter_mut().find(|(c, _)| *c == color) {
            Some((_, paths)) => paths.push(path),
            None => self.layers.push((color, vec![path])),
        }
    }

    fn color_of(&self, pen: u32) -> Rgb {
        self.pen_colors
            .iter()
            .find(|(n, _)| *n == pen)
            .map_or_else(|| default_pen_color(pen), |(_, c)| *c)
    }

    /// Capas en mm y los avisos de lo que se ignoró.
    fn finish(self) -> (Vec<(Rgb, Vec<BezPath>)>, Vec<String>) {
        let to_mm = |p: Point| Point::new(p.x / UNITS_PER_MM, p.y / UNITS_PER_MM);
        let layers = self
            .layers
            .into_iter()
            .map(|(c, paths)| (c, paths.iter().map(|p| p.map_points(to_mm)).collect()))
            .collect();
        let mut warnings = Vec::new();
        if self.labels > 0 {
            warnings.push(tr!("pipeline.import_hpgl.labels_skipped", n = self.labels));
        }
        if !self.unknown.is_empty() {
            warnings.push(tr!(
                "pipeline.import_hpgl.ignored_commands",
                commands = self.unknown.join(", ")
            ));
        }
        if self.isotropic {
            warnings.push(tr!("pipeline.import_hpgl.isotropic_scale"));
        }
        if self.scale_without_ip {
            warnings.push(tr!("pipeline.import_hpgl.scale_without_ip"));
        }
        (layers, warnings)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tlaku_doc::ObjectKind;

    fn run(text: &str) -> Result<ImportResult, String> {
        import(
            Path::new("a.plt"),
            text.as_bytes(),
            &ImportOptions::default(),
            &ImportExtras::default(),
        )
    }

    fn approx(a: f64, b: f64) {
        assert!((a - b).abs() < 1e-6, "{a} != {b}");
    }

    fn paths(r: &ImportResult, i: usize) -> &Vec<BezPath> {
        match &r.objects[i].kind {
            ObjectKind::Vector(v) => &v.paths,
            _ => panic!(),
        }
    }

    #[test]
    fn rectangulo_en_unidades_de_40_por_mm() {
        let r = run("IN;SP1;PU400,400;PD2400,400,2400,1400,400,1400,400,400;PU;SP0;").unwrap();
        assert_eq!(r.objects.len(), 1);
        assert_eq!(r.objects[0].layer, Rgb(0, 0, 0));
        let b = r.objects[0].bounds().unwrap();
        approx(b.min.x, 10.0);
        approx(b.min.y, 10.0);
        approx(b.max.x, 60.0);
        approx(b.max.y, 35.0);
        // Cierra solo: 3 lados + Close.
        let p = &paths(&r, 0)[0].0;
        assert_eq!(p.len(), 5, "{p:?}");
        assert_eq!(p.last(), Some(&PathSeg::Close));
        assert!(r.warnings[0].contains("40 unidades por mm"));
    }

    #[test]
    fn plumas_son_capas_y_pr_es_relativo() {
        let r = run("IN;SP1;PA0,0;PD;PR400,0;SP2;PU;PA0,800;PD;PR0,400;PU;").unwrap();
        assert_eq!(r.objects.len(), 2);
        assert_eq!(r.objects[0].layer, Rgb(0, 0, 0));
        assert_eq!(r.objects[1].layer, Rgb(255, 0, 0));
        let b = r.objects[0].bounds().unwrap();
        approx(b.max.x, 10.0);
        let b = r.objects[1].bounds().unwrap();
        approx(b.min.y, 20.0);
        approx(b.max.y, 30.0);
    }

    #[test]
    fn pc_define_el_color_y_los_numeros_pegados_se_leen() {
        let r = run("IN;PC3,10,20,30;SP3;PU0,0PD400,0-400,400;").unwrap();
        assert_eq!(r.objects[0].layer, Rgb(10, 20, 30));
        let b = r.objects[0].bounds().unwrap();
        approx(b.min.x, -10.0);
        approx(b.max.y, 10.0);
    }

    #[test]
    fn circulo_arco_y_rectangulo() {
        let r = run("IN;SP1;PA800,800;CI400;PU;PA2000,0;PD;AA2000,400,90;PU;PA3000,0;EA3400,400;")
            .unwrap();
        let all = paths(&r, 0);
        assert_eq!(all.len(), 3);
        // Círculo de 10 mm de radio centrado en (20, 20).
        let c = all[0].bounds().unwrap();
        approx(c.min.x, 10.0);
        approx(c.max.x, 30.0);
        approx(c.max.y, 30.0);
        // Cuarto de arco antihorario alrededor de (50, 10) desde (50, 0): termina en (60, 10).
        let PathSeg::CubicTo { to, .. } = *all[1].0.last().unwrap() else {
            panic!()
        };
        approx(to.x, 60.0);
        approx(to.y, 10.0);
        // Rectángulo de 10 x 10 mm.
        let b = all[2].bounds().unwrap();
        approx(b.max.x - b.min.x, 10.0);
    }

    #[test]
    fn escala_de_usuario_con_ip_y_sc() {
        // 1 unidad de usuario = 1 mm: P1=(0,0), P2=(4000,4000) para una ventana de 0..100.
        let r = run("IN;IP0,0,4000,4000;SC0,100,0,100;PU10,10;PD60,10;PR0,30;PU;").unwrap();
        let b = r.objects[0].bounds().unwrap();
        approx(b.min.x, 10.0);
        approx(b.max.x, 60.0);
        approx(b.max.y, 40.0);
    }

    #[test]
    fn polilinea_codificada_pe() {
        // Codifica a mano en base 32: valor v -> (|v|<<1 | signo), 5 bits por carácter,
        // continuación 63..94 y terminal 95..126 (el último trozo).
        fn enc(v: i64) -> String {
            let mut n = (v.abs() << 1) | i64::from(v < 0);
            let mut s = String::new();
            loop {
                let chunk = n & 31;
                n >>= 5;
                if n == 0 {
                    s.push((95 + chunk as u8) as char);
                    break;
                }
                s.push((63 + chunk as u8) as char);
            }
            s
        }
        let data = format!(
            "IN;SP1;PU;PA400,400;PE7={}{}{}{}{}{};",
            enc(400),
            enc(400),
            enc(800),
            enc(0),
            enc(0),
            enc(-400)
        );
        let r = run(&data).unwrap();
        let b = r.objects[0].bounds().unwrap();
        // Absoluto (400,400) -> relativo (+800,0) -> relativo (0,-400).
        approx(b.min.x, 10.0);
        approx(b.max.x, 30.0);
        approx(b.min.y, 0.0);
        approx(b.max.y, 10.0);
    }

    #[test]
    fn envoltura_pcl_y_comandos_de_roland() {
        let text = "\u{1b}%-12345X@PJL ENTER LANGUAGE=HPGL2\n\u{1b}%0B!MC1;IN;VS10;SP1;PU0,0;PD400,0;LBhola\u{3};XX1;";
        let r = run(text).unwrap();
        assert_eq!(r.objects.len(), 1);
        let all = r.warnings.join(" | ");
        assert!(all.contains("rótulo"), "{all}");
        assert!(all.contains("XX"), "{all}");
    }

    #[test]
    fn sin_trazos_es_error() {
        let e = run("IN;SP1;PU0,0;PA100,100;").unwrap_err();
        assert!(e.contains("no contiene trazos"), "{e}");
    }
}
