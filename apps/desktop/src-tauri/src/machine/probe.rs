//! Sonda de contacto (M48): los tipos y los límites de seguridad de un toque con `G38.3`. El
//! intercambio con la controladora está en `worker/probe.rs`.

use tlaku_i18n::tr;

/// Velocidad máxima de sondeo (mm/min). Una sonda que no frena a tiempo daña el cabezal.
pub const PROBE_MAX_FEED: f64 = 500.0;
/// Recorrido máximo de un toque (mm).
pub const PROBE_MAX_TRAVEL: f64 = 100.0;
/// Retroceso máximo tras el contacto (mm).
pub const PROBE_MAX_RETRACT: f64 = 20.0;

/// Resultado de un toque. Las posiciones son del cabezal al terminar el sondeo (donde tocó, o el
/// final del recorrido si no hubo contacto), antes de retroceder.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
pub struct ProbeTouch {
    /// El pin de la sonda cambió dentro del recorrido.
    pub hit: bool,
    /// Posición de máquina [x, y], mm.
    pub machine_pos: [f64; 2],
    /// Posición de trabajo [x, y], mm (la misma que usan `jog` y `jog_to`).
    pub work_pos: [f64; 2],
}

/// Codificación del resultado en el canal de respuestas (`Vec<String>`), que solo lleva texto.
pub(crate) fn encode(t: &ProbeTouch) -> Vec<String> {
    vec![
        u8::from(t.hit).to_string(),
        t.machine_pos[0].to_string(),
        t.machine_pos[1].to_string(),
        t.work_pos[0].to_string(),
        t.work_pos[1].to_string(),
    ]
}

pub(crate) fn decode(v: &[String]) -> Option<ProbeTouch> {
    let n = |i: usize| v.get(i)?.parse::<f64>().ok();
    Some(ProbeTouch {
        hit: v.first()? == "1",
        machine_pos: [n(1)?, n(2)?],
        work_pos: [n(3)?, n(4)?],
    })
}

/// Comprueba los límites de seguridad de un toque; devuelve el recorrido en mm.
pub(crate) fn check_args(dx: f64, dy: f64, feed: f64, retract: f64) -> Result<f64, String> {
    if !(dx.is_finite() && dy.is_finite()) {
        return Err(tr!("desktop.common.offset_not_number"));
    }
    let travel = dx.hypot(dy);
    if travel < 0.01 {
        return Err(tr!("desktop.worker.probe_no_travel"));
    }
    if travel > PROBE_MAX_TRAVEL {
        return Err(tr!("desktop.worker.probe_travel", max = PROBE_MAX_TRAVEL));
    }
    if !(feed.is_finite() && feed > 0.0 && feed <= PROBE_MAX_FEED) {
        return Err(tr!("desktop.worker.probe_feed", max = PROBE_MAX_FEED));
    }
    if !(retract.is_finite() && (0.0..=PROBE_MAX_RETRACT).contains(&retract)) {
        return Err(tr!("desktop.worker.probe_retract", max = PROBE_MAX_RETRACT));
    }
    Ok(travel)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_text_channel_roundtrips() {
        let t = ProbeTouch {
            hit: true,
            machine_pos: [1.5, -2.0],
            work_pos: [0.5, -3.0],
        };
        assert_eq!(decode(&encode(&t)), Some(t));
        assert_eq!(decode(&["1".into()]), None);
    }

    #[test]
    fn limits_reject_unsafe_probing() {
        assert!(check_args(10.0, 0.0, 100.0, 2.0).is_ok());
        assert!(check_args(f64::NAN, 0.0, 100.0, 2.0).is_err());
        assert!(check_args(0.0, 0.0, 100.0, 2.0).is_err());
        assert!(check_args(PROBE_MAX_TRAVEL + 1.0, 0.0, 100.0, 2.0).is_err());
        assert!(check_args(3.0, 4.0, PROBE_MAX_FEED + 1.0, 2.0).is_err());
        assert!(check_args(3.0, 4.0, 0.0, 2.0).is_err());
        assert!(check_args(3.0, 4.0, 100.0, -1.0).is_err());
        assert!(check_args(3.0, 4.0, 100.0, PROBE_MAX_RETRACT + 1.0).is_err());
    }
}
