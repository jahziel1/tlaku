//! Banco de calidad y velocidad del nesting.
//!
//! ```text
//! cargo run --release -p tlaku-nest --example nest_bench -- [opciones]
//!   --dir CARPETA    instancias públicas (JSON de ESICUP en el formato de jagua-rs/sparrow);
//!                    sin ella solo corren los casos propios
//!   --time S         segundos por instancia (por omisión 10)
//!   --seed N         semilla (por omisión 1)
//!   --only a,b       solo estas instancias (por nombre)
//!   --evals N        tope de evaluaciones (resultado reproducible, sin depender del reloj)
//!   --svg CARPETA    escribe un SVG del acomodo de cada instancia en tira
//! ```
//!
//! # Instancias públicas
//! Son las clásicas de la literatura de «strip packing» (ESICUP: albano, dagli, fu, jakobs1/2, mao,
//! marques, shapes0/1, shirts, swim, trousers). **No se incluyen en el repositorio**: su licencia
//! no es explícita. Se descargan de `data/input/*.json` del repositorio de `sparrow`
//! (<https://github.com/JeroenGar/sparrow>, MIT) a una carpeta cualquiera y se pasa con `--dir`.
//!
//! El problema es: una tira de alto fijo (`strip_height`) y largo libre; se minimiza el largo
//! usado. La **densidad** es `suma de áreas / (alto × largo usado)`. La columna `ref` es la densidad
//! publicada por sparrow en su propio repositorio (`data/records/final_best_*.svg`), con mucho
//! más tiempo de cómputo: es un techo de comparación, no una meta.
//!
//! # Casos propios
//! Piezas de los generadores (`tlaku-parts`: caja con dientes, engranajes), formas irregulares
//! pseudoaleatorias y un reparto en varias hojas.

use serde_json::Value;
use std::sync::atomic::AtomicBool;
use std::time::Duration;
use tlaku_core::{Point, Polyline};
use tlaku_nest::{
    Gravity, NestContainer, NestInput, NestOptions, NestPiece, PieceRotation, Rotations,
    SheetsInput, nest, nest_sheets,
};

/// Densidades publicadas por sparrow (por ciento).
const SPARROW: &[(&str, f64)] = &[
    ("albano", 89.821),
    ("dagli", 90.172),
    ("fu", 92.396),
    ("jakobs1", 89.247),
    ("jakobs2", 87.717),
    ("mao", 86.867),
    ("marques", 92.015),
    ("shapes0", 69.998),
    ("shapes1", 76.719),
    ("shirts", 90.920),
    ("swim", 79.826),
    ("trousers", 92.615),
];

struct Args {
    dir: Option<String>,
    time: f64,
    seed: u64,
    only: Vec<String>,
    evals: Option<u64>,
    svg: Option<String>,
}

fn args() -> Args {
    let mut a = Args {
        dir: None,
        time: 10.0,
        seed: 1,
        only: Vec::new(),
        evals: None,
        svg: None,
    };
    let mut it = std::env::args().skip(1);
    while let Some(k) = it.next() {
        let v = it.next().unwrap_or_default();
        match k.as_str() {
            "--dir" => a.dir = Some(v),
            "--time" => a.time = v.parse().expect("--time S"),
            "--seed" => a.seed = v.parse().expect("--seed N"),
            "--only" => a.only = v.split(',').map(str::to_string).collect(),
            "--evals" => a.evals = Some(v.parse().expect("--evals N")),
            "--svg" => a.svg = Some(v),
            _ => panic!("opción desconocida: {k}"),
        }
    }
    a
}

fn poly(pts: &[[f64; 2]]) -> Polyline {
    Polyline {
        points: pts.iter().map(|p| Point::new(p[0], p[1])).collect(),
        closed: true,
    }
}

fn area(p: &Polyline) -> f64 {
    let n = p.points.len();
    (0..n)
        .map(|i| {
            let (a, b) = (p.points[i], p.points[(i + 1) % n]);
            a.x * b.y - b.x * a.y
        })
        .sum::<f64>()
        / 2.0
}

/// Área neta de una región con la regla par-impar de los anidados de un nivel (exterior menos
/// huecos): basta con el valor absoluto del mayor menos los demás cuando hay anidamiento.
fn net_area(region: &[Polyline]) -> f64 {
    let mut areas: Vec<f64> = region.iter().map(|p| area(p).abs()).collect();
    areas.sort_by(|a, b| b.total_cmp(a));
    match areas.split_first() {
        Some((big, rest)) => big - rest.iter().sum::<f64>(),
        None => 0.0,
    }
}

struct Piece {
    region: Vec<Polyline>,
    copies: u32,
    rotation: PieceRotation,
}

fn load_instance(path: &str) -> (String, f64, Vec<Piece>) {
    let v: Value = serde_json::from_str(&std::fs::read_to_string(path).expect("leer JSON"))
        .expect("JSON válido");
    let name = v["name"].as_str().unwrap_or("?").to_string();
    let height = v["strip_height"].as_f64().expect("strip_height");
    let mut pieces = Vec::new();
    for it in v["items"].as_array().expect("items") {
        let mut pts: Vec<[f64; 2]> = it["shape"]["data"]
            .as_array()
            .expect("shape.data")
            .iter()
            .map(|p| [p[0].as_f64().unwrap(), p[1].as_f64().unwrap()])
            .collect();
        if pts.len() > 1 && pts.first() == pts.last() {
            pts.pop();
        }
        let angles: Vec<f64> = it["orientation"]["rotation"]["angles"]
            .as_array()
            .map(|a| a.iter().filter_map(Value::as_f64).collect())
            .unwrap_or_else(|| vec![0.0]);
        pieces.push(Piece {
            region: vec![poly(&pts)],
            copies: it["demand"].as_u64().unwrap_or(1) as u32,
            rotation: if angles == [0.0] {
                PieceRotation::Fixed
            } else {
                PieceRotation::Only(angles)
            },
        });
    }
    (name, height, pieces)
}

fn to_nest(pieces: &[Piece]) -> Vec<NestPiece> {
    pieces
        .iter()
        .map(|p| NestPiece {
            region: p.region.clone(),
            copies: p.copies,
            rotation: p.rotation.clone(),
        })
        .collect()
}

fn options(a: &Args, rotations: Rotations, spacing: f64) -> NestOptions {
    NestOptions {
        spacing,
        margin: 0.0,
        rotations,
        allow_mirror: false,
        use_holes: false,
        gravity: Gravity::Gravity,
        time_limit: Duration::from_secs_f64(a.time),
        seed: a.seed,
        max_evaluations: a.evals,
    }
}

/// Largo usado de la tira (máximo X de las piezas colocadas).
fn used_length(pieces: &[Piece], placements: &[tlaku_nest::Placement]) -> f64 {
    let mut max_x = 0.0f64;
    for pl in placements {
        let t = pl.transform;
        for p in &pieces[pl.piece].region {
            for q in &p.points {
                max_x = max_x.max(t[0] * q.x + t[2] * q.y + t[4]);
            }
        }
    }
    max_x
}

fn strip_case(
    a: &Args,
    name: &str,
    height: f64,
    pieces: &[Piece],
    opts: NestOptions,
    ref_pct: Option<f64>,
) {
    let total: f64 = pieces
        .iter()
        .map(|p| net_area(&p.region) * p.copies as f64)
        .sum();
    let n: u32 = pieces.iter().map(|p| p.copies).sum();
    // Largo máximo generoso: 3 veces el que daría una densidad del 100 %.
    let max_len = (total / height * 3.0).max(height);
    let input = NestInput {
        pieces: to_nest(pieces),
        container: NestContainer::Rect {
            width: max_len,
            height,
        },
        options: opts,
    };
    let out = nest(&input, &AtomicBool::new(false), &mut |_| {}).expect("instancia válida");
    let len = used_length(pieces, &out.placements);
    let density = total / (height * len) * 100.0;
    if let Some(dir) = &a.svg {
        write_svg(dir, name, height, len, pieces, &out.placements);
    }
    println!(
        "{name:<22} {n:>4} piezas  densidad {density:>6.2} %  ref {:>7}  largo {len:>10.1}  sin colocar {:>2}  evals {:>6}  {:>6.1} s",
        ref_pct.map_or("-".to_string(), |r| format!("{r:.2} %")),
        out.unplaced.len(),
        out.evaluations,
        out.elapsed.as_secs_f64(),
    );
}

fn write_svg(
    dir: &str,
    name: &str,
    height: f64,
    len: f64,
    pieces: &[Piece],
    placements: &[tlaku_nest::Placement],
) {
    use std::fmt::Write;
    let mut s = String::new();
    let _ = writeln!(
        s,
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 {} {len} {height}" width="1200">"#,
        0.0
    );
    let _ = writeln!(
        s,
        r##"<rect x="0" y="0" width="{len}" height="{height}" fill="#fff" stroke="#000" stroke-width="{}"/>"##,
        len / 1000.0
    );
    for pl in placements {
        let t = pl.transform;
        for p in &pieces[pl.piece].region {
            let pts: Vec<String> = p
                .points
                .iter()
                .map(|q| {
                    format!(
                        "{:.3},{:.3}",
                        t[0] * q.x + t[2] * q.y + t[4],
                        height - (t[1] * q.x + t[3] * q.y + t[5])
                    )
                })
                .collect();
            let _ = writeln!(
                s,
                r##"<polygon points="{}" fill="#9cf" fill-opacity="0.6" stroke="#036" stroke-width="{}"/>"##,
                pts.join(" "),
                len / 1500.0
            );
        }
    }
    s.push_str("</svg>\n");
    let _ = std::fs::create_dir_all(dir);
    let _ = std::fs::write(format!("{dir}/{name}.svg"), s);
}

// ---- Casos propios ----

fn parts_of(id: &str, params: Value) -> Vec<Vec<Polyline>> {
    let r = tlaku_parts::generate(id, &params, None, 0.0).expect("generador");
    assert!(r.error.is_none(), "{id}: {:?}", r.error);
    r.parts
        .iter()
        .map(|p| {
            p.cuts
                .iter()
                .filter(|c| c.closed)
                .map(|c| poly(&c.points))
                .collect::<Vec<_>>()
        })
        .filter(|v: &Vec<Polyline>| !v.is_empty())
        .collect()
}

/// Generador congruencial lineal pequeño (reproducible en cualquier máquina).
struct Lcg(u64);
impl Lcg {
    fn next(&mut self) -> f64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((self.0 >> 33) as f64) / (1u64 << 31) as f64
    }
}

fn blob(rng: &mut Lcg, r_min: f64, r_max: f64) -> Polyline {
    let n = 7 + (rng.next() * 8.0) as usize;
    let base = r_min + rng.next() * (r_max - r_min);
    let pts: Vec<[f64; 2]> = (0..n)
        .map(|i| {
            let a = i as f64 * std::f64::consts::TAU / n as f64;
            let r = base * (0.55 + 0.45 * rng.next());
            [r * a.cos(), r * a.sin()]
        })
        .collect();
    poly(&pts)
}

fn plain(regions: Vec<Vec<Polyline>>, copies: u32) -> Vec<Piece> {
    regions
        .into_iter()
        .map(|region| Piece {
            region,
            copies,
            rotation: PieceRotation::Inherit,
        })
        .collect()
}

fn own_cases(a: &Args) {
    let want = |n: &str| a.only.is_empty() || a.only.iter().any(|o| o == n);

    if want("propio-caja") {
        let b = plain(
            parts_of(
                "box",
                serde_json::json!({"width_mm": 100, "depth_mm": 80, "height_mm": 60}),
            ),
            4,
        );
        strip_case(
            a,
            "propio-caja",
            300.0,
            &b,
            options(a, Rotations::Quarter, 2.0),
            None,
        );
    }
    if want("propio-engranajes") {
        let mut regs = Vec::new();
        for teeth in [12, 16, 20, 24, 30, 40] {
            regs.extend(parts_of(
                "gears",
                serde_json::json!({"kind": "single", "teeth": teeth, "bore_mm": 5}),
            ));
        }
        let g = plain(regs, 2);
        strip_case(
            a,
            "propio-engranajes",
            250.0,
            &g,
            options(a, Rotations::Quarter, 2.0),
            None,
        );
    }
    if want("propio-irregulares") {
        let mut rng = Lcg(7);
        let regs: Vec<Vec<Polyline>> = (0..30).map(|_| vec![blob(&mut rng, 12.0, 40.0)]).collect();
        let g = plain(regs, 1);
        strip_case(
            a,
            "propio-irregulares",
            300.0,
            &g,
            options(a, Rotations::Step(30.0), 1.0),
            None,
        );
    }
    if want("propio-rectangulos") {
        let mut rng = Lcg(11);
        let regs: Vec<Vec<Polyline>> = (0..40)
            .map(|_| {
                let (w, h) = (15.0 + rng.next() * 60.0, 10.0 + rng.next() * 40.0);
                vec![poly(&[[0.0, 0.0], [w, 0.0], [w, h], [0.0, h]])]
            })
            .collect();
        let g = plain(regs, 1);
        strip_case(
            a,
            "propio-rectangulos",
            300.0,
            &g,
            options(a, Rotations::Quarter, 1.0),
            None,
        );
    }
    if want("propio-hojas") {
        let mut pieces = plain(
            parts_of(
                "box",
                serde_json::json!({"width_mm": 120, "depth_mm": 90, "height_mm": 70}),
            ),
            6,
        );
        pieces.extend(plain(
            parts_of(
                "gears",
                serde_json::json!({"kind": "single", "teeth": 30, "bore_mm": 5}),
            ),
            6,
        ));
        let input = SheetsInput {
            pieces: to_nest(&pieces),
            sheets: Vec::new(),
            refill: Some(NestContainer::Rect {
                width: 300.0,
                height: 200.0,
            }),
            max_sheets: 20,
            options: options(a, Rotations::Quarter, 2.0),
        };
        let out = nest_sheets(&input, &AtomicBool::new(false), &mut |_| {}).expect("válida");
        let per: Vec<String> = out
            .sheets
            .iter()
            .map(|s| format!("{:.1}", s.utilization))
            .collect();
        println!(
            "{:<22} {:>4} piezas  hojas {:>2}  global {:>6.2} %  por hoja [{}]  sin colocar {}  {:>6.1} s",
            "propio-hojas",
            out.placed_count(),
            out.sheets.len(),
            out.utilization,
            per.join(", "),
            out.unplaced.len(),
            out.elapsed.as_secs_f64(),
        );
    }
}

fn main() {
    let a = args();
    println!(
        "# tlaku-nest bench: {} s por instancia, semilla {}",
        a.time, a.seed
    );
    if let Some(dir) = &a.dir {
        for (name, ref_pct) in SPARROW {
            if !a.only.is_empty() && !a.only.iter().any(|o| o == name) {
                continue;
            }
            let path = format!("{dir}/{name}.json");
            if !std::path::Path::new(&path).exists() {
                println!("{name:<22} (falta {path})");
                continue;
            }
            let (_, height, pieces) = load_instance(&path);
            strip_case(
                &a,
                name,
                height,
                &pieces,
                options(&a, Rotations::Fixed, 0.0),
                Some(*ref_pct),
            );
        }
    }
    own_cases(&a);
}
