//! Lector de fuentes SHX compiladas de AutoCAD (`shapes 1.0/1.1` y `unifont 1.0`).
//!
//! Fuentes públicas del formato (se leyeron para entender los datos; el código es propio):
//! - Autodesk, "Shape Description Codes" y "Shape and Font Definition Files" (documentación
//!   de AutoCAD): significado de cada código de forma (vectores, arcos, subformas, escala).
//! - `ezdxf` (MIT, Manfred Moitzi), `src/ezdxf/fonts/shapefile.py`: disposición binaria del
//!   archivo compilado y cálculo de los arcos fraccionarios.
//!
//! Distribución del archivo (todo en little endian):
//! - `shapes`: `AutoCAD-86 shapes 1.0|1.1\r\n` `0x1A`, números primero/último de forma, cantidad
//!   de formas, tabla `(número u16, tamaño u16)` y los registros `nombre\0 + códigos`; termina
//!   en `EOF`. La forma 0 trae la cabecera: `arriba, abajo, modo`. El número de forma es el
//!   código del carácter.
//! - `unifont`: `AutoCAD-86 unifont 1.0\r\n` `0x1A`, 6 bytes de conteos, `nombre\0`,
//!   `arriba, abajo, modo, codificación, incrustación, 0` y registros
//!   `(número u16, tamaño u16, nombre\0, códigos)`. El número es el punto de código Unicode.
//! - `bigfont` (CJK) no se admite todavía.
//!
//! No se interpretan los códigos 14 (solo texto vertical) más allá de saltarlos.

use crate::linefont::{LineFont, LineFontSource, LineGlyph, Stroke};
use std::collections::HashMap;
use std::f64::consts::PI;
use tlaku_i18n::tr;

const SHAPES_START: usize = 0x17;
const UNIFONT_START: usize = 0x18;
/// Profundidad máxima de subformas (evita recursión infinita en archivos corruptos).
const MAX_SUB_DEPTH: usize = 16;
/// Tope de órdenes ejecutadas por glifo.
const MAX_OPS: usize = 20_000;
/// Paso angular al aplanar arcos (grados).
const ARC_STEP_DEG: f64 = 6.0;

//        0    1    2    3    4    5    6    7    8    9    A    B    C    D    E    F
const VEC_X: [f64; 16] = [
    1.0, 1.0, 1.0, 0.5, 0.0, -0.5, -1.0, -1.0, -1.0, -1.0, -1.0, -0.5, 0.0, 0.5, 1.0, 1.0,
];
const VEC_Y: [f64; 16] = [
    0.0, 0.5, 1.0, 1.0, 1.0, 1.0, 1.0, 0.5, 0.0, -0.5, -1.0, -1.0, -1.0, -1.0, -1.0, -0.5,
];

/// Una forma sin interpretar: sus bytes de código.
struct Shape {
    codes: Vec<u8>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Shapes,
    Unifont,
}

struct Reader<'a> {
    data: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn u8(&mut self) -> Result<u8, String> {
        let b = *self.data.get(self.at).ok_or("Archivo SHX truncado")?;
        self.at += 1;
        Ok(b)
    }
    fn u16(&mut self) -> Result<u16, String> {
        let lo = self.u8()?;
        let hi = self.u8()?;
        Ok(u16::from(lo) | (u16::from(hi) << 8))
    }
    fn take(&mut self, n: usize) -> Result<&'a [u8], String> {
        let end = self.at.checked_add(n).ok_or("Archivo SHX truncado")?;
        let s = self.data.get(self.at..end).ok_or("Archivo SHX truncado")?;
        self.at = end;
        Ok(s)
    }
    fn cstr(&mut self) -> Result<&'a [u8], String> {
        let rest = self.data.get(self.at..).ok_or("Archivo SHX truncado")?;
        let n = rest
            .iter()
            .position(|&b| b == 0)
            .ok_or("Archivo SHX truncado")?;
        self.at += n + 1;
        Ok(&rest[..n])
    }
}

/// Lee una fuente SHX. `name` es el nombre con que se ofrecerá al usuario.
pub fn parse_shx(name: &str, data: &[u8]) -> Result<LineFont, String> {
    let (kind, above, shapes) = if data.starts_with(b"AutoCAD-86 shapes 1.0")
        || data.starts_with(b"AutoCAD-86 shapes 1.1")
    {
        let (above, shapes) = parse_shapes(data)?;
        (Kind::Shapes, above, shapes)
    } else if data.starts_with(b"AutoCAD-86 unifont 1.0") {
        let (above, shapes) = parse_unifont(data)?;
        (Kind::Unifont, above, shapes)
    } else if data.starts_with(b"AutoCAD-86 bigfont 1.0") {
        return Err(tr!("text.shx.bigfont"));
    } else {
        return Err(tr!("text.shx.not_shx"));
    };

    let mut glyphs: HashMap<char, LineGlyph> = HashMap::new();
    let mut numbers: Vec<u16> = shapes.keys().copied().collect();
    numbers.sort_unstable();
    for number in numbers {
        // En `shapes` los números son códigos de carácter (Latin-1); los menores de 32 no son
        // texto imprimible (en unifont tampoco).
        let Some(ch) = char::from_u32(u32::from(number)).filter(|c| *c as u32 >= 32) else {
            continue;
        };
        if kind == Kind::Shapes && number > 255 {
            continue;
        }
        if let Some(g) = render_glyph(&shapes, number, kind) {
            glyphs.insert(ch, g);
        }
    }
    if glyphs.is_empty() {
        return Err(tr!("text.shx.no_glyphs"));
    }
    let mut cap = f64::from(above);
    if cap <= 0.0 {
        cap = glyphs
            .get(&'H')
            .and_then(|g| {
                g.strokes
                    .iter()
                    .flatten()
                    .map(|p| p.1)
                    .fold(None, |m: Option<f64>, y| Some(m.map_or(y, |m| m.max(y))))
            })
            .filter(|h| *h > 0.0)
            .unwrap_or(1.0);
    }
    Ok(LineFont::new(name, LineFontSource::Shx, cap, glyphs))
}

fn parse_shapes(data: &[u8]) -> Result<(u8, HashMap<u16, Shape>), String> {
    let mut r = Reader {
        data,
        at: SHAPES_START,
    };
    if r.u8()? != 0x1A {
        return Err(tr!("text.shx.missing_mark"));
    }
    let _first = r.u16()?;
    let _last = r.u16()?;
    let count = usize::from(r.u16()?);
    let mut table = Vec::with_capacity(count);
    for _ in 0..count {
        table.push((r.u16()?, usize::from(r.u16()?)));
    }
    let mut shapes = HashMap::new();
    let mut above = 0u8;
    for (number, size) in table {
        let record = r.take(size)?;
        let mut rr = Reader {
            data: record,
            at: 0,
        };
        let _name = rr.cstr()?;
        let body = &record[rr.at..];
        if number == 0 {
            // Cabecera de la fuente: arriba, abajo, modo.
            above = body.first().copied().unwrap_or(0);
            continue;
        }
        shapes.insert(
            number,
            Shape {
                codes: body.to_vec(),
            },
        );
    }
    Ok((above, shapes))
}

fn parse_unifont(data: &[u8]) -> Result<(u8, HashMap<u16, Shape>), String> {
    let mut r = Reader {
        data,
        at: UNIFONT_START,
    };
    if r.u8()? != 0x1A {
        return Err(tr!("text.shx.missing_mark"));
    }
    r.take(6)?;
    let _font_name = r.cstr()?;
    let above = r.u8()?;
    let _below = r.u8()?;
    let _mode = r.u8()?;
    let _encoding = r.u8()?;
    let _embed = r.u8()?;
    r.u8()?;
    let mut shapes = HashMap::new();
    while r.at < data.len() {
        let number = r.u16()?;
        let size = usize::from(r.u16()?);
        let record = r.take(size)?;
        let mut rr = Reader {
            data: record,
            at: 0,
        };
        let _name = rr.cstr()?;
        shapes.insert(
            number,
            Shape {
                codes: record[rr.at..].to_vec(),
            },
        );
    }
    Ok((above, shapes))
}

/// Estado del dibujo de una forma.
struct Pen<'a> {
    shapes: &'a HashMap<u16, Shape>,
    kind: Kind,
    x: f64,
    y: f64,
    /// Longitud de un vector unitario (cambia con los códigos 3 y 4).
    scale: f64,
    down: bool,
    stack: Vec<(f64, f64)>,
    strokes: Vec<Stroke>,
    current: Stroke,
    ops: usize,
}

impl Pen<'_> {
    fn move_to(&mut self, x: f64, y: f64) {
        if self.down {
            if self.current.is_empty() {
                self.current.push((self.x, self.y));
            }
            self.current.push((x, y));
        } else {
            self.flush();
        }
        self.x = x;
        self.y = y;
    }

    fn flush(&mut self) {
        // Un trazo de longitud cero (todos los puntos iguales) no dibuja nada.
        let moved = self.current.windows(2).any(|w| w[0] != w[1]);
        if self.current.len() > 1 && moved {
            self.strokes.push(std::mem::take(&mut self.current));
        } else {
            self.current.clear();
        }
    }

    /// Arco con centro `c`, de `a0` a `a1` (radianes, el signo de `a1 - a0` da el sentido).
    fn arc(&mut self, cx: f64, cy: f64, r: f64, a0: f64, a1: f64) {
        let steps = ((a1 - a0).abs().to_degrees() / ARC_STEP_DEG)
            .ceil()
            .max(1.0) as usize;
        for i in 1..=steps {
            let a = a0 + (a1 - a0) * i as f64 / steps as f64;
            self.move_to(cx + r * a.cos(), cy + r * a.sin());
        }
    }

    fn run(&mut self, number: u16, depth: usize) -> Result<(), String> {
        if depth > MAX_SUB_DEPTH {
            return Err(tr!("text.shx.nested_subshapes"));
        }
        let shapes = self.shapes;
        let Some(shape) = shapes.get(&number) else {
            return Ok(());
        };
        let codes = &shape.codes;
        let mut i = 0usize;
        let mut skip = false; // el código 14 omite la orden siguiente
        let byte = |i: usize| -> Result<u8, String> {
            codes
                .get(i)
                .copied()
                .ok_or_else(|| tr!("text.shx.truncated_shape"))
        };
        let sbyte = |i: usize| -> Result<f64, String> { Ok(f64::from(byte(i)? as i8)) };
        while i < codes.len() {
            self.ops += 1;
            if self.ops > MAX_OPS {
                return Err(tr!("text.shx.shape_too_long"));
            }
            let code = codes[i];
            i += 1;
            let active = !skip;
            skip = false;
            match code {
                0 => break,
                1 => self.down = self.down || active,
                2 => {
                    if active {
                        self.flush();
                        self.down = false;
                    }
                }
                3 | 4 => {
                    let f = f64::from(byte(i)?);
                    i += 1;
                    if active && f > 0.0 {
                        if code == 3 {
                            self.scale /= f;
                        } else {
                            self.scale *= f;
                        }
                    }
                }
                5 => {
                    if active {
                        self.stack.push((self.x, self.y));
                    }
                }
                6 => {
                    if active {
                        let (x, y) = self
                            .stack
                            .pop()
                            .ok_or_else(|| tr!("text.shx.empty_stack"))?;
                        self.flush();
                        self.x = x;
                        self.y = y;
                    }
                }
                7 => {
                    let sub = if self.kind == Kind::Unifont {
                        let v = u16::from(byte(i)?) | (u16::from(byte(i + 1)?) << 8);
                        i += 2;
                        v
                    } else {
                        let v = u16::from(byte(i)?);
                        i += 1;
                        v
                    };
                    if active {
                        self.run(sub, depth + 1)?;
                    }
                }
                8 => {
                    let (dx, dy) = (sbyte(i)?, sbyte(i + 1)?);
                    i += 2;
                    if active {
                        self.move_to(self.x + dx * self.scale, self.y + dy * self.scale);
                    }
                }
                9 => loop {
                    let (dx, dy) = (sbyte(i)?, sbyte(i + 1)?);
                    i += 2;
                    if dx == 0.0 && dy == 0.0 {
                        break;
                    }
                    if active {
                        self.move_to(self.x + dx * self.scale, self.y + dy * self.scale);
                    }
                },
                10 => {
                    let radius = f64::from(byte(i)?);
                    let (start, span, ccw) = octants(byte(i + 1)? as i8);
                    i += 2;
                    if active && radius > 0.0 {
                        let span = if span == 0 { 8 } else { span };
                        let r = radius * self.scale;
                        let a0 = f64::from(start) * PI / 4.0;
                        let sweep = f64::from(span) * PI / 4.0 * if ccw { 1.0 } else { -1.0 };
                        let (cx, cy) = (self.x - r * a0.cos(), self.y - r * a0.sin());
                        self.arc(cx, cy, r, a0, a0 + sweep);
                    }
                }
                11 => {
                    let (so, eo) = (f64::from(byte(i)?), f64::from(byte(i + 1)?));
                    let radius =
                        f64::from((u16::from(byte(i + 2)?) << 8) | u16::from(byte(i + 3)?));
                    let (start, span, ccw) = octants(byte(i + 4)? as i8);
                    i += 5;
                    if active && radius > 0.0 {
                        let eo = if eo == 0.0 { 256.0 } else { eo };
                        let bin = 45.0 / 256.0;
                        let (a0, a1) = if ccw {
                            let end_oct = f64::from(start) + f64::from(span) - 1.0;
                            (
                                f64::from(start) * 45.0 + so * bin,
                                end_oct * 45.0 + eo * bin,
                            )
                        } else {
                            let end_oct = f64::from(start) - f64::from(span) + 1.0;
                            (
                                f64::from(start) * 45.0 - so * bin,
                                end_oct * 45.0 - eo * bin,
                            )
                        };
                        let (a0, a1) = (a0.to_radians(), a1.to_radians());
                        let r = radius * self.scale;
                        let (cx, cy) = (self.x - r * a0.cos(), self.y - r * a0.sin());
                        self.arc(cx, cy, r, a0, a1);
                    }
                }
                12 => {
                    let (dx, dy, bulge) = (sbyte(i)?, sbyte(i + 1)?, sbyte(i + 2)?);
                    i += 3;
                    if active {
                        self.bulge(dx, dy, bulge);
                    }
                }
                13 => loop {
                    let (dx, dy) = (sbyte(i)?, sbyte(i + 1)?);
                    i += 2;
                    if dx == 0.0 && dy == 0.0 {
                        break;
                    }
                    let bulge = sbyte(i)?;
                    i += 1;
                    if active {
                        self.bulge(dx, dy, bulge);
                    }
                },
                14 => skip = true,
                // 15 no está definido: se ignora. Del 16 en adelante, vector corto.
                15 => {}
                _ => {
                    if active {
                        let len = f64::from(code >> 4);
                        let dir = usize::from(code & 0xF);
                        self.move_to(
                            self.x + VEC_X[dir] * len * self.scale,
                            self.y + VEC_Y[dir] * len * self.scale,
                        );
                    }
                }
            }
        }
        Ok(())
    }

    /// Arco entre el punto actual y `+(dx, dy)`; `bulge` va de -127 a 127 (127 = semicírculo,
    /// positivo en sentido antihorario).
    fn bulge(&mut self, dx: f64, dy: f64, bulge: f64) {
        let (ex, ey) = (self.x + dx * self.scale, self.y + dy * self.scale);
        if bulge == 0.0 || (dx == 0.0 && dy == 0.0) {
            self.move_to(ex, ey);
            return;
        }
        let b = bulge / 127.0;
        let (sx, sy) = (self.x, self.y);
        let chord = (ex - sx).hypot(ey - sy);
        // bulge = tan(ángulo/4); el sagita define el radio.
        let sweep = 4.0 * b.atan();
        let radius = chord / (2.0 * (sweep / 2.0).sin().abs());
        let (mx, my) = ((sx + ex) / 2.0, (sy + ey) / 2.0);
        // Distancia del punto medio de la cuerda al centro; el signo depende del sentido.
        let h = radius * (sweep / 2.0).cos() * sweep.signum();
        let (ux, uy) = ((ex - sx) / chord, (ey - sy) / chord);
        // Normal a la izquierda de la cuerda: el centro está a la izquierda si el arco es
        // antihorario y a la derecha si es horario (`h` ya lleva el signo).
        let (nx, ny) = (-uy, ux);
        let (cx, cy) = (mx + nx * h, my + ny * h);
        let a0 = (sy - cy).atan2(sx - cx);
        self.arc(cx, cy, radius, a0, a0 + sweep);
        // Evita acumular error: termina exactamente en el punto de destino.
        if let Some(last) = self.current.last_mut()
            && self.down
        {
            *last = (ex, ey);
        }
        self.x = ex;
        self.y = ey;
    }
}

/// Octante inicial, cantidad de octantes y sentido antihorario de un byte con signo.
fn octants(spec: i8) -> (u8, u8, bool) {
    let ccw = spec >= 0;
    let s = spec.unsigned_abs();
    ((s >> 4) & 0xF, s & 0xF, ccw)
}

fn render_glyph(shapes: &HashMap<u16, Shape>, number: u16, kind: Kind) -> Option<LineGlyph> {
    let mut pen = Pen {
        shapes,
        kind,
        x: 0.0,
        y: 0.0,
        scale: 1.0,
        down: true,
        stack: Vec::new(),
        strokes: Vec::new(),
        current: Vec::new(),
        ops: 0,
    };
    // Una forma corrupta se descarta; no debe impedir cargar el resto de la fuente.
    pen.run(number, 0).ok()?;
    pen.flush();
    Some(LineGlyph {
        advance: pen.x,
        strokes: pen.strokes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Construye un `unifont` mínimo con las formas dadas `(unicode, códigos)`.
    pub(crate) fn unifont(above: u8, shapes: &[(u16, &[u8])]) -> Vec<u8> {
        let mut out = b"AutoCAD-86 unifont 1.0\r\n\x1a".to_vec();
        out.extend([shapes.len() as u8 + 1, 0, 0, 0, 20, 0]);
        out.extend(b"Prueba\0");
        out.extend([above, 4, 0, 0, 0, 0]);
        for (n, codes) in shapes {
            let size = 2 + codes.len(); // nombre "" + terminador
            out.extend(n.to_le_bytes());
            out.extend((size as u16).to_le_bytes());
            out.push(b'x');
            out.push(0);
            out.extend(*codes);
        }
        out
    }

    /// Construye un `shapes 1.0` con la forma 0 de cabecera.
    pub(crate) fn shapes10(above: u8, shapes: &[(u16, &[u8])]) -> Vec<u8> {
        let mut out = b"AutoCAD-86 shapes 1.0\r\n\x1a".to_vec();
        let mut all: Vec<(u16, Vec<u8>)> = vec![(0, vec![b'F', 0, above, 4, 0])];
        for (n, c) in shapes {
            let mut v = vec![b'x', 0];
            v.extend(*c);
            all.push((*n, v));
        }
        out.extend(0u16.to_le_bytes());
        out.extend(all.last().unwrap().0.to_le_bytes());
        out.extend((all.len() as u16).to_le_bytes());
        for (n, v) in &all {
            out.extend(n.to_le_bytes());
            out.extend((v.len() as u16).to_le_bytes());
        }
        for (_, v) in &all {
            out.extend(v);
        }
        out.extend(b"EOF");
        out
    }

    fn glyph(font: &LineFont, c: char) -> LineGlyph {
        font.glyph(c).unwrap()
    }

    #[test]
    fn unifont_draws_vectors_and_advances() {
        // 'L': pen up no; vector hacia arriba de 4 (0x44), abajo (0x4C), derecha 3 (0x30), fin.
        // Dirección 4 = arriba (+Y), C = abajo, 0 = derecha.
        let font = parse_shx(
            "t",
            &unifont(
                4,
                &[
                    (u16::from(b'L'), &[0x44, 0x4C, 0x30, 0]),
                    (u16::from(b' '), &[2, 8, 3, 0, 0]),
                ],
            ),
        )
        .unwrap();
        assert_eq!(font.cap_units, 4.0);
        let l = glyph(&font, 'L');
        assert_eq!(l.strokes.len(), 1);
        assert_eq!(
            l.strokes[0],
            vec![(0.0, 0.0), (0.0, 4.0), (0.0, 0.0), (3.0, 0.0)]
        );
        assert_eq!(l.advance, 3.0);
        // La subforma de pluma arriba solo avanza.
        let sp = glyph(&font, ' ');
        assert!(sp.strokes.is_empty());
        assert_eq!(sp.advance, 3.0);
    }

    #[test]
    fn shapes_format_reads_header_and_displacements() {
        // 8 = desplazamiento (3, 4); 2/8 = pluma arriba y mover; 1 = bajar; 9 = múltiple.
        let font = parse_shx(
            "t",
            &shapes10(
                6,
                &[(
                    u16::from(b'A'),
                    &[8, 3, 4, 2, 8, 2, 0, 1, 9, 1, 1, 0xFF, 0xFF, 0, 0, 0],
                )],
            ),
        )
        .unwrap();
        assert_eq!(font.cap_units, 6.0);
        let a = glyph(&font, 'A');
        assert_eq!(a.strokes.len(), 2);
        assert_eq!(a.strokes[0], vec![(0.0, 0.0), (3.0, 4.0)]);
        assert_eq!(a.strokes[1], vec![(5.0, 4.0), (6.0, 5.0), (5.0, 4.0)]);
    }

    #[test]
    fn scale_push_pop_and_subshape() {
        // Subforma 'b' en unifont (u16): dibuja un vector 2 a la derecha.
        // 'a': mult x2 (4,2), subforma b, divide (3,2) y push/pop.
        let font = parse_shx(
            "t",
            &unifont(
                4,
                &[
                    (u16::from(b'b'), &[0x20, 0]),
                    (u16::from(b'a'), &[4, 2, 5, 7, b'b', 0, 6, 3, 2, 0x04, 0]),
                ],
            ),
        )
        .unwrap();
        let a = glyph(&font, 'a');
        // Con escala 2, la subforma dibuja 4 mm; pop vuelve al origen; escala 1 y vector 0 de
        // longitud 0 no mueve nada.
        assert_eq!(a.strokes[0], vec![(0.0, 0.0), (4.0, 0.0)]);
        assert_eq!(a.advance, 0.0);
    }

    #[test]
    fn octant_arc_full_circle_closes_on_itself() {
        // Código 10: radio 5, especificación 0x00 (octante 0, tramo 0 = círculo completo, ccw).
        let font = parse_shx("t", &unifont(5, &[(u16::from(b'o'), &[10, 5, 0, 0])])).unwrap();
        let o = glyph(&font, 'o');
        let s = &o.strokes[0];
        assert!(s.len() > 20);
        let (first, last) = (s[0], *s.last().unwrap());
        assert!((first.0 - last.0).abs() < 1e-9 && (first.1 - last.1).abs() < 1e-9);
        // El arco arranca en el ángulo 0 del círculo: el centro queda 5 a la izquierda.
        let xmax = s.iter().map(|p| p.0).fold(f64::NEG_INFINITY, f64::max);
        let xmin = s.iter().map(|p| p.0).fold(f64::INFINITY, f64::min);
        assert!((xmax - 0.0).abs() < 1e-9 && (xmin + 10.0).abs() < 0.1);
    }

    #[test]
    fn half_circle_octant_arc_ends_opposite_to_the_start() {
        // Radio 4, empieza en el octante 0 y recorre 4 octantes (180°) antihorario: 0x04.
        let font = parse_shx("t", &unifont(5, &[(u16::from(b'u'), &[10, 4, 0x04, 0])])).unwrap();
        let g = glyph(&font, 'u');
        let end = *g.strokes[0].last().unwrap();
        assert!((end.0 + 8.0).abs() < 1e-9 && end.1.abs() < 1e-9, "{end:?}");
        assert!((g.advance + 8.0).abs() < 1e-9);
    }

    #[test]
    fn bulge_arc_semicircle() {
        // Código 12: de (0,0) a (10,0) con bulge 127 (semicírculo antihorario): pasa por (5,-5).
        let font = parse_shx("t", &unifont(5, &[(u16::from(b'c'), &[12, 10, 0, 127, 0])])).unwrap();
        let g = glyph(&font, 'c');
        let s = &g.strokes[0];
        let last = *s.last().unwrap();
        assert_eq!(last, (10.0, 0.0));
        let ymin = s.iter().map(|p| p.1).fold(f64::INFINITY, f64::min);
        assert!((ymin + 5.0).abs() < 0.05, "{ymin}");
        // Todos los puntos están a 5 del centro (5, 0).
        for p in s {
            assert!(((p.0 - 5.0).hypot(p.1) - 5.0).abs() < 1e-6);
        }
    }

    #[test]
    fn negative_bulge_goes_the_other_way() {
        let font = parse_shx(
            "t",
            &unifont(5, &[(u16::from(b'c'), &[12, 10, 0, 0x81, 0])]),
        )
        .unwrap();
        // 0x81 como i8 = -127.
        let s = &glyph(&font, 'c').strokes[0];
        let ymax = s.iter().map(|p| p.1).fold(f64::NEG_INFINITY, f64::max);
        assert!((ymax - 5.0).abs() < 0.05);
    }

    #[test]
    fn partial_bulge_has_the_expected_sagitta() {
        // Bulge 64/127 sobre una cuerda de 10: sagita = bulge * cuerda / 2.
        let font = parse_shx("t", &unifont(5, &[(u16::from(b'c'), &[12, 10, 0, 64, 0])])).unwrap();
        let s = &glyph(&font, 'c').strokes[0];
        let ymin = s.iter().map(|p| p.1).fold(f64::INFINITY, f64::min);
        assert!((ymin + 64.0 / 127.0 * 5.0).abs() < 0.03, "{ymin}");
        assert_eq!(*s.last().unwrap(), (10.0, 0.0));
        let cw = parse_shx(
            "t",
            &unifont(5, &[(u16::from(b'c'), &[12, 10, 0, 0xC0, 0])]),
        )
        .unwrap();
        let ymax = glyph(&cw, 'c').strokes[0]
            .iter()
            .map(|p| p.1)
            .fold(f64::NEG_INFINITY, f64::max);
        assert!((ymax - 64.0 / 127.0 * 5.0).abs() < 0.03, "{ymax}");
    }

    #[test]
    fn vertical_flag_skips_the_next_command() {
        // 14 salta el vector siguiente (0x20); el 0x30 sí se dibuja.
        let font = parse_shx("t", &unifont(5, &[(u16::from(b'v'), &[14, 0x20, 0x30, 0])])).unwrap();
        let g = glyph(&font, 'v');
        assert_eq!(g.strokes[0], vec![(0.0, 0.0), (3.0, 0.0)]);
    }

    #[test]
    fn corrupt_shapes_are_skipped_not_fatal() {
        // 'p' hace pop con la pila vacía; 'q' está bien.
        let font = parse_shx(
            "t",
            &unifont(
                5,
                &[(u16::from(b'p'), &[6, 0]), (u16::from(b'q'), &[0x20, 0])],
            ),
        )
        .unwrap();
        assert!(font.glyph('p').is_none());
        assert!(font.glyph('q').is_some());
    }

    #[test]
    fn recursion_is_bounded() {
        // 'r' se llama a sí misma.
        let font = parse_shx(
            "t",
            &unifont(
                5,
                &[
                    (u16::from(b'r'), &[7, b'r', 0, 0]),
                    (u16::from(b's'), &[0x10, 0]),
                ],
            ),
        )
        .unwrap();
        assert!(font.glyph('r').is_none());
        assert!(font.glyph('s').is_some());
    }

    #[test]
    fn rejects_foreign_and_truncated_files() {
        assert!(parse_shx("t", b"no es una fuente").is_err());
        assert!(
            parse_shx("t", b"AutoCAD-86 bigfont 1.0\r\n\x1a")
                .unwrap_err()
                .contains("BigFont")
        );
        let full = unifont(4, &[(u16::from(b'L'), &[0x44, 0])]);
        assert!(parse_shx("t", &full[..full.len() - 3]).is_err());
        assert!(parse_shx("t", b"AutoCAD-86 shapes 1.0\r\n\x1a\x00").is_err());
    }
}
