//! El escritor del punto de control: un hilo propio que, mientras dura el envío, guarda cada
//! ~1 s la última línea confirmada con `ok`, la posición de máquina y el estado modal.
//!
//! El bucle del trabajador solo anota (`note_acked`, `note_status`): un atómico y un `Mutex` de
//! muy corta duración, sin tocar el disco. Todo lo que pesa (guardar el G-code, recorrerlo para
//! sacar el estado modal, escribir y renombrar) pasa en este hilo.

use super::store::{CheckpointMeta, CheckpointProgress, CheckpointStore, VERSION, now_ms};
use super::walker::{Walker, is_sendable};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

/// Lo que importa de un reporte de estado (`?`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StatusSnap {
    pub mpos: [f64; 2],
    pub wpos: [f64; 2],
    pub wco: [f64; 2],
    pub planner_free: u32,
    pub rx_free: u32,
}

/// Qué hacer con el punto de control al terminar el envío.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Finish {
    /// El trabajo no terminó: se guarda el último avance y queda para continuarlo.
    Keep,
    /// Terminó bien o se canceló a propósito: se borra.
    Discard,
}

struct Shared {
    /// `ok` recibidos hasta ahora.
    acked: AtomicU32,
    /// Líneas entregadas a la máquina hasta ahora (con o sin `ok`).
    sent: AtomicU32,
    status: Mutex<Option<StatusSnap>>,
    stop: Mutex<Option<Finish>>,
    wake: Condvar,
    error: Mutex<Option<String>>,
}

pub struct CheckpointWriter {
    shared: Arc<Shared>,
    thread: Option<JoinHandle<()>>,
}

impl CheckpointWriter {
    /// Empieza el punto de control de un trabajo. `original` es el G-code que se guarda (el
    /// completo) y `sent` el que realmente se envía (el mismo, o preámbulo + resto en una
    /// reanudación). Borra el punto de control anterior ya, antes de que empiece el envío.
    pub fn start(
        store: Arc<CheckpointStore>,
        meta: CheckpointMeta,
        original: Arc<Vec<String>>,
        sent: Arc<Vec<String>>,
        interval: Duration,
    ) -> Self {
        store.clear();
        let shared = Arc::new(Shared {
            acked: AtomicU32::new(0),
            sent: AtomicU32::new(0),
            status: Mutex::new(None),
            stop: Mutex::new(None),
            wake: Condvar::new(),
            error: Mutex::new(None),
        });
        let s = shared.clone();
        let thread = thread::Builder::new()
            .name("tlaku-checkpoint".into())
            .spawn(move || run(&store, &meta, &original, &sent, &s, interval))
            .ok();
        Self { shared, thread }
    }

    /// Cuántos `ok` van. No bloquea.
    pub fn note_acked(&self, acked: usize) {
        self.shared
            .acked
            .store(u32::try_from(acked).unwrap_or(u32::MAX), Ordering::Relaxed);
    }

    /// Cuántas líneas se han entregado a la máquina (las que aún no tienen `ok` ya están en su
    /// memoria). No bloquea.
    pub fn note_sent(&self, sent: usize) {
        self.shared
            .sent
            .store(u32::try_from(sent).unwrap_or(u32::MAX), Ordering::Relaxed);
    }

    /// Último reporte de estado. No bloquea (la sección crítica es una asignación).
    pub fn note_status(&self, snap: StatusSnap) {
        if let Ok(mut g) = self.shared.status.lock() {
            *g = Some(snap);
        }
    }

    /// Un error de escritura pendiente de avisar (una sola vez).
    pub fn take_error(&self) -> Option<String> {
        self.shared.error.try_lock().ok()?.take()
    }

    /// Termina el hilo y espera. Con [`Finish::Discard`] borra el punto de control.
    pub fn finish(mut self, how: Finish, store: &CheckpointStore) {
        self.stop(how);
        if how == Finish::Discard {
            store.clear();
        }
    }

    fn stop(&mut self, how: Finish) {
        if let Ok(mut g) = self.shared.stop.lock() {
            *g = Some(how);
        }
        self.shared.wake.notify_all();
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

impl Drop for CheckpointWriter {
    fn drop(&mut self) {
        // Sin `finish` (hilo caído, conexión descartada): el punto de control se conserva.
        if self.thread.is_some() {
            self.stop(Finish::Keep);
        }
    }
}

struct State {
    sendable: Vec<u32>,
    walker: Walker,
    walked: usize,
    seq: u64,
    last_acked: u32,
    last_sent: u32,
    last_status: Option<StatusSnap>,
    planner_cap: u32,
    rx_cap: u32,
    wrote_any: bool,
}

fn run(
    store: &CheckpointStore,
    meta: &CheckpointMeta,
    original: &[String],
    sent: &[String],
    shared: &Shared,
    interval: Duration,
) {
    if let Err(e) = store.begin(meta, Some(original)) {
        set_error(shared, e);
        return;
    }
    let mut st = State {
        sendable: sent
            .iter()
            .enumerate()
            .filter(|(_, l)| is_sendable(l))
            .map(|(i, _)| u32::try_from(i).unwrap_or(u32::MAX))
            .collect(),
        walker: Walker::new(),
        walked: 0,
        seq: 0,
        last_acked: u32::MAX,
        last_sent: u32::MAX,
        last_status: None,
        planner_cap: 0,
        rx_cap: 0,
        wrote_any: false,
    };
    loop {
        let stop = {
            let Ok(mut g) = shared.stop.lock() else {
                return;
            };
            if g.is_none() {
                g = match shared.wake.wait_timeout(g, interval) {
                    Ok((g, _)) => g,
                    Err(_) => return,
                };
            }
            *g
        };
        if stop == Some(Finish::Discard) {
            return;
        }
        write_now(store, sent, shared, &mut st);
        if stop.is_some() {
            return;
        }
    }
}

fn set_error(shared: &Shared, e: String) {
    if let Ok(mut g) = shared.error.lock() {
        g.get_or_insert(e);
    }
}

/// Escribe el avance si cambió algo desde la última vez.
fn write_now(store: &CheckpointStore, sent: &[String], shared: &Shared, st: &mut State) {
    let acked = shared.acked.load(Ordering::Relaxed);
    // Lo entregado nunca va por detrás de lo confirmado.
    let sent_n = shared.sent.load(Ordering::Relaxed).max(acked);
    let status = shared.status.lock().ok().and_then(|g| *g);
    if st.wrote_any && acked == st.last_acked && sent_n == st.last_sent && status == st.last_status
    {
        return;
    }
    if let Some(s) = status {
        st.planner_cap = st.planner_cap.max(s.planner_free);
        st.rx_cap = st.rx_cap.max(s.rx_free);
    }
    let sent_line = (acked > 0)
        .then(|| st.sendable.get(acked as usize - 1).copied())
        .flatten();
    let sent_to_line = (sent_n > 0)
        .then(|| st.sendable.get(sent_n as usize - 1).copied())
        .flatten();
    if let Some(line) = sent_line {
        while st.walked <= line as usize && st.walked < sent.len() {
            let _ = st.walker.feed(
                u32::try_from(st.walked).unwrap_or(u32::MAX),
                &sent[st.walked],
            );
            st.walked += 1;
        }
    }
    st.seq += 1;
    let p = CheckpointProgress {
        version: VERSION,
        seq: st.seq,
        acked,
        sent_line,
        sent: sent_n,
        sent_to_line,
        mpos: status.map(|s| s.mpos),
        wpos: status.map(|s| s.wpos),
        wco: status.map(|s| s.wco),
        planner_free: status.map(|s| s.planner_free),
        rx_free: status.map(|s| s.rx_free),
        planner_cap: (st.planner_cap > 0).then_some(st.planner_cap),
        rx_cap: (st.rx_cap > 0).then_some(st.rx_cap),
        modal: st.walker.modal(),
        t_ms: now_ms(),
    };
    match store.write_progress(&p) {
        Ok(()) => {
            st.last_acked = acked;
            st.last_sent = sent_n;
            st.last_status = status;
            st.wrote_any = true;
        }
        Err(e) => set_error(shared, e),
    }
}
