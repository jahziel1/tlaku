//! Importación de archivos `.rd` de Ruida (CO2): los movimientos y cortes del flujo de comandos
//! pasan a trayectorias, una capa de Tlaku por capa del `.rd`, con su velocidad, su potencia, su
//! color y el aire.
//!
//! El archivo en disco va ofuscado byte a byte (swizzle, `tlaku_ruida::swizzle`) con una clave
//! (magic) que depende de la tarjeta y que el archivo no declara: se prueban las claves conocidas
//! y se queda la primera que da un flujo de comandos válido hasta el final. Después el flujo se
//! recorre con el decodificador de `tlaku_ruida::cmd`:
//!
//! - `88/89/8A/8B` y `D9` (saltos) levantan el láser; `A8/A9/AA/AB` cortan. La posición se lleva
//!   en µm enteros.
//! - La capa en curso la fija `CA 02`; su velocidad (`C9 02`), su potencia (`C6 01/02`), el aire y
//!   el color (`CA 06`) vienen de la cabecera y del cuerpo de la capa.
//! - `C7` (potencia inmediata, que usan los rellenos y las imágenes) no se puede representar como
//!   una potencia por trazo: la capa se importa con la mayor potencia que usa, y los cortes con
//!   `C7 0` (el overscan) no se dibujan.
//!
//! **Coordenadas.** El archivo no dice dónde está el origen de la máquina; se supone el de la
//! mayoría de las Ruida y de RDWorks: arriba a la izquierda, con Y hacia abajo. Por eso se invierte
//! Y respecto a la altura de la cama del perfil por defecto (o la del dibujo, si es mayor): el
//! dibujo sale con el mismo aspecto que en RDWorks, y un `.rd` que Tlaku exportó para un perfil con
//! origen arriba a la izquierda y esa cama vuelve a su sitio exacto. Si la máquina tiene otro
//! origen o cama, el dibujo sale desplazado o reflejado: muévelo o refléjalo.

use crate::import::{ImportExtras, ImportOptions, object};
use std::collections::BTreeMap;
use std::path::Path;
use tlaku_core::bezpath::{BezPath, PathSeg};
use tlaku_core::{Point, Rgb};
use tlaku_doc::{DeviceProfile, ImportResult, LayerSettings, ObjectKind, VectorObject};
use tlaku_i18n::tr;
use tlaku_ruida::cmd::{Command, Limit, ProcessOp, RapidRef, decode_stream};
use tlaku_ruida::num;
use tlaku_ruida::swizzle::{Magic, probe_candidates};

/// Colores para las capas que el archivo no colorea (o repite).
pub(crate) const PALETTE: [Rgb; 10] = [
    Rgb(0, 0, 0),
    Rgb(255, 0, 0),
    Rgb(0, 0, 255),
    Rgb(0, 160, 0),
    Rgb(255, 128, 0),
    Rgb(160, 0, 160),
    Rgb(0, 160, 160),
    Rgb(128, 128, 0),
    Rgb(255, 0, 255),
    Rgb(128, 128, 128),
];

/// Velocidad (mm/min) de una capa que no declara ninguna.
const DEFAULT_SPEED_MM_MIN: f64 = 1000.0;
/// Distancia (µm) bajo la cual el último punto cierra el trazo.
const CLOSE_EPS_UM: i64 = 1;

pub(crate) fn import(
    path: &Path,
    data: &[u8],
    _opts: &ImportOptions,
    _extras: &ImportExtras,
) -> Result<ImportResult, String> {
    let name = path.display();
    let (cmds, magic) =
        decode(data).ok_or_else(|| tr!("pipeline.import_rd.not_rd", name = name))?;
    let mut warnings = Vec::new();
    if magic != Magic::default() {
        warnings.push(tr!(
            "pipeline.import_rd.unusual_key",
            key = format!("{:02X}", magic.0)
        ));
    }
    let unknown = cmds
        .iter()
        .filter(|c| matches!(c, Command::Unknown { .. }))
        .count();
    if unknown > 0 {
        warnings.push(tr!("pipeline.import_rd.unknown_commands", n = unknown));
    }
    let walk = walk(&cmds);
    let result = layers_result(walk, &mut warnings);
    if result.0.is_empty() {
        return Err(tr!("pipeline.import_rd.no_cuts", name = name));
    }
    Ok(ImportResult {
        objects: result.0,
        new_layers: result.1,
        warnings,
    })
}

/// Desofusca `data` con la clave que produce un flujo de comandos válido de principio a fin, con
/// al menos un movimiento y casi sin opcodes desconocidos (con una clave equivocada salen bytes
/// al azar, y casi todos sus opcodes lo son). Si varias valen, la de menos desconocidos. Un
/// archivo ya en claro también se acepta.
fn decode(data: &[u8]) -> Option<(Vec<Command>, Magic)> {
    let unknown = |cmds: &[Command]| {
        cmds.iter()
            .filter(|c| matches!(c, Command::Unknown { .. }))
            .count()
    };
    let mut best: Option<(Vec<Command>, Magic, usize)> = None;
    let plain = std::iter::once((data.to_vec(), Magic::default()));
    let swizzled = probe_candidates()
        .into_iter()
        .map(|m| (m.unswizzle_all(data), m));
    // Un flujo ya en claro (una prueba o `tlaku ruida` sin ofuscar) se prueba el último.
    for (bytes, magic) in swizzled.chain(plain) {
        let Ok(cmds) = decode_stream(&bytes) else {
            continue;
        };
        let u = unknown(&cmds);
        // A lo sumo 2 % de comandos desconocidos.
        if cmds.is_empty() || !cmds.iter().any(is_motion) || u * 50 > cmds.len() {
            continue;
        }
        if best.as_ref().is_none_or(|b| u < b.2) {
            best = Some((cmds, magic, u));
        }
    }
    best.map(|(c, m, _)| (c, m))
}

fn is_motion(c: &Command) -> bool {
    matches!(
        c,
        Command::MoveAbs { .. }
            | Command::MoveRel { .. }
            | Command::MoveRelX(_)
            | Command::MoveRelY(_)
            | Command::CutAbs { .. }
            | Command::CutRel { .. }
            | Command::CutRelX(_)
            | Command::CutRelY(_)
    )
}

/// Lo que se sabe de una capa del `.rd`.
#[derive(Default)]
struct RdLayer {
    speed_um_s: Option<u64>,
    power_min: Option<u16>,
    power_max: Option<u16>,
    color: Option<u64>,
    air: bool,
    /// Mayor potencia inmediata (`C7`) con la que se cortó y si hubo más de un valor.
    imm_max: u16,
    imm_levels: Vec<u16>,
    /// Trazos con el láser encendido, en µm del archivo.
    paths: Vec<Vec<(i64, i64)>>,
}

#[derive(Default)]
struct Walk {
    layers: BTreeMap<u8, RdLayer>,
    cuts: usize,
}

/// Recorre los comandos llevando la posición, la capa en curso y la potencia inmediata.
fn walk(cmds: &[Command]) -> Walk {
    let mut w = Walk::default();
    let mut cur: u8 = 0;
    let mut pos = (0i64, 0i64);
    let mut path: Vec<(i64, i64)> = Vec::new();
    let mut imm: Option<u16> = None;

    fn flush(w: &mut Walk, cur: u8, path: &mut Vec<(i64, i64)>) {
        if path.len() >= 2 {
            w.layers
                .entry(cur)
                .or_default()
                .paths
                .push(std::mem::take(path));
        } else {
            path.clear();
        }
    }
    let rel_i = |v: i32| i64::from(v);

    for c in cmds {
        match *c {
            Command::LayerSelect(l) => {
                flush(&mut w, cur, &mut path);
                cur = l;
                imm = None;
            }
            Command::EndLayer => flush(&mut w, cur, &mut path),
            Command::LayerSpeed { layer, um_s } => {
                let l = w.layers.entry(layer).or_default();
                l.speed_um_s.get_or_insert(um_s);
            }
            Command::Speed(um_s) => w.layers.entry(cur).or_default().speed_um_s = Some(um_s),
            Command::LayerPower {
                laser: 1,
                limit,
                layer,
                raw,
            } => {
                let l = w.layers.entry(layer).or_default();
                match limit {
                    Limit::Min => l.power_min.get_or_insert(raw),
                    Limit::Max => l.power_max.get_or_insert(raw),
                };
            }
            Command::LaserPower {
                laser: 1,
                limit,
                raw,
            } => {
                let l = w.layers.entry(cur).or_default();
                match limit {
                    Limit::Min => l.power_min = Some(raw),
                    Limit::Max => l.power_max = Some(raw),
                }
            }
            Command::LayerColor { layer, color } => {
                w.layers.entry(layer).or_default().color = Some(color);
            }
            Command::AirOn => w.layers.entry(cur).or_default().air = true,
            Command::AirOff => w.layers.entry(cur).or_default().air = false,
            Command::ImmPower(p) => imm = Some(p),
            Command::Process(ProcessOp::Stop) => flush(&mut w, cur, &mut path),
            // Saltos con el láser apagado.
            Command::MoveAbs { x, y } => {
                flush(&mut w, cur, &mut path);
                pos = (x, y);
            }
            Command::MoveRel { dx, dy } => {
                flush(&mut w, cur, &mut path);
                pos = (pos.0 + rel_i(dx), pos.1 + rel_i(dy));
            }
            Command::MoveRelX(dx) => {
                flush(&mut w, cur, &mut path);
                pos.0 += rel_i(dx);
            }
            Command::MoveRelY(dy) => {
                flush(&mut w, cur, &mut path);
                pos.1 += rel_i(dy);
            }
            Command::RapidX { rel, v } => {
                flush(&mut w, cur, &mut path);
                pos.0 = if rel == RapidRef::Machine {
                    v
                } else {
                    pos.0 + v
                };
            }
            Command::RapidY { rel, v } => {
                flush(&mut w, cur, &mut path);
                pos.1 = if rel == RapidRef::Machine {
                    v
                } else {
                    pos.1 + v
                };
            }
            Command::RapidXY { rel, x, y } => {
                flush(&mut w, cur, &mut path);
                pos = if rel == RapidRef::Machine {
                    (x, y)
                } else {
                    (pos.0 + x, pos.1 + y)
                };
            }
            // Cortes.
            Command::CutAbs { .. }
            | Command::CutRel { .. }
            | Command::CutRelX(_)
            | Command::CutRelY(_) => {
                let to = match *c {
                    Command::CutAbs { x, y } => (x, y),
                    Command::CutRel { dx, dy } => (pos.0 + rel_i(dx), pos.1 + rel_i(dy)),
                    Command::CutRelX(dx) => (pos.0 + rel_i(dx), pos.1),
                    Command::CutRelY(dy) => (pos.0, pos.1 + rel_i(dy)),
                    _ => unreachable!("solo los cortes llegan aquí"),
                };
                // Potencia inmediata 0 (el overscan de un relleno): el cabezal recorre el tramo
                // sin quemar, como un salto.
                if imm == Some(0) {
                    flush(&mut w, cur, &mut path);
                } else {
                    if let Some(p) = imm {
                        let l = w.layers.entry(cur).or_default();
                        l.imm_max = l.imm_max.max(p);
                        if !l.imm_levels.contains(&p) {
                            l.imm_levels.push(p);
                        }
                    }
                    if path.is_empty() {
                        path.push(pos);
                    }
                    if path.last() != Some(&to) {
                        path.push(to);
                    }
                    w.cuts += 1;
                }
                pos = to;
            }
            _ => {}
        }
    }
    flush(&mut w, cur, &mut path);
    w
}

/// Objetos y capas de Tlaku a partir del recorrido; los avisos se agregan a `warnings`.
fn layers_result(
    walk: Walk,
    warnings: &mut Vec<String>,
) -> (Vec<tlaku_doc::DesignObject>, Vec<LayerSettings>) {
    // Altura para invertir Y: la cama por defecto, o el dibujo si es más alto.
    let max_y_um = walk
        .layers
        .values()
        .flat_map(|l| l.paths.iter().flatten())
        .map(|p| p.1)
        .max()
        .unwrap_or(0);
    let bed_h = DeviceProfile::default().bed[1];
    let height_mm = bed_h.max(max_y_um as f64 / 1000.0);

    let mut objects = Vec::new();
    let mut layers = Vec::new();
    let mut used: Vec<Rgb> = Vec::new();
    let mut recolored = 0usize;
    let mut variable = Vec::new();
    let mut no_speed = Vec::new();
    for (index, l) in walk.layers {
        if l.paths.is_empty() {
            continue;
        }
        // El color del archivo si es único; si no, uno de la paleta.
        let wanted = l.color.map(|c| {
            let (r, g, b) = num::color_to_rgb(c);
            Rgb(r, g, b)
        });
        let color = match wanted {
            Some(c) if !used.contains(&c) => c,
            _ => {
                recolored += 1;
                PALETTE
                    .iter()
                    .copied()
                    .find(|c| !used.contains(c))
                    .unwrap_or_else(|| {
                        let n = used.len() as u8;
                        Rgb(n.wrapping_mul(37), n.wrapping_mul(91), n.wrapping_mul(53))
                    })
            }
        };
        used.push(color);

        let mut s = LayerSettings::for_color(color);
        s.name = format!("Ruida {}", index + 1);
        match l.speed_um_s {
            Some(v) if v > 0 => s.speed = num::speed_to_mm_min(v),
            _ => {
                s.speed = DEFAULT_SPEED_MM_MIN;
                no_speed.push(index + 1);
            }
        }
        let max_raw = if l.imm_max > 0 {
            l.imm_max
        } else {
            l.power_max.unwrap_or(0)
        };
        if l.imm_levels.len() > 1 {
            variable.push((index + 1, num::power_to_pct(max_raw)));
        }
        if max_raw > 0 {
            s.power = num::power_to_pct(max_raw).clamp(0.0, 100.0);
        }
        if l.imm_levels.len() <= 1 {
            let min = num::power_to_pct(l.power_min.unwrap_or(max_raw).min(max_raw));
            s.min_power = min.clamp(0.0, s.power);
        } else {
            s.min_power = 0.0;
        }
        s.air = l.air;

        let paths: Vec<BezPath> = l.paths.iter().map(|p| to_path(p, height_mm)).collect();
        objects.push(object(color, ObjectKind::Vector(VectorObject { paths })));
        layers.push(s);
    }
    if recolored > 0 {
        warnings.push(tr!("pipeline.import_rd.recolored", n = recolored));
    }
    for (n, pct) in variable {
        warnings.push(tr!(
            "pipeline.import_rd.variable_power",
            n = n,
            pct = format!("{pct:.0}")
        ));
    }
    if !no_speed.is_empty() {
        warnings.push(tr!(
            "pipeline.import_rd.no_speed",
            layers = format!("{no_speed:?}"),
            speed = format!("{DEFAULT_SPEED_MM_MIN:.0}")
        ));
    }
    warnings.push(tr!("pipeline.import_rd.assumed_origin"));
    (objects, layers)
}

/// Trazo en mm de Tlaku (Y hacia arriba) a partir de puntos en µm de la máquina. Si el último
/// punto coincide con el primero, el trazo se cierra.
fn to_path(points: &[(i64, i64)], height_mm: f64) -> BezPath {
    let conv = |p: (i64, i64)| Point::new(p.0 as f64 / 1000.0, height_mm - p.1 as f64 / 1000.0);
    let closed = points.len() >= 3
        && (points[0].0 - points[points.len() - 1].0).abs() <= CLOSE_EPS_UM
        && (points[0].1 - points[points.len() - 1].1).abs() <= CLOSE_EPS_UM;
    let pts = if closed {
        &points[..points.len() - 1]
    } else {
        points
    };
    let mut segs = Vec::with_capacity(pts.len() + 1);
    for (i, &p) in pts.iter().enumerate() {
        segs.push(if i == 0 {
            PathSeg::MoveTo { to: conv(p) }
        } else {
            PathSeg::LineTo { to: conv(p) }
        });
    }
    if closed {
        segs.push(PathSeg::Close);
    }
    BezPath(segs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tlaku_core::toolpath::{Move, PowerMode, Toolpath, ToolpathBlock};
    use tlaku_ruida::rd::{LayerOptions, RuidaJobOptions, ZeroPower, build_rd, to_swizzled};
    use tlaku_ruida::safety::SafetyLimits;

    fn cut(x: f64, y: f64, power_pct: f64) -> Move {
        Move::Cut {
            to: Point::new(x, y),
            power_pct,
        }
    }

    fn travel(x: f64, y: f64) -> Move {
        Move::Travel {
            to: Point::new(x, y),
        }
    }

    fn block(speed: f64, air: bool, moves: Vec<Move>) -> ToolpathBlock {
        ToolpathBlock {
            label: "capa".into(),
            speed_mm_min: speed,
            air_assist: air,
            power_mode: PowerMode::Dynamic,
            moves,
            z_mm: None,
            galvo: None,
        }
    }

    fn options(colors: &[[u8; 3]]) -> RuidaJobOptions {
        RuidaJobOptions {
            limits: SafetyLimits {
                bed_mm: [400.0, 415.0],
                ..SafetyLimits::default()
            },
            layers: colors
                .iter()
                .map(|&color| LayerOptions {
                    color,
                    min_power_pct: None,
                })
                .collect(),
            ..RuidaJobOptions::default()
        }
    }

    fn run(bytes: &[u8]) -> Result<ImportResult, String> {
        import(
            Path::new("a.rd"),
            bytes,
            &ImportOptions::default(),
            &ImportExtras::default(),
        )
    }

    fn pts(r: &ImportResult, i: usize) -> Vec<Vec<(f64, f64)>> {
        let ObjectKind::Vector(v) = &r.objects[i].kind else {
            panic!("no es un vector")
        };
        v.paths
            .iter()
            .map(|p| {
                p.0.iter()
                    .filter_map(|s| match s {
                        PathSeg::MoveTo { to } | PathSeg::LineTo { to } => Some((to.x, to.y)),
                        _ => None,
                    })
                    .collect()
            })
            .collect()
    }

    fn close(a: f64, b: f64) {
        assert!((a - b).abs() < 1e-3, "{a} != {b}");
    }

    /// Dos capas (un cuadrado cerrado y una polilínea abierta) con velocidades, potencias, colores
    /// y aire distintos: salen del `.rd` con los mismos trazos, ajustes y colores.
    #[test]
    fn a_tlaku_rd_round_trips_with_layers_speeds_and_powers() {
        let square = vec![
            travel(10.0, 20.0),
            cut(30.0, 20.0, 60.0),
            cut(30.0, 40.0, 60.0),
            cut(10.0, 40.0, 60.0),
            cut(10.0, 20.0, 60.0),
        ];
        let line = vec![
            travel(50.0, 5.0),
            cut(80.5, 5.25, 25.0),
            cut(90.0, 33.0, 25.0),
        ];
        let tp = Toolpath {
            blocks: vec![block(1200.0, false, square), block(3000.0, true, line)],
        };
        let rd = build_rd(&tp, &options(&[[255, 0, 0], [0, 0, 255]])).unwrap();
        let r = run(&to_swizzled(&rd, Magic::default())).unwrap();
        assert_eq!(r.objects.len(), 2);
        assert_eq!(r.new_layers.len(), 2);
        // Capa 1: cuadrado cerrado (Y respecto a la cama de 415 mm, hacia arriba).
        let s = &r.new_layers[0];
        assert_eq!(s.color, Rgb(255, 0, 0));
        close(s.speed, 1200.0);
        assert!((s.power - 60.0).abs() < 0.01, "{}", s.power);
        assert!(!s.air);
        let p = pts(&r, 0);
        assert_eq!(p.len(), 1);
        let want = [(10.0, 20.0), (30.0, 20.0), (30.0, 40.0), (10.0, 40.0)];
        for ((x, y), (wx, wy)) in p[0].iter().zip(want) {
            close(*x, wx);
            close(*y, wy);
        }
        let ObjectKind::Vector(v) = &r.objects[0].kind else {
            panic!()
        };
        assert!(matches!(v.paths[0].0.last(), Some(PathSeg::Close)));
        // Capa 2: abierta, con aire.
        let s = &r.new_layers[1];
        assert_eq!(s.color, Rgb(0, 0, 255));
        close(s.speed, 3000.0);
        assert!(s.air);
        assert!((s.power - 25.0).abs() < 0.01);
        let p = pts(&r, 1);
        for ((x, y), (wx, wy)) in p[0].iter().zip([(50.0, 5.0), (80.5, 5.25), (90.0, 33.0)]) {
            close(*x, wx);
            close(*y, wy);
        }
        assert_eq!(p[0].len(), 3);
    }

    /// El `.rd` en disco usa la clave de su tarjeta: se encuentra sola.
    #[test]
    fn other_magics_are_found_and_reported() {
        let tp = Toolpath {
            blocks: vec![block(
                600.0,
                false,
                vec![travel(5.0, 5.0), cut(20.0, 5.0, 40.0)],
            )],
        };
        let rd = build_rd(&tp, &options(&[[0, 0, 0]])).unwrap();
        let r = run(&to_swizzled(&rd, Magic(0x11))).unwrap();
        assert!(
            r.warnings.iter().any(|w| w.contains("0x11")),
            "{:?}",
            r.warnings
        );
        assert_eq!(pts(&r, 0)[0].len(), 2);
    }

    /// Un relleno con potencia variable (`C7`): se importa a la potencia máxima y sin el overscan
    /// a potencia 0.
    #[test]
    fn variable_power_layers_use_the_maximum_and_skip_the_overscan() {
        let moves = vec![
            travel(10.0, 10.0),
            cut(12.0, 10.0, 0.0),
            cut(20.0, 10.0, 30.0),
            cut(28.0, 10.0, 80.0),
            cut(30.0, 10.0, 0.0),
            travel(30.0, 11.0),
            cut(28.0, 11.0, 0.0),
            cut(20.0, 11.0, 80.0),
        ];
        let tp = Toolpath {
            blocks: vec![block(6000.0, false, moves)],
        };
        let mut o = options(&[[0, 128, 0]]);
        o.zero_power = ZeroPower::Cut;
        let rd = build_rd(&tp, &o).unwrap();
        let r = run(&to_swizzled(&rd, Magic::default())).unwrap();
        assert!((r.new_layers[0].power - 80.0).abs() < 0.01);
        assert!(r.warnings.iter().any(|w| w.contains("varía la potencia")));
        let p = pts(&r, 0);
        // Sin los tramos de overscan: 20 a 28 en la primera fila y 28 a 20 en la segunda.
        assert_eq!(p.len(), 2, "{p:?}");
        close(p[0][0].0, 12.0);
        close(p[0].last().unwrap().0, 28.0);
    }

    #[test]
    fn many_layers_with_repeated_colors_get_distinct_ones() {
        let mk = |x: f64| block(900.0, false, vec![travel(x, 5.0), cut(x + 5.0, 5.0, 50.0)]);
        let tp = Toolpath {
            blocks: vec![mk(5.0), mk(20.0), mk(40.0)],
        };
        let rd = build_rd(&tp, &options(&[[9, 9, 9], [9, 9, 9], [9, 9, 9]])).unwrap();
        let r = run(&to_swizzled(&rd, Magic::default())).unwrap();
        // Las capas vecinas idénticas no se fusionan (menos de 128), pero el color repetido sí
        // se corrige.
        assert_eq!(r.new_layers.len(), 3);
        let colors: Vec<Rgb> = r.new_layers.iter().map(|l| l.color).collect();
        assert_eq!(colors[0], Rgb(9, 9, 9));
        assert_ne!(colors[1], colors[0]);
        assert_ne!(colors[2], colors[1]);
        assert_ne!(colors[2], colors[0]);
        assert!(r.warnings.iter().any(|w| w.contains("2 capa(s)")));
    }

    #[test]
    fn garbage_and_empty_files_are_errors() {
        assert!(run(&[]).unwrap_err().contains("no parece un archivo .rd"));
        assert!(
            run(b"Esto no es un RD, es texto plano, nada mas.")
                .unwrap_err()
                .contains("no parece")
        );
    }

    /// Una pasada con corte y salto en relativo y por eje (`89`, `8A`, `8B`, `A9`, `AA`, `AB`).
    #[test]
    fn relative_and_axis_commands_follow_the_position() {
        use Command::*;
        let cmds = vec![
            Process(ProcessOp::RefPoint),
            LayerSelect(0),
            Speed(10_000),
            MoveAbs { x: 1000, y: 2000 },
            CutRelX(3000),
            CutRelY(1000),
            CutRel { dx: -3000, dy: 0 },
            MoveRel { dx: 500, dy: 500 },
            MoveRelX(1000),
            MoveRelY(1000),
            CutRel { dx: 2000, dy: 2000 },
            EndOfFile,
        ];
        let bytes = tlaku_ruida::cmd::encode_stream(&cmds).unwrap();
        let r = run(&to_swizzled(&bytes, Magic::default())).unwrap();
        let p = pts(&r, 0);
        assert_eq!(p.len(), 2);
        let y = |v: f64| 415.0 - v;
        let want0 = [(1.0, y(2.0)), (4.0, y(2.0)), (4.0, y(3.0)), (1.0, y(3.0))];
        for ((a, b), (c, d)) in p[0].iter().zip(want0) {
            close(*a, c);
            close(*b, d);
        }
        close(p[1][0].0, 1.0 + 0.5 + 1.0);
        close(p[1][0].1, y(3.0 + 0.5 + 1.0));
        close(p[1][1].0, 4.5);
        close(p[1][1].1, y(6.5));
        close(r.new_layers[0].speed, 600.0);
    }
}
