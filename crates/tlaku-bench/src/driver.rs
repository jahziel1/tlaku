//! Padre: lanza un proceso trabajador por archivo y modo, con plazo duro, y reúne los resultados.

use crate::report::{CSV_HEADER, Report};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

pub struct RunOptions {
    pub modes: Vec<String>,
    pub threshold_s: f64,
    pub hard_timeout_s: f64,
    pub emu: bool,
    pub emu_max_lines: usize,
    pub emu_max_speed: f64,
    pub emu_deadline_s: f64,
}

const EXTENSIONS: &[&str] = &[
    "svg", "dxf", "png", "jpg", "jpeg", "bmp", "webp", "tif", "tiff", "gif", "tga", "pdf", "ai",
    "plt", "hpgl", "hpg", "nc", "gcode", "gc", "ngc", "lbrn", "lbrn2",
];

/// Archivos importables bajo `root` (recursivo), ordenados.
pub fn collect(root: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(root) else {
        return;
    };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            if p.file_name()
                .is_some_and(|n| n.to_string_lossy().starts_with('_'))
            {
                continue;
            }
            collect(&p, out);
        } else if p
            .extension()
            .and_then(|x| x.to_str())
            .is_some_and(|x| EXTENSIONS.contains(&x.to_ascii_lowercase().as_str()))
        {
            out.push(p);
        }
    }
    out.sort();
}

/// Ejecuta un archivo en un proceso aparte. Siempre devuelve un informe.
pub fn run_one(path: &Path, mode: &str, opts: &RunOptions) -> Report {
    let exe = std::env::current_exe().expect("ruta del ejecutable");
    let mut cmd = Command::new(exe);
    cmd.arg("worker")
        .arg(path)
        .args(["--mode", mode])
        .args(["--threshold", &opts.threshold_s.to_string()])
        .args(["--emu-max-lines", &opts.emu_max_lines.to_string()])
        .args(["--emu-max-speed", &opts.emu_max_speed.to_string()])
        .args(["--emu-deadline", &opts.emu_deadline_s.to_string()]);
    if !opts.emu {
        cmd.arg("--no-emu");
    }
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    let start = Instant::now();
    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => return failed(path, mode, "crash", &format!("no se pudo lanzar: {e}")),
    };
    let stdout = child.stdout.take().expect("stdout");
    let stderr = child.stderr.take().expect("stderr");
    let (tx, rx) = mpsc::channel::<String>();
    std::thread::spawn(move || {
        for l in BufReader::new(stdout).lines().map_while(Result::ok) {
            if tx.send(l).is_err() {
                break;
            }
        }
    });
    let err_thread = std::thread::spawn(move || {
        let mut tail = String::new();
        for l in BufReader::new(stderr).lines().map_while(Result::ok) {
            tail = l; // la última línea basta
        }
        tail
    });
    let hard = Duration::from_secs_f64(opts.hard_timeout_s);
    let mut last: Option<Report> = None;
    let mut timed_out = false;
    loop {
        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(line) => {
                if let Ok(r) = serde_json::from_str::<Report>(&line) {
                    last = Some(r);
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if start.elapsed() > hard {
                    timed_out = true;
                    let _ = child.kill();
                    break;
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
    let exit = child.wait().ok();
    let stderr_tail = err_thread.join().unwrap_or_default();
    // Lo que quedó en el canal tras la muerte.
    while let Ok(line) = rx.try_recv() {
        if let Ok(r) = serde_json::from_str::<Report>(&line) {
            last = Some(r);
        }
    }
    let mut rep = last.unwrap_or_else(|| failed(path, mode, "crash", "sin informe"));
    if timed_out {
        rep.status = "timeout".into();
        rep.error_phase = rep.running.clone();
        rep.error = format!("más de {:.0} s (plazo duro)", opts.hard_timeout_s);
        rep.total_s = start.elapsed().as_secs_f64();
    } else if !rep.running.is_empty() {
        // El proceso murió dentro de una fase: señal (pila, memoria) o abort.
        rep.status = "crash".into();
        rep.error_phase = rep.running.clone();
        rep.error = format!("{} {}", describe_exit(exit), stderr_tail.trim());
        rep.total_s = start.elapsed().as_secs_f64();
    }
    rep.running.clear();
    rep
}

#[cfg(unix)]
fn describe_exit(st: Option<std::process::ExitStatus>) -> String {
    use std::os::unix::process::ExitStatusExt;
    match st {
        Some(s) => match s.signal() {
            Some(sig) => format!("señal {sig}"),
            None => format!("código {:?}", s.code()),
        },
        None => "sin estado".into(),
    }
}

#[cfg(not(unix))]
fn describe_exit(st: Option<std::process::ExitStatus>) -> String {
    format!("{st:?}")
}

fn failed(path: &Path, mode: &str, status: &str, msg: &str) -> Report {
    Report {
        file: path.display().to_string(),
        mode: mode.into(),
        status: status.into(),
        error: msg.into(),
        ..Report::default()
    }
}

pub fn csv_text(rows: &[Report]) -> String {
    let mut s = CSV_HEADER.join(",");
    s.push('\n');
    for r in rows {
        s.push_str(&r.csv_row());
        s.push('\n');
    }
    s
}
