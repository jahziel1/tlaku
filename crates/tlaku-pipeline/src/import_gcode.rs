//! Importación de G-code (`.nc`, `.gcode`, `.gc`, `.ngc`) como trayectorias de láser.
//!
//! Solo cuenta lo que quema: un avance (`G1`, `G2`, `G3`) con el láser encendido, es decir con
//! `M3` o `M4` activo y `S` mayor que 0. Los `G0` nunca se dibujan. Se entienden `G90`/`G91`,
//! `G20`/`G21`, `G90.1`/`G91.1` (arcos con `I J` absolutos o relativos), `G92` y los arcos con
//! `I J` o con `R`. Referencia pública: documentación de GRBL y de LinuxCNC sobre el lenguaje
//! RS-274; aquí solo se interpreta la geometría, no se copia código de ellos.
//!
//! Capa: por defecto una por nivel de potencia (`S`), con hasta 8 niveles (si hay más se
//! reparten en 8 tramos entre la potencia mínima y la máxima). Con `gcode_by_power = false`, una
//! sola capa. Un archivo sin `M3`/`M4` se interpreta por el valor de `S` (o, sin `S` tampoco,
//! como una trayectoria cualquiera) y se avisa.

use crate::import::{ImportExtras, ImportOptions, arc_to_cubics, from_curves};
use std::f64::consts::TAU;
use std::path::Path;
use tlaku_core::bezpath::{BezPath, PathSeg};
use tlaku_core::{Point, Rgb};
use tlaku_doc::ImportResult;
use tlaku_i18n::tr;

/// Niveles de potencia distintos que se llevan a capas.
const MAX_LEVELS: usize = 8;
/// Colores de las capas por potencia, de menor a mayor.
const PALETTE: [Rgb; MAX_LEVELS] = [
    Rgb(0, 0, 255),
    Rgb(0, 190, 220),
    Rgb(0, 160, 0),
    Rgb(230, 190, 0),
    Rgb(255, 128, 0),
    Rgb(255, 0, 0),
    Rgb(255, 0, 255),
    Rgb(0, 0, 0),
];
/// Distancia (mm) bajo la cual el final de un trazo cierra la forma.
const CLOSE_EPS_MM: f64 = 1e-3;
/// Tope de líneas que se interpretan (contra archivos enormes o corruptos).
const MAX_LINES: usize = 20_000_000;

/// Trayectorias del G-code (movimientos con el láser encendido).
pub(crate) fn import(
    path: &Path,
    data: &[u8],
    _opts: &ImportOptions,
    extras: &ImportExtras,
) -> Result<ImportResult, String> {
    let text = String::from_utf8_lossy(data);
    let mut interp = Interp::new(&text, extras.gcode_default_power);
    for (n, line) in text.lines().enumerate() {
        if n >= MAX_LINES {
            interp.truncated = true;
            break;
        }
        interp.line(line);
    }
    interp.flush();
    interp.into_result(path, extras.gcode_by_power)
}

/// Una trayectoria continua a una misma potencia.
struct Run {
    power: f64,
    path: BezPath,
}

struct Interp {
    /// Posición de máquina (mm).
    pos: Point,
    /// Desfase de `G92`: coordenada de trabajo = máquina - desfase.
    offset: (f64, f64),
    absolute: bool,
    arc_ij_absolute: bool,
    unit_mm: f64,
    /// Modo de movimiento modal: 0, 1, 2 o 3.
    motion: Option<u8>,
    /// Plano distinto de XY (G18/G19): los arcos no se interpretan.
    plane_xy: bool,
    spindle_on: bool,
    power: f64,
    /// El archivo usa `M3`/`M4` (si no, manda `S`).
    uses_spindle: bool,
    /// El archivo usa alguna `S`.
    uses_power: bool,
    /// `S` con el que se toman los avances de potencia 0 (opción del diálogo de importar).
    default_power: Option<f64>,
    cur: Option<Run>,
    runs: Vec<Run>,
    /// Avances G1/G2/G3 con movimiento que no se dibujaron por tener potencia 0 (con el husillo
    /// encendido si el archivo usa M3/M4).
    zero_power_feeds: usize,
    /// Avances con potencia 0 que se tomaron como quemados con `default_power`.
    defaulted_feeds: usize,
    ignored: Vec<String>,
    non_xy_arcs: usize,
    bad_arcs: usize,
    truncated: bool,
}

impl Interp {
    fn new(text: &str, default_power: Option<f64>) -> Self {
        // Un recorrido previo decide si el encendido lo gobierna M3/M4 o solo S.
        let (mut spindle, mut power) = (false, false);
        for line in text.lines() {
            for (letter, value) in words(line) {
                match letter {
                    'M' if value == 3.0 || value == 4.0 => spindle = true,
                    'S' => power = true,
                    _ => {}
                }
            }
        }
        Self {
            pos: Point::new(0.0, 0.0),
            offset: (0.0, 0.0),
            absolute: true,
            arc_ij_absolute: false,
            unit_mm: 1.0,
            motion: None,
            plane_xy: true,
            spindle_on: false,
            power: 0.0,
            uses_spindle: spindle,
            uses_power: power,
            default_power: default_power.filter(|p| p.is_finite() && *p > 0.0),
            cur: None,
            runs: Vec::new(),
            zero_power_feeds: 0,
            defaulted_feeds: 0,
            ignored: Vec::new(),
            non_xy_arcs: 0,
            bad_arcs: 0,
            truncated: false,
        }
    }

    /// Potencia con la que quema el avance en curso: la `S` activa o, si es 0, la que el usuario
    /// eligió para los cortes sin `S`.
    fn effective_power(&self) -> f64 {
        if self.power > 0.0 {
            self.power
        } else {
            self.default_power.unwrap_or(0.0)
        }
    }

    fn laser_on(&self) -> bool {
        if self.uses_spindle {
            self.spindle_on && self.effective_power() > 0.0
        } else if self.uses_power {
            self.effective_power() > 0.0
        } else {
            true
        }
    }

    fn note_ignored(&mut self, what: String) {
        if !self.ignored.contains(&what) {
            self.ignored.push(what);
        }
    }

    fn line(&mut self, line: &str) {
        let ws = words(line);
        if ws.is_empty() {
            return;
        }
        let (mut x, mut y, mut i, mut j, mut r) = (None, None, None, None, None);
        let mut set_offset = false;
        // Primero lo que no es movimiento: unidades, modos, S y M (el orden estándar).
        for &(letter, v) in &ws {
            match letter {
                'X' => x = Some(v),
                'Y' => y = Some(v),
                'I' => i = Some(v),
                'J' => j = Some(v),
                'R' => r = Some(v),
                'S' => self.power = v.max(0.0),
                _ => {}
            }
        }
        for &(letter, v) in &ws {
            let code = (v * 10.0).round() as i64;
            match (letter, code) {
                ('G', 200) => self.unit_mm = 25.4,
                ('G', 210) => self.unit_mm = 1.0,
                ('G', 900) => self.absolute = true,
                ('G', 910) => self.absolute = false,
                ('G', 901) => self.arc_ij_absolute = true,
                ('G', 911) => self.arc_ij_absolute = false,
                ('G', 170) => self.plane_xy = true,
                ('G', 180) | ('G', 190) => self.plane_xy = false,
                ('G', 920) => set_offset = true,
                ('G', 0) => self.motion = Some(0),
                ('G', 10) => self.motion = Some(1),
                ('G', 20) => self.motion = Some(2),
                ('G', 30) => self.motion = Some(3),
                ('M', 30) | ('M', 40) => self.spindle_on = true,
                ('M', 50) => self.spindle_on = false,
                ('G', c) | ('M', c) => {
                    // Códigos válidos que no cambian la geometría que se importa (los valores
                    // son el código por 10: G4 = 40, M106 = 1060).
                    const QUIET_G: [i64; 12] =
                        [40, 400, 410, 420, 430, 490, 540, 550, 560, 570, 580, 590];
                    const QUIET_M: [i64; 11] = [0, 10, 20, 60, 70, 80, 90, 300, 1060, 1070, 1120];
                    let quiet = if letter == 'G' {
                        QUIET_G.contains(&c)
                    } else {
                        QUIET_M.contains(&c)
                    };
                    if !quiet {
                        self.note_ignored(format!("{letter}{}", fmt_code(v)));
                    }
                }
                _ => {}
            }
        }

        if set_offset {
            // G92 X.. Y..: la posición actual pasa a ser la dada; sin ejes, el desfase se anula.
            let k = self.unit_mm;
            if x.is_none() && y.is_none() {
                self.offset = (0.0, 0.0);
            } else {
                if let Some(x) = x {
                    self.offset.0 = self.pos.x - x * k;
                }
                if let Some(y) = y {
                    self.offset.1 = self.pos.y - y * k;
                }
            }
            return;
        }

        let moves = x.is_some() || y.is_some();
        if !moves {
            // Un G1 con solo S o F no se mueve; un arco sin destino tampoco.
            return;
        }
        let Some(mode) = self.motion else {
            return;
        };
        let k = self.unit_mm;
        let (wx, wy) = self.work(self.pos);
        let (tx, ty) = if self.absolute {
            (x.map_or(wx, |v| v * k), y.map_or(wy, |v| v * k))
        } else {
            (wx + x.unwrap_or(0.0) * k, wy + y.unwrap_or(0.0) * k)
        };
        let target = Point::new(tx + self.offset.0, ty + self.offset.1);
        let start = self.pos;
        match mode {
            0 => {
                self.flush();
                self.pos = target;
            }
            1 => {
                self.draw_line(start, target);
                self.pos = target;
            }
            _ => {
                let clockwise = mode == 2;
                self.arc(start, target, clockwise, i, j, r);
                self.pos = target;
            }
        }
    }

    fn work(&self, p: Point) -> (f64, f64) {
        (p.x - self.offset.0, p.y - self.offset.1)
    }

    /// Cuenta un avance con movimiento que no se dibuja por tener potencia 0.
    fn note_unpowered(&mut self) {
        if self.effective_power() <= 0.0 && (!self.uses_spindle || self.spindle_on) {
            self.zero_power_feeds += 1;
        }
    }

    fn draw_line(&mut self, start: Point, target: Point) {
        if !self.laser_on() || start.dist(target) < 1e-9 {
            if start.dist(target) >= 1e-9 {
                self.note_unpowered();
            }
            self.flush();
            return;
        }
        let segs = vec![PathSeg::LineTo { to: target }];
        self.push_drawn(start, segs);
    }

    fn arc(
        &mut self,
        start: Point,
        end: Point,
        clockwise: bool,
        i: Option<f64>,
        j: Option<f64>,
        r: Option<f64>,
    ) {
        if !self.plane_xy {
            self.non_xy_arcs += 1;
            self.flush();
            return;
        }
        let k = self.unit_mm;
        let center = if let Some(r) = r {
            let r = r * k;
            let (dx, dy) = (end.x - start.x, end.y - start.y);
            let d = dx.hypot(dy);
            if d < 1e-9 || r.abs() < d / 2.0 - 1e-6 {
                self.bad_arcs += 1;
                self.draw_line(start, end);
                return;
            }
            let h = (r * r - d * d / 4.0).max(0.0).sqrt();
            // Normal a la izquierda del sentido de la cuerda.
            let (nx, ny) = (-dy / d, dx / d);
            // Antihorario con R > 0: centro a la izquierda. Horario o R < 0 lo invierten.
            let side = if clockwise { -1.0 } else { 1.0 } * if r < 0.0 { -1.0 } else { 1.0 };
            Point::new(
                (start.x + end.x) / 2.0 + side * h * nx,
                (start.y + end.y) / 2.0 + side * h * ny,
            )
        } else if i.is_some() || j.is_some() {
            let (ci, cj) = (i.unwrap_or(0.0) * k, j.unwrap_or(0.0) * k);
            if self.arc_ij_absolute {
                Point::new(ci + self.offset.0, cj + self.offset.1)
            } else {
                Point::new(start.x + ci, start.y + cj)
            }
        } else {
            self.bad_arcs += 1;
            self.draw_line(start, end);
            return;
        };
        let radius = start.dist(center);
        if radius < 1e-9 {
            self.bad_arcs += 1;
            self.draw_line(start, end);
            return;
        }
        if !self.laser_on() {
            self.note_unpowered();
            self.flush();
            return;
        }
        let a0 = (start.y - center.y).atan2(start.x - center.x);
        let a1 = (end.y - center.y).atan2(end.x - center.x);
        let mut sweep = if clockwise { a0 - a1 } else { a1 - a0 };
        sweep = sweep.rem_euclid(TAU);
        // Mismo punto de inicio y fin: círculo completo.
        if sweep < 1e-9 && start.dist(end) < 1e-6 {
            sweep = TAU;
        }
        if clockwise {
            sweep = -sweep;
        }
        let segs = arc_to_cubics((center.x, center.y), radius, a0, sweep, end);
        self.push_drawn(start, segs);
    }

    /// Añade segmentos dibujados a la trayectoria en curso si continúan donde esta termina y
    /// con la misma potencia; si no, empieza otra.
    fn push_drawn(&mut self, start: Point, segs: Vec<PathSeg>) {
        let power = self.effective_power();
        if self.power <= 0.0 {
            self.defaulted_feeds += 1;
        }
        let continues = self.cur.as_ref().is_some_and(|run| {
            (run.power - power).abs() < 1e-9
                && last_point(&run.path).is_some_and(|p| p.dist(start) < 1e-9)
        });
        if !continues {
            self.flush();
            self.cur = Some(Run {
                power,
                path: BezPath(vec![PathSeg::MoveTo { to: start }]),
            });
        }
        if let Some(run) = &mut self.cur {
            run.path.0.extend(segs);
        }
    }

    fn flush(&mut self) {
        let Some(mut run) = self.cur.take() else {
            return;
        };
        let first = match run.path.0.first() {
            Some(PathSeg::MoveTo { to }) => *to,
            _ => return,
        };
        if run.path.0.len() > 2
            && last_point(&run.path).is_some_and(|p| p.dist(first) < CLOSE_EPS_MM)
        {
            if matches!(run.path.0.last(), Some(PathSeg::LineTo { .. })) {
                run.path.0.pop();
            }
            run.path.0.push(PathSeg::Close);
        }
        self.runs.push(run);
    }

    fn into_result(self, path: &Path, by_power: bool) -> Result<ImportResult, String> {
        let name = path.display();
        let mut warnings = Vec::new();
        if self.runs.is_empty() {
            if self.zero_power_feeds > 0 {
                // La interfaz reconoce este error por su código (no por el texto).
                return Err(tlaku_i18n::coded(
                    "gcode_zero_power",
                    tr!(
                        "pipeline.import_gcode.zero_power",
                        name = name,
                        n = self.zero_power_feeds
                    ),
                ));
            }
            return Err(tr!("pipeline.import_gcode.nothing_drawn", name = name));
        }
        if let (Some(p), true) = (self.default_power, self.defaulted_feeds > 0) {
            warnings.push(tr!(
                "pipeline.import_gcode.defaulted_feeds",
                n = self.defaulted_feeds,
                s = fmt_code(p)
            ));
        }
        if !self.uses_spindle {
            warnings.push(if self.uses_power {
                tr!("pipeline.import_gcode.no_spindle")
            } else {
                tr!("pipeline.import_gcode.no_spindle_no_power")
            });
        }
        // Niveles de potencia distintos, de menor a mayor.
        let mut levels: Vec<f64> = Vec::new();
        for run in &self.runs {
            if !levels.iter().any(|l| (l - run.power).abs() < 1e-9) {
                levels.push(run.power);
            }
        }
        levels.sort_by(f64::total_cmp);
        if by_power && levels.len() > MAX_LEVELS {
            warnings.push(tr!(
                "pipeline.import_gcode.many_levels",
                n = levels.len(),
                max = MAX_LEVELS
            ));
        }
        let bucket = |power: f64| -> usize {
            if !by_power || levels.len() == 1 {
                return 0;
            }
            if levels.len() <= MAX_LEVELS {
                return levels
                    .iter()
                    .position(|l| (l - power).abs() < 1e-9)
                    .unwrap_or(0);
            }
            let (lo, hi) = (levels[0], levels[levels.len() - 1]);
            (((power - lo) / (hi - lo) * MAX_LEVELS as f64) as usize).min(MAX_LEVELS - 1)
        };
        let mut layers: Vec<(usize, Vec<BezPath>)> = Vec::new();
        for run in self.runs {
            let b = bucket(run.power);
            match layers.iter_mut().find(|(k, _)| *k == b) {
                Some((_, paths)) => paths.push(run.path),
                None => layers.push((b, vec![run.path])),
            }
        }
        layers.sort_by_key(|(b, _)| *b);
        // Una sola capa sale en negro; varias, con la paleta por potencia. Con menos niveles
        // que colores se usan los extremos de la paleta para distinguirlos mejor.
        let count = layers.len();
        let layers: Vec<(Rgb, Vec<BezPath>)> = layers
            .into_iter()
            .enumerate()
            .map(|(n, (_, paths))| {
                let color = if count == 1 {
                    PALETTE[MAX_LEVELS - 1]
                } else {
                    PALETTE[(n * (MAX_LEVELS - 1) / (count - 1).max(1)).min(MAX_LEVELS - 1)]
                };
                (color, paths)
            })
            .collect();
        if !self.ignored.is_empty() {
            warnings.push(tr!(
                "pipeline.import_gcode.ignored_codes",
                codes = self.ignored.join(", ")
            ));
        }
        if self.non_xy_arcs > 0 {
            warnings.push(tr!(
                "pipeline.import_gcode.non_xy_arcs",
                n = self.non_xy_arcs
            ));
        }
        if self.bad_arcs > 0 {
            warnings.push(tr!("pipeline.import_gcode.bad_arcs", n = self.bad_arcs));
        }
        if self.truncated {
            warnings.push(tr!("pipeline.import_gcode.truncated", max = MAX_LINES));
        }
        Ok(from_curves(layers, warnings))
    }
}

fn last_point(path: &BezPath) -> Option<Point> {
    match path.0.last()? {
        PathSeg::MoveTo { to }
        | PathSeg::LineTo { to }
        | PathSeg::QuadTo { to, .. }
        | PathSeg::CubicTo { to, .. } => Some(*to),
        PathSeg::Close => None,
    }
}

/// Un código como `1` o `90.1` para mostrar en un aviso.
fn fmt_code(v: f64) -> String {
    if (v - v.round()).abs() < 1e-9 {
        format!("{}", v as i64)
    } else {
        format!("{v}")
    }
}

/// Palabras `(letra, valor)` de una línea, sin comentarios (`;...` y `(...)`) ni números de
/// línea (`N`). Acepta palabras pegadas (`G1X10Y5`).
fn words(line: &str) -> Vec<(char, f64)> {
    let mut out = Vec::new();
    let mut chars = line.chars().peekable();
    if line.trim_start().starts_with('%') {
        return out;
    }
    while let Some(c) = chars.next() {
        match c {
            ';' => break,
            '(' => {
                for d in chars.by_ref() {
                    if d == ')' {
                        break;
                    }
                }
            }
            c if c.is_ascii_alphabetic() => {
                let mut num = String::new();
                while let Some(&d) = chars.peek() {
                    // Un blanco entre la letra y el número se admite ("G 1").
                    if d == ' ' && num.is_empty() {
                        chars.next();
                        continue;
                    }
                    if d.is_ascii_digit() || d == '.' || ((d == '-' || d == '+') && num.is_empty())
                    {
                        num.push(d);
                        chars.next();
                    } else {
                        break;
                    }
                }
                if let Ok(v) = num.parse::<f64>()
                    && !c.eq_ignore_ascii_case(&'N')
                {
                    out.push((c.to_ascii_uppercase(), v));
                }
            }
            _ => {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use tlaku_doc::ObjectKind;

    fn run_with(text: &str, by_power: bool) -> Result<ImportResult, String> {
        import(
            Path::new("a.nc"),
            text.as_bytes(),
            &ImportOptions::default(),
            &ImportExtras {
                gcode_by_power: by_power,
                ..Default::default()
            },
        )
    }

    fn run(text: &str) -> Result<ImportResult, String> {
        run_with(text, true)
    }

    fn approx(a: f64, b: f64) {
        assert!((a - b).abs() < 1e-4, "{a} != {b}");
    }

    fn paths(r: &ImportResult, i: usize) -> &Vec<BezPath> {
        match &r.objects[i].kind {
            ObjectKind::Vector(v) => &v.paths,
            _ => panic!(),
        }
    }

    #[test]
    fn solo_dibuja_con_el_laser_encendido() {
        // Cuadrado de 10 mm: el primer lado se corre con láser apagado (S0) y no cuenta.
        let g = "G90 G21\nM4 S0\nG0 X0 Y0\nG1 X10 Y0 S0\nG1 X10 Y10 S500\nG1 X0 Y10\nG1 X0 Y0\nM5\nG0 X50 Y50\n";
        let r = run(g).unwrap();
        assert_eq!(r.objects.len(), 1);
        let b = r.objects[0].bounds().unwrap();
        approx(b.min.x, 0.0);
        approx(b.max.x, 10.0);
        approx(b.max.y, 10.0);
        // Tres lados de un solo trazo (no se dibujó el primero, que iba con S0).
        let p = &paths(&r, 0)[0].0;
        assert_eq!(p.len(), 4, "{p:?}");
        assert!(r.warnings.is_empty(), "{:?}", r.warnings);
    }

    #[test]
    fn g0_nunca_se_dibuja_y_g91_g20_funcionan() {
        // En pulgadas y relativo: 1 pulgada a la derecha con láser, luego rápido hacia arriba.
        let g = "G20 G91\nM3 S1000\nG1 X1 Y0\nG0 X0 Y1\nG1 X1\nM5\n";
        let r = run(g).unwrap();
        assert_eq!(paths(&r, 0).len(), 2);
        let b = r.objects[0].bounds().unwrap();
        approx(b.min.x, 0.0);
        approx(b.max.x, 50.8);
        approx(b.min.y, 0.0);
        approx(b.max.y, 25.4);
    }

    #[test]
    fn arcos_con_ij_y_con_r() {
        // Semicírculo antihorario (G3) de (0,0) a (10,0) con centro (5,0).
        let r = run("M3 S100\nG0 X0 Y0\nG3 X10 Y0 I5 J0\n").unwrap();
        let b = r.objects[0].bounds().unwrap();
        approx(b.min.x, 0.0);
        approx(b.max.x, 10.0);
        approx(b.min.y, -5.0 + 0.0);
        approx(b.max.y, 0.0);
        // Antihorario de (0,0) a (10,0) pasa por debajo (y < 0). El mismo con R=5 coincide.
        let r2 = run("M3 S100\nG0 X0 Y0\nG3 X10 Y0 R5\n").unwrap();
        let b2 = r2.objects[0].bounds().unwrap();
        approx(b2.min.y, b.min.y);
        // En sentido horario (G2) con R positivo pasa por arriba.
        let r3 = run("M3 S100\nG0 X0 Y0\nG2 X10 Y0 R5\n").unwrap();
        let b3 = r3.objects[0].bounds().unwrap();
        approx(b3.max.y, 5.0);
        // Círculo completo: destino igual al inicio.
        let r4 = run("M3 S100\nG0 X0 Y0\nG2 X0 Y0 I5 J0\n").unwrap();
        let b4 = r4.objects[0].bounds().unwrap();
        approx(b4.max.x - b4.min.x, 10.0);
        approx(b4.max.y - b4.min.y, 10.0);
        assert_eq!(paths(&r4, 0)[0].0.last(), Some(&PathSeg::Close));
    }

    #[test]
    fn capas_por_potencia_o_una_sola() {
        let g = "M4\nG0 X0 Y0\nG1 X10 S200\nG0 Y5\nG1 X0 S800\nG0 Y9\nG1 X10 S800\n";
        let r = run(g).unwrap();
        assert_eq!(r.objects.len(), 2);
        assert_ne!(r.objects[0].layer, r.objects[1].layer);
        assert_eq!(paths(&r, 0).len(), 1);
        assert_eq!(paths(&r, 1).len(), 2);
        let one = run_with(g, false).unwrap();
        assert_eq!(one.objects.len(), 1);
        assert_eq!(paths(&one, 0).len(), 3);
    }

    #[test]
    fn comentarios_palabras_pegadas_y_g92() {
        let g = "; encabezado\n(otro comentario)\nN10 G21 G90\nM3 S50\nG92 X100 Y100\nG0 X100Y100\nG1X110Y100(a la derecha)\nG28\n";
        let r = run(g).unwrap();
        let b = r.objects[0].bounds().unwrap();
        // G92 hace que (100,100) sea el origen de trabajo; el avance mide 10 mm.
        approx(b.max.x - b.min.x, 10.0);
        assert!(
            r.warnings.iter().any(|w| w.contains("G28")),
            "{:?}",
            r.warnings
        );
    }

    #[test]
    fn sin_m3_usa_s_y_sin_nada_importa_todo() {
        let r = run("G0 X0 Y0\nG1 X10 Y0 S300\nG1 X10 Y10 S0\n").unwrap();
        assert_eq!(paths(&r, 0).len(), 1);
        assert!(r.warnings.iter().any(|w| w.contains("no usa M3 ni M4")));
        let r = run("G0 X0 Y0\nG1 X10 Y0\nG1 X10 Y10\n").unwrap();
        assert_eq!(paths(&r, 0)[0].0.len(), 3);
        assert!(r.warnings.iter().any(|w| w.contains("no usa M3, M4 ni S")));
    }

    #[test]
    fn sin_trazos_es_error() {
        let e = run("M3 S300\nM5\nG0 X1 Y1\nG1 X5 Y5\n").unwrap_err();
        assert!(e.contains("no dibuja nada"), "{e}");
        // Con el láser apagado por M5 no hay nada que sugerir sobre la potencia.
        assert!(!e.contains("potencia 0"), "{e}");
    }

    #[test]
    fn potencia_cero_sugiere_fijar_una_potencia() {
        let e = run("M3 S0\nG0 X1 Y1\nG1 X5 Y5\nG1 X9 Y5\nM5\n").unwrap_err();
        assert!(e.contains("2 avances") && e.contains("potencia 0"), "{e}");
        assert!(e.contains("M3 S500"), "{e}");
    }

    #[test]
    fn una_potencia_por_omision_rescata_el_gcode_con_m3_s0() {
        let g = "G90 G21\nM3 S0\nG0 X0 Y0\nG1 X10 Y0\nG1 X10 Y10\nM5\n";
        // Sin la opción: error que explica cómo salir.
        let e = run(g).unwrap_err();
        assert!(
            e.contains("potencia 0") && e.contains("potencia para cortes sin S"),
            "{e}"
        );
        let with = |p: Option<f64>| {
            import(
                Path::new("a.nc"),
                g.as_bytes(),
                &ImportOptions::default(),
                &ImportExtras {
                    gcode_default_power: p,
                    ..Default::default()
                },
            )
        };
        // Una potencia que no es mayor que 0 (o no es un número) no cambia nada.
        assert!(with(Some(0.0)).is_err() && with(Some(f64::NAN)).is_err());
        let r = with(Some(500.0)).unwrap();
        assert_eq!(r.objects.len(), 1);
        let b = r.objects[0].bounds().unwrap();
        approx(b.max.x, 10.0);
        approx(b.max.y, 10.0);
        assert!(
            r.warnings.iter().any(|w| w.contains("S500")),
            "{:?}",
            r.warnings
        );
        // Un archivo con S propia no se toca: la opción solo cubre los cortes de potencia 0.
        let own = "M3 S300\nG1 X5 Y0\nG1 X5 Y5\n";
        let r = import(
            Path::new("a.nc"),
            own.as_bytes(),
            &ImportOptions::default(),
            &ImportExtras {
                gcode_default_power: Some(900.0),
                ..Default::default()
            },
        )
        .unwrap();
        assert!(
            r.warnings.iter().all(|w| !w.contains("S900")),
            "{:?}",
            r.warnings
        );
    }
}
