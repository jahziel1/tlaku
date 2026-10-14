//! Dónde reanudar y con qué G-code: localizar la línea real comparando la última posición con la
//! geometría, retroceder con prudencia y escribir el preámbulo que deja la máquina en el estado
//! modal correcto antes de seguir.
//!
//! `ok` en GRBL significa que la línea entró al búfer, no que se ejecutó: hasta 127 bytes y 15
//! bloques (127 en la Falcon2) van por delante de la cabeza. La posición del último reporte
//! (MPos) sí es de la cabeza, así que se busca entre las líneas confirmadas la que pasa por ella.
//! El reporte tiene hasta ~1,25 s de antigüedad y la cabeza avanzó desde entonces, de modo que el
//! punto estimado siempre cae antes del real: sobra, nunca falta.

use super::walker::{Parsed, Step, StepKind, is_sendable, modal_at};
use tlaku_doc::{GcodeDialect, ResumeAir, ResumeModal, ResumeSpindle};
use tlaku_i18n::tr;

/// Cuánto puede alejarse la MPos de la geometría y seguir contando como «encima de la línea».
const ON_PATH_MM: f64 = 0.5;
/// Empates: entre líneas a esta distancia de la mejor se elige la primera (retroceder sobra,
/// adelantar falta).
const TIE_MM: f64 = 0.02;
/// Un trazo con tantos pasos o más es una fila de barrido de imagen.
const ROW_MIN_STEPS: usize = 4;

/// Cuánto retroceder desde la línea estimada.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlanOptions {
    pub back_lines: usize,
    pub back_mm: f64,
}

impl Default for PlanOptions {
    fn default() -> Self {
        Self {
            back_lines: 3,
            back_mm: 1.5,
        }
    }
}

/// Dónde estaba la cabeza según la geometría.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Estimate {
    /// Paso (índice en `Parsed::steps`) que se estima en ejecución.
    pub step: usize,
    /// Fracción recorrida de ese paso, 0 a 1.
    pub t: f64,
    pub distance_mm: f64,
    /// No hubo posición o no encajó con ninguna línea cercana: `step` es el comienzo de la ventana.
    pub mismatch: bool,
}

/// Distancia de `p` al segmento `a`-`b` y fracción del punto más cercano.
fn project(p: [f64; 2], a: [f64; 2], b: [f64; 2]) -> (f64, f64) {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let len2 = dx * dx + dy * dy;
    let t = if len2 <= f64::EPSILON {
        0.0
    } else {
        (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / len2).clamp(0.0, 1.0)
    };
    let (qx, qy) = (a[0] + t * dx, a[1] + t * dy);
    ((p[0] - qx).hypot(p[1] - qy), t)
}

/// Cuántas líneas confirmadas pueden ir por delante de la cabeza, según los búferes del último
/// reporte (bloques ocupados del planificador y bytes ocupados del búfer serie).
pub fn window_for(planner_used: u32, rx_used: u32) -> usize {
    (planner_used as usize + rx_used as usize / 8 + 24).clamp(32, 20_000)
}

/// Busca, entre los pasos confirmados con `ok` (hasta la línea `last_line`, inclusive) y dentro
/// de la ventana, el que pasa por `wpos`. `floor` es el primer paso que se puede elegir (en una
/// reanudación, el primero del resto del trabajo). `None` si no hay pasos confirmados.
pub fn locate(
    parsed: &Parsed,
    last_line: u32,
    wpos: Option<[f64; 2]>,
    window: usize,
    floor: usize,
) -> Option<Estimate> {
    let hi = parsed.step_at_or_after(last_line.saturating_add(1));
    let lo = hi.saturating_sub(window).max(floor);
    if lo >= hi {
        return None;
    }
    let steps = &parsed.steps[lo..hi];
    let fallback = Estimate {
        step: lo,
        t: 0.0,
        distance_mm: 0.0,
        mismatch: true,
    };
    let Some(p) = wpos else {
        return Some(fallback);
    };
    let best = steps
        .iter()
        .map(|s| project(p, s.from, s.to).0)
        .fold(f64::INFINITY, f64::min);
    if best > ON_PATH_MM {
        return Some(Estimate {
            distance_mm: best,
            ..fallback
        });
    }
    let (k, s) = steps
        .iter()
        .enumerate()
        .find(|(_, s)| project(p, s.from, s.to).0 <= best + TIE_MM)?;
    let (d, t) = project(p, s.from, s.to);
    Some(Estimate {
        step: lo + k,
        t,
        distance_mm: d,
        mismatch: false,
    })
}

/// Dónde se reanuda.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Landing {
    pub step: usize,
    /// Punto dentro del paso (solo en cortes rectos y absolutos).
    pub at: Option<[f64; 2]>,
    pub row: bool,
}

/// Primer paso del trazo (secuencia sin `G0`) que contiene al paso `k`.
fn stroke_start(steps: &[Step], k: usize, floor: usize) -> usize {
    if steps[k].kind == StepKind::Rapid {
        return k;
    }
    let mut j = k;
    while j > floor && steps[j - 1].kind != StepKind::Rapid {
        j -= 1;
    }
    j
}

/// ¿Es el trazo de `a` a `b` una fila de barrido: varios pasos alineados con el mismo eje?
fn is_scan_row(steps: &[Step]) -> bool {
    if steps.len() < ROW_MIN_STEPS {
        return false;
    }
    let horizontal = steps.iter().all(|s| (s.to[1] - s.from[1]).abs() < 1e-6);
    let vertical = steps.iter().all(|s| (s.to[0] - s.from[0]).abs() < 1e-6);
    horizontal || vertical
}

/// Retrocede desde la estimación: lo que pida `opts` (líneas o mm, lo que quede más atrás), sin
/// cruzar un `G0` (lo de antes del desplazamiento es otro trazo y ya está hecho) y, en una fila de
/// barrido de imagen, hasta el principio de la fila.
pub fn landing(parsed: &Parsed, est: &Estimate, opts: &PlanOptions, floor: usize) -> Landing {
    let steps = &parsed.steps;
    let k = est.step;
    if steps[k].kind == StepKind::Rapid || est.mismatch {
        // En un desplazamiento (láser apagado) no hay nada que repetir; sin posición fiable el
        // comienzo de la ventana ya es lo más atrás que se justifica.
        let j = if est.mismatch {
            stroke_start(steps, k, floor)
        } else {
            k
        };
        return Landing {
            step: j,
            at: None,
            row: false,
        };
    }
    let start = stroke_start(steps, k, floor);
    let mut end = k;
    while end + 1 < steps.len() && steps[end + 1].kind != StepKind::Rapid {
        end += 1;
    }
    if is_scan_row(&steps[start..=end]) {
        return Landing {
            step: start,
            at: None,
            row: true,
        };
    }
    // Regla de los mm: se camina hacia atrás por el trazo desde el punto estimado.
    let along = est.t * steps[k].length();
    let mut need = opts.back_mm.max(0.0);
    let (j_mm, o_mm) = if along >= need {
        (k, along - need)
    } else {
        need -= along;
        let mut j = k;
        loop {
            if j == start {
                break (start, 0.0);
            }
            j -= 1;
            let l = steps[j].length();
            if l >= need {
                break (j, l - need);
            }
            need -= l;
        }
    };
    // Regla de las líneas: tantos pasos atrás como se pidan.
    let j_ln = k.saturating_sub(opts.back_lines).max(start);
    let (j, o) = if opts.back_lines > 0 && (j_ln < j_mm || (j_ln == j_mm && o_mm > 0.0)) {
        (j_ln, 0.0)
    } else {
        (j_mm, o_mm)
    };
    let s = &steps[j];
    let len = s.length();
    let at =
        (o >= 0.05 && o < len - 0.05 && s.splittable && s.kind != StepKind::Rapid).then(|| {
            let f = o / len;
            [
                s.from[0] + (s.to[0] - s.from[0]) * f,
                s.from[1] + (s.to[1] - s.from[1]) * f,
            ]
        });
    Landing {
        step: j,
        at,
        row: false,
    }
}

// ---------------------------------------------------------------------------
// Preámbulo
// ---------------------------------------------------------------------------

/// El G-code que se envía: preámbulo + resto del trabajo, y cómo se corresponde con el original.
#[derive(Debug, Clone, PartialEq)]
pub struct Program {
    pub lines: Vec<String>,
    /// Líneas del preámbulo (con comentarios).
    pub preamble_len: usize,
    /// Línea del original con la que sigue el trabajo.
    pub start: usize,
    /// Dónde termina el desplazamiento del preámbulo (láser apagado).
    pub travel_to: Option<[f64; 2]>,
}

/// Número para G-code: hasta 3 decimales, sin ceros sobrantes ni `-0`.
fn num(v: f64) -> String {
    let s = format!("{v:.3}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" || s.is_empty() {
        "0".into()
    } else {
        s.into()
    }
}

/// `plantilla` (`M4 S0`, `M3 S0 I`) con la `S` cambiada por `s`.
fn with_s(template: &str, s: Option<f64>) -> String {
    let Some(s) = s else {
        return template.trim().to_string();
    };
    let mut found = false;
    let mut words: Vec<String> = template
        .split_whitespace()
        .map(|w| {
            if w.len() > 1 && w.starts_with('S') && w[1..].parse::<f64>().is_ok() {
                found = true;
                format!("S{}", num(s))
            } else {
                w.to_string()
            }
        })
        .collect();
    if !found {
        words.push(format!("S{}", num(s)));
    }
    words.join(" ")
}

/// ¿Puede reanudar este dialecto? La potencia tiene que ir en las líneas de movimiento (`S`
/// dentro de `G1`) y `G0` debe apagar el láser: en otro caso el desplazamiento al punto de inicio
/// podría quemar.
pub fn dialect_supported(d: &GcodeDialect) -> bool {
    d.s_inline && d.g0_laser_off && !d.constant_on.trim().is_empty()
}

/// Arma el G-code de la reanudación: restaura el estado modal en el punto `start` (línea del
/// original), se desplaza hasta allí con `G0` y el láser apagado y solo entonces enciende
/// `M3`/`M4` con su `S` y `F`. `at` es el punto a mitad de línea, si lo hay; `z_delta` el ajuste de
/// Z (solo con eje Z).
pub fn build_program(
    original: &[String],
    start: usize,
    at: Option<[f64; 2]>,
    z_delta: f64,
    dialect: &GcodeDialect,
    z_axis: bool,
) -> Result<Program, String> {
    if start >= original.len() {
        return Err(tr!("desktop.resume.line_out_of_range"));
    }
    if !dialect_supported(dialect) {
        return Err(tr!("desktop.resume.dialect_unsupported"));
    }
    let (modal, pos) = modal_at(original, start, false);
    let travel_to = at.or(pos);
    let total = original.len();
    let mut lines: Vec<String> = vec![
        format!(
            "; {}",
            tr!(
                "desktop.resume.preamble_comment",
                line = start + 1,
                total = total
            )
        ),
        // Lo primero, el láser apagado: nada de lo que sigue puede quemar.
        dialect.laser_off.trim().to_string(),
        "G90".into(),
        "G21".into(),
    ];
    match modal.air {
        ResumeAir::Off => {}
        ResumeAir::Mist => lines.push("M7".into()),
        ResumeAir::Flood => lines.push("M8".into()),
    }
    if z_axis && z_delta.abs() > 1e-6 {
        lines.push("G91".into());
        lines.push(format!("G0 Z{}", num(z_delta)));
        lines.push("G90".into());
    }
    if let Some([x, y]) = travel_to {
        lines.push(format!("G0 X{} Y{}", num(x), num(y)));
    }
    if modal.inches {
        lines.push("G20".into());
    }
    if !modal.absolute {
        lines.push("G91".into());
    }
    if let Some(f) = modal.f {
        lines.push(format!("F{}", num(f)));
    }
    match modal.spindle {
        ResumeSpindle::Off => {}
        ResumeSpindle::M3 => lines.push(with_s(&dialect.constant_on, modal.s)),
        ResumeSpindle::M4 => lines.push(with_s(
            dialect
                .dynamic_on
                .as_deref()
                .unwrap_or(&dialect.constant_on),
            modal.s,
        )),
    }
    let preamble_len = lines.len();
    verify_preamble(&lines)?;
    lines.extend(original[start..].iter().cloned());
    Ok(Program {
        lines,
        preamble_len,
        start,
        travel_to,
    })
}

/// Comprobación de seguridad del preámbulo: ningún movimiento suyo puede ir con el láser
/// encendido y el desplazamiento tiene que ser `G0`.
pub fn verify_preamble(preamble: &[String]) -> Result<(), String> {
    let p = Parsed::parse(preamble);
    if p.steps.iter().any(|s| s.kind != StepKind::Rapid) {
        return Err(tr!("desktop.resume.preamble_burns"));
    }
    Ok(())
}

/// Líneas enviables de `lines`.
pub fn sendable_count(lines: &[String]) -> usize {
    lines.iter().filter(|l| is_sendable(l)).count()
}

/// Encuadre del trabajo completo con el láser apagado, a partir del G-code guardado (que ya está
/// en coordenadas de la máquina).
pub fn frame_lines(parsed: &Parsed, speed: f64) -> Result<Vec<String>, String> {
    if !(speed.is_finite() && speed > 0.0) {
        return Err(tr!("desktop.common.frame_speed"));
    }
    let b = parsed
        .bounds()
        .ok_or_else(|| tr!("desktop.common.nothing_to_frame"))?;
    Ok(tlaku_grbl::frame::frame_gcode(
        tlaku_core::Bounds {
            min: tlaku_core::Point::new(b[0][0], b[0][1]),
            max: tlaku_core::Point::new(b[1][0], b[1][1]),
        },
        speed,
    ))
}

/// Variación de Z entre la altura que dejó el trabajo al interrumpirse (tras la línea
/// `exec_line`) y la que necesita el punto de reinicio (antes de la línea `start`).
pub fn z_delta(original: &[String], exec_line: u32, start: usize) -> f64 {
    let (at_exec, _): (ResumeModal, _) = modal_at(original, exec_line as usize, true);
    let (at_start, _) = modal_at(original, start, false);
    at_start.z - at_exec.z
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(text: &str) -> Vec<String> {
        text.lines().map(str::to_string).collect()
    }

    fn square() -> Vec<String> {
        lines(
            "G90\nG21\nM4 S0\nG0 X0 Y0\nG1 X10 S500 F6000\nG1 Y10\nG1 X0\nG1 Y0\nG0 X20 Y0\nG1 X30\nG1 Y10\nM5\nG0 X0 Y0",
        )
    }

    #[test]
    fn locates_the_line_under_the_head_and_retreats_inside_the_stroke() {
        let l = square();
        let p = Parsed::parse(&l);
        // Todas las líneas hasta la 7 (`G1 Y0`) tienen `ok`; la cabeza va a medio lado derecho.
        let est = locate(&p, 7, Some([10.0, 6.0]), 64, 0).unwrap();
        assert!(!est.mismatch);
        assert_eq!(p.steps[est.step].line, 5);
        assert!((est.t - 0.6).abs() < 1e-9);
        let land = landing(
            &p,
            &est,
            &PlanOptions {
                back_lines: 0,
                back_mm: 2.0,
            },
            0,
        );
        assert_eq!(p.steps[land.step].line, 5);
        let at = land.at.unwrap();
        assert!(
            (at[0] - 10.0).abs() < 1e-9 && (at[1] - 4.0).abs() < 1e-9,
            "{at:?}"
        );
        // Con más líneas que mm, manda lo que queda más atrás, pero sin pasar del G0.
        let land = landing(
            &p,
            &est,
            &PlanOptions {
                back_lines: 10,
                back_mm: 0.1,
            },
            0,
        );
        assert_eq!(
            p.steps[land.step].line, 4,
            "empieza en el primer G1 tras el G0"
        );
        assert_eq!(land.at, None);
    }

    #[test]
    fn a_position_off_the_path_falls_back_to_the_window_start() {
        let l = square();
        let p = Parsed::parse(&l);
        let est = locate(&p, 7, Some([50.0, 50.0]), 64, 0).unwrap();
        assert!(est.mismatch);
        assert!(est.distance_mm > 10.0);
        let est = locate(&p, 7, None, 64, 0).unwrap();
        assert!(est.mismatch);
        assert!(
            locate(&p, 2, Some([0.0, 0.0]), 64, 0).is_none(),
            "nada confirmado aún"
        );
    }

    #[test]
    fn ties_pick_the_earliest_line() {
        // Dos pasadas por el mismo trazo: ante el empate se repite de más, no se salta.
        let l = lines("M4 S0\nG0 X0 Y0\nG1 X10 S500 F600\nG0 X0 Y0\nG1 X10");
        let p = Parsed::parse(&l);
        let est = locate(&p, 4, Some([5.0, 0.0]), 64, 0).unwrap();
        assert_eq!(p.steps[est.step].line, 2);
    }

    #[test]
    fn image_rows_restart_at_the_start_of_the_row() {
        let l = lines(
            "M4 S0\nG0 X0 Y1\nG1 X1 S0 F9000\nG1 X2 S300\nG1 X3 S0\nG1 X4 S800\nG1 X5 S0\nG0 Y1.1\nG1 X4 S500\nG1 X3 S0\nG1 X2 S100\nG1 X1 S0\nG1 X0 S200",
        );
        let p = Parsed::parse(&l);
        let est = locate(&p, 10, Some([2.5, 1.1]), 64, 0).unwrap();
        let land = landing(&p, &est, &PlanOptions::default(), 0);
        assert!(land.row);
        assert_eq!(
            p.steps[land.step].line, 8,
            "el primer G1 de la segunda fila"
        );
    }

    #[test]
    fn the_floor_keeps_a_second_resume_from_going_back_into_finished_work() {
        let l = square();
        let p = Parsed::parse(&l);
        let floor = p.step_at_or_after(5);
        let est = locate(&p, 6, Some([8.0, 10.0]), 64, floor).unwrap();
        let land = landing(
            &p,
            &est,
            &PlanOptions {
                back_lines: 10,
                back_mm: 100.0,
            },
            floor,
        );
        assert!(land.step >= floor);
    }

    #[test]
    fn program_restores_modal_state_and_never_burns_while_travelling() {
        let l = square();
        let d = GcodeDialect::default();
        let p = build_program(&l, 6, None, 0.0, &d, false).unwrap();
        // M5 primero, el G0 antes que M4 y la S/F recuperadas.
        let i = |s: &str| p.lines.iter().position(|x| x == s).unwrap();
        assert_eq!(p.lines[1], "M5");
        assert!(i("G0 X10 Y10") < i("M4 S500"));
        assert!(i("F6000") < i("M4 S500"));
        assert_eq!(&p.lines[p.preamble_len..], &l[6..]);
        verify_preamble(&p.lines[..p.preamble_len]).unwrap();
        // A mitad de línea: el desplazamiento va al punto dado.
        let p = build_program(&l, 5, Some([10.0, 4.0]), 0.0, &d, false).unwrap();
        assert!(p.lines.contains(&"G0 X10 Y4".to_string()));
        // Una `G1` en el preámbulo se rechaza.
        assert!(verify_preamble(&lines("M4 S100\nG1 X5")).is_err());
        assert!(verify_preamble(&lines("M5\nG0 X5 Y5\nM4 S100")).is_ok());
    }

    #[test]
    fn program_keeps_marlin_inline_flag_and_air_and_z() {
        let l = lines("M8\nM3 S0 I\nG0 X1 Y1\nG91\nG0 Z-1.5\nG90\nG1 X5 S200 F900\nG1 X6\nM5");
        let d = GcodeDialect {
            constant_on: "M3 S0 I".into(),
            ..GcodeDialect::default()
        };
        let p = build_program(&l, 7, None, 0.0, &d, true).unwrap();
        assert!(p.lines.contains(&"M8".to_string()));
        assert!(p.lines.contains(&"M3 S200 I".to_string()));
        assert!(p.lines.contains(&"G0 X5 Y1".to_string()));
        let p = build_program(&l, 7, None, -0.5, &d, true).unwrap();
        let g91 = p.lines.iter().position(|x| x == "G91").unwrap();
        assert_eq!(p.lines[g91 + 1], "G0 Z-0.5");
        assert_eq!(p.lines[g91 + 2], "G90");
        // Sin eje Z no se mueve.
        let p = build_program(&l, 7, None, -0.5, &d, false).unwrap();
        assert!(!p.lines.iter().any(|x| x.starts_with("G0 Z")));
    }

    #[test]
    fn unsupported_dialects_and_bad_lines_are_refused() {
        let l = square();
        let d = GcodeDialect {
            s_inline: false,
            ..GcodeDialect::default()
        };
        assert!(build_program(&l, 1, None, 0.0, &d, false).is_err());
        assert!(build_program(&l, 99, None, 0.0, &GcodeDialect::default(), false).is_err());
    }

    #[test]
    fn frame_covers_the_burned_area() {
        let p = Parsed::parse(&square());
        let f = frame_lines(&p, 3000.0).unwrap();
        assert!(f.iter().any(|x| x.starts_with("G0 X0 Y0")));
        assert!(f.iter().any(|x| x.starts_with("G1 X30 Y0")), "{f:?}");
        assert!(frame_lines(&p, 0.0).is_err());
    }

    #[test]
    fn z_delta_is_the_difference_between_the_two_points() {
        let l = lines("G0 X1 Y1\nG91\nG0 Z-1.5\nG90\nG1 X2 S100\nG1 X3\nG91\nG0 Z1.5\nG90");
        // Se interrumpió tras `G1 X3` (línea 5); reiniciar antes de la 4 es la misma altura.
        assert!(z_delta(&l, 5, 4).abs() < 1e-9);
        // Reiniciar antes de bajar el foco (línea 1) pide subir 1,5 mm.
        assert!((z_delta(&l, 5, 1) - 1.5).abs() < 1e-9);
    }
}
