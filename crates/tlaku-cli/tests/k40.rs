//! Pruebas de integración de `tlaku k40`: ejecutan el binario contra la placa simulada (`--emu`)
//! y contra el USB real (donde no hay ninguna placa conectada).

use std::path::{Path, PathBuf};
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

fn tmp(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("tlaku-k40");
    std::fs::create_dir_all(&dir).unwrap();
    dir.join(name)
}

#[test]
fn send_with_the_emulator_end_to_end() {
    let o = run(&["k40", "send", &design(), "--mode", "line", "--emu", "--yes"]);
    let (out, err) = text(&o);
    assert!(o.status.success(), "{out}\n{err}");
    assert!(out.contains("EXPERIMENTAL"), "{out}");
    assert!(out.contains("Enviado:"), "{out}");
    assert!(out.contains("terminó con el láser apagado"), "{out}");
    assert!(out.contains("quemó 33"), "{out}");
}

#[test]
fn status_with_the_emulator_is_read_only() {
    let o = run(&["k40", "status", "--emu"]);
    let (out, err) = text(&o);
    assert!(o.status.success(), "{out}\n{err}");
    assert!(out.contains("lista (206)"), "{out}");
    assert!(out.contains("Nada se movió"), "{out}");
}

#[test]
fn a_moshiboard_makes_status_fail_with_an_explanation() {
    let o = run(&["k40", "status", "--emu-moshiboard"]);
    let (_, err) = text(&o);
    assert!(!o.status.success());
    assert!(err.contains("Moshiboard"), "{err}");
}

#[test]
fn export_egv_then_dry_run_agree_on_the_job_size() {
    let egv = tmp("diseno.egv");
    let o = run(&[
        "k40",
        "export-egv",
        &design(),
        egv.to_str().unwrap(),
        "--mode",
        "line",
    ]);
    let (out, err) = text(&o);
    assert!(o.status.success(), "{out}\n{err}");
    let file = std::fs::read_to_string(&egv).unwrap();
    assert!(file.starts_with("Document type : LHYMICRO-GL file"));
    assert!(file.contains("FNSE-"));
    let d = run(&["k40", "send", &design(), "--mode", "line", "--dry-run"]);
    let (dry, _) = text(&d);
    assert!(d.status.success(), "{dry}");
    // El tamaño del flujo es el mismo que el del archivo sin cabecera.
    let stream = file.split("%0%0%0%0%\n").nth(1).unwrap();
    assert!(dry.contains(&format!("{} bytes", stream.len())), "{dry}");
}

#[test]
fn without_a_board_the_usb_commands_explain_what_to_check() {
    let o = run(&["k40", "status"]);
    let (_, err) = text(&o);
    // Sin placa conectada (lo normal en CI) falla con un mensaje útil; con una conectada, no.
    if !o.status.success() {
        assert!(err.contains("placa"), "{err}");
    }
    let o = run(&["k40", "list"]);
    assert!(o.status.success());
}
