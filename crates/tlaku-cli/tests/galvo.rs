//! Pruebas de integración de `tlaku galvo`: ejecutan el binario contra la tarjeta simulada
//! (`--emu`) y comprueban que con hardware real nada se abre sin `--i-understand-laser-safety`.

use std::path::Path;
use std::process::{Command, Output, Stdio};

fn design() -> String {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/design.svg")
        .to_str()
        .unwrap()
        .to_string()
}

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_tlaku"))
        .env("TLAKU_LANG", "es")
        .args(args)
        .stdin(Stdio::null())
        .output()
        .unwrap()
}

fn text(o: &Output) -> (String, String) {
    (
        String::from_utf8_lossy(&o.stdout).into_owned(),
        String::from_utf8_lossy(&o.stderr).into_owned(),
    )
}

#[test]
fn mark_with_the_emulator_end_to_end() {
    let o = run(&[
        "galvo",
        "mark",
        &design(),
        "--mode",
        "line",
        "--frequency",
        "30",
        "--emu",
    ]);
    let (out, err) = text(&o);
    assert!(o.status.success(), "{out}\n{err}");
    assert!(out.contains("EXPERIMENTAL"), "{out}");
    assert!(out.contains("Enviado:"), "{out}");
    assert!(out.contains("la fuente terminó apagada"), "{out}");
    assert!(out.contains("ninguno disparó"), "{out}");
}

#[test]
fn mark_on_real_hardware_needs_the_flag_and_does_not_touch_usb() {
    let o = run(&[
        "galvo",
        "mark",
        &design(),
        "--mode",
        "line",
        "--frequency",
        "30",
    ]);
    let (out, err) = text(&o);
    assert!(!o.status.success());
    assert!(err.contains("--i-understand-laser-safety"), "{err}");
    assert!(!out.contains("Tarjeta:"), "{out}");
}

#[test]
fn light_with_the_emulator_never_enables_the_laser() {
    let o = run(&[
        "galvo",
        "light",
        &design(),
        "--mode",
        "line",
        "--emu",
        "--loops",
        "2",
    ]);
    let (out, err) = text(&o);
    assert!(o.status.success(), "{out}\n{err}");
    assert!(out.contains("el láser NO se habilita"), "{out}");
    assert!(out.contains("0 tramos con el haz saliendo"), "{out}");
}

#[test]
fn list_runs_without_hardware_or_the_flag() {
    let o = run(&["galvo", "list"]);
    let (out, err) = text(&o);
    assert!(o.status.success(), "{out}\n{err}");
    assert!(!out.is_empty());
}

#[test]
fn dry_run_needs_no_hardware() {
    let o = run(&[
        "galvo",
        "mark",
        &design(),
        "--mode",
        "line",
        "--frequency",
        "30",
        "--dry-run",
    ]);
    let (out, err) = text(&o);
    assert!(o.status.success(), "{out}\n{err}");
    assert!(out.contains("Simulacro"), "{out}");
}
