//! Trabajos para máquinas galvo (`DeviceKind::Galvo`): parámetros por bloque, límites del campo,
//! bamboleo y modelo de tiempo. Ver `docs/hardware/galvo.md`, sección 8.2.
//!
//! Convenciones del trabajo galvo:
//!
//! - **Coordenadas.** El `Toolpath` sigue en mm del documento (origen abajo a la izquierda de la
//!   cama, Y hacia arriba). El campo de la lente (`GalvoDevice::field_mm`, cuadrado) está
//!   centrado en el centro de la cama (`bed / 2`); el driver lo pasa al centro del campo.
//! - **Velocidad.** `LayerSettings::speed` y `ToolpathBlock::speed_mm_min` son mm/min, como en
//!   todas las máquinas; el galvo se piensa en mm/s, así que la interfaz debe mostrar mm/s y
//!   guardar `mm/s * 60`. En cambio la velocidad de salto (`jump_speed`) ya es mm/s. El driver
//!   convierte los mm/min del bloque a la unidad de la tarjeta.
//! - **Sin overscan, salvo que el perfil lo pida.** Los espejos tienen poca inercia: rellenos e
//!   imágenes salen con overscan 0 aunque la capa pida otro valor (se avisa), a menos que el
//!   perfil fije `GalvoDevice::overscan_mm`: entonces cada capa usa el suyo si es mayor que 0 y
//!   si no el del perfil, y el driver recorre esos tramos con el láser apagado a la velocidad de
//!   marcado.
//! - **Potencia constante.** El modo dinámico no existe en el galvo: los bloques salen con
//!   `PowerMode::Constant`.
//! - **Repeticiones.** `GalvoLayer::repeat` va en `GalvoParams::repeat` (la tarjeta repite el
//!   bloque) y no multiplica los movimientos; es independiente de `passes`, que sigue repitiendo
//!   movimientos (o dividiendo por Z).
//! - **Frecuencia y pulso** siempre salen de la capa; los retardos y saltos, de la capa solo
//!   con `override_timings` y si no del perfil.
//! - **Límites.** Se comprueban contra el campo, no contra la cama: `build` avisa y `compute`
//!   falla (salvo con marcado dividido, que parte el diseño en piezas del tamaño del campo).
//! - `ppi` no tiene efecto en el galvo y se avisa.

use crate::job::{ComputedJob, LayerReport};
use crate::layers::{ResolvedImage, ResolvedLayer};
use tlaku_core::toolpath::{GalvoParams, Move, PowerMode, ToolpathBlock};
use tlaku_core::{Bounds, Point};
use tlaku_doc::{
    DeviceKind, DeviceProfile, GalvoDevice, LaserSource, LayerMode, LayerSettings, Overscan, Wobble,
};
use tlaku_sim::{Estimate, MachineProfile};

/// Margen (mm) para no rechazar un diseño que toca el borde del campo por error de redondeo.
const FIELD_EPS_MM: f64 = 1e-6;
/// Muestras por vuelta del bamboleo.
const WOBBLE_SAMPLES_PER_TURN: f64 = 12.0;
/// Tope de puntos que genera el bamboleo en un bloque; con más se omite (aviso).
const MAX_WOBBLE_POINTS: f64 = 4_000_000.0;
/// Tope de `repeat` de una capa (protege contra errores de dedo).
pub(crate) const MAX_REPEAT: u32 = 100_000;

/// Rectángulo del campo en coordenadas del documento: cuadrado de `field_mm` centrado en la cama.
pub fn field_rect(device: &DeviceProfile) -> Bounds {
    let c = Point::new(device.bed[0] / 2.0, device.bed[1] / 2.0);
    let h = device.galvo.field_mm / 2.0;
    Bounds {
        min: Point::new(c.x - h, c.y - h),
        max: Point::new(c.x + h, c.y + h),
    }
}

/// Contexto del trabajo galvo: lo que depende del perfil y no de la capa.
pub(crate) struct JobCtx {
    galvo: GalvoDevice,
    projection: Option<crate::cylinder::Projection>,
}

impl JobCtx {
    /// Overscan de rellenos e imágenes del perfil, mm (0 = ninguno).
    pub fn overscan_mm(&self) -> f64 {
        self.galvo.overscan_mm
    }

    /// `None` si la máquina no es galvo. Valida el campo y la corrección de cilindro y de cono.
    pub fn new(device: &DeviceProfile) -> Result<Option<JobCtx>, String> {
        if device.kind != DeviceKind::Galvo {
            return Ok(None);
        }
        let g = &device.galvo;
        if !(g.field_mm.is_finite() && g.field_mm > 0.0) {
            return Err(format!(
                "la máquina '{}': el campo del galvo debe ser mayor que 0 mm (es {})",
                device.name, g.field_mm
            ));
        }
        let center = Point::new(device.bed[0] / 2.0, device.bed[1] / 2.0);
        let projection = crate::cylinder::Projection::from_device(g, center)
            .map_err(|e| format!("la máquina '{}': {e}", device.name))?;
        Ok(Some(JobCtx {
            galvo: g.clone(),
            projection,
        }))
    }
}

/// Lo que el bloque de una capa necesita para salir como bloque galvo.
pub(crate) struct LayerCtx {
    params: GalvoParams,
    /// Solo capas line con bamboleo.
    wobble: Option<Wobble>,
    projection: Option<crate::cylinder::Projection>,
}

fn finite_non_negative(layer: &LayerSettings, name: &str, v: f64) -> Result<(), String> {
    if v.is_finite() && v >= 0.0 {
        Ok(())
    } else {
        Err(format!("capa {}: {name} inválido: {v}", layer.color))
    }
}

/// Contexto de galvo de la capa `l`: `Some` solo en máquinas galvo. En una máquina de pórtico
/// avisa de los ajustes de galvo que se ignoran.
pub(crate) fn layer_ctx(
    job: Option<&JobCtx>,
    l: &LayerSettings,
    warnings: &mut Vec<String>,
) -> Result<Option<LayerCtx>, String> {
    let Some(job) = job else {
        if l.galvo.wobble.enabled && l.mode == LayerMode::Line {
            warnings.push(format!(
                "capa {}: el bamboleo solo funciona con máquinas galvo; se ignora",
                l.color
            ));
        }
        return Ok(None);
    };
    let (gl, dev) = (&l.galvo, &job.galvo);
    let name = l.color;
    if !(gl.frequency_khz.is_finite() && gl.frequency_khz > 0.0) {
        return Err(format!(
            "capa {name}: frecuencia inválida: {} kHz",
            gl.frequency_khz
        ));
    }
    if gl.repeat == 0 || gl.repeat > MAX_REPEAT {
        return Err(format!(
            "capa {name}: repeat debe estar entre 1 y {MAX_REPEAT} (es {})",
            gl.repeat
        ));
    }
    finite_non_negative(l, "pausa antes", gl.pause_before_ms)?;
    finite_non_negative(l, "pausa después", gl.pause_after_ms)?;
    if gl.override_timings {
        let t = &gl.timing;
        for (n, v) in [
            ("laser_on", t.laser_on_us),
            ("laser_off", t.laser_off_us),
            ("end", t.end_us),
            ("polygon", t.polygon_us),
            ("retardo mínimo de salto", gl.jump.delay_min_us),
            ("retardo máximo de salto", gl.jump.delay_max_us),
            ("distancia límite de salto", gl.jump.distance_limit),
        ] {
            if !v.is_finite() {
                return Err(format!("capa {name}: retardo '{n}' inválido: {v}"));
            }
        }
        if !(gl.jump.speed.is_finite() && gl.jump.speed > 0.0) {
            return Err(format!(
                "capa {name}: velocidad de salto inválida: {}",
                gl.jump.speed
            ));
        }
    }
    // Frecuencia y ancho de pulso respecto a lo que admite la fuente del perfil.
    if dev.freq_max_khz > 0.0
        && (gl.frequency_khz < dev.freq_min_khz || gl.frequency_khz > dev.freq_max_khz)
    {
        warnings.push(format!(
            "capa {name}: la frecuencia de {} kHz está fuera del rango de la fuente ({}-{} kHz)",
            gl.frequency_khz, dev.freq_min_khz, dev.freq_max_khz
        ));
    }
    let pulse = match (dev.source, gl.pulse_width_ns) {
        (LaserSource::FiberMopa | LaserSource::Uv, p) => {
            if let Some(w) = p
                && !dev.pulse_widths_ns.is_empty()
                && !dev.pulse_widths_ns.contains(&w)
            {
                warnings.push(format!(
                    "capa {name}: el ancho de pulso de {w} ns no está entre los de la fuente \
                     ({:?} ns)",
                    dev.pulse_widths_ns
                ));
            }
            p
        }
        (_, Some(w)) => {
            warnings.push(format!(
                "capa {name}: la fuente de la máquina no tiene ancho de pulso ajustable; se \
                 ignora el de {w} ns"
            ));
            None
        }
        (_, None) => None,
    };
    if l.power > dev.max_power_pct {
        warnings.push(format!(
            "capa {name}: la potencia de {} % supera el límite de la máquina ({} %)",
            l.power, dev.max_power_pct
        ));
    }
    if gl.ppi > 0.0 {
        warnings.push(format!(
            "capa {name}: ppi no tiene efecto en una máquina galvo; se ignora"
        ));
    }
    if l.mode != LayerMode::Line
        && dev.overscan_mm <= 0.0
        && l.overscan != Overscan::Auto
        && l.overscan != Overscan::Mm(0.0)
    {
        warnings.push(format!(
            "capa {name}: el galvo no necesita overscan; se ignora (el perfil puede fijar uno)"
        ));
    }
    let wobble = if gl.wobble.enabled {
        if l.mode != LayerMode::Line {
            warnings.push(format!(
                "capa {name}: el bamboleo solo se aplica a capas line; se ignora"
            ));
            None
        } else if !(gl.wobble.size.is_finite()
            && gl.wobble.size > 0.0
            && gl.wobble.step.is_finite()
            && gl.wobble.step > 0.0)
        {
            return Err(format!(
                "capa {name}: el bamboleo necesita tamaño y paso mayores que 0 (tamaño {}, paso {})",
                gl.wobble.size, gl.wobble.step
            ));
        } else {
            Some(gl.wobble.clone())
        }
    } else {
        None
    };
    let (timing, jump) = if gl.override_timings {
        (&gl.timing, &gl.jump)
    } else {
        (&dev.timing, &dev.jump)
    };
    Ok(Some(LayerCtx {
        params: GalvoParams {
            frequency_khz: gl.frequency_khz,
            pulse_width_ns: pulse,
            laser_on_us: timing.laser_on_us,
            laser_off_us: timing.laser_off_us,
            end_us: timing.end_us,
            polygon_us: timing.polygon_us,
            jump_speed: jump.speed,
            jump_delay_min_us: jump.delay_min_us,
            jump_delay_max_us: jump.delay_max_us,
            jump_distance_limit: jump.distance_limit,
            repeat: gl.repeat,
            pause_before_ms: gl.pause_before_ms,
            pause_after_ms: gl.pause_after_ms,
        },
        wobble,
        projection: job.projection,
    }))
}

impl LayerCtx {
    /// Deja el bloque listo para el driver galvo: bamboleo, corrección de cilindro y de cono,
    /// parámetros y potencia constante. Los avisos van a `warnings`.
    pub fn finish(&self, block: &mut ToolpathBlock, warnings: &mut Vec<String>) {
        if let Some(w) = &self.wobble {
            match wobble_moves(&block.moves, w) {
                Some(moves) => block.moves = moves,
                None => warnings.push(format!(
                    "{}: el bamboleo generaría demasiados puntos (más de {MAX_WOBBLE_POINTS}); \
                     se omite (aumenta el paso o reduce el trazo)",
                    block.label
                )),
            }
        }
        if let Some(p) = &self.projection {
            let clipped = p.project_moves(&mut block.moves);
            if clipped > 0 {
                warnings.push(format!(
                    "{}: {clipped} punto(s) del diseño quedan fuera de lo visible desde el espejo \
                     (más de 90 grados de arco en el cilindro); se recortan al borde",
                    block.label
                ));
            }
        }
        block.galvo = Some(self.params.clone());
        block.power_mode = PowerMode::Constant;
    }
}

/// Overscan que usa una capa de relleno o imagen en el galvo: el suyo si lo fijó en mm, y si no el
/// del perfil (0 = ninguno).
fn layer_overscan(own: Overscan, profile_mm: f64) -> f64 {
    match own {
        Overscan::Mm(v) if v > 0.0 => v,
        _ => profile_mm.max(0.0),
    }
}

/// Rellenos con el overscan del perfil (sin overscan si el perfil no pide ninguno).
pub(crate) fn overscan_layer(r: &mut ResolvedLayer, profile_mm: f64) {
    if let ResolvedLayer::Fill {
        settings, overscan, ..
    } = r
    {
        let mm = if profile_mm > 0.0 {
            layer_overscan(*overscan, profile_mm)
        } else {
            0.0
        };
        settings.overscan_mm = mm;
        *overscan = Overscan::Mm(mm);
    }
}

/// Imágenes con el overscan del perfil (sin overscan si el perfil no pide ninguno).
pub(crate) fn overscan_image(r: &mut ResolvedImage, profile_mm: f64) {
    let mm = if profile_mm > 0.0 {
        layer_overscan(r.overscan, profile_mm)
    } else {
        0.0
    };
    r.raster.overscan_mm = mm;
    r.overscan = Overscan::Mm(mm);
}

/// Perfil de tiempo del galvo para el simulador: sin aceleración apreciable de los ejes (los
/// espejos son muy rápidos) y los saltos a `jump.speed` mm/s. No modela los retardos de la
/// tarjeta; el tiempo de las repeticiones y pausas se suma con [`adjust_estimate`].
pub(crate) fn sim_profile(g: &GalvoDevice) -> MachineProfile {
    const HUGE: f64 = 1e9;
    let jump_mm_min = if g.jump.speed.is_finite() && g.jump.speed > 0.0 {
        g.jump.speed * 60.0
    } else {
        GalvoDevice::default().jump.speed * 60.0
    };
    MachineProfile {
        max_rate_mm_min: [HUGE, HUGE],
        accel_mm_s2: [HUGE, HUGE],
        rapid_rate_mm_min: jump_mm_min,
        junction_deviation_mm: 0.0,
        planner_blocks: None,
        stop_on_cut_change: true,
    }
}

/// Suma al tiempo estimado las repeticiones de cada bloque (`repeat`) y sus pausas. Devuelve el
/// factor con que reescalar los tiempos acumulados por movimiento para que el último valga el
/// total (1 si no cambió nada).
pub(crate) fn adjust_estimate(est: &mut Estimate, blocks: &[ToolpathBlock]) -> f64 {
    let before = est.total_s;
    for (e, b) in est.blocks.iter_mut().zip(blocks) {
        if let Some(g) = &b.galvo {
            e.seconds = e.seconds * f64::from(g.repeat.max(1))
                + (g.pause_before_ms + g.pause_after_ms) / 1000.0;
        }
    }
    est.total_s = est.blocks.iter().map(|e| e.seconds).sum();
    if before > 0.0 {
        est.total_s / before
    } else {
        1.0
    }
}

/// Avisos de lo que se sale del campo de la lente (ninguno con marcado dividido, que parte el
/// diseño en piezas).
pub(crate) fn field_warnings(layers: &[LayerReport], device: &DeviceProfile) -> Vec<String> {
    if device.galvo.split.enabled {
        return Vec::new();
    }
    let f = field_rect(device);
    let size = device.galvo.field_mm;
    layers
        .iter()
        .filter_map(|l| l.bounds.filter(|&b| outside(b, f)).map(|b| (l, b)))
        .map(|(l, b)| {
            format!(
                "la capa {} se sale del campo del galvo ({size:.0}x{size:.0} mm): ocupa X de \
                 {:.2} a {:.2} mm, Y de {:.2} a {:.2} mm",
                l.name, b.min.x, b.max.x, b.min.y, b.max.y
            )
        })
        .collect()
}

fn outside(b: Bounds, f: Bounds) -> bool {
    b.min.x < f.min.x - FIELD_EPS_MM
        || b.max.x > f.max.x + FIELD_EPS_MM
        || b.min.y < f.min.y - FIELD_EPS_MM
        || b.max.y > f.max.y + FIELD_EPS_MM
}

/// Como `check_device_bounds` para galvo: todo movimiento debe caer dentro del campo, salvo con
/// marcado dividido (`GalvoDevice::split`), que parte el diseño en piezas del tamaño del campo.
pub(crate) fn check_field(job: &ComputedJob, device: &DeviceProfile) -> Result<(), String> {
    if device.galvo.split.enabled {
        return Ok(());
    }
    let f = field_rect(device);
    let size = device.galvo.field_mm;
    let mut problems = Vec::new();
    for (layer, block) in job.layers.iter().zip(&job.toolpath.blocks) {
        if block
            .moves
            .iter()
            .map(Move::to)
            .any(|p| !(p.x.is_finite() && p.y.is_finite()))
        {
            problems.push(format!(
                "la capa {} tiene coordenadas que no son números",
                layer.name
            ));
            continue;
        }
        let Some(b) = layer.bounds else { continue };
        if outside(b, f) {
            problems.push(format!(
                "la capa {} ({}) se sale del campo del galvo de {size:.0}x{size:.0} mm: ocupa X de \
                 {:.2} a {:.2} mm, Y de {:.2} a {:.2} mm",
                layer.name, layer.mode, b.min.x, b.max.x, b.min.y, b.max.y
            ));
        }
    }
    if problems.is_empty() {
        Ok(())
    } else {
        problems.push(
            "reduce el diseño al campo (o activa el marcado dividido para partirlo en piezas)"
                .into(),
        );
        Err(problems.join("\n"))
    }
}

/// Camino del bamboleo (wobble) sobre cada trazo.
///
/// Cada trazo (una racha de `Cut` con potencia, que empieza en un `Travel`) se reemplaza por una
/// espiral: el haz describe un círculo de diámetro `size` cuyo centro avanza `step` mm por vuelta
/// a lo largo del trazo. Con `s` la distancia recorrida, `N` la normal izquierda y `T` la
/// tangente, el punto es `c(s) + (size/2)(cos(2π s/step) N + sin(2π s/step) T)`; así el ancho
/// cubierto es `size` y el avance por vuelta, `step`. Las muestras están en múltiplos de
/// `step / 12` de `s` (12 por vuelta), de modo que los extremos del círculo siempre se muestrean.
/// Cada trazo termina en su punto final exacto, para que las formas cerradas cierren. Los `Cut`
/// con potencia 0, los `Travel` y las pausas pasan tal cual. `None` si generaría demasiados
/// puntos.
pub(crate) fn wobble_moves(moves: &[Move], w: &Wobble) -> Option<Vec<Move>> {
    let r = w.size / 2.0;
    let ds = w.step / WOBBLE_SAMPLES_PER_TURN;
    // Largo total con potencia, para no generar millones de puntos por error.
    let mut total = 0.0;
    let mut head: Option<Point> = None;
    for m in moves {
        if let (Move::Cut { to, power_pct }, Some(a)) = (m, head)
            && *power_pct > 0.0
        {
            total += a.dist(*to);
        }
        head = Some(m.to());
    }
    if total / ds > MAX_WOBBLE_POINTS {
        return None;
    }
    let mut out = Vec::with_capacity(moves.len());
    let mut head: Option<Point> = None;
    // Distancia recorrida desde el inicio del trazo.
    let mut s = 0.0;
    for (i, m) in moves.iter().enumerate() {
        match *m {
            Move::Travel { to } => {
                s = 0.0;
                head = Some(to);
                out.push(*m);
            }
            Move::Dwell { .. } => out.push(*m),
            Move::Cut { to, power_pct } => {
                let a = head.unwrap_or(to);
                let len = a.dist(to);
                head = Some(to);
                if power_pct <= 0.0 || len < 1e-9 {
                    s += len;
                    out.push(*m);
                    continue;
                }
                let (tx, ty) = ((to.x - a.x) / len, (to.y - a.y) / len);
                let (nx, ny) = (-ty, tx);
                // Muestras en múltiplos de `ds` dentro de (s, s + len).
                let mut k = (s / ds).floor() + 1.0;
                while k * ds < s + len - 1e-9 {
                    let sk = k * ds;
                    let t = (sk - s) / len;
                    let phi = std::f64::consts::TAU * sk / w.step;
                    let (sn, cs) = phi.sin_cos();
                    out.push(Move::Cut {
                        to: Point::new(
                            a.x + (to.x - a.x) * t + r * (cs * nx + sn * tx),
                            a.y + (to.y - a.y) * t + r * (cs * ny + sn * ty),
                        ),
                        power_pct,
                    });
                    k += 1.0;
                }
                s += len;
                // El final exacto solo cuando el trazo termina aquí.
                let continues = matches!(
                    moves.get(i + 1),
                    Some(Move::Cut { power_pct: p, .. }) if *p > 0.0
                );
                if !continues {
                    out.push(Move::Cut { to, power_pct });
                }
            }
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(len: f64, power: f64) -> Vec<Move> {
        vec![
            Move::Travel {
                to: Point::new(0.0, 0.0),
            },
            Move::Cut {
                to: Point::new(len, 0.0),
                power_pct: power,
            },
        ]
    }

    fn wobble(size: f64, step: f64) -> Wobble {
        Wobble {
            enabled: true,
            size,
            step,
        }
    }

    #[test]
    fn covered_width_equals_size() {
        let w = wobble(0.4, 0.1);
        let out = wobble_moves(&line(10.0, 50.0), &w).unwrap();
        let ys: Vec<f64> = out.iter().map(|m| m.to().y).collect();
        let (lo, hi) = ys
            .iter()
            .fold((f64::MAX, f64::MIN), |(a, b), &y| (a.min(y), b.max(y)));
        assert!((hi - lo - 0.4).abs() < 1e-9, "ancho {}", hi - lo);
        assert!((hi - 0.2).abs() < 1e-9 && (lo + 0.2).abs() < 1e-9);
    }

    #[test]
    fn advance_per_turn_equals_step() {
        // En una línea a lo largo de X, el desplazamiento normal es (size/2) cos(2π x/step): los
        // máximos están separados exactamente `step` y hay `len / step` vueltas.
        let w = wobble(0.3, 0.05);
        let out = wobble_moves(&line(2.0, 50.0), &w).unwrap();
        let peaks: Vec<f64> = out
            .iter()
            .map(Move::to)
            .filter(|p| (p.y - 0.15).abs() < 1e-9)
            .map(|p| p.x)
            .collect();
        assert!(peaks.len() >= 39, "{} máximos", peaks.len());
        for pair in peaks.windows(2) {
            // El avance del centro es x - (size/2) sin(phi): en los máximos sin(phi)=0.
            assert!((pair[1] - pair[0] - 0.05).abs() < 1e-9, "{pair:?}");
        }
        // Vueltas completas = cruces ascendentes de y = 0.
        let ups = out
            .windows(2)
            .filter(|p| p[0].to().y < 0.0 && p[1].to().y >= 0.0)
            .count();
        assert!((ups as f64 - 2.0 / 0.05).abs() <= 1.0, "{ups} vueltas");
    }

    #[test]
    fn samples_are_bounded_per_turn() {
        let w = wobble(0.2, 0.05);
        let out = wobble_moves(&line(1.0, 50.0), &w).unwrap();
        // Entre muestras: la cuerda de 1/12 de vuelta del círculo (r = 0.1) más el avance del
        // centro (step / 12). Se omiten el salto inicial al círculo y el cierre exacto.
        let chord = 2.0 * 0.1 * (std::f64::consts::PI / 12.0).sin() + 0.05 / 12.0;
        for p in out[1..out.len() - 1].windows(2) {
            let d = p[0].to().dist(p[1].to());
            assert!(d < chord + 1e-9, "{d} > {chord}");
        }
    }

    #[test]
    fn stroke_ends_exactly_at_its_end_and_keeps_power() {
        let w = wobble(0.2, 0.05);
        let out = wobble_moves(&line(3.33, 42.0), &w).unwrap();
        assert_eq!(
            out[0],
            Move::Travel {
                to: Point::new(0.0, 0.0)
            }
        );
        assert_eq!(out.last().unwrap().to(), Point::new(3.33, 0.0));
        assert!(
            out[1..]
                .iter()
                .all(|m| matches!(m, Move::Cut { power_pct, .. } if *power_pct == 42.0))
        );
    }

    #[test]
    fn corner_is_followed_and_phase_continues() {
        let w = wobble(0.2, 0.05);
        let moves = vec![
            Move::Travel {
                to: Point::new(0.0, 0.0),
            },
            Move::Cut {
                to: Point::new(5.0, 0.0),
                power_pct: 10.0,
            },
            Move::Cut {
                to: Point::new(5.0, 5.0),
                power_pct: 10.0,
            },
        ];
        let out = wobble_moves(&moves, &w).unwrap();
        // En el segundo tramo (vertical) el desplazamiento está en X: se queda a <= r de x = 5.
        let second: Vec<Point> = out
            .iter()
            .map(Move::to)
            .filter(|p| p.y > 0.2 && p.y < 4.8)
            .collect();
        assert!(!second.is_empty());
        assert!(second.iter().all(|p| (p.x - 5.0).abs() <= 0.1 + 1e-9));
        assert_eq!(out.last().unwrap().to(), Point::new(5.0, 5.0));
    }

    #[test]
    fn zero_power_travel_and_dwell_pass_through() {
        let w = wobble(0.2, 0.05);
        let moves = vec![
            Move::Travel {
                to: Point::new(0.0, 0.0),
            },
            Move::Cut {
                to: Point::new(10.0, 0.0),
                power_pct: 0.0,
            },
            Move::Dwell {
                to: Point::new(10.0, 0.0),
                ms: 5.0,
                power_pct: 0.0,
            },
        ];
        assert_eq!(wobble_moves(&moves, &w).unwrap(), moves);
    }

    #[test]
    fn too_many_points_are_refused() {
        let w = wobble(0.2, 0.0001);
        assert!(wobble_moves(&line(1000.0, 50.0), &w).is_none());
    }

    #[test]
    fn field_is_centered_on_the_bed() {
        let device = DeviceProfile {
            bed: [200.0, 200.0],
            galvo: GalvoDevice {
                field_mm: 110.0,
                ..GalvoDevice::default()
            },
            ..DeviceProfile::default()
        };
        let f = field_rect(&device);
        assert_eq!(
            (f.min.x, f.max.x, f.min.y, f.max.y),
            (45.0, 155.0, 45.0, 155.0)
        );
    }

    #[test]
    fn sim_profile_has_no_acceleration_limit() {
        let mut g = GalvoDevice::default();
        g.jump.speed = 5000.0;
        let p = sim_profile(&g);
        assert_eq!(p.rapid_rate_mm_min, 300_000.0);
        assert!(p.accel_mm_s2[0] >= 1e6);
    }
}
