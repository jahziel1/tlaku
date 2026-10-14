//! Humo: la herramienta mide un SVG pequeño de punta a punta (incluido el emulador GRBL) y
//! clasifica un archivo roto como error, sin colgarse ni propagar el fallo.

use std::path::PathBuf;
use std::process::Command;

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_tlaku-bench")
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("tlaku-bench-test-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn csv_rows(csv: &str) -> Vec<std::collections::HashMap<String, String>> {
    let mut lines = csv.lines();
    let head: Vec<&str> = lines.next().unwrap().split(',').collect();
    lines
        .map(|l| {
            // Los campos con comas van entre comillas; estos archivos no las usan salvo `error`.
            let mut cols = Vec::new();
            let (mut cur, mut quoted) = (String::new(), false);
            for c in l.chars() {
                match c {
                    '"' => quoted = !quoted,
                    ',' if !quoted => cols.push(std::mem::take(&mut cur)),
                    _ => cur.push(c),
                }
            }
            cols.push(cur);
            head.iter()
                .map(|h| (*h).to_string())
                .zip(cols)
                .collect::<std::collections::HashMap<_, _>>()
        })
        .collect()
}

#[test]
fn measures_a_small_svg_end_to_end() {
    let dir = scratch("ok");
    std::fs::write(
        dir.join("cuadrado.svg"),
        "<svg xmlns='http://www.w3.org/2000/svg' width='40mm' height='40mm' viewBox='0 0 40 40'>\
         <rect x='5' y='5' width='20' height='20' fill='none' stroke='red'/></svg>",
    )
    .unwrap();
    std::fs::write(dir.join("roto.svg"), "esto no es un svg").unwrap();
    let csv = dir.join("out.csv");
    let out = Command::new(bin())
        .args(["run", dir.to_str().unwrap(), "--csv", csv.to_str().unwrap()])
        .output()
        .unwrap();
    // Código 2 = hubo problemas (el archivo roto); 0 sería "todo bien".
    assert_eq!(out.status.code(), Some(2), "{out:?}");
    let rows = csv_rows(&std::fs::read_to_string(&csv).unwrap());
    assert_eq!(rows.len(), 2);
    let good = rows
        .iter()
        .find(|r| r["file"].ends_with("cuadrado.svg"))
        .unwrap();
    assert_eq!(good["status"], "ok", "{good:?}");
    assert_eq!(good["emu_status"], "ok", "{good:?}");
    assert_eq!(good["emu_rapid_burns"], "0");
    assert_eq!(good["paths"], "1");
    assert_eq!(good["nodes"], "4");
    assert!(good["gcode_lines"].parse::<usize>().unwrap() > 5);
    assert!(good["rd_bytes"].parse::<usize>().unwrap() > 0);
    // El emulador reproduce el tiempo estimado de un trabajo corto con poca diferencia.
    let diff: f64 = good["emu_diff_pct"].parse().unwrap();
    assert!(diff.abs() < 25.0, "diferencia {diff} %");
    let len_diff: f64 = good["emu_len_diff_pct"].parse().unwrap();
    assert!(len_diff.abs() < 1.0, "longitud quemada {len_diff} %");
    let bad = rows
        .iter()
        .find(|r| r["file"].ends_with("roto.svg"))
        .unwrap();
    assert_eq!(bad["status"], "error");
    assert_eq!(bad["error_phase"], "import");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn generates_synthetic_files() {
    let dir = scratch("gen");
    let status = Command::new(bin())
        .args(["gen", dir.to_str().unwrap(), "--only", "dxf-splines"])
        .status()
        .unwrap();
    assert!(status.success());
    let text = std::fs::read_to_string(dir.join("synth-dxf-splines-5k.dxf")).unwrap();
    assert!(text.contains("SPLINE") && text.ends_with("EOF\n"));
    let _ = std::fs::remove_dir_all(&dir);
}

/// Los archivos reales versionados en `tests/corpus` (CC0 o dominio público) pasan el pipeline
/// completo sin errores ni pánicos. Sin emulador y con umbral holgado: el binario de pruebas va
/// sin optimizar.
#[test]
fn versioned_corpus_runs_clean() {
    let corpus = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/corpus");
    let dir = scratch("corpus");
    let csv = dir.join("out.csv");
    let out = Command::new(bin())
        .args([
            "run",
            corpus.to_str().unwrap(),
            "--no-emu",
            "--threshold",
            "600",
            "--csv",
            csv.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    let rows = csv_rows(&std::fs::read_to_string(&csv).unwrap());
    assert!(rows.len() >= 10, "{} archivos", rows.len());
    for r in &rows {
        assert_eq!(r["status"], "ok", "{}: {}", r["file"], r["error"]);
    }
    assert!(out.status.success(), "{out:?}");
    let _ = std::fs::remove_dir_all(&dir);
}
