//! Placa M2 Nano simulada, solo para pruebas (feature `emu` o `cargo test`).
//!
//! Habla el protocolo del CH341 en modo EPP como lo describen las fuentes (ver [`crate::crc`] y
//! [`crate::status`]): reúne los bloques `A6 <datos>` en paquetes lógicos de 32 bytes, verifica el
//! CRC, contesta a `A0` con 6 bytes de estado, decodifica la carga con [`crate::decode`] y
//! **ejecuta** los movimientos en un reloj virtual para llevar el rastro del láser (dónde estuvo
//! encendido), igual que `tlaku-emu` para GRBL.
//!
//! # Tiempo
//!
//! El reloj virtual avanza `poll_dt` en cada consulta de estado: una consulta es la única forma
//! de que pase el tiempo. La placa acepta paquetes mientras haya menos de `buffer_packets`
//! pendientes de ejecutar (si no, contesta 238 "ocupada") y mueve el cabezal a la velocidad del
//! último código de velocidad (los saltos rápidos, a `rapid_mm_s`).
//!
//! # Lo que es hipótesis
//!
//! No es la placa real. Son suposiciones (no verificadas en hardware): que la placa queda en 236
//! al vaciar su búfer tras haber trabajado y en 206 antes; que `PN` pausa y despausa; que `I`
//! cancela el programa en curso; el tamaño del búfer y la velocidad de los saltos rápidos.

use crate::crc::{self, EPP_STATUS, EPP_WRITE, LOGICAL_LEN, PAYLOAD_LEN, PacketCheck};
use crate::decode::{Decoder, Event, MoveMode};
use crate::speed::{self, Board, MM_PER_MIL, ParsedSpeed};
use crate::status::{
    MOSHI_OK, STATUS_BUSY, STATUS_CRC_ERROR, STATUS_FINISH, STATUS_LEN, STATUS_LOW_POWER, STATUS_OK,
};
use crate::transport::{Transport, TransportError, TransportInfo};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex, MutexGuard};

/// Fallos que se pueden provocar a propósito. Los índices cuentan desde 0.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct EmuFaults {
    /// Estropea el CRC del paquete n (una sola vez: el reenvío llega bien).
    pub corrupt_packet: Option<usize>,
    /// Estropea el CRC de todos los paquetes desde el n (CRC repetido).
    pub corrupt_from: Option<usize>,
    /// Estropea el CRC del paquete n, pero la placa solo lo delata a partir de la segunda
    /// consulta de estado posterior (rechazo tardío).
    pub late_crc_packet: Option<usize>,
    /// El USB se desconecta después de n paquetes recibidos.
    pub disconnect_after_packets: Option<usize>,
    /// El USB se desconecta después de n consultas de estado.
    pub disconnect_after_polls: Option<usize>,
    /// Desde la consulta n el estado es siempre 238 (ocupada).
    pub busy_from_poll: Option<usize>,
    /// Tras n paquetes aceptados la placa informa 239 (energía insuficiente).
    pub low_power_after_packets: Option<usize>,
    /// Las primeras n consultas no devuelven estado (código 0).
    pub empty_polls: usize,
    /// Desde la consulta n no se devuelve estado (código 0).
    pub empty_from_poll: Option<usize>,
    /// El estado llega con un solo byte.
    pub short_status: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EmuConfig {
    pub board: Board,
    /// Paquetes que caben a la vez (el que se ejecuta y los que esperan).
    pub buffer_packets: usize,
    /// Tiempo virtual (s) que avanza cada consulta de estado.
    pub poll_dt: f64,
    /// Velocidad de los saltos rápidos (mm/s).
    pub rapid_mm_s: f64,
    /// Se comporta como una Moshiboard (mismo VID/PID, otros códigos de estado).
    pub moshiboard: bool,
    pub chip_version: u16,
    pub faults: EmuFaults,
}

impl Default for EmuConfig {
    fn default() -> Self {
        EmuConfig {
            board: Board::M2,
            buffer_packets: 2,
            poll_dt: 0.02,
            rapid_mm_s: 200.0,
            moshiboard: false,
            chip_version: 0x30,
            faults: EmuFaults::default(),
        }
    }
}

/// Tramo recorrido por el cabezal (mils, ejes de la placa).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TracePiece {
    pub from: (f64, f64),
    pub to: (f64, f64),
    pub laser: bool,
    pub mode: MoveMode,
    /// Tiempo virtual (s) al final del tramo.
    pub t_end: f64,
}

impl TracePiece {
    pub fn length_mm(&self) -> f64 {
        ((self.to.0 - self.from.0).powi(2) + (self.to.1 - self.from.1).powi(2)).sqrt() * MM_PER_MIL
    }
}

#[derive(Debug, Clone)]
struct Running {
    from: (f64, f64),
    to: (f64, f64),
    laser: bool,
    mode: MoveMode,
    len_mm: f64,
    done_mm: f64,
    speed_mm_s: f64,
}

#[derive(Debug)]
pub struct EmuBoard {
    cfg: EmuConfig,
    decoder: Decoder,
    /// Bytes de datos EPP aún sin completar un paquete lógico.
    rx: Vec<u8>,
    /// Respuestas pendientes de leer.
    out: VecDeque<u8>,
    queue: VecDeque<VecDeque<Event>>,
    cur: Option<Running>,
    head: (f64, f64),
    exec_speed_mm_s: f64,
    paused: bool,
    time: f64,
    trace: Vec<TracePiece>,
    laser_now: bool,
    activity: bool,
    crc_error: bool,
    /// Consultas de estado durante las que el error de CRC aún no se ve.
    crc_hidden_polls: u32,
    /// Paquetes recibidos (válidos o no).
    received: usize,
    polls: usize,
    /// Paquetes aceptados.
    pub accepted: usize,
    /// Paquetes rechazados por CRC o cabecera.
    pub rejected: usize,
    /// Paquetes recibidos con el búfer lleno: la sesión nunca debe provocarlos.
    pub overruns: usize,
    /// Veces que llegó `PN`.
    pub pause_toggles: usize,
    /// Veces que llegó un `I` solo (cancelar).
    pub aborts: usize,
    /// Veces que la placa contestó 238 "ocupada".
    pub busy_replies: usize,
    /// Bytes sueltos que no eran `A6` ni `A0`.
    pub garbage_bytes: usize,
    /// Códigos de velocidad que no se entendieron.
    pub bad_speed_codes: usize,
    /// Cargas aceptadas, en orden.
    pub packets: Vec<[u8; PAYLOAD_LEN]>,
    /// Estado del riel según el último `S1P`/`S2P`.
    pub rail_locked: bool,
}

impl EmuBoard {
    pub fn new(cfg: EmuConfig) -> Self {
        EmuBoard {
            cfg,
            decoder: Decoder::new(),
            rx: Vec::new(),
            out: VecDeque::new(),
            queue: VecDeque::new(),
            cur: None,
            head: (0.0, 0.0),
            exec_speed_mm_s: 10.0,
            paused: false,
            time: 0.0,
            trace: Vec::new(),
            laser_now: false,
            activity: false,
            crc_error: false,
            crc_hidden_polls: 0,
            received: 0,
            polls: 0,
            accepted: 0,
            rejected: 0,
            overruns: 0,
            pause_toggles: 0,
            aborts: 0,
            busy_replies: 0,
            garbage_bytes: 0,
            bad_speed_codes: 0,
            packets: Vec::new(),
            rail_locked: false,
        }
    }

    /// La placa y un transporte que habla con ella; la placa queda compartida para inspeccionarla
    /// mientras la sesión usa el transporte.
    pub fn into_transport(self) -> (EmuTransport, Arc<Mutex<EmuBoard>>) {
        let info = TransportInfo {
            name: if self.cfg.moshiboard {
                "Moshiboard simulada".into()
            } else {
                "M2 Nano simulada".into()
            },
            vendor_id: crate::transport::VENDOR_ID,
            product_id: crate::transport::PRODUCT_ID,
            chip_version: Some(self.cfg.chip_version),
        };
        let shared = Arc::new(Mutex::new(self));
        (
            EmuTransport {
                board: Arc::clone(&shared),
                info,
            },
            shared,
        )
    }

    // ---------------------------------------------------------------------------------------
    // Consulta

    pub fn config(&self) -> &EmuConfig {
        &self.cfg
    }

    /// Posición del cabezal (mils, ejes de la placa).
    pub fn head_mil(&self) -> (f64, f64) {
        self.head
    }

    /// Posición del cabezal en mm desde el home.
    pub fn head_mm(&self) -> (f64, f64) {
        (self.head.0 * MM_PER_MIL, self.head.1 * MM_PER_MIL)
    }

    /// El láser está disparando ahora mismo.
    pub fn laser_on(&self) -> bool {
        self.laser_now
    }

    pub fn is_paused(&self) -> bool {
        self.paused
    }

    /// Tiempo virtual transcurrido (s).
    pub fn elapsed_s(&self) -> f64 {
        self.time
    }

    pub fn trace(&self) -> &[TracePiece] {
        &self.trace
    }

    /// Longitud recorrida con el láser encendido (mm).
    pub fn burn_length_mm(&self) -> f64 {
        self.trace
            .iter()
            .filter(|p| p.laser)
            .map(TracePiece::length_mm)
            .sum()
    }

    /// Tramos quemados `(inicio, fin)` en mm desde el home.
    pub fn burn_pieces_mm(&self) -> Vec<((f64, f64), (f64, f64))> {
        self.trace
            .iter()
            .filter(|p| p.laser)
            .map(|p| {
                (
                    (p.from.0 * MM_PER_MIL, p.from.1 * MM_PER_MIL),
                    (p.to.0 * MM_PER_MIL, p.to.1 * MM_PER_MIL),
                )
            })
            .collect()
    }

    /// Caja `(min, max)` de lo quemado, en mm desde el home.
    pub fn burn_bbox_mm(&self) -> Option<((f64, f64), (f64, f64))> {
        let mut bb: Option<((f64, f64), (f64, f64))> = None;
        for (a, b) in self.burn_pieces_mm() {
            for p in [a, b] {
                bb = Some(match bb {
                    None => (p, p),
                    Some((lo, hi)) => (
                        (lo.0.min(p.0), lo.1.min(p.1)),
                        (hi.0.max(p.0), hi.1.max(p.1)),
                    ),
                });
            }
        }
        bb
    }

    /// Hay trabajo pendiente de ejecutar.
    pub fn busy(&self) -> bool {
        self.cur.is_some() || !self.queue.is_empty()
    }

    /// Avanza el tiempo hasta vaciar la cola (si no está en pausa). Para cerrar un trabajo en
    /// pruebas y en la línea de comandos.
    pub fn run_to_idle(&mut self) {
        let mut guard = 0;
        while self.busy() && !self.paused && guard < 10_000_000 {
            self.advance(self.cfg.poll_dt.max(0.001));
            guard += 1;
        }
    }

    // ---------------------------------------------------------------------------------------
    // Estado

    fn status_code(&self) -> u8 {
        if self.cfg.moshiboard {
            return MOSHI_OK;
        }
        let f = &self.cfg.faults;
        if let Some(n) = f.low_power_after_packets
            && self.accepted >= n
        {
            return STATUS_LOW_POWER;
        }
        if self.crc_error && self.crc_hidden_polls == 0 {
            return STATUS_CRC_ERROR;
        }
        if f.busy_from_poll.is_some_and(|n| self.polls > n) {
            return STATUS_BUSY;
        }
        if self.queue.len() >= self.cfg.buffer_packets {
            return STATUS_BUSY;
        }
        if self.paused && self.busy() {
            return STATUS_BUSY;
        }
        if !self.busy() && self.activity {
            return STATUS_FINISH;
        }
        STATUS_OK
    }

    fn status_reply(&mut self) -> Vec<u8> {
        let f = self.cfg.faults;
        if self.polls <= f.empty_polls || f.empty_from_poll.is_some_and(|n| self.polls > n) {
            return vec![255, 0, 0, 0, 0, 1];
        }
        let code = self.status_code();
        if f.short_status {
            return vec![255];
        }
        if code == STATUS_BUSY {
            self.busy_replies += 1;
        }
        let mut v = vec![255, code, 0, 0, 0, 1];
        v.truncate(STATUS_LEN);
        v
    }

    // ---------------------------------------------------------------------------------------
    // Escritura (cable)

    /// Procesa lo que la sesión escribe en el endpoint de salida.
    pub fn handle_write(&mut self, data: &[u8]) -> Result<(), TransportError> {
        let f = self.cfg.faults;
        if f.disconnect_after_packets
            .is_some_and(|n| self.received >= n)
            || f.disconnect_after_polls.is_some_and(|n| self.polls >= n)
        {
            return Err(TransportError::Disconnected);
        }
        let mut i = 0;
        while i < data.len() {
            match data[i] {
                EPP_WRITE => {
                    let n = (data.len() - i - 1).min(31);
                    self.rx.extend_from_slice(&data[i + 1..i + 1 + n]);
                    i += 1 + n;
                    while self.rx.len() >= LOGICAL_LEN {
                        let rest = self.rx.split_off(LOGICAL_LEN);
                        let mut pkt = [0u8; LOGICAL_LEN];
                        pkt.copy_from_slice(&self.rx);
                        self.rx = rest;
                        self.receive_packet(pkt);
                    }
                }
                EPP_STATUS => {
                    self.polls += 1;
                    let dt = self.cfg.poll_dt;
                    self.advance(dt);
                    let reply = self.status_reply();
                    self.crc_hidden_polls = self.crc_hidden_polls.saturating_sub(1);
                    self.out.extend(reply);
                    i += 1;
                }
                _ => {
                    self.garbage_bytes += 1;
                    i += 1;
                }
            }
        }
        Ok(())
    }

    /// Lee del endpoint de entrada.
    pub fn handle_read(&mut self, len: usize) -> Result<Vec<u8>, TransportError> {
        if self.out.is_empty() {
            return Err(TransportError::Timeout);
        }
        let n = len.min(self.out.len());
        Ok(self.out.drain(..n).collect())
    }

    fn receive_packet(&mut self, mut pkt: [u8; LOGICAL_LEN]) {
        let idx = self.received;
        self.received += 1;
        let f = self.cfg.faults;
        let late = f.late_crc_packet == Some(idx);
        if f.corrupt_packet == Some(idx) || f.corrupt_from.is_some_and(|n| idx >= n) || late {
            pkt[LOGICAL_LEN - 1] ^= 0x5A;
        }
        if crc::check_logical(&pkt) != PacketCheck::Ok {
            self.rejected += 1;
            self.crc_error = true;
            self.crc_hidden_polls = u32::from(late);
            return;
        }
        let mut payload = [0u8; PAYLOAD_LEN];
        payload.copy_from_slice(&pkt[1..=PAYLOAD_LEN]);
        let tail_is_pad = |from: usize| payload[from..].iter().all(|&b| b == b'F');
        let realtime_pause = payload.starts_with(b"PN") && tail_is_pad(2);
        let realtime_abort = payload[0] == b'I' && tail_is_pad(1);
        if !realtime_pause && !realtime_abort && self.queue.len() >= self.cfg.buffer_packets {
            // En la placa real la escritura se detendría; aquí se cuenta y se descarta.
            self.overruns += 1;
            return;
        }
        self.crc_error = false;
        self.crc_hidden_polls = 0;
        self.accepted += 1;
        self.packets.push(payload);
        if realtime_pause {
            self.pause_toggles += 1;
            self.paused = !self.paused;
            if self.paused {
                self.laser_now = false;
            } else if let Some(r) = &self.cur {
                self.laser_now = r.laser;
            }
            return;
        }
        if realtime_abort {
            self.aborts += 1;
            self.abort();
            return;
        }
        self.activity = true;
        let events = self.decoder.feed_payload(&payload);
        for e in &events {
            match e {
                Event::BadSpeed(_) => self.bad_speed_codes += 1,
                Event::Rail { locked } => self.rail_locked = *locked,
                _ => {}
            }
        }
        if !events.is_empty() {
            self.queue.push_back(events.into());
        }
    }

    /// `I` solo: la placa deja lo que hacía.
    fn abort(&mut self) {
        let mut d = Decoder::new();
        d.feed_payload(b"I");
        self.decoder = d;
        // El intérprete empieza de cero pero el cabezal sigue donde está.
        self.decoder.x = self.head.0.round() as i64;
        self.decoder.y = self.head.1.round() as i64;
        self.queue.clear();
        self.cur = None;
        self.paused = false;
        self.laser_now = false;
        self.activity = false;
        self.crc_error = false;
    }

    // ---------------------------------------------------------------------------------------
    // Ejecución

    fn speed_of(&self, p: &ParsedSpeed) -> f64 {
        let (b, m) = speed::equation(self.cfg.board, p.accel, p.suffix_c, false);
        speed::speed_from_value(p.code_value as f64, b, m).max(0.05)
    }

    fn start_move(&mut self, from: (f64, f64), to: (f64, f64), laser: bool, mode: MoveMode) {
        let len_mm = ((to.0 - from.0).powi(2) + (to.1 - from.1).powi(2)).sqrt() * MM_PER_MIL;
        let speed = match mode {
            MoveMode::Rapid => self.cfg.rapid_mm_s,
            MoveMode::Program => self.exec_speed_mm_s,
        };
        self.laser_now = laser;
        self.cur = Some(Running {
            from,
            to,
            laser,
            mode,
            len_mm,
            done_mm: 0.0,
            speed_mm_s: speed,
        });
    }

    fn next_event(&mut self) -> Option<Event> {
        loop {
            let chunk = self.queue.front_mut()?;
            match chunk.pop_front() {
                Some(e) => {
                    if chunk.is_empty() {
                        self.queue.pop_front();
                    }
                    return Some(e);
                }
                None => {
                    self.queue.pop_front();
                }
            }
        }
    }

    /// Ejecuta `dt` segundos de tiempo virtual.
    fn advance(&mut self, dt: f64) {
        let mut left = dt;
        while left > 1e-12 && !self.paused {
            if self.cur.is_none() {
                let Some(ev) = self.next_event() else {
                    // Sin trabajo: el tiempo pasa igual.
                    self.time += left;
                    break;
                };
                match ev {
                    Event::Move {
                        from,
                        to,
                        laser,
                        mode,
                    } => {
                        let f = (from.0 as f64, from.1 as f64);
                        let t = (to.0 as f64, to.1 as f64);
                        self.start_move(f, t, laser, mode);
                    }
                    Event::Speed(p) => self.exec_speed_mm_s = self.speed_of(&p),
                    Event::Home => {
                        let from = self.head;
                        self.start_move(from, (0.0, 0.0), false, MoveMode::Rapid);
                    }
                    Event::Finished => self.laser_now = false,
                    Event::Init | Event::BadSpeed(_) | Event::Rail { .. } | Event::Power(_) => {}
                }
                continue;
            }
            let r = self.cur.as_mut().expect("movimiento en curso");
            let remaining = r.len_mm - r.done_mm;
            let can = r.speed_mm_s * left;
            let (frac_end, used) = if can >= remaining {
                (
                    1.0,
                    if r.speed_mm_s > 0.0 {
                        remaining / r.speed_mm_s
                    } else {
                        0.0
                    },
                )
            } else {
                ((r.done_mm + can) / r.len_mm, left)
            };
            let start_frac = if r.len_mm > 0.0 {
                r.done_mm / r.len_mm
            } else {
                0.0
            };
            let lerp = |t: f64| {
                (
                    r.from.0 + (r.to.0 - r.from.0) * t,
                    r.from.1 + (r.to.1 - r.from.1) * t,
                )
            };
            let (p0, p1) = (lerp(start_frac), lerp(frac_end));
            self.time += used;
            left -= used;
            if p0 != p1 {
                self.trace.push(TracePiece {
                    from: p0,
                    to: p1,
                    laser: r.laser,
                    mode: r.mode,
                    t_end: self.time,
                });
            }
            self.head = p1;
            if frac_end >= 1.0 {
                self.head = r.to;
                self.cur = None;
            } else {
                r.done_mm += can;
            }
        }
        if !self.busy() && !self.paused {
            self.laser_now = false;
        }
    }
}

fn lock(b: &Arc<Mutex<EmuBoard>>) -> MutexGuard<'_, EmuBoard> {
    b.lock().unwrap_or_else(|e| e.into_inner())
}

/// Transporte que habla con una [`EmuBoard`].
pub struct EmuTransport {
    board: Arc<Mutex<EmuBoard>>,
    info: TransportInfo,
}

impl EmuTransport {
    pub fn shared(&self) -> Arc<Mutex<EmuBoard>> {
        Arc::clone(&self.board)
    }
}

impl Transport for EmuTransport {
    fn write(&mut self, data: &[u8]) -> Result<(), TransportError> {
        lock(&self.board).handle_write(data)
    }

    fn read(&mut self, len: usize) -> Result<Vec<u8>, TransportError> {
        lock(&self.board).handle_read(len)
    }

    fn info(&self) -> TransportInfo {
        self.info.clone()
    }
}

/// Atajo: placa simulada con la configuración dada y su transporte.
pub fn emulator(cfg: EmuConfig) -> (EmuTransport, Arc<Mutex<EmuBoard>>) {
    EmuBoard::new(cfg).into_transport()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::packet::{Frame, frame_stream};
    use crate::status::{Status, StatusReport};
    use crate::transport::{read_status, write_frame};

    fn setup() -> (EmuTransport, Arc<Mutex<EmuBoard>>) {
        emulator(EmuConfig::default())
    }

    fn send_stream(t: &mut EmuTransport, stream: &[u8]) {
        for f in frame_stream(stream).unwrap() {
            // Respeta el protocolo: espera a que haya sitio.
            loop {
                let s = read_status(t).unwrap().status;
                if s.accepts_packets() {
                    break;
                }
            }
            write_frame(t, &f).unwrap();
        }
    }

    #[test]
    fn status_poll_returns_six_bytes_and_starts_ready() {
        let (mut t, _b) = setup();
        let r = read_status(&mut t).unwrap();
        assert_eq!(r.status, Status::Ok);
        assert_eq!(r.raw[0], 255);
        assert_eq!(t.info().chip_version, Some(0x30));
    }

    #[test]
    fn a_rapid_move_executes_with_the_laser_off() {
        let (mut t, b) = setup();
        send_stream(&mut t, b"IBzzzvRzzzvS1P\n");
        b.lock().unwrap().run_to_idle();
        let board = b.lock().unwrap();
        assert_eq!(board.head_mil(), (787.0, 787.0));
        assert_eq!(board.burn_length_mm(), 0.0);
        assert_eq!(board.accepted, 1);
        assert!(board.rail_locked);
        assert_eq!(board.garbage_bytes, 0);
    }

    #[test]
    fn meerk40t_rectangle_burns_the_square_and_nothing_else() {
        let (mut t, b) = setup();
        send_stream(
            &mut t,
            b"IBzzzvRzzzvS1P\nICV2490731016000027CNLBS1EDz139Rz139Tz139Lz139FNSE-\n",
        );
        b.lock().unwrap().run_to_idle();
        let board = b.lock().unwrap();
        let want = 4.0 * 394.0 * MM_PER_MIL;
        assert!(
            (board.burn_length_mm() - want).abs() < 0.01,
            "{}",
            board.burn_length_mm()
        );
        let (lo, hi) = board.burn_bbox_mm().unwrap();
        assert!(
            (lo.0 - 787.0 * MM_PER_MIL).abs() < 0.01 && (lo.1 - 787.0 * MM_PER_MIL).abs() < 0.01
        );
        assert!(
            (hi.0 - 1181.0 * MM_PER_MIL).abs() < 0.01 && (hi.1 - 1181.0 * MM_PER_MIL).abs() < 0.01
        );
        assert_eq!(board.head_mil(), (787.0, 787.0));
        assert!(!board.laser_on());
        // 15 mm/s: el cuadrado de 40 mm de perímetro tarda unos 2,7 s (más el salto).
        let t_burn: f64 = board
            .trace()
            .iter()
            .filter(|p| p.laser)
            .map(|p| p.length_mm())
            .sum::<f64>()
            / 15.0;
        assert!((t_burn - want / 15.0).abs() < 0.2);
    }

    #[test]
    fn a_bad_crc_is_rejected_and_reported_as_207_until_a_good_packet_arrives() {
        let (mut t, b) = emulator(EmuConfig {
            faults: EmuFaults {
                corrupt_packet: Some(0),
                ..Default::default()
            },
            ..Default::default()
        });
        let f = Frame::from_text(b"IBaS1P", false);
        write_frame(&mut t, &f).unwrap();
        assert_eq!(read_status(&mut t).unwrap().status, Status::CrcError);
        assert_eq!(b.lock().unwrap().rejected, 1);
        assert_eq!(b.lock().unwrap().accepted, 0);
        // El reenvío llega bien.
        write_frame(&mut t, &f).unwrap();
        assert_ne!(read_status(&mut t).unwrap().status, Status::CrcError);
        assert_eq!(b.lock().unwrap().accepted, 1);
    }

    #[test]
    fn the_buffer_fills_and_reports_busy_while_the_head_moves() {
        let (mut t, b) = setup();
        // Tres paquetes con un movimiento largo cada uno, sin sondear entre ellos.
        let long = Frame::from_text(b"ICV2490731016000027CNRBS1E", false);
        write_frame(&mut t, &long).unwrap();
        let mv = Frame::from_text(b"DzzzzzzzzzzzzzzzzzzzzzzzzzzU", false);
        write_frame(&mut t, &mv).unwrap();
        write_frame(&mut t, &mv).unwrap();
        // La orden `U` ejecuta la distancia pendiente de la `D` anterior: cada paquete deja
        // un movimiento en cola. Con dos paquetes pendientes el tercero no cabe.
        assert_eq!(b.lock().unwrap().overruns, 1, "el tercero no cabía");
        // Sin que pase el tiempo el estado es "ocupada"; al avanzar la cabeza hay sitio.
        assert_eq!(b.lock().unwrap().status_code(), STATUS_BUSY);
        for _ in 0..3 {
            read_status(&mut t).unwrap();
        }
        assert!(b.lock().unwrap().busy());
    }

    #[test]
    fn pause_stops_the_head_and_the_laser_and_resume_continues() {
        let (mut t, b) = setup();
        send_stream(&mut t, b"IBzzzvRzzzvS1P\n");
        send_stream(&mut t, b"ICV2490731016000027CNLBS1EDzzzzzzzzzFNSE-\n");
        // Avanza un rato y pausa.
        for _ in 0..30 {
            read_status(&mut t).unwrap();
        }
        let before = {
            let board = b.lock().unwrap();
            assert!(board.laser_on() || board.busy());
            board.burn_length_mm()
        };
        write_frame(&mut t, &crate::packet::pause_toggle_frame()).unwrap();
        for _ in 0..50 {
            read_status(&mut t).unwrap();
        }
        let during = {
            let board = b.lock().unwrap();
            assert!(board.is_paused());
            assert!(!board.laser_on(), "en pausa el láser está apagado");
            board.burn_length_mm()
        };
        assert!(during >= before);
        let paused_len = during;
        for _ in 0..50 {
            read_status(&mut t).unwrap();
        }
        assert_eq!(
            b.lock().unwrap().burn_length_mm(),
            paused_len,
            "no avanza en pausa"
        );
        write_frame(&mut t, &crate::packet::pause_toggle_frame()).unwrap();
        b.lock().unwrap().run_to_idle();
        let board = b.lock().unwrap();
        assert!(!board.is_paused());
        assert!(board.burn_length_mm() > paused_len);
        assert_eq!(board.pause_toggles, 2);
    }

    #[test]
    fn abort_clears_the_queue_and_turns_the_laser_off() {
        let (mut t, b) = setup();
        send_stream(&mut t, b"ICV2490731016000027CNRBS1EDzzzzzzzzzzzzzzFNSE-\n");
        for _ in 0..20 {
            read_status(&mut t).unwrap();
        }
        assert!(b.lock().unwrap().laser_on());
        for f in crate::packet::stop_frames() {
            write_frame(&mut t, &f).unwrap();
        }
        b.lock().unwrap().run_to_idle();
        let board = b.lock().unwrap();
        assert!(!board.laser_on());
        assert!(!board.busy());
        assert_eq!(board.aborts, 1);
        let burned = board.burn_length_mm();
        assert!(burned > 0.0 && burned < 14.0 * 255.0 * MM_PER_MIL);
    }

    #[test]
    fn home_goes_to_the_origin() {
        let (mut t, b) = setup();
        send_stream(&mut t, b"IBzzzvRzzzvS1P\n");
        b.lock().unwrap().run_to_idle();
        send_stream(&mut t, b"IPP\n");
        b.lock().unwrap().run_to_idle();
        assert_eq!(b.lock().unwrap().head_mil(), (0.0, 0.0));
    }

    #[test]
    fn faults_disconnect_low_power_and_empty_status() {
        let (mut t, _b) = emulator(EmuConfig {
            faults: EmuFaults {
                disconnect_after_packets: Some(1),
                ..Default::default()
            },
            ..Default::default()
        });
        write_frame(&mut t, &Frame::from_text(b"IBaS1P", false)).unwrap();
        assert_eq!(
            write_frame(&mut t, &Frame::from_text(b"IBaS1P", false)),
            Err(TransportError::Disconnected)
        );

        let (mut t, _b) = emulator(EmuConfig {
            faults: EmuFaults {
                low_power_after_packets: Some(1),
                ..Default::default()
            },
            ..Default::default()
        });
        write_frame(&mut t, &Frame::from_text(b"IBaS1P", false)).unwrap();
        assert_eq!(read_status(&mut t).unwrap().status, Status::LowPower);

        let (mut t, _b) = emulator(EmuConfig {
            faults: EmuFaults {
                empty_polls: 2,
                short_status: false,
                ..Default::default()
            },
            ..Default::default()
        });
        assert_eq!(read_status(&mut t).unwrap().status, Status::NoReply);
        assert_eq!(read_status(&mut t).unwrap().status, Status::NoReply);
        assert_eq!(read_status(&mut t).unwrap().status, Status::Ok);
    }

    #[test]
    fn a_moshiboard_answers_with_its_own_codes() {
        let (mut t, _b) = emulator(EmuConfig {
            moshiboard: true,
            ..Default::default()
        });
        let r: StatusReport = read_status(&mut t).unwrap();
        assert_eq!(r.code(), 205);
        assert!(crate::status::looks_like_moshiboard(r.code()));
    }

    #[test]
    fn status_after_work_is_finished_and_idle_before_is_ok() {
        let (mut t, b) = setup();
        assert_eq!(read_status(&mut t).unwrap().status, Status::Ok);
        send_stream(&mut t, b"IBaS1P\n");
        b.lock().unwrap().run_to_idle();
        assert_eq!(read_status(&mut t).unwrap().status, Status::Finished);
    }
}
