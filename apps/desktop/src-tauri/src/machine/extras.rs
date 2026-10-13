//! Control manual extra de la máquina: ejes Z y U (jog relativo y absoluto) y el disparo de
//! prueba del láser. Todo pasa por [`Handle::command`], así que hereda sus reglas: la línea se
//! normaliza y valida, y con un trabajo en curso la máquina lo rechaza ("ocupada").

use super::{Handle, num};
use tlaku_grbl::link::LinkFamily;

/// Duración mínima y máxima del disparo de prueba, en milisegundos.
pub const FIRE_MIN_MS: u32 = 10;
pub const FIRE_MAX_MS: u32 = 2000;
/// Potencia máxima del disparo de prueba, en % de la S máx. Es una prueba: no corta nada.
pub const FIRE_MAX_PCT: f64 = 20.0;
/// Letras de eje que aceptan los movimientos extra (Z y los ejes adicionales habituales).
const AXES: &str = "ZUABC";

/// `Ok` si la respuesta termina en `ok`; si no, el mensaje de rechazo de la controladora.
fn expect_ok(reply: &[String], what: &str) -> Result<(), String> {
    match reply.last().map(String::as_str) {
        Some(l) if l.starts_with("ok") => Ok(()),
        Some(e) => Err(format!("La máquina rechazó {what} ({e}).")),
        None => Err(format!("La máquina no respondió a {what}.")),
    }
}

fn check_axis(axis: char) -> Result<char, String> {
    let a = axis.to_ascii_uppercase();
    if AXES.contains(a) {
        Ok(a)
    } else {
        Err(format!("El eje '{axis}' no es válido; usa uno de {AXES}."))
    }
}

fn check_speed(speed: f64) -> Result<(), String> {
    if speed.is_finite() && speed > 0.0 {
        Ok(())
    } else {
        Err("La velocidad del movimiento debe ser mayor que 0.".into())
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

    fn run_axis(&self, lines: Vec<String>, what: &str) -> Result<(), String> {
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
            return Err("El desplazamiento no es un número.".into());
        }
        check_speed(speed)?;
        self.run_axis(
            Self::axis_lines(family, axis, delta, speed, true),
            &format!("el movimiento del eje {axis}"),
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
            return Err("La posición de destino no es un número.".into());
        }
        check_speed(speed)?;
        self.run_axis(
            Self::axis_lines(family, axis, pos, speed, false),
            &format!("el movimiento del eje {axis}"),
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
            return Err(format!(
                "La potencia del disparo de prueba debe estar entre 0 y {FIRE_MAX_PCT} %."
            ));
        }
        if !(s_max.is_finite() && s_max > 0.0) {
            return Err("La S máxima del perfil debe ser mayor que 0.".into());
        }
        if !(FIRE_MIN_MS..=FIRE_MAX_MS).contains(&duration_ms) {
            return Err(format!(
                "La duración del disparo debe estar entre {FIRE_MIN_MS} y {FIRE_MAX_MS} ms."
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
            .and_then(|r| expect_ok(&r, "encender el láser"))
            .and_then(|()| self.command(&dwell))
            .and_then(|r| expect_ok(&r, "la pausa del disparo"));
        // Siempre se apaga, también si lo anterior falló.
        let off = self
            .command("M5")
            .and_then(|r| expect_ok(&r, "apagar el láser"));
        match (fired, off) {
            (_, Err(e)) => Err(format!(
                "No se pudo confirmar que el láser se apagó: {e} Pulsa Reiniciar (o corta la \
                 energía) y revisa la máquina."
            )),
            (Err(e), Ok(())) => Err(e),
            (Ok(()), Ok(())) => Ok(()),
        }
    }
}
