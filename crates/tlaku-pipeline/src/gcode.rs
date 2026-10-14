use crate::job::{ComputedJob, LayerReport};
use crate::rotary;
use tlaku_core::toolpath::{PowerMode, Toolpath};
use tlaku_core::{Bounds, Point};
use tlaku_doc::{DeviceKind, DeviceProfile, Origin};
use tlaku_grbl::frame::frame_gcode_scaled;
use tlaku_grbl::gcode::{GcodeOptions, generate_mapped};
use tlaku_i18n::tr;

/// Pasa de coordenadas del documento (origen abajo a la izquierda) a las de la máquina,
/// cuyo X0 Y0 está en la esquina `origin` y crecen hacia dentro de la cama. Es su propia
/// inversa, así que también pasa de máquina a documento.
pub(crate) fn point_to_machine(p: Point, bed: [f64; 2], origin: Origin) -> Point {
    let (flip_x, flip_y) = match origin {
        Origin::BottomLeft => (false, false),
        Origin::TopLeft => (false, true),
        Origin::BottomRight => (true, false),
        Origin::TopRight => (true, true),
    };
    Point::new(
        if flip_x { bed[0] - p.x } else { p.x },
        if flip_y { bed[1] - p.y } else { p.y },
    )
}

/// El intercambio X/Y del perfil está en uso. Con rotativo se ignora: su Y es el eje del
/// accesorio y la escala de vueltas no tiene sentido sobre el eje cruzado (`dialect_warnings`
/// avisa).
pub fn swap_active(device: &DeviceProfile) -> bool {
    device.swap_xy && !device.rotary.enabled
}

/// Pasa un punto del documento a las coordenadas de la máquina: cambio de origen y, con
/// `swap_xy`, intercambio de X e Y (lo último, así `bed` sigue en orientación de documento).
pub fn doc_to_machine(p: Point, device: &DeviceProfile) -> Point {
    let q = point_to_machine(p, device.bed, device.origin);
    if swap_active(device) {
        Point::new(q.y, q.x)
    } else {
        q
    }
}

/// Inversa de [`doc_to_machine`]: de las coordenadas de la máquina a las del documento.
pub fn machine_to_doc(p: Point, device: &DeviceProfile) -> Point {
    let q = if swap_active(device) {
        Point::new(p.y, p.x)
    } else {
        p
    };
    point_to_machine(q, device.bed, device.origin)
}

fn script(text: &str) -> Option<String> {
    (!text.trim().is_empty()).then(|| text.to_string())
}

/// Suma `min + max` de la Y de todos los movimientos (con `map` aplicado): con ella se refleja la
/// Y dentro de la caja del trabajo (`suma - y`), de modo que la caja queda igual y el diseño sale
/// espejado en el eje del rotativo. `None` si no hay ninguna Y finita.
fn mirror_sum(tp: &Toolpath, map: impl Fn(Point) -> Point) -> Option<f64> {
    let (lo, hi) = tp
        .blocks
        .iter()
        .flat_map(|b| &b.moves)
        .map(|m| map(m.to()).y)
        .filter(|y| y.is_finite())
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), y| {
            (lo.min(y), hi.max(y))
        });
    (lo <= hi).then_some(lo + hi)
}

/// G-code del trabajo para esa máquina: dialecto del perfil, modo de potencia, S máxima,
/// origen de la cama, espejo y escala del rotativo, sentido de Z y guiones de inicio y fin.
pub fn gcode(job: &ComputedJob, device: &DeviceProfile) -> String {
    let toolpath = &job.toolpath;
    let mode = match device.power_mode {
        tlaku_doc::PowerMode::Dynamic => PowerMode::Dynamic,
        tlaku_doc::PowerMode::Constant => PowerMode::Constant,
    };
    // Las coordenadas se pasan a las de la máquina al escribir cada punto, sin copiar el
    // trabajo (con una imagen grande son cientos de MB).
    let to_machine = |p: Point| doc_to_machine(p, device);
    let mirror = if device.rotary.enabled && device.rotary.mirror {
        mirror_sum(toolpath, to_machine)
    } else {
        None
    };
    let map = |p: Point| {
        let p = to_machine(p);
        match mirror {
            Some(sum) => Point::new(p.x, sum - p.y),
            None => p,
        }
    };
    generate_mapped(
        toolpath,
        &GcodeOptions {
            dialect: device.dialect.clone(),
            // Si el rotativo es inválido `compute` ya falló; aquí se cae a Y sin escalar.
            y_scale: rotary::y_scale(device).unwrap_or(1.0),
            s_max: device.s_max,
            return_to_origin: device.return_to_origin,
            start_gcode: script(&device.start_gcode),
            end_gcode: script(&device.end_gcode),
            rapid_feed: Some(device.rapid_rate),
            z_invert: device.z_invert,
            merge_tolerance_mm: tlaku_grbl::gcode::DEFAULT_MERGE_TOLERANCE_MM,
            u_offset_mm: if device.u_axis { device.u_offset } else { 0.0 },
            // Sin segundo láser en el perfil todas las capas salen con el láser 1 (ver
            // `dialect_warnings`).
            block_laser2: if device.laser2 {
                job.layers.iter().map(|l| l.laser2).collect()
            } else {
                Vec::new()
            },
        },
        Some(mode),
        &map,
    )
}

/// ¿La controladora planifica `G0` y `G1` seguidos sin detenerse al cambiar entre ellos? Es lo que
/// hace GRBL (y sus derivados grblHAL, FluidNC y grbl-LPC) en modo láser, con `M4`: el rayo lo
/// gobierna la `S` de cada línea y no hay sincronización entre un salto y un corte, así que la
/// unión es una esquina más (desviación de unión). Con `M3` (sin `dynamic_on`) o en el resto de
/// firmwares (Marlin, Smoothie, G-code propio) no hay evidencia de que se encadenen, y el
/// estimado se detiene en cada cambio, que es lo conservador.
pub(crate) fn plans_laser_moves_together(device: &DeviceProfile) -> bool {
    matches!(
        device.kind,
        DeviceKind::Grbl | DeviceKind::GrblHal | DeviceKind::FluidNc | DeviceKind::GrblLpc
    ) && device.dialect.dynamic_on.is_some()
}

/// Carga que el trabajo pone al enlace serie de `baud` baudios: líneas por segundo en su tramo más
/// denso (ventana de 1 s, sobre el trabajo ya fusionado como lo escribe [`gcode`]) y largo medio
/// de línea del `gcode` generado. Pásala a `tlaku_grbl::preflight::preflight_link_with_load`.
pub fn link_load(
    job: &ComputedJob,
    device: &DeviceProfile,
    gcode: &str,
    baud: u32,
) -> tlaku_grbl::preflight::LinkLoad {
    // Mismos límites que el estimado de `compute`; con rotativo, en mm de superficie.
    let k = rotary::y_scale(device).unwrap_or(1.0);
    let profile = tlaku_sim::MachineProfile {
        max_rate_mm_min: [device.max_rate[0], device.max_rate[1] / k],
        accel_mm_s2: [device.accel[0], device.accel[1] / k],
        rapid_rate_mm_min: device.rapid_rate,
        stop_on_cut_change: !plans_laser_moves_together(device),
        ..tlaku_sim::MachineProfile::default()
    };
    let compact = tlaku_grbl::gcode::compact_toolpath(
        &job.toolpath,
        tlaku_grbl::gcode::DEFAULT_MERGE_TOLERANCE_MM,
    );
    let rate = tlaku_sim::line_rate(Point::new(0.0, 0.0), &compact, &profile, 1.0);
    tlaku_grbl::preflight::LinkLoad::new(baud, rate.peak_per_s, gcode.lines().count(), gcode.len())
}

/// Avisos que dependen de lo que el dialecto de la máquina no puede hacer.
pub(crate) fn dialect_warnings(
    toolpath: &Toolpath,
    layers: &[LayerReport],
    device: &DeviceProfile,
) -> Vec<String> {
    // El galvo no usa G-code: su dialecto no cuenta.
    if device.kind == tlaku_doc::DeviceKind::Galvo {
        return Vec::new();
    }
    let d = &device.dialect;
    let mut out = Vec::new();
    if device.swap_xy && device.rotary.enabled {
        out.push(tr!(
            "pipeline.gcode.swap_with_rotary",
            machine = device.name
        ));
    }
    if d.air_on.is_none() && toolpath.blocks.iter().any(|b| b.air_assist) {
        out.push(tr!("pipeline.gcode.no_air_command", machine = device.name));
    }
    if layers.iter().any(|l| l.laser2) {
        if !device.laser2 {
            out.push(tr!("pipeline.gcode.no_laser2", machine = device.name));
        } else if d.laser2_select.is_none() {
            out.push(tr!(
                "pipeline.gcode.no_laser2_select",
                machine = device.name
            ));
        }
    }
    if device.power_mode == tlaku_doc::PowerMode::Dynamic && d.dynamic_on.is_none() {
        out.push(if d.constant_on.is_empty() {
            tr!("pipeline.gcode.no_dynamic_power", machine = device.name)
        } else {
            tr!(
                "pipeline.gcode.no_dynamic_power_with",
                machine = device.name,
                command = d.constant_on
            )
        });
    }
    if layers.iter().any(|l| l.mode == "image") {
        if !d.s_inline {
            out.push(tr!("pipeline.gcode.s_not_inline", machine = device.name));
        } else if device.kind == tlaku_doc::DeviceKind::GrblM3 {
            out.push(tr!(
                "pipeline.gcode.grbl_no_laser_mode",
                machine = device.name
            ));
        }
    }
    if d.s_decimals == 0 && device.s_max < 100.0 {
        out.push(tr!(
            "pipeline.gcode.s_few_levels",
            machine = device.name,
            s_max = device.s_max,
            levels = device.s_max.max(0.0).round() as u64 + 1
        ));
    }
    out
}

/// G-code del encuadre del trabajo (rectángulo envolvente con overscan, láser apagado) en
/// coordenadas de la máquina: aplica el mismo cambio de origen, el mismo intercambio X/Y y la
/// misma escala de Y del rotativo que [`gcode`].
pub fn frame_gcode(job: &ComputedJob, device: &DeviceProfile, speed_mm_min: f64) -> Vec<String> {
    let Some(b) = job.bounds() else {
        return Vec::new();
    };
    frame_gcode_scaled(
        machine_bounds(b, device),
        speed_mm_min,
        rotary::y_scale(device).unwrap_or(1.0),
    )
}

/// Caja del documento en coordenadas de la máquina.
fn machine_bounds(b: Bounds, device: &DeviceProfile) -> Bounds {
    let a = doc_to_machine(b.min, device);
    let c = doc_to_machine(b.max, device);
    Bounds {
        min: Point::new(a.x.min(c.x), a.y.min(c.y)),
        max: Point::new(a.x.max(c.x), a.y.max(c.y)),
    }
}

/// Desfase del puntero rojo respecto del rayo, en coordenadas de la máquina: el puntero cae en
/// `rayo + pointer_shift`. El desfase del perfil está en el sistema del documento; aquí se pasa
/// por los flips del origen y el intercambio X/Y. Para que el puntero pase por un punto, el
/// rayo (la cabeza) va a ese punto menos este valor.
pub fn pointer_shift(device: &DeviceProfile) -> Point {
    let [ox, oy] = device.pointer_offset;
    if !(ox.is_finite() && oy.is_finite()) {
        return Point::new(0.0, 0.0);
    }
    let zero = doc_to_machine(Point::new(0.0, 0.0), device);
    let off = doc_to_machine(Point::new(ox, oy), device);
    Point::new(off.x - zero.x, off.y - zero.y)
}

/// Como [`frame_gcode`], pero para que sea el **puntero rojo** (no el rayo) el que recorra el
/// rectángulo: la cabeza va desfasada `pointer_offset`. Falla si ese recorrido se sale del área
/// de la máquina (los límites por software darían `ALARM:2`).
pub fn frame_gcode_pointer(
    job: &ComputedJob,
    device: &DeviceProfile,
    speed_mm_min: f64,
) -> Result<Vec<String>, String> {
    let Some(b) = job.bounds() else {
        return Ok(Vec::new());
    };
    let shift = pointer_shift(device);
    let m = machine_bounds(b, device);
    let moved = Bounds {
        min: Point::new(m.min.x - shift.x, m.min.y - shift.y),
        max: Point::new(m.max.x - shift.x, m.max.y - shift.y),
    };
    let (w, h) = if swap_active(device) {
        (device.bed[1], device.bed[0])
    } else {
        (device.bed[0], device.bed[1])
    };
    let k = rotary::y_scale(device).unwrap_or(1.0);
    let eps = 1e-6;
    if shift != Point::new(0.0, 0.0)
        && (moved.min.x < -eps
            || moved.min.y < -eps
            || moved.max.x > w + eps
            || moved.max.y > h / k + eps)
    {
        return Err(tlaku_i18n::coded(
            "bed_overflow",
            tr!(
                "pipeline.gcode.pointer_frame_outside",
                machine = device.name,
                ox = device.pointer_offset[0],
                oy = device.pointer_offset[1],
                x0 = format!("{:.1}", moved.min.x),
                x1 = format!("{:.1}", moved.max.x),
                y0 = format!("{:.1}", moved.min.y),
                y1 = format!("{:.1}", moved.max.y)
            ),
        ));
    }
    Ok(frame_gcode_scaled(moved, speed_mm_min, k))
}
