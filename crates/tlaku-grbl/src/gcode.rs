//! Generación de G-code para GRBL 1.1+ en modo láser (`$32=1`).
//!
//! Consume un [`Toolpath`]. Cada bloque elige M4 (potencia dinámica: GRBL baja la
//! potencia al desacelerar en las esquinas) o M3 (potencia constante). El láser
//! se apaga solo en los G0; el overscan es un G1 con S0.
//!
//! La salida es compacta: S y F son modales y las coordenadas X o Y que no
//! cambian se omiten, lo que reduce mucho el tamaño del G-code de imágenes.
//! No se usan offsets G54..G59 porque el firmware de Creality no los maneja bien.

use std::fmt::Write;
use tlaku_core::toolpath::{Move, PowerMode, Toolpath};

#[derive(Debug, Clone, PartialEq)]
pub struct GcodeOptions {
    /// Valor de S que equivale al 100 % de potencia (ajuste `$30` de GRBL).
    pub s_max: f64,
    /// Volver a X0 Y0 al terminar.
    pub return_to_origin: bool,
    /// Líneas que se emiten tras `G90`/`G21` y antes del primer bloque.
    pub start_gcode: Option<String>,
    /// Líneas que se emiten al final, después de `M5` y del regreso al origen.
    pub end_gcode: Option<String>,
    /// Decimales máximos de las coordenadas (se quitan los ceros sobrantes).
    pub decimals: usize,
}

impl Default for GcodeOptions {
    fn default() -> Self {
        Self {
            s_max: 1000.0,
            return_to_origin: true,
            start_gcode: None,
            end_gcode: None,
            decimals: 3,
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

/// Valor de S para un porcentaje de potencia.
///
/// Se recorta a `0..=s_max` aunque el llamador ya valide: un S negativo hace que
/// GRBL rechace la línea (y el resto del trabajo se desfasa), uno por encima de
/// `$30` se interpreta según el firmware, y un NaN produciría texto inválido.
/// Se conservan decimales porque algunos firmwares (Smoothieware) usan S de 0 a 1.
fn s_value(power_pct: f64, s_max: f64) -> String {
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
    num(s, 3)
}

/// Estado modal del programa generado.
struct Writer {
    out: String,
    decimals: usize,
    x: Option<String>,
    y: Option<String>,
    s: Option<String>,
    f: Option<String>,
}

impl Writer {
    fn line(&mut self, s: &str) {
        self.out.push_str(s);
        self.out.push('\n');
    }

    /// Emite un G0/G1 con solo lo que cambió. No escribe nada si el punto es el
    /// mismo (tras redondear) o si sus coordenadas no son finitas.
    fn motion(&mut self, g: &str, to: tlaku_core::Point, s: Option<&str>, f: Option<&str>) {
        if !to.x.is_finite() || !to.y.is_finite() {
            return;
        }
        let x = num(to.x, self.decimals);
        let y = num(to.y, self.decimals);
        let dx = self.x.as_deref() != Some(&x);
        let dy = self.y.as_deref() != Some(&y);
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
        if let Some(f) = f.filter(|f| self.f.as_deref() != Some(f)) {
            write!(cmd, " F{f}").unwrap();
            self.f = Some(f.into());
        }
        self.line(&cmd);
    }
}

fn mode_cmd(mode: PowerMode) -> &'static str {
    match mode {
        PowerMode::Dynamic => "M4 S0",
        PowerMode::Constant => "M3 S0",
    }
}

/// Copia un script del usuario línea a línea, sin espacios de sobra ni líneas
/// vacías al principio o al final.
fn script(w: &mut Writer, text: &str) {
    for l in text.trim().lines() {
        w.line(l.trim_end());
    }
}

/// Las etiquetas van tras `;`; un salto de línea las convertiría en G-code.
fn comment(label: &str) -> String {
    let flat: Vec<&str> = label.split(['\n', '\r']).collect();
    format!("; {}", flat.join(" ").trim())
}

pub fn generate(toolpath: &Toolpath, opts: &GcodeOptions) -> String {
    let mut w = Writer {
        out: String::new(),
        decimals: opts.decimals,
        x: None,
        y: None,
        s: None,
        f: None,
    };
    let blocks: Vec<_> = toolpath
        .blocks
        .iter()
        .filter(|b| !b.moves.is_empty())
        .collect();

    w.line("; Generado por Tlaku");
    w.line("G90");
    w.line("G21");
    if let Some(start) = &opts.start_gcode {
        script(&mut w, start);
    }
    let mut mode = blocks.first().map_or(PowerMode::Dynamic, |b| b.power_mode);
    w.line(mode_cmd(mode));
    w.s = Some("0".into());

    let mut air_on = false;
    for block in blocks {
        w.line(&comment(&block.label));
        if block.power_mode != mode {
            // S0 deja el láser apagado hasta el siguiente G1 con potencia.
            mode = block.power_mode;
            w.line(mode_cmd(mode));
            w.s = Some("0".into());
        }
        if block.air_assist != air_on {
            w.line(if block.air_assist { "M8" } else { "M9" });
            air_on = block.air_assist;
        }
        let f = (block.speed_mm_min.is_finite() && block.speed_mm_min > 0.0)
            .then(|| num(block.speed_mm_min, opts.decimals));
        for mv in &block.moves {
            match *mv {
                Move::Travel { to } => w.motion("G0", to, None, None),
                Move::Cut { to, power_pct } => {
                    let s = s_value(power_pct, opts.s_max);
                    w.motion("G1", to, Some(&s), f.as_deref());
                }
            }
        }
    }

    if air_on {
        w.line("M9");
    }
    w.line("M5");
    if opts.return_to_origin {
        w.line("G0 X0 Y0");
    }
    if let Some(end) = &opts.end_gcode {
        script(&mut w, end);
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
        assert_eq!(s_value(150.0, 1000.0), "1000");
        assert_eq!(s_value(-20.0, 1000.0), "0");
        assert_eq!(s_value(f64::NAN, 1000.0), "0");
        assert_eq!(s_value(f64::INFINITY, 255.0), "255");
        assert_eq!(s_value(50.0, 255.0), "127.5");
        assert_eq!(s_value(50.0, 1.0), "0.5");
        assert_eq!(s_value(100.0, 255.5), "255.5");
        assert_eq!(s_value(50.0, -10.0), "0");
        assert_eq!(s_value(50.0, f64::NAN), "0");

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
                decimals: 2,
                ..Default::default()
            },
        );
        assert!(l.contains(&"G0 X1.23 Y2".to_string()));
        // Redondeadas iguales: no hay movimiento que emitir.
        assert!(!l.iter().any(|x| x.starts_with("G1")));
    }
}
