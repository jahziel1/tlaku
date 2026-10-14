//! De `Toolpath` a LHYMICRO-GL.
//!
//! El codificador sigue el esquema de `lihuiyu/driver.py` de MeerK40t (MIT; ver THIRD_PARTY.md),
//! reescrito: un modo normal para los saltos y un modo compacto para cortar.
//!
//! ```text
//! salto (láser apagado)   I B<dist> R<dist> S1P \n
//! entrar a cortar         I CV<velocidad> N <direcciones> S1E
//! mover (en compacto)     [B|T|R|L|M] [D|U] <dist>      (el láser va con `D`/`U`)
//! salir de cortar         [U] F N S E - \n              (el `-` espera a que la placa termine)
//! ```
//!
//! # Lo que la placa no hace
//!
//! - **Solo se mueve en ocho direcciones** (recto o diagonal exacta). Un segmento cualquiera se
//!   descompone con Bresenham en tramos rectos y diagonales. Las posiciones se redondean al mil
//!   (0,0254 mm) en absoluto, así que el error no se acumula.
//! - **No controla la potencia** (las M2 de serie): una pasada con potencia mayor que 0 enciende
//!   el láser y con 0 lo apaga. La potencia de la capa se ignora con un [`Warning`]; se ajusta en
//!   el panel o con el potenciómetro de la máquina. Las imágenes con niveles de gris se
//!   binarizan (cualquier potencia mayor que 0 quema): conviene un tramado.
//! - **Sin pulsos en el sitio.** `Move::Dwell` (modo punto, corte pasante, pausas) necesitaría
//!   que el anfitrión temporice el láser encendido, y si falla la conexión el láser quedaría
//!   encendido. Se omite con aviso.
//! - **Trama.** Las filas de un grabado se codifican como movimientos vectoriales con el láser
//!   encendido y apagado. El modo `G` de la placa (que invierte el sentido y avanza solo) no se
//!   usa: no se puede probar sin máquina y el ahorro es de bytes, no de calidad.
//! - **Eje Z, asistencia de aire**: ignorados con aviso.

mod config;
mod encoder;
mod errors;
mod program;
#[cfg(test)]
mod tests;

use crate::speed::{self, MM_PER_MIL, SpeedError, SpeedParams};
pub use config::{EncodeConfig, HomeCorner, mil_to_mm, mm_to_mil};
use encoder::{Encoder, Mode};
pub use errors::{EncodeError, Warning};
pub use program::{Program, ProgramStats, RAPID_ESTIMATE_MM_S, distance, line_runs, rapid_text};
use tlaku_core::Point;
use tlaku_core::toolpath::{Move, Toolpath};

fn validate_config(cfg: &EncodeConfig) -> Result<(), EncodeError> {
    if !(cfg.bed_mm[0].is_finite() && cfg.bed_mm[1].is_finite())
        || cfg.bed_mm[0] <= 0.0
        || cfg.bed_mm[1] <= 0.0
    {
        return Err(EncodeError::BadConfig("la cama debe medir más de 0 mm"));
    }
    if !(cfg.max_speed_mm_s.is_finite() && cfg.max_speed_mm_s > 0.0) {
        return Err(EncodeError::BadConfig(
            "la velocidad máxima debe ser mayor que 0",
        ));
    }
    if cfg.min_jog_mil < 0 {
        return Err(EncodeError::BadConfig("min_jog_mil no puede ser negativo"));
    }
    Ok(())
}

/// Margen (mm) para no rechazar un diseño que toca el borde por el redondeo al mil.
const BED_EPS_MM: f64 = MM_PER_MIL;

/// Codifica un `Toolpath` (mm, origen abajo a la izquierda, Y arriba) en LHYMICRO-GL.
pub fn encode(toolpath: &Toolpath, cfg: &EncodeConfig) -> Result<Program, EncodeError> {
    validate_config(cfg)?;
    let bed = cfg.bed_mm;
    let to_mil = |p: Point| -> (i64, i64) {
        let m = cfg.home.to_machine_mm(p, bed);
        (mm_to_mil(m.0), mm_to_mil(m.1))
    };

    // 1) Validación de todo el trabajo antes de producir un solo byte.
    let mut codes: Vec<String> = Vec::with_capacity(toolpath.blocks.len());
    let mut warnings: Vec<Warning> = Vec::new();
    for (bi, block) in toolpath.blocks.iter().enumerate() {
        if !(block.speed_mm_min.is_finite() && block.speed_mm_min > 0.0) {
            return Err(EncodeError::NonFinite {
                block: bi,
                what: "la velocidad",
            });
        }
        let mm_s = block.speed_mm_min / 60.0;
        if mm_s > cfg.max_speed_mm_s {
            return Err(EncodeError::Speed {
                block: bi,
                mm_s,
                reason: format!("supera el máximo de {} mm/s", cfg.max_speed_mm_s),
            });
        }
        let mut params = SpeedParams::new(cfg.board, mm_s);
        params.fix_speeds = cfg.fix_speeds;
        params.d_ratio = cfg.d_ratio;
        if cfg.constant_speed {
            params.suffix_c = Some(true);
        }
        let code = speed::speed_code(&params).map_err(|e: SpeedError| EncodeError::Speed {
            block: bi,
            mm_s,
            reason: e.to_string(),
        })?;
        codes.push(code);

        let mut max_power = 0.0f64;
        let mut levels: Vec<u32> = Vec::new();
        for (mi, mv) in block.moves.iter().enumerate() {
            let to = mv.to();
            if !(to.x.is_finite() && to.y.is_finite()) {
                return Err(EncodeError::NonFinite {
                    block: bi,
                    what: "una coordenada",
                });
            }
            let m = cfg.home.to_machine_mm(to, bed);
            if m.0 < -BED_EPS_MM
                || m.1 < -BED_EPS_MM
                || m.0 > bed[0] + BED_EPS_MM
                || m.1 > bed[1] + BED_EPS_MM
            {
                return Err(EncodeError::OutOfBed {
                    block: bi,
                    index: mi,
                    x_mm: to.x,
                    y_mm: to.y,
                });
            }
            match *mv {
                Move::Cut { power_pct, .. } => {
                    if !power_pct.is_finite() || power_pct < 0.0 {
                        return Err(EncodeError::NonFinite {
                            block: bi,
                            what: "la potencia",
                        });
                    }
                    if power_pct > 0.0 {
                        max_power = max_power.max(power_pct);
                        let level = (power_pct * 100.0).round() as u32;
                        if !levels.contains(&level) {
                            levels.push(level);
                        }
                    }
                }
                Move::Dwell { ms, power_pct, .. } => {
                    warnings.push(Warning::DwellSkipped {
                        block: bi,
                        ms,
                        fires: power_pct > 0.0,
                    });
                }
                Move::Travel { .. } => {}
            }
        }
        if max_power > 0.0 {
            warnings.push(Warning::PowerIgnored {
                block: bi,
                pct: max_power,
            });
        }
        if levels.len() > 1 {
            warnings.push(Warning::GrayThresholded { block: bi });
        }
        if block.air_assist {
            warnings.push(Warning::AirAssistIgnored { block: bi });
        }
        if block.z_mm.is_some() {
            warnings.push(Warning::ZIgnored { block: bi });
        }
    }

    // 2) Codificación.
    let mut enc = Encoder::new(cfg);
    for (bi, block) in toolpath.blocks.iter().enumerate() {
        let code = &codes[bi];
        if enc.mode == Mode::Program && enc.code.as_deref() != Some(code) {
            enc.exit_program();
        }
        let mm_s = block.speed_mm_min / 60.0;
        let before = enc.stats.burn_mm + enc.stats.travel_mm;
        for mv in &block.moves {
            match *mv {
                Move::Travel { to } => enc.travel(to_mil(to)),
                Move::Cut { to, power_pct } => enc.cut(to_mil(to), power_pct > 0.0, code),
                Move::Dwell { .. } => {}
            }
        }
        enc.stats.est_seconds += (enc.stats.burn_mm + enc.stats.travel_mm - before) / mm_s;
    }
    enc.exit_program();
    enc.flush_pending();

    Ok(Program {
        stream: enc.out,
        start_mil: cfg.start_mil,
        end_mil: enc.pos,
        warnings,
        stats: enc.stats,
        board: cfg.board,
        bed_mm: bed,
        home: cfg.home,
    })
}
