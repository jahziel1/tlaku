//! Generación de G-code para láseres de la familia GRBL y otros firmwares (Smoothieware,
//! Marlin) a partir de un [`GcodeDialect`].
//!
//! Consume un [`Toolpath`]. Por defecto (dialecto GRBL 1.1 en modo láser, `$32=1`) cada bloque
//! elige M4 (potencia dinámica: GRBL baja la potencia al desacelerar en las esquinas) o M3
//! (potencia constante). El láser se apaga solo en los G0; el overscan es un G1 con S0.
//!
//! Los demás firmwares cambian *cómo se escribe* lo mismo, no el generador: el dialecto fija la
//! escala y los decimales de S, la línea de encendido, si S va dentro del G1 o en una línea
//! aparte, si G0 apaga el láser (si no, se escribe `S0` en el G0 o se apaga con un comando
//! antes de desplazarse), si el G0 lleva F, los comandos de aire, el cierre (`M5`, `M400`) y el
//! estilo de comentario. Ver `docs/hardware/dialectos-gcode.md`.
//!
//! La salida es compacta: S y F son modales y las coordenadas X o Y que no
//! cambian se omiten, lo que reduce mucho el tamaño del G-code de imágenes.
//! No se usan offsets G54..G59 porque el firmware de Creality no los maneja bien.
//!
//! Eje Z: `z_mm` de un bloque es la **profundidad** del foco respecto a la altura del cabezal al
//! empezar el trabajo (positivo = hacia dentro del material; 0 = sin mover). Se emite **relativa**
//! (`G91` / `G0 Z..` / `G90`) llevando la cuenta de lo acumulado, nunca como Z absoluta: Tlaku no
//! conoce el cero de Z de la máquina (no usa G54 ni G10), y una Z absoluta con el cero en otro
//! sitio choca el cabezal con la pieza. El foco baja justo antes del primer corte del bloque (los
//! desplazamientos de aproximación se hacen a la altura segura) y **al terminar cada bloque vuelve
//! a la altura inicial**, así ningún desplazamiento entre capas ni el regreso al origen se hace
//! con el cabezal bajo. Sin `z_invert`, la profundidad positiva sale como Z negativa (en GRBL +Z
//! aleja el cabezal); con `z_invert`, como Z positiva. Los movimientos Z son siempre `G0`.
//!
//! Rotativo: con `y_scale != 1` la Y del `Toolpath` está en mm de superficie de la pieza y se
//! multiplica por `y_scale` para obtener los mm de la controladora. El avance F de un G1 se
//! reescala por segmento para que la velocidad sobre la superficie sea la del bloque (ver
//! [`GcodeOptions::y_scale`]).

use std::fmt::Write;
use tlaku_core::Point;
use tlaku_core::toolpath::{Move, PowerMode, Toolpath};
use tlaku_doc::{CommentStyle, GcodeDialect};

#[derive(Debug, Clone, PartialEq)]
pub struct GcodeOptions {
    /// Cómo escribe el G-code el firmware (por defecto GRBL 1.1).
    pub dialect: GcodeDialect,
    /// Valor de S que equivale al 100 % de potencia (ajuste `$30` de GRBL).
    pub s_max: f64,
    /// Volver a X0 Y0 al terminar.
    pub return_to_origin: bool,
    /// Líneas que se emiten tras `G90`/`G21` y antes del primer bloque.
    pub start_gcode: Option<String>,
    /// Líneas que se emiten al final, después de `M5` y del regreso al origen.
    pub end_gcode: Option<String>,
    /// Velocidad de los `G0` en mm/min; solo se escribe con `dialect.feed_on_g0` (Marlin
    /// comparte la F entre `G0` y `G1`). Sin valor, los `G0` van sin F.
    pub rapid_feed: Option<f64>,
    /// Invierte el sentido de Z en el G-code (ver el módulo): `false` = convención de GRBL.
    pub z_invert: bool,
    /// Mm de la controladora por mm de Y del `Toolpath` (rotativo); 1 = sin rotativo.
    ///
    /// Una Y de superficie `y` sale como `y * y_scale`. La velocidad del bloque es sobre la
    /// superficie, pero GRBL aplica F a la longitud del movimiento en mm de la controladora:
    /// un tramo con componente Y sale con `F = velocidad * |tramo ctrl| / |tramo superficie|`.
    /// Un tramo solo en X conserva la F del bloque, y por eso F deja de ser modal solo donde
    /// hay Y.
    pub y_scale: f64,
}

impl Default for GcodeOptions {
    fn default() -> Self {
        Self {
            dialect: GcodeDialect::default(),
            s_max: 1000.0,
            return_to_origin: true,
            start_gcode: None,
            end_gcode: None,
            rapid_feed: None,
            z_invert: false,
            y_scale: 1.0,
        }
    }
}

/// Máximo de decimales que se respeta; más no aporta nada a una máquina de mm.
const MAX_DECIMALS: usize = 6;

/// Formatea con `decimals` decimales como máximo y sin ceros sobrantes.
fn num(v: f64, decimals: usize) -> String {
    let s = format!("{v:.*}", decimals.min(MAX_DECIMALS));
    let s = if s.contains('.') {
        s.trim_end_matches('0').trim_end_matches('.')
    } else {
        &s
    };
    if s == "-0" { "0".into() } else { s.into() }
}

/// Valor de S para un porcentaje de potencia, con `decimals` decimales.
///
/// Se recorta a `0..=s_max` aunque el llamador ya valide: un S negativo hace que
/// GRBL rechace la línea (y el resto del trabajo se desfasa), uno por encima de
/// `$30` se interpreta según el firmware, y un NaN produciría texto inválido.
/// Se conservan decimales porque algunos firmwares (Smoothieware) usan S de 0 a 1; con 0
/// decimales (Marlin, GRBL-M3) se redondea al entero más cercano (0.5 sube).
fn s_value(power_pct: f64, s_max: f64, decimals: usize) -> String {
    let max = if s_max.is_finite() {
        s_max.max(0.0)
    } else {
        0.0
    };
    let s = if power_pct.is_nan() {
        0.0
    } else {
        (power_pct / 100.0 * max).clamp(0.0, max)
    };
    num(if decimals == 0 { s.round() } else { s }, decimals)
}

/// Estado modal del programa generado.
struct Writer<'a> {
    out: String,
    d: &'a GcodeDialect,
    decimals: usize,
    s_decimals: usize,
    x: Option<String>,
    y: Option<String>,
    s: Option<String>,
    f: Option<String>,
    /// Mm de la controladora por mm de Y del `Toolpath`.
    y_scale: f64,
    /// Última posición en las coordenadas del `Toolpath` (antes de escalar Y).
    pos: Option<Point>,
    /// Velocidad de los G0 (si el dialecto la escribe).
    rapid: Option<f64>,
    /// Profundidad de Z acumulada, en unidades de 10^-decimales mm (0 = altura inicial).
    z_units: i64,
    z_invert: bool,
    /// El último comando de potencia fue el apagado explícito (o aún no hubo ninguno).
    off: bool,
}

impl Writer<'_> {
    fn line(&mut self, s: &str) {
        self.out.push_str(s);
        self.out.push('\n');
    }

    /// La potencia está encendida con una `S` distinta de 0: un G0 la dejaría disparando si el
    /// firmware no lo apaga solo.
    fn hot(&self) -> bool {
        !self.off && matches!(&self.s, Some(s) if s != "0")
    }

    /// Coordenadas ya formateadas de `to` y cuáles cambian respecto de las modales.
    fn xy(&self, to: Point) -> (String, String, bool, bool) {
        let x = num(to.x, self.decimals);
        let y = num(to.y * self.y_scale, self.decimals);
        let dx = self.x.as_deref() != Some(&x);
        let dy = self.y.as_deref() != Some(&y);
        (x, y, dx, dy)
    }

    /// Emite un G0/G1 con solo lo que cambió. No escribe nada si el punto es el
    /// mismo (tras redondear) o si sus coordenadas no son finitas. `feed` es la velocidad
    /// del bloque en mm/min sobre la superficie; con `y_scale != 1` y `rescale` se reescala por
    /// tramo (los G0 llevan una velocidad de la controladora y no se reescalan).
    fn motion(&mut self, g: &str, to: Point, s: Option<&str>, feed: Option<f64>, rescale: bool) {
        if !to.x.is_finite() || !to.y.is_finite() {
            return;
        }
        let from = self.pos.replace(to);
        let (x, y, dx, dy) = self.xy(to);
        if !dx && !dy {
            return;
        }
        let mut cmd = String::from(g);
        if dx {
            write!(cmd, " X{x}").unwrap();
            self.x = Some(x);
        }
        if dy {
            write!(cmd, " Y{y}").unwrap();
            self.y = Some(y);
        }
        if let Some(s) = s.filter(|s| self.s.as_deref() != Some(s)) {
            write!(cmd, " S{s}").unwrap();
            self.s = Some(s.into());
        }
        if let Some(feed) = feed {
            let feed = if rescale {
                self.scaled_feed(feed, from, to)
            } else {
                feed
            };
            let f = num(feed, self.decimals);
            if self.f.as_deref() != Some(&f) {
                write!(cmd, " F{f}").unwrap();
                self.f = Some(f);
            }
        }
        self.line(&cmd);
    }

    /// F para un tramo `from -> to` de modo que la velocidad sobre la superficie sea `feed`.
    fn scaled_feed(&self, feed: f64, from: Option<Point>, to: Point) -> f64 {
        let Some(from) = from.filter(|_| self.y_scale != 1.0) else {
            return feed;
        };
        let (dx, dy) = (to.x - from.x, to.y - from.y);
        let surface = dx.hypot(dy);
        if surface <= 0.0 {
            return feed;
        }
        feed * dx.hypot(dy * self.y_scale) / surface
    }

    /// Lo que hay que hacer para que un G0 no deje el láser disparando cuando el firmware no
    /// lo apaga solo. Con la potencia en una línea aparte (`M106`) emite el apagado ahora;
    /// con S en línea devuelve `true` si el G0 debe llevar `S0`.
    fn prepare_travel(&mut self) -> bool {
        let d = self.d;
        if !d.s_inline {
            if !d.g0_laser_off && !self.off && self.s.is_some() && !d.laser_off.is_empty() {
                let off = d.laser_off.clone();
                self.line(&off);
                self.off = true;
            }
            // Tras un G0 (o el apagado) el firmware no conserva la potencia pedida.
            self.s = None;
            return false;
        }
        d.travel_s0 && self.hot()
    }

    /// Desplazamiento rápido (G0) a `to`.
    fn travel(&mut self, to: Point) {
        if !to.x.is_finite() || !to.y.is_finite() {
            return;
        }
        let (_, _, dx, dy) = self.xy(to);
        if !dx && !dy {
            self.pos = Some(to);
            return;
        }
        let s0 = self.prepare_travel();
        let feed = self.d.feed_on_g0.then_some(self.rapid).flatten();
        self.motion("G0", to, s0.then_some("0"), feed, false);
    }

    /// Corte (G1) a `to` con `power_pct` por ciento de la potencia.
    fn cut(&mut self, to: Point, power_pct: f64, s_max: f64, feed: Option<f64>) {
        if !to.x.is_finite() || !to.y.is_finite() {
            return;
        }
        let s = s_value(power_pct, s_max, self.s_decimals);
        if self.d.s_inline {
            self.motion("G1", to, Some(&s), feed, true);
            return;
        }
        let (_, _, dx, dy) = self.xy(to);
        if dx || dy {
            self.set_power(&s);
        }
        self.motion("G1", to, None, feed, true);
    }

    /// Potencia en una línea aparte (`power_cmd`): solo si cambia. Con el láser apagado, pedir
    /// `S0` no dice nada.
    fn set_power(&mut self, s: &str) {
        if self.s.as_deref() == Some(s) || (s == "0" && self.s.is_none()) {
            return;
        }
        if !self.d.power_cmd.is_empty() {
            let cmd = self.d.power_cmd.replace("{s}", s);
            self.line(&cmd);
        }
        self.s = Some(s.into());
        self.off = false;
    }

    /// Lleva el foco a `depth_mm` (profundidad respecto a la altura inicial) con un movimiento
    /// relativo. Ver el módulo.
    fn z_goto(&mut self, depth_mm: f64) {
        let per_mm = 10f64.powi(self.decimals.min(MAX_DECIMALS) as i32);
        let target = (depth_mm * per_mm).round() as i64;
        let delta = target - self.z_units;
        if delta == 0 {
            return;
        }
        let s0 = self.prepare_travel();
        let signed = if self.z_invert { delta } else { -delta };
        let z = num(signed as f64 / per_mm, self.decimals);
        self.line("G91");
        if s0 {
            self.line(&format!("G0 Z{z} S0"));
            self.s = Some("0".into());
        } else {
            self.line(&format!("G0 Z{z}"));
        }
        self.line("G90");
        self.z_units = target;
    }

    /// Línea de comentario según el estilo del dialecto, recortada para no pasar de
    /// `max_line_len` (con su salto de línea). Un salto dentro de la etiqueta la convertiría
    /// en G-code, y los paréntesis cerrarían un comentario `( )`.
    fn comment(&self, label: &str) -> String {
        let flat: Vec<&str> = label.split(['\n', '\r']).collect();
        let text = flat.join(" ");
        let mut text = match self.d.comment {
            CommentStyle::Semicolon => text.trim().to_string(),
            CommentStyle::Parentheses => text
                .chars()
                .filter(|c| !matches!(c, '(' | ')'))
                .collect::<String>()
                .trim()
                .to_string(),
        };
        if self.d.max_line_len > 0 {
            // "; " o "()" más el salto de línea.
            let budget = (self.d.max_line_len as usize).saturating_sub(3);
            if text.len() > budget {
                let mut cut = budget;
                while !text.is_char_boundary(cut) {
                    cut -= 1;
                }
                text.truncate(cut);
                text = text.trim_end().to_string();
            }
        }
        match self.d.comment {
            CommentStyle::Semicolon => format!("; {text}"),
            CommentStyle::Parentheses => format!("({text})"),
        }
    }
}

/// Línea de encendido del modo de potencia (vacía si el dialecto no la tiene).
fn mode_cmd(d: &GcodeDialect, mode: PowerMode) -> &str {
    match mode {
        PowerMode::Dynamic => d.dynamic_on.as_deref().unwrap_or(&d.constant_on),
        PowerMode::Constant => &d.constant_on,
    }
}

/// Copia un script del usuario línea a línea, sin espacios de sobra ni líneas
/// vacías al principio o al final.
fn script(w: &mut Writer, text: &str) {
    for l in text.trim().lines() {
        w.line(l.trim_end());
    }
}

pub fn generate(toolpath: &Toolpath, opts: &GcodeOptions) -> String {
    let d = &opts.dialect;
    let mut w = Writer {
        out: String::new(),
        d,
        decimals: usize::from(d.decimals),
        s_decimals: usize::from(d.s_decimals),
        x: None,
        y: None,
        s: None,
        f: None,
        y_scale: if opts.y_scale.is_finite() && opts.y_scale > 0.0 {
            opts.y_scale
        } else {
            1.0
        },
        pos: None,
        rapid: opts.rapid_feed.filter(|f| f.is_finite() && *f > 0.0),
        z_units: 0,
        z_invert: opts.z_invert,
        off: false,
    };
    let blocks: Vec<_> = toolpath
        .blocks
        .iter()
        .filter(|b| !b.moves.is_empty())
        .collect();

    let header = w.comment("Generado por Tlaku");
    w.line(&header);
    w.line("G90");
    w.line("G21");
    if let Some(start) = &opts.start_gcode {
        script(&mut w, start);
    }
    let first = blocks.first().map_or(PowerMode::Dynamic, |b| b.power_mode);
    let mut cmd = mode_cmd(d, first).to_string();
    if !cmd.is_empty() {
        w.line(&cmd);
        w.s = Some("0".into());
    }

    let mut air_on = false;
    for block in blocks {
        let label = w.comment(&block.label);
        w.line(&label);
        let next = mode_cmd(d, block.power_mode);
        if next != cmd {
            // S0 deja el láser apagado hasta el siguiente G1 con potencia.
            cmd = next.to_string();
            if !cmd.is_empty() {
                w.line(&cmd);
                w.s = Some("0".into());
                w.off = false;
            }
        }
        let want_air = block.air_assist && d.air_on.is_some();
        if want_air != air_on {
            let c = if want_air { &d.air_on } else { &d.air_off };
            if let Some(c) = c {
                w.line(c);
            }
            air_on = want_air;
        }
        // El foco baja justo antes del primer corte: lo anterior es aproximación.
        let mut depth = block.z_mm.filter(|z| z.is_finite());
        let feed = (block.speed_mm_min.is_finite() && block.speed_mm_min > 0.0)
            .then_some(block.speed_mm_min);
        for mv in &block.moves {
            match *mv {
                Move::Travel { to } => w.travel(to),
                Move::Cut { to, power_pct } => {
                    if let Some(z) = depth.take() {
                        w.z_goto(z);
                    }
                    w.cut(to, power_pct, opts.s_max, feed);
                }
            }
        }
        // Vuelve a la altura inicial antes de cualquier desplazamiento posterior.
        w.z_goto(0.0);
    }

    if air_on && let Some(c) = &d.air_off {
        w.line(c);
    }
    if !w.off && !d.laser_off.is_empty() {
        w.line(&d.laser_off);
    }
    w.off = true;
    if opts.return_to_origin {
        let mut cmd = String::from("G0 X0 Y0");
        if d.feed_on_g0
            && let Some(f) = w.rapid
        {
            let f = num(f, w.decimals);
            if w.f.as_deref() != Some(&f) {
                write!(cmd, " F{f}").unwrap();
                w.f = Some(f);
            }
        }
        w.line(&cmd);
    }
    if let Some(end) = &opts.end_gcode {
        script(&mut w, end);
    }
    if let Some(sync) = &d.end_sync {
        w.line(sync);
    }
    w.out
}

#[cfg(test)]
mod tests {
    use super::*;
    use tlaku_core::toolpath::ToolpathBlock;
    use tlaku_core::{CutSettings, Job, Layer, Point, Polyline, Rgb};

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    fn block(label: &str, mode: PowerMode, moves: Vec<Move>) -> ToolpathBlock {
        ToolpathBlock {
            label: label.into(),
            speed_mm_min: 6000.0,
            air_assist: false,
            power_mode: mode,
            moves,
            z_mm: None,
        }
    }

    fn gen_lines(blocks: Vec<ToolpathBlock>, opts: &GcodeOptions) -> Vec<String> {
        generate(&Toolpath { blocks }, opts)
            .lines()
            .map(String::from)
            .collect()
    }

    #[test]
    fn number_format() {
        assert_eq!(num(10.0, 3), "10");
        assert_eq!(num(10.25, 3), "10.25");
        assert_eq!(num(-0.0001, 3), "0");
        assert_eq!(num(1.23456, 3), "1.235");
        assert_eq!(num(1.23456, 1), "1.2");
        assert_eq!(num(100.0, 0), "100");
        assert_eq!(num(1.5, 99), "1.5");
    }

    #[test]
    fn closed_path_returns_to_start_and_s_f_are_modal() {
        let job = Job {
            layers: vec![Layer {
                color: Rgb(0, 0, 0),
                settings: CutSettings {
                    speed_mm_min: 600.0,
                    power_pct: 50.0,
                    passes: 2,
                    air_assist: true,
                },
                paths: vec![Polyline {
                    points: vec![p(0.0, 0.0), p(10.0, 0.0), p(10.0, 10.0)],
                    closed: true,
                }],
            }],
        };
        let g = generate(&Toolpath::from_line_job(&job), &GcodeOptions::default());
        let lines: Vec<&str> = g.lines().collect();
        assert!(lines.contains(&"M8"));
        assert_eq!(lines.iter().filter(|l| l.contains("S500")).count(), 1);
        assert_eq!(lines.iter().filter(|l| l.contains("F600")).count(), 1);
        // Segundo paso: el cierre vuelve a X0 Y0 (Y0 se omite si ya estaba en Y0).
        assert_eq!(lines.iter().filter(|l| **l == "G1 X0 Y0").count(), 2);
        assert_eq!(&lines[lines.len() - 3..], &["M9", "M5", "G0 X0 Y0"]);
    }

    #[test]
    fn raster_line_omits_unchanged_y_and_varies_s() {
        let b = block(
            "Raster",
            PowerMode::Dynamic,
            vec![
                Move::Travel { to: p(10.0, 5.0) },
                Move::Cut {
                    to: p(12.3, 5.0),
                    power_pct: 50.0,
                },
                Move::Cut {
                    to: p(15.0, 5.0),
                    power_pct: 50.0,
                },
                Move::Cut {
                    to: p(20.0, 5.0),
                    power_pct: 0.0,
                },
                Move::Cut {
                    to: p(25.0, 5.0),
                    power_pct: 100.0,
                },
                Move::Travel { to: p(25.0, 5.1) },
                Move::Cut {
                    to: p(10.0, 5.1),
                    power_pct: 25.0,
                },
            ],
        );
        let l = gen_lines(vec![b], &GcodeOptions::default());
        let body = &l[l.iter().position(|x| x == "; Raster").unwrap() + 1..];
        assert_eq!(
            &body[..7],
            &[
                "G0 X10 Y5",
                "G1 X12.3 S500 F6000",
                "G1 X15",
                "G1 X20 S0",
                "G1 X25 S1000",
                "G0 Y5.1",
                "G1 X10 S250",
            ]
        );
    }

    #[test]
    fn first_coordinate_is_complete_even_at_zero() {
        let b = block(
            "A",
            PowerMode::Dynamic,
            vec![
                Move::Travel { to: p(0.0, 0.0) },
                Move::Cut {
                    to: p(0.0, 5.0),
                    power_pct: 10.0,
                },
            ],
        );
        let l = gen_lines(vec![b], &GcodeOptions::default());
        assert!(l.contains(&"G0 X0 Y0".to_string()));
        assert!(l.contains(&"G1 Y5 S100 F6000".to_string()));
    }

    #[test]
    fn repeated_point_emits_nothing() {
        let b = block(
            "A",
            PowerMode::Dynamic,
            vec![
                Move::Travel { to: p(1.0, 1.0) },
                Move::Travel { to: p(1.0001, 1.0) },
                Move::Cut {
                    to: p(1.0, 1.0),
                    power_pct: 10.0,
                },
            ],
        );
        let g = generate(&Toolpath { blocks: vec![b] }, &GcodeOptions::default());
        assert_eq!(g.lines().filter(|l| l.starts_with("G0")).count(), 2); // + regreso
        assert!(!g.contains("G1"));
    }

    #[test]
    fn power_mode_changes_only_when_needed() {
        let mv = || {
            vec![
                Move::Travel { to: p(0.0, 0.0) },
                Move::Cut {
                    to: p(1.0, 0.0),
                    power_pct: 50.0,
                },
            ]
        };
        let l = gen_lines(
            vec![
                block("uno", PowerMode::Constant, mv()),
                block("dos", PowerMode::Constant, mv()),
                block("tres", PowerMode::Dynamic, mv()),
            ],
            &GcodeOptions::default(),
        );
        let modes: Vec<&str> = l
            .iter()
            .filter(|x| x.starts_with("M3") || x.starts_with("M4"))
            .map(String::as_str)
            .collect();
        assert_eq!(modes, ["M3 S0", "M4 S0"]);
        // El M4 llega tras el comentario del tercer bloque y antes de sus movimientos.
        let i = l.iter().position(|x| x == "; tres").unwrap();
        assert_eq!(l[i + 1], "M4 S0");
        // S vuelve a emitirse porque el cambio de modo lo dejó en 0.
        assert!(l[i + 2..].iter().any(|x| x.contains("S500")));
    }

    #[test]
    fn empty_toolpath_and_empty_blocks() {
        let l = gen_lines(vec![], &GcodeOptions::default());
        assert_eq!(
            l,
            [
                "; Generado por Tlaku",
                "G90",
                "G21",
                "M4 S0",
                "M5",
                "G0 X0 Y0"
            ]
        );
        let l = gen_lines(
            vec![block("vacío", PowerMode::Constant, vec![])],
            &GcodeOptions {
                return_to_origin: false,
                ..Default::default()
            },
        );
        assert_eq!(l, ["; Generado por Tlaku", "G90", "G21", "M4 S0", "M5"]);
    }

    #[test]
    fn start_and_end_scripts() {
        let b = block(
            "A",
            PowerMode::Dynamic,
            vec![Move::Travel { to: p(1.0, 2.0) }],
        );
        let opts = GcodeOptions {
            start_gcode: Some("\n  \nM7 ; aire  \nG4 P1\n\n".into()),
            end_gcode: Some("M2\n".into()),
            ..Default::default()
        };
        let l = gen_lines(vec![b], &opts);
        assert_eq!(
            &l[..6],
            [
                "; Generado por Tlaku",
                "G90",
                "G21",
                "M7 ; aire",
                "G4 P1",
                "M4 S0"
            ]
        );
        assert_eq!(&l[l.len() - 3..], ["M5", "G0 X0 Y0", "M2"]);
    }

    #[test]
    fn power_is_clamped() {
        assert_eq!(s_value(150.0, 1000.0, 3), "1000");
        assert_eq!(s_value(-20.0, 1000.0, 3), "0");
        assert_eq!(s_value(f64::NAN, 1000.0, 3), "0");
        assert_eq!(s_value(f64::INFINITY, 255.0, 3), "255");
        assert_eq!(s_value(50.0, 255.0, 3), "127.5");
        assert_eq!(s_value(50.0, 1.0, 3), "0.5");
        assert_eq!(s_value(100.0, 255.5, 3), "255.5");
        assert_eq!(s_value(50.0, -10.0, 3), "0");
        assert_eq!(s_value(50.0, f64::NAN, 3), "0");

        let b = block(
            "A",
            PowerMode::Dynamic,
            vec![
                Move::Travel { to: p(0.0, 0.0) },
                Move::Cut {
                    to: p(1.0, 0.0),
                    power_pct: 400.0,
                },
                Move::Cut {
                    to: p(2.0, 0.0),
                    power_pct: -5.0,
                },
            ],
        );
        let l = gen_lines(
            vec![b],
            &GcodeOptions {
                s_max: 255.0,
                ..Default::default()
            },
        );
        assert!(l.contains(&"G1 X1 S255 F6000".to_string()));
        assert!(l.contains(&"G1 X2 S0".to_string()));
    }

    #[test]
    fn air_assist_per_block_and_closed_at_end() {
        let mut a = block(
            "A",
            PowerMode::Dynamic,
            vec![Move::Travel { to: p(1.0, 1.0) }],
        );
        a.air_assist = true;
        let mut b = block(
            "B",
            PowerMode::Dynamic,
            vec![Move::Travel { to: p(2.0, 2.0) }],
        );
        b.air_assist = true;
        let c = block(
            "C",
            PowerMode::Dynamic,
            vec![Move::Travel { to: p(3.0, 3.0) }],
        );
        let mut d = block(
            "D",
            PowerMode::Dynamic,
            vec![Move::Travel { to: p(4.0, 4.0) }],
        );
        d.air_assist = true;
        let l = gen_lines(vec![a, b, c, d], &GcodeOptions::default());
        let air: Vec<&str> = l
            .iter()
            .filter(|x| x.starts_with("M8") || x.starts_with("M9"))
            .map(String::as_str)
            .collect();
        assert_eq!(air, ["M8", "M9", "M8", "M9"]);
    }

    #[test]
    fn label_cannot_inject_gcode_and_bad_values_are_skipped() {
        let b = block(
            "Capa\nM3 S1000\r\nG0 X999",
            PowerMode::Dynamic,
            vec![
                Move::Travel {
                    to: p(f64::NAN, 1.0),
                },
                Move::Cut {
                    to: p(f64::INFINITY, 1.0),
                    power_pct: 10.0,
                },
                Move::Travel { to: p(1.0, 1.0) },
            ],
        );
        let l = gen_lines(vec![b], &GcodeOptions::default());
        assert!(l.contains(&"; Capa M3 S1000  G0 X999".to_string()));
        assert_eq!(l.iter().filter(|x| x.starts_with("G0")).count(), 2);
        assert!(!l.iter().any(|x| x.contains("NaN") || x.contains("inf")));
    }

    #[test]
    fn decimals_option_rounds_coordinates() {
        let b = block(
            "A",
            PowerMode::Dynamic,
            vec![
                Move::Travel {
                    to: p(1.23456, 2.0),
                },
                Move::Cut {
                    to: p(1.2349, 2.0),
                    power_pct: 10.0,
                },
            ],
        );
        let l = gen_lines(
            vec![b],
            &GcodeOptions {
                dialect: GcodeDialect {
                    decimals: 2,
                    ..GcodeDialect::default()
                },
                ..Default::default()
            },
        );
        assert!(l.contains(&"G0 X1.23 Y2".to_string()));
        // Redondeadas iguales: no hay movimiento que emitir.
        assert!(!l.iter().any(|x| x.starts_with("G1")));
    }

    fn cut(x: f64, y: f64) -> Move {
        Move::Cut {
            to: p(x, y),
            power_pct: 50.0,
        }
    }

    #[test]
    fn z_is_relative_lowers_before_the_first_cut_and_returns_after_each_block() {
        let mut a = block(
            "A",
            PowerMode::Dynamic,
            vec![Move::Travel { to: p(1.0, 1.0) }, cut(2.0, 1.0)],
        );
        a.z_mm = Some(1.5);
        let mut b = a.clone();
        b.label = "B".into();
        b.z_mm = Some(3.0);
        let mut none = a.clone();
        none.label = "C".into();
        none.z_mm = None;
        let mut zero = a.clone();
        zero.label = "D".into();
        zero.z_mm = Some(0.0);
        let l = gen_lines(vec![a, b, none, zero], &GcodeOptions::default());
        let z: Vec<&str> = l[3..] // sin la cabecera (G90, G21)
            .iter()
            .map(String::as_str)
            .filter(|x| x.contains('Z') || *x == "G91" || *x == "G90")
            .collect();
        // Profundidad positiva = Z negativa de GRBL. Cada bloque baja y sube; los que no tienen
        // profundidad (sin Z o a 0) no emiten nada.
        assert_eq!(
            z,
            [
                "G91", "G0 Z-1.5", "G90", "G91", "G0 Z1.5", "G90", "G91", "G0 Z-3", "G90", "G91",
                "G0 Z3", "G90"
            ]
        );
        // El foco baja tras el desplazamiento de aproximación y justo antes del primer corte.
        let at = |s: &str| l.iter().position(|x| x == s).unwrap();
        assert!(at("; A") < at("G0 X1 Y1") && at("G0 X1 Y1") < at("G0 Z-1.5"));
        assert!(at("G0 Z-1.5") < at("G1 X2 S500 F6000"));
        // Y vuelve a la altura inicial antes de lo que sigue.
        assert!(at("G0 Z1.5") < at("; B"));
        // La suma de lo emitido es 0: al cerrar el trabajo el cabezal está en la altura inicial.
        let n = l.len();
        assert_eq!(&l[n - 2..], ["M5", "G0 X0 Y0"]);
    }

    #[test]
    fn z_invert_flips_the_sign_of_every_z_word_and_nothing_else() {
        let mut a = block(
            "A",
            PowerMode::Dynamic,
            vec![Move::Travel { to: p(1.0, 1.0) }, cut(2.0, 1.0)],
        );
        a.z_mm = Some(-0.75);
        let normal = gen_lines(vec![a.clone()], &GcodeOptions::default());
        let inverted = gen_lines(
            vec![a],
            &GcodeOptions {
                z_invert: true,
                ..Default::default()
            },
        );
        let flip = |l: &str| {
            if let Some(v) = l.strip_prefix("G0 Z-") {
                format!("G0 Z{v}")
            } else if let Some(v) = l.strip_prefix("G0 Z") {
                format!("G0 Z-{v}")
            } else {
                l.to_string()
            }
        };
        let flipped: Vec<String> = normal.iter().map(|l| flip(l)).collect();
        assert_eq!(flipped, inverted);
        assert!(normal.contains(&"G0 Z0.75".to_string()));
        assert!(inverted.contains(&"G0 Z-0.75".to_string()));
    }

    #[test]
    fn z_moves_are_g0_only_and_net_to_zero() {
        // Varias profundidades seguidas con redondeo: lo emitido suma exactamente 0.
        let mut blocks = Vec::new();
        for (i, z) in [0.3333, 0.6667, 1.0001, -0.2, 0.1].into_iter().enumerate() {
            let mut b = block(
                &format!("B{i}"),
                PowerMode::Dynamic,
                vec![Move::Travel { to: p(0.0, 0.0) }, cut(1.0 + i as f64, 0.0)],
            );
            b.z_mm = Some(z);
            blocks.push(b);
        }
        let l = gen_lines(blocks, &GcodeOptions::default());
        let sum: f64 = l
            .iter()
            .filter_map(|x| x.strip_prefix("G0 Z"))
            .map(|v| v.parse::<f64>().unwrap())
            .sum();
        assert!(sum.abs() < 1e-9, "{sum}");
        assert!(!l.iter().any(|x| x.starts_with("G1 Z")));
        // Ninguna línea con Z lleva potencia ni velocidad.
        assert!(
            l.iter()
                .filter(|x| x.contains('Z'))
                .all(|x| !x.contains('F') && !x.contains(" S"))
        );
    }

    #[test]
    fn rotary_scales_y_and_rescales_the_feed_per_segment() {
        let b = block(
            "A",
            PowerMode::Dynamic,
            vec![
                Move::Travel { to: p(0.0, 0.0) },
                cut(10.0, 0.0),  // solo X: F del bloque
                cut(10.0, 10.0), // solo Y: F * k
                cut(13.0, 14.0), // diagonal 3-4-5 en superficie
            ],
        );
        let l = gen_lines(
            vec![b],
            &GcodeOptions {
                y_scale: 2.0,
                ..Default::default()
            },
        );
        assert!(l.contains(&"G1 X10 S500 F6000".to_string()), "{l:?}");
        assert!(l.contains(&"G1 Y20 F12000".to_string()), "{l:?}");
        // |(3, 8)| / |(3, 4)| * 6000 = 10252.804
        assert!(l.contains(&"G1 X13 Y28 F10252.804".to_string()), "{l:?}");
    }

    #[test]
    fn unit_scale_matches_the_plain_output() {
        let moves = vec![
            Move::Travel { to: p(0.0, 0.0) },
            cut(10.0, 5.0),
            cut(2.0, 9.0),
        ];
        let plain = gen_lines(
            vec![block("A", PowerMode::Dynamic, moves.clone())],
            &GcodeOptions::default(),
        );
        let unit = gen_lines(
            vec![block("A", PowerMode::Dynamic, moves)],
            &GcodeOptions {
                y_scale: 1.0,
                ..Default::default()
            },
        );
        assert_eq!(plain, unit);
        assert!(plain.contains(&"G1 X10 Y5 S500 F6000".to_string()));
    }

    #[test]
    fn invalid_scale_is_treated_as_one() {
        let moves = vec![Move::Travel { to: p(0.0, 3.0) }];
        for k in [0.0, -2.0, f64::NAN, f64::INFINITY] {
            let l = gen_lines(
                vec![block("A", PowerMode::Dynamic, moves.clone())],
                &GcodeOptions {
                    y_scale: k,
                    ..Default::default()
                },
            );
            assert!(l.contains(&"G0 X0 Y3".to_string()), "{k}: {l:?}");
        }
    }
}
