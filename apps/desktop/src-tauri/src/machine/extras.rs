//! Control manual extra de la máquina: ejes Z y U (jog relativo y absoluto) y el disparo de
//! prueba del láser. Todo pasa por [`Handle::command`], así que hereda sus reglas: la línea se
//! normaliza y valida, y con un trabajo en curso la máquina lo rechaza ("ocupada").

use super::{Handle, num};
use tlaku_grbl::link::LinkFamily;
use tlaku_i18n::tr;

/// Duración mínima y máxima del disparo de prueba, en milisegundos.
pub const FIRE_MIN_MS: u32 = 10;
pub const FIRE_MAX_MS: u32 = 2000;
/// Potencia máxima del disparo de prueba, en % de la S máx. Es una prueba: no corta nada.
pub const FIRE_MAX_PCT: f64 = 20.0;
/// Letras de eje que aceptan los movimientos extra (Z y los ejes adicionales habituales).
const AXES: &str = "ZUABC";

/// Qué se le pidió a la máquina; decide el texto del error si no contesta `ok`.
#[derive(Clone, Copy)]
enum Step {
    AxisMove(char),
    LaserOn,
    FirePause,
    LaserOff,
}

/// `Ok` si la respuesta termina en `ok`; si no, el mensaje de rechazo de la controladora.
fn expect_ok(reply: &[String], what: Step) -> Result<(), String> {
    match reply.last().map(String::as_str) {
        Some(l) if l.starts_with("ok") => Ok(()),
        Some(e) => Err(match what {
            Step::AxisMove(axis) => {
                tr!("desktop.extras.axis_move_rejected", axis = axis, reply = e)
            }
            Step::LaserOn => tr!("desktop.extras.laser_on_rejected", reply = e),
            Step::FirePause => tr!("desktop.extras.fire_pause_rejected", reply = e),
            Step::LaserOff => tr!("desktop.extras.laser_off_rejected", reply = e),
        }),
        None => Err(match what {
            Step::AxisMove(axis) => tr!("desktop.extras.axis_move_silent", axis = axis),
            Step::LaserOn => tr!("desktop.extras.laser_on_silent"),
            Step::FirePause => tr!("desktop.extras.fire_pause_silent"),
            Step::LaserOff => tr!("desktop.extras.laser_off_silent"),
        }),
    }
}

fn check_axis(axis: char) -> Result<char, String> {
    let a = axis.to_ascii_uppercase();
    if AXES.contains(a) {
        Ok(a)
    } else {
        Err(tr!("desktop.extras.bad_axis", axis = axis, axes = AXES))
    }
}

fn check_speed(speed: f64) -> Result<(), String> {
    if speed.is_finite() && speed > 0.0 {
        Ok(())
    } else {
        Err(tr!("desktop.common.move_speed"))
    }
}

impl Handle {
    /// Líneas de un movimiento de un eje extra. En GRBL es un solo `$J=`; en Marlin y Smoothie
    /// `G91`/`G90` alrededor de un `G1`, y se vuelve siempre a coordenadas absolutas.
    fn axis_lines(
        family: LinkFamily,
        axis: char,
        value: f64,
        speed: f64,
        relative: bool,
    ) -> Vec<String> {
        let mode = if relative { "G91" } else { "G90" };
        if family == LinkFamily::Grbl {
            return vec![format!(
                "$J={mode} G21 {axis}{} F{}",
                num(value),
                num(speed)
            )];
        }
        let mut v = vec![
            mode.to_string(),
            format!("G1 {axis}{} F{}", num(value), num(speed)),
        ];
        if relative {
            v.push("G90".into());
        }
        v
    }

    fn run_axis(&self, lines: Vec<String>, what: Step) -> Result<(), String> {
        let mut failure = None;
        for line in &lines {
            let r = self.command(line);
            // Siempre se recorren todas las líneas: la última (`G90`) restaura el modo absoluto.
            if failure.is_none() {
                failure = match r {
                    Ok(reply) => expect_ok(&reply, what).err(),
                    Err(e) => Some(e),
                };
            }
        }
        failure.map_or(Ok(()), Err)
    }

    /// Mueve un eje extra (`Z`, `U`...) `delta` mm respecto a donde está, a `speed` mm/min.
    pub fn jog_axis(
        &self,
        family: LinkFamily,
        axis: char,
        delta: f64,
        speed: f64,
    ) -> Result<(), String> {
        let axis = check_axis(axis)?;
        if !delta.is_finite() {
            return Err(tr!("desktop.common.offset_not_number"));
        }
        check_speed(speed)?;
        self.run_axis(
            Self::axis_lines(family, axis, delta, speed, true),
            Step::AxisMove(axis),
        )
    }

    /// Lleva un eje extra a la posición `pos` (coordenadas de trabajo, mm) a `speed` mm/min.
    /// Es lo que usa "Enfocar" para ir a la altura de enfoque.
    pub fn jog_axis_to(
        &self,
        family: LinkFamily,
        axis: char,
        pos: f64,
        speed: f64,
    ) -> Result<(), String> {
        let axis = check_axis(axis)?;
        if !pos.is_finite() {
            return Err(tr!("desktop.common.target_not_number"));
        }
        check_speed(speed)?;
        self.run_axis(
            Self::axis_lines(family, axis, pos, speed, false),
            Step::AxisMove(axis),
        )
    }

    /// Disparo de prueba: enciende el láser a `power_pct` % de `s_max` durante `duration_ms` y lo
    /// apaga. La potencia se limita a [`FIRE_MAX_PCT`] y la duración a [`FIRE_MAX_MS`].
    ///
    /// Usa `M3` (potencia constante) y una pausa `G4`, porque `M4` en modo láser no dispara sin
    /// movimiento. **Siempre** manda `M5` al final, falle lo que falle (también si la pausa no
    /// llegó a cumplirse porque se canceló con un reinicio): lo último que debe pasar es que el
    /// láser quede apagado, y si ni eso se puede confirmar se avisa con claridad.
    pub fn fire(
        &self,
        family: LinkFamily,
        power_pct: f64,
        s_max: f64,
        duration_ms: u32,
    ) -> Result<(), String> {
        if !(power_pct.is_finite() && power_pct > 0.0 && power_pct <= FIRE_MAX_PCT) {
            return Err(tr!("desktop.extras.fire_power_range", max = FIRE_MAX_PCT));
        }
        if !(s_max.is_finite() && s_max > 0.0) {
            return Err(tr!("desktop.common.s_max_positive"));
        }
        if !(FIRE_MIN_MS..=FIRE_MAX_MS).contains(&duration_ms) {
            return Err(tr!(
                "desktop.extras.fire_duration_range",
                min = FIRE_MIN_MS,
                max = FIRE_MAX_MS
            ));
        }
        let s = s_max * power_pct / 100.0;
        // G4 P: segundos en GRBL y Smoothie, milisegundos en Marlin.
        let dwell = match family {
            LinkFamily::Marlin => format!("G4 P{duration_ms}"),
            LinkFamily::Grbl | LinkFamily::Smoothie => {
                format!("G4 P{}", num(f64::from(duration_ms) / 1000.0))
            }
        };
        let fired = self
            .command(&format!("M3 S{}", num(s)))
            .and_then(|r| expect_ok(&r, Step::LaserOn))
            .and_then(|()| self.command(&dwell))
            .and_then(|r| expect_ok(&r, Step::FirePause));
        // Siempre se apaga, también si lo anterior falló.
        let off = self
            .command("M5")
            .and_then(|r| expect_ok(&r, Step::LaserOff));
        match (fired, off) {
            (_, Err(e)) => Err(tr!("desktop.extras.laser_off_unconfirmed", err = e)),
            (Err(e), Ok(())) => Err(e),
            (Ok(()), Ok(())) => Ok(()),
        }
    }
}
