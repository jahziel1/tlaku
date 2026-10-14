//! Importación de `.egv` (LHYMICRO-GL, las placas Lihuiyu M2 Nano de los K40): los movimientos con
//! el láser encendido pasan a trayectorias, una capa de Tlaku por velocidad.
//!
//! El archivo es una cabecera de texto (`Document type : LHYMICRO-GL file`, que termina en la
//! línea `%0%0%0%0%`) y el flujo de comandos. El flujo lo interpreta `tlaku_lhymicro::decode`.
//! Lo que el formato **no** guarda:
//!
//! - **La potencia.** Las M2 de serie no la controlan por software: las capas salen con la
//!   potencia por defecto y hay que ajustarla.
//! - **La placa.** El código de velocidad depende del modelo (A, B, B1, B2, M, M1, M2, M3); se
//!   supone una M2, la más común.
//! - **El origen.** Los ejes de la placa crecen alejándose del home; se supuso el home arriba a la
//!   izquierda (lo habitual) y se invirtió Y respecto a la cama del perfil por defecto.
//!
//! Las filas de un grabado en modo trama (`G`) no se importan como vectores: se avisa de cuántas
//! se omitieron.

use crate::import::{ImportExtras, ImportOptions, object};
use crate::import_rd::PALETTE;
use std::path::Path;
use tlaku_core::Point;
use tlaku_core::bezpath::{BezPath, PathSeg};
use tlaku_doc::{DeviceProfile, ImportResult, LayerSettings, ObjectKind, VectorObject};
use tlaku_i18n::tr;
use tlaku_lhymicro::decode::{Event, decode_stream};
use tlaku_lhymicro::encode::mil_to_mm;
use tlaku_lhymicro::speed::{Board, ParsedSpeed, RasterStep, equation, speed_from_value};

/// Tolerancia (mm) al quitar los escalones de Bresenham de los tramos oblicuos: algo más de un
/// mil (0,0254 mm), la resolución de la placa.
const SIMPLIFY_MM: f64 = 0.03;
/// Velocidades que difieren menos que esto (mm/s) son la misma capa.
const SAME_SPEED_MM_S: f64 = 0.05;

pub(crate) fn import(
    path: &Path,
    data: &[u8],
    _opts: &ImportOptions,
    _extras: &ImportExtras,
) -> Result<ImportResult, String> {
    let name = path.display();
    let stream = strip_header(data);
    let events = decode_stream(stream);
    if !events.iter().any(|e| matches!(e, Event::Move { .. })) {
        return Err(tr!("pipeline.import_egv.no_motion", name = name));
    }

    // Trazos con el láser encendido, agrupados por la velocidad vigente.
    struct Group {
        mm_s: f64,
        paths: Vec<Vec<(i64, i64)>>,
    }
    let mut groups: Vec<Group> = Vec::new();
    let mut speed: Option<ParsedSpeed> = None;
    let mut cur: Option<usize> = None;
    let mut last_end: Option<(i64, i64)> = None;
    let mut raster_moves = 0usize;
    let mut bad_speeds = 0usize;
    let mut no_speed = false;
    for e in &events {
        match e {
            Event::Speed(p) => speed = Some(p.clone()),
            Event::BadSpeed(_) => bad_speeds += 1,
            Event::Move {
                from, to, laser, ..
            } => {
                if !laser || from == to {
                    continue;
                }
                if speed.as_ref().is_some_and(|p| p.raster != RasterStep::None) {
                    raster_moves += 1;
                    continue;
                }
                let mm_s = match &speed {
                    Some(p) => {
                        let (b, m) = equation(Board::M2, p.accel, p.suffix_c, false);
                        speed_from_value(p.code_value as f64, b, m)
                    }
                    None => {
                        no_speed = true;
                        20.0
                    }
                };
                let g = match groups
                    .iter()
                    .position(|g| (g.mm_s - mm_s).abs() < SAME_SPEED_MM_S)
                {
                    Some(g) => g,
                    None => {
                        groups.push(Group {
                            mm_s,
                            paths: Vec::new(),
                        });
                        groups.len() - 1
                    }
                };
                // Un movimiento que sigue al anterior alarga el trazo; si no, empieza otro.
                if cur == Some(g) && last_end == Some(*from) {
                    groups[g].paths.last_mut().expect("trazo abierto").push(*to);
                } else {
                    groups[g].paths.push(vec![*from, *to]);
                }
                cur = Some(g);
                last_end = Some(*to);
            }
            _ => {}
        }
    }
    if groups.is_empty() {
        return Err(if raster_moves > 0 {
            tr!("pipeline.import_egv.no_cut_raster", name = name)
        } else {
            tr!("pipeline.import_egv.no_cut", name = name)
        });
    }

    let height_mm = {
        let max_y = groups
            .iter()
            .flat_map(|g| g.paths.iter().flatten())
            .map(|p| mil_to_mm(p.1))
            .fold(0.0f64, f64::max);
        DeviceProfile::default().bed[1].max(max_y)
    };
    let mut objects = Vec::new();
    let mut layers = Vec::new();
    for (i, g) in groups.iter().enumerate() {
        let color = PALETTE
            .get(i)
            .copied()
            .unwrap_or_else(|| tlaku_core::Rgb((i as u8).wrapping_mul(37), 0, 128));
        let mut s = LayerSettings::for_color(color);
        s.name = format!("K40 {}", i + 1);
        s.speed = g.mm_s * 60.0;
        let paths = g
            .paths
            .iter()
            .map(|p| to_path(p, height_mm))
            .collect::<Vec<_>>();
        objects.push(object(color, ObjectKind::Vector(VectorObject { paths })));
        layers.push(s);
    }

    let mut warnings = vec![
        tr!("pipeline.import_egv.no_power"),
        tr!("pipeline.import_egv.assumed_m2"),
    ];
    if raster_moves > 0 {
        warnings.push(tr!("pipeline.import_egv.raster_skipped", n = raster_moves));
    }
    if bad_speeds > 0 {
        warnings.push(tr!("pipeline.import_egv.bad_speeds", n = bad_speeds));
    }
    if no_speed {
        warnings.push(tr!("pipeline.import_egv.no_speed"));
    }
    Ok(ImportResult {
        objects,
        new_layers: layers,
        warnings,
    })
}

/// El flujo de comandos sin la cabecera de texto, si la trae.
fn strip_header(data: &[u8]) -> &[u8] {
    const MARK: &[u8] = b"%0%0%0%0%";
    if !data.starts_with(b"Document type") {
        return data;
    }
    match data.windows(MARK.len()).position(|w| w == MARK) {
        Some(i) => {
            let rest = &data[i + MARK.len()..];
            let skip = rest
                .iter()
                .take_while(|b| matches!(b, b'\r' | b'\n'))
                .count();
            &rest[skip..]
        }
        None => data,
    }
}

/// Trazo en mm de Tlaku a partir de puntos en mils de la placa, sin los escalones de los tramos
/// oblicuos. Cerrado si termina donde empezó.
fn to_path(points: &[(i64, i64)], height_mm: f64) -> BezPath {
    let pts: Vec<Point> = points
        .iter()
        .map(|&(x, y)| Point::new(mil_to_mm(x), height_mm - mil_to_mm(y)))
        .collect();
    let closed = pts.len() >= 4 && points[0] == points[points.len() - 1];
    let mut keep = vec![false; pts.len()];
    keep[0] = true;
    keep[pts.len() - 1] = true;
    simplify(&pts, 0, pts.len() - 1, &mut keep);
    // Un contorno cerrado no repite el punto inicial: lo cierra `Close`.
    let last = if closed { pts.len() - 1 } else { pts.len() };
    let mut segs = Vec::new();
    for (i, p) in pts.iter().enumerate().take(last).filter(|(i, _)| keep[*i]) {
        segs.push(if i == 0 {
            PathSeg::MoveTo { to: *p }
        } else {
            PathSeg::LineTo { to: *p }
        });
    }
    if closed {
        segs.push(PathSeg::Close);
    }
    BezPath(segs)
}

/// Douglas-Peucker entre `a` y `b` (índices), marcando los puntos que se conservan.
fn simplify(pts: &[Point], a: usize, b: usize, keep: &mut [bool]) {
    if b <= a + 1 {
        return;
    }
    let (p, q) = (pts[a], pts[b]);
    let (dx, dy) = (q.x - p.x, q.y - p.y);
    let len = dx.hypot(dy);
    let mut far = (0.0f64, a);
    for (i, r) in pts.iter().enumerate().take(b).skip(a + 1) {
        let d = if len < 1e-12 {
            r.dist(p)
        } else {
            ((r.x - p.x) * dy - (r.y - p.y) * dx).abs() / len
        };
        if d > far.0 {
            far = (d, i);
        }
    }
    if far.0 > SIMPLIFY_MM {
        keep[far.1] = true;
        simplify(pts, a, far.1, keep);
        simplify(pts, far.1, b, keep);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tlaku_core::toolpath::{Move, PowerMode, Toolpath, ToolpathBlock};
    use tlaku_lhymicro::encode::{EncodeConfig, encode};

    fn cut(x: f64, y: f64) -> Move {
        Move::Cut {
            to: Point::new(x, y),
            power_pct: 50.0,
        }
    }

    fn travel(x: f64, y: f64) -> Move {
        Move::Travel {
            to: Point::new(x, y),
        }
    }

    fn block(speed_mm_min: f64, moves: Vec<Move>) -> ToolpathBlock {
        ToolpathBlock {
            label: "capa".into(),
            speed_mm_min,
            air_assist: false,
            power_mode: PowerMode::Dynamic,
            moves,
            z_mm: None,
            galvo: None,
        }
    }

    fn run(bytes: &[u8]) -> Result<ImportResult, String> {
        import(
            Path::new("a.egv"),
            bytes,
            &ImportOptions::default(),
            &ImportExtras::default(),
        )
    }

    fn vertices(r: &ImportResult, i: usize) -> Vec<Vec<(f64, f64)>> {
        let ObjectKind::Vector(v) = &r.objects[i].kind else {
            panic!()
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

    fn cfg() -> EncodeConfig {
        EncodeConfig {
            bed_mm: [400.0, 415.0],
            ..EncodeConfig::default()
        }
    }

    /// Un `.egv` que escribe Tlaku (cuadrado cerrado a 30 mm/s y diagonal larga a 60 mm/s)
    /// vuelve con las mismas formas, una capa por velocidad.
    #[test]
    fn an_egv_written_by_tlaku_round_trips() {
        let square = vec![
            travel(20.0, 20.0),
            cut(60.0, 20.0),
            cut(60.0, 50.0),
            cut(20.0, 50.0),
            cut(20.0, 20.0),
        ];
        let diagonal = vec![travel(100.0, 100.0), cut(160.0, 130.0), cut(160.0, 190.0)];
        let tp = Toolpath {
            blocks: vec![block(1800.0, square), block(3600.0, diagonal)],
        };
        let program = encode(&tp, &cfg()).unwrap();
        let egv = program.to_egv_file("Tlaku prueba");
        let r = run(&egv).unwrap();
        assert_eq!(r.objects.len(), 2);
        assert!(
            (r.new_layers[0].speed - 1800.0).abs() < 60.0,
            "{}",
            r.new_layers[0].speed
        );
        assert!(
            (r.new_layers[1].speed - 3600.0).abs() < 120.0,
            "{}",
            r.new_layers[1].speed
        );
        let sq = vertices(&r, 0);
        assert_eq!(sq.len(), 1);
        let want = [(20.0, 20.0), (60.0, 20.0), (60.0, 50.0), (20.0, 50.0)];
        assert_eq!(sq[0].len(), 4, "{:?}", sq[0]);
        for ((x, y), (wx, wy)) in sq[0].iter().zip(want) {
            assert!(
                (x - wx).abs() < 0.05 && (y - wy).abs() < 0.05,
                "{x},{y} contra {wx},{wy}"
            );
        }
        let ObjectKind::Vector(v) = &r.objects[0].kind else {
            panic!()
        };
        assert!(matches!(v.paths[0].0.last(), Some(PathSeg::Close)));
        let dg = vertices(&r, 1);
        let want = [(100.0, 100.0), (160.0, 130.0), (160.0, 190.0)];
        assert_eq!(dg[0].len(), 3, "{:?}", dg[0]);
        for ((x, y), (wx, wy)) in dg[0].iter().zip(want) {
            assert!(
                (x - wx).abs() < 0.06 && (y - wy).abs() < 0.06,
                "{x},{y} contra {wx},{wy}"
            );
        }
        assert!(r.warnings.iter().any(|w| w.contains("potencia")));
    }

    #[test]
    fn the_stream_without_header_also_imports_and_garbage_does_not() {
        let tp = Toolpath {
            blocks: vec![block(1200.0, vec![travel(10.0, 10.0), cut(40.0, 10.0)])],
        };
        let program = encode(&tp, &cfg()).unwrap();
        let r = run(&program.stream).unwrap();
        assert_eq!(vertices(&r, 0)[0].len(), 2);
        assert!(run(b"Document type : LHYMICRO-GL file\n\n%0%0%0%0%\n").is_err());
        assert!(run(b"hola").is_err());
    }
}
