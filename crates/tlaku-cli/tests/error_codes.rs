//! Los errores del motor llevan un código estable (`[bed_overflow] ...`) para la interfaz gráfica.
//! La terminal no debe mostrarlo, en ningún idioma ni en ningún comando que imprima el error.

use std::path::Path;
use std::process::{Command, Output, Stdio};

fn fixture(name: &str) -> String {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
        .to_str()
        .unwrap()
        .to_string()
}

fn run_in(lang: &str, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_tlaku"))
        .env("TLAKU_LANG", lang)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .unwrap()
}

fn stderr(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

/// Comandos que fallan con el error `bed_overflow` del motor (el diseño no cabe en 50x50 mm).
fn overflow_commands(out: &str) -> Vec<Vec<String>> {
    let design = fixture("design.svg");
    let s = |v: &[&str]| v.iter().map(|x| (*x).to_string()).collect::<Vec<_>>();
    vec![
        s(&["gcode", &design, "-o", out, "--bed", "50x50"]),
        s(&[
            "frame",
            &design,
            "--bed",
            "50x50",
            "--port",
            "/dev/no-existe",
            "--yes",
        ]),
    ]
}

#[test]
fn spanish_errors_have_no_code_prefix() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("tlaku-cli-error-codes.gcode");
    for args in overflow_commands(out.to_str().unwrap()) {
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        let o = run_in("es", &args);
        assert!(!o.status.success(), "{args:?} debía fallar");
        let e = stderr(&o);
        assert!(e.starts_with("error: la capa "), "{args:?}:\n{e}");
        assert!(e.contains("se sale del área de trabajo"), "{args:?}:\n{e}");
        assert!(!e.contains("[bed_overflow]"), "{args:?}:\n{e}");
        assert!(!e.contains("bed_overflow"), "{args:?}:\n{e}");
    }
}

#[test]
fn english_errors_have_no_code_prefix_either() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("tlaku-cli-error-codes-en.gcode");
    let args = overflow_commands(out.to_str().unwrap()).remove(0);
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let e = stderr(&run_in("en", &args));
    assert!(e.starts_with("error: layer "), "{e}");
    assert!(!e.contains("bed_overflow"), "{e}");
}
