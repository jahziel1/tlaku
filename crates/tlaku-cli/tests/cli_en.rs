//! La CLI en inglés: con `TLAKU_LANG=en` los resúmenes, avisos y errores salen en inglés. El
//! español (idéntico al de siempre) lo cubren las demás pruebas de integración.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

fn fixture(name: &str) -> String {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
        .to_str()
        .unwrap()
        .to_string()
}

fn run_in(lang: &str, args: &[&str], stdin: &str) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_tlaku"))
        .env("TLAKU_LANG", lang)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(stdin.as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}

fn run_en(args: &[&str]) -> Output {
    run_in("en", args, "")
}

fn out(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}

fn err(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

fn workdir(test: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("tlaku-cli-en")
        .join(test);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn gcode_summary_is_in_english() {
    let dir = workdir("gcode_summary");
    let output = dir.join("a.gcode");
    let o = run_en(&[
        "gcode",
        &fixture("design.svg"),
        "-o",
        output.to_str().unwrap(),
    ]);
    assert!(o.status.success(), "{}\n{}", out(&o), err(&o));
    let text = out(&o);
    assert!(text.contains("Layers (in the order they run):"), "{text}");
    assert!(text.contains("Estimated total time:"), "{text}");
    assert!(text.contains("Occupied area (with overscan):"), "{text}");
    assert!(text.contains("pass(es)"), "{text}");
    assert!(text.contains("Wrote "), "{text}");
    for spanish in ["Capas", "Tiempo total", "Escrito", "pasada"] {
        assert!(!text.contains(spanish), "{spanish} en:\n{text}");
    }
}

#[test]
fn plan_explains_the_steps_in_english() {
    let o = run_en(&["plan", &fixture("design.svg")]);
    assert!(o.status.success(), "{}\n{}", out(&o), err(&o));
    let text = out(&o);
    assert!(text.contains("Layer 1 of "), "{text}");
    assert!(text.contains("Travel moves:"), "{text}");
    assert!(!text.contains("Capa "), "{text}");
}

#[test]
fn design_errors_are_in_english() {
    let o = run_en(&[
        "plan",
        &fixture("design.svg"),
        "--layer",
        "#ff0000:speed=300",
        "--layer",
        "#ff0000:power=50",
    ]);
    assert!(!o.status.success());
    assert!(
        err(&o).contains("appears more than once in --layer"),
        "{}",
        err(&o)
    );
}

#[test]
fn argument_parse_errors_are_in_english() {
    let o = run_en(&["plan", &fixture("design.svg"), "--speed", "abc"]);
    assert!(!o.status.success());
    assert!(err(&o).contains("is not a valid number"), "{}", err(&o));

    let o = run_en(&["plan", &fixture("design.svg"), "--layer", "#ff0000:foo=1"]);
    assert!(err(&o).contains("unknown key 'foo'"), "{}", err(&o));
}

#[test]
fn cmd_confirmation_is_in_english_and_spanish_stays_spanish() {
    let args = ["cmd", "--port", "/dev/no-existe", "$X"];
    let o = run_in("en", &args, "n\n");
    assert!(!o.status.success());
    assert!(
        out(&o).contains("This will be sent exactly as: $X"),
        "{}",
        out(&o)
    );
    assert!(
        out(&o).contains("Warning: $X clears the alarm"),
        "{}",
        out(&o)
    );
    assert!(out(&o).contains("Send it? [y/N]"), "{}", out(&o));
    assert!(
        err(&o).contains("cancelled: nothing was sent to the machine"),
        "{}",
        err(&o)
    );

    // Se acepta "y" o "s" en cualquier idioma; aquí confirma en inglés y falla al abrir el puerto.
    let o = run_in("en", &args, "y\n");
    assert!(
        out(&o).contains("Connecting to /dev/no-existe (115200 baud)..."),
        "{}",
        out(&o)
    );

    let o = run_in("es", &args, "n\n");
    assert!(
        out(&o).contains("Se enviará exactamente: $X"),
        "{}",
        out(&o)
    );
    assert!(out(&o).contains("¿Enviarlo? [s/N]"), "{}", out(&o));
}

#[test]
fn k40_dry_run_is_in_english() {
    let o = run_en(&["k40", "send", &fixture("design.svg"), "--dry-run"]);
    assert!(o.status.success(), "{}\n{}", out(&o), err(&o));
    let text = out(&o);
    assert!(text.contains("LHYMICRO-GL job (board "), "{text}");
    assert!(
        text.contains("Dry run (--dry-run): the machine was not contacted."),
        "{text}"
    );
}
