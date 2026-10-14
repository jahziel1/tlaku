//! Pruebas de integración de `tlaku ruida status` y `tlaku ruida send`: ejecutan el binario
//! contra el emulador de controladora Ruida por UDP en localhost (puerto efímero).

use std::io::Write;
use std::path::Path;
use std::process::{Command, Output, Stdio};
use tlaku_ruida::cmd::Command as RdCommand;
use tlaku_ruida::emu::{EmuConfig, UdpEmulator};

fn design() -> String {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/design.svg")
        .to_str()
        .unwrap()
        .to_string()
}

fn run(args: &[&str], stdin: &str) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_tlaku"))
        .env("TLAKU_LANG", "es")
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

fn text(b: &[u8]) -> String {
    String::from_utf8_lossy(b).into_owned()
}

fn net_args(emu: &UdpEmulator) -> Vec<String> {
    vec![
        "--port".into(),
        emu.addr().port().to_string(),
        "--local-port".into(),
        "0".into(),
        "--timeout-ms".into(),
        "500".into(),
        "--home".into(),
        "bottom-left".into(),
    ]
}

#[test]
fn status_reads_the_machine_without_writing() {
    let emu = UdpEmulator::spawn(EmuConfig::default()).unwrap();
    emu.emulator().pos_um = (50_000, 20_000);
    let mut args = vec!["ruida".to_string(), "status".into(), "127.0.0.1".into()];
    args.extend(net_args(&emu));
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let o = run(&refs, "");
    assert!(
        o.status.success(),
        "{}\n{}",
        text(&o.stdout),
        text(&o.stderr)
    );
    let out = text(&o.stdout);
    assert!(out.contains("RDC6442S"), "{out}");
    assert!(out.contains("X 50.000 mm, Y 20.000 mm"), "{out}");
    assert!(
        emu.emulator()
            .commands()
            .unwrap()
            .iter()
            .all(|c| matches!(c, RdCommand::MemRead { .. }))
    );
}

#[test]
fn send_stores_a_document_and_run_needs_the_confirmation() {
    let emu = UdpEmulator::spawn(EmuConfig::default()).unwrap();
    let base = |extra: &[&str]| -> Vec<String> {
        let mut a: Vec<String> = ["ruida", "send", "127.0.0.1"]
            .into_iter()
            .map(String::from)
            .collect();
        a.push(design());
        a.extend(["--mode", "line", "--bed", "600x400"].map(String::from));
        a.extend(net_args(&emu));
        a.extend(extra.iter().map(|s| s.to_string()));
        a
    };
    let go = |args: Vec<String>, stdin: &str| {
        let refs: Vec<&str> = args.iter().map(String::as_str).collect();
        run(&refs, stdin)
    };

    // Respuesta negativa: no se envía nada y falla.
    let o = go(base(&["--run"]), "n\n");
    assert!(!o.status.success());
    assert!(text(&o.stderr).contains("cancelado"), "{}", text(&o.stderr));
    assert!(!emu.emulator().running && emu.emulator().stored.is_empty());

    // Guardar como documento (por defecto) con confirmación.
    let o = go(base(&["--name", "PRUEBA"]), "s\n");
    assert!(
        o.status.success(),
        "{}\n{}",
        text(&o.stdout),
        text(&o.stderr)
    );
    assert!(text(&o.stdout).contains("Documento guardado"));
    {
        let e = emu.emulator();
        assert!(!e.running);
        assert_eq!(e.stored.len(), 1);
        assert_eq!(e.stored[0].name, "PRUEBA");
        let cmds = tlaku_ruida::cmd::decode_stream(&e.stored[0].stream).unwrap();
        assert_eq!(cmds.last(), Some(&RdCommand::EndOfFile));
    }

    // Ejecutar con --yes: arranca.
    let o = go(base(&["--run", "--yes"]), "");
    assert!(
        o.status.success(),
        "{}\n{}",
        text(&o.stdout),
        text(&o.stderr)
    );
    assert!(text(&o.stdout).contains("se está ejecutando"));
    assert!(emu.emulator().running);

    // Con la máquina en marcha, un segundo envío se rechaza.
    let o = go(base(&["--run", "--yes"]), "");
    assert!(!o.status.success());
    assert!(
        text(&o.stderr).contains("no está en reposo"),
        "{}",
        text(&o.stderr)
    );
}
