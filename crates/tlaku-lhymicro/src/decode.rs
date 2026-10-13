//! Decodificador de LHYMICRO-GL: reconstruye los movimientos y el estado del láser a partir del
//! flujo de comandos. Lo usa el emulador y las pruebas del codificador; no sirve para enviar.
//!
//! La semántica sigue el intérprete de MeerK40t (`lihuiyu/parser.py`, MIT, Copyright (c) 2021
//! meerk40t; ver THIRD_PARTY.md), reescrito aquí con estados explícitos. Resumen del lenguaje:
//!
//! - Mayúsculas son comandos y minúsculas, magnitudes: `a`-`y` = 1-25, `z` = 255, `|` suma 25
//!   (`|a` = 26 ... `|z` = 51) y tres dígitos son un valor literal. La unidad es el mil
//!   (0,0254 mm).
//! - `B`/`T` derecha/izquierda (eje X) y `R`/`L` abajo/arriba (eje Y) fijan dirección y eje;
//!   `M` activa los dos ejes (diagonal). `D`/`U` encienden/apagan el láser.
//! - En modo normal las distancias se acumulan y se ejecutan con `S`, `P` o `N`. En modo
//!   compacto (después de `S1E`) cada comando ejecuta primero la distancia pendiente.
//! - `I` reinicia el intérprete; `@` y `F` preparan la salida; `N` cierra la distancia; `S<n>`
//!   elige un modo y `E` lo ejecuta (`S1E` entra en compacto, `SE` tras `F` termina); `P` pausa y
//!   dos `P` seguidas en el mismo paquete son el home.
//! - `V...`/`CV...` son códigos de velocidad (ver [`crate::speed`]) y `Wnnn` fija la potencia
//!   (solo M3).
//!
//! Lo que el modelo no sabe (no verificado): la trayectoria real de un movimiento rápido con los
//! dos ejes (se informa como línea recta) y la velocidad del movimiento rápido de cada placa.

use crate::speed::{self, ParsedSpeed, RasterStep};

/// Cómo se ejecutó un desplazamiento.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoveMode {
    /// Modo normal (`...S1P`): movimiento rápido de la placa.
    Rapid,
    /// Modo compacto: a la velocidad del último código de velocidad.
    Program,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    /// El cabezal recorre de `from` a `to` (mils, ejes de la placa: X a la derecha, Y hacia
    /// abajo, origen en el home) con el láser en ese estado.
    Move {
        from: (i64, i64),
        to: (i64, i64),
        laser: bool,
        mode: MoveMode,
    },
    /// Llegó un código de velocidad completo.
    Speed(ParsedSpeed),
    /// Código de velocidad que no se entiende.
    BadSpeed(String),
    /// `I`: reinicio del intérprete.
    Init,
    /// Secuencia de home (`PP`): el cabezal va al origen.
    Home,
    /// El flujo terminó (`FNSE`): la placa queda en reposo hasta el próximo `I`.
    Finished,
    /// `S1P` (riel bloqueado) o `S2P` (desbloqueado).
    Rail { locked: bool },
    /// `Wnnn`: potencia en milésimas (0-1000). Solo existe en la M3 (no verificado).
    Power(u16),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Digits {
    Distance,
    Mode,
    Power,
}

/// Intérprete del flujo. Acepta la carga de los paquetes tal cual (con el relleno `F`).
#[derive(Debug, Clone)]
pub struct Decoder {
    /// Posición actual del cabezal (mils).
    pub x: i64,
    pub y: i64,
    /// El láser está encendido ahora.
    pub laser: bool,
    /// Último código de velocidad recibido.
    pub speed: Option<ParsedSpeed>,
    compact: bool,
    finished: bool,
    paused: bool,
    left: bool,
    top: bool,
    x_on: bool,
    y_on: bool,
    horizontal_major: bool,
    dist_x: i64,
    dist_y: i64,
    small_jump: bool,
    digits: String,
    digit_mode: Digits,
    mode: Option<u8>,
    returning_compact: bool,
    returning_finished: bool,
    raster_step: i64,
    /// Texto de un código de velocidad en curso.
    speed_text: Option<String>,
    c_prefix: bool,
}

impl Default for Decoder {
    fn default() -> Self {
        Decoder {
            x: 0,
            y: 0,
            laser: false,
            speed: None,
            compact: false,
            finished: false,
            paused: false,
            left: false,
            top: false,
            x_on: false,
            y_on: false,
            horizontal_major: false,
            dist_x: 0,
            dist_y: 0,
            small_jump: false,
            digits: String::new(),
            digit_mode: Digits::Distance,
            mode: None,
            returning_compact: true,
            returning_finished: false,
            raster_step: 0,
            speed_text: None,
            c_prefix: false,
        }
    }
}

impl Decoder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Modo compacto activo (la placa está ejecutando un programa).
    pub fn in_program(&self) -> bool {
        self.compact
    }

    /// Después de `FNSE`: ignora todo hasta el próximo `I`.
    pub fn is_finished(&self) -> bool {
        self.finished
    }

    /// Procesa la carga de un paquete (30 bytes) y devuelve los eventos que produce.
    pub fn feed_payload(&mut self, payload: &[u8]) -> Vec<Event> {
        let mut out = Vec::new();
        self.feed(payload, &mut out);
        out
    }

    /// Procesa un flujo de bytes (por ejemplo un `.egv` sin cabecera).
    pub fn feed(&mut self, data: &[u8], out: &mut Vec<Event>) {
        for &b in data {
            self.byte(b, out);
        }
        // Un código de velocidad al final del flujo se da por completo.
        self.end_speed(out);
    }

    fn end_speed(&mut self, out: &mut Vec<Event>) {
        if let Some(text) = self.speed_text.take() {
            let prefix = if self.c_prefix { "C" } else { "" };
            self.c_prefix = false;
            let full = format!("{prefix}{text}");
            match speed::parse_speed_code(&full) {
                Ok(p) => {
                    self.raster_step = match p.raster {
                        RasterStep::None => 0,
                        RasterStep::Bidirectional(g) => g as i64,
                        RasterStep::Unidirectional(_, g) => g as i64,
                    };
                    self.speed = Some(p.clone());
                    out.push(Event::Speed(p));
                }
                Err(_) => out.push(Event::BadSpeed(full)),
            }
        }
    }

    fn append_distance(&mut self, amount: i64) {
        if self.x_on {
            self.dist_x += amount;
        }
        if self.y_on {
            self.dist_y += amount;
        }
    }

    fn execute_distance(&mut self, out: &mut Vec<Event>) {
        if self.dist_x == 0 && self.dist_y == 0 {
            return;
        }
        let dx = if self.left { -self.dist_x } else { self.dist_x };
        let dy = if self.top { -self.dist_y } else { self.dist_y };
        self.dist_x = 0;
        self.dist_y = 0;
        let from = (self.x, self.y);
        self.x += dx;
        self.y += dy;
        out.push(Event::Move {
            from,
            to: (self.x, self.y),
            laser: self.laser,
            mode: if self.compact {
                MoveMode::Program
            } else {
                MoveMode::Rapid
            },
        });
    }

    fn byte(&mut self, b: u8, out: &mut Vec<Event>) {
        // Código de velocidad en curso: dígitos, pasos `G` y una `C` final.
        if self.speed_text.is_some() {
            match b {
                b'0'..=b'9' | b'G' => {
                    if let Some(t) = self.speed_text.as_mut() {
                        t.push(b as char);
                    }
                    return;
                }
                b'C' => {
                    if let Some(t) = self.speed_text.as_mut() {
                        t.push('C');
                    }
                    self.end_speed(out);
                    return;
                }
                _ => self.end_speed(out),
            }
        }
        if b == b'I' {
            self.finished = false;
            self.compact = false;
            self.paused = false;
            self.dist_x = 0;
            self.dist_y = 0;
            self.digits.clear();
            self.digit_mode = Digits::Distance;
            self.c_prefix = false;
            out.push(Event::Init);
            return;
        }
        if self.finished {
            return;
        }
        if b.is_ascii_digit() {
            self.digit(b, out);
            return;
        }
        self.digits.clear();
        self.digit_mode = Digits::Distance;
        if self.compact {
            self.execute_distance(out);
        }
        match b {
            b'|' => {
                self.append_distance(25);
                self.small_jump = true;
            }
            b'a'..=b'y' => {
                self.append_distance((b - b'a') as i64 + 1);
                self.small_jump = false;
            }
            b'z' => {
                self.append_distance(if self.small_jump { 26 } else { 255 });
                self.small_jump = false;
            }
            b'B' => {
                if self.left && self.horizontal_major && self.raster_step != 0 && self.compact {
                    self.dist_y += self.raster_step;
                }
                self.left = false;
                self.x_on = true;
                self.y_on = false;
            }
            b'T' => {
                if !self.left && self.horizontal_major && self.raster_step != 0 && self.compact {
                    self.dist_y += self.raster_step;
                }
                self.left = true;
                self.x_on = true;
                self.y_on = false;
            }
            b'R' => {
                if self.top && !self.horizontal_major && self.raster_step != 0 && self.compact {
                    self.dist_x += self.raster_step;
                }
                self.top = false;
                self.x_on = false;
                self.y_on = true;
            }
            b'L' => {
                if !self.top && !self.horizontal_major && self.raster_step != 0 && self.compact {
                    self.dist_x += self.raster_step;
                }
                self.top = true;
                self.x_on = false;
                self.y_on = true;
            }
            b'U' => self.laser = false,
            b'D' => self.laser = true,
            b'F' => {
                self.returning_compact = false;
                self.returning_finished = true;
            }
            b'@' => {
                self.returning_finished = false;
                self.returning_compact = false;
            }
            b'C' => self.c_prefix = true,
            b'V' => {
                self.raster_step = 0;
                self.speed_text = Some("V".to_string());
            }
            b'S' => {
                self.laser = false;
                self.execute_distance(out);
                self.mode = None;
                self.digit_mode = Digits::Mode;
            }
            b'E' => self.execute_state(out),
            b'P' => {
                self.laser = false;
                if self.paused {
                    // Dos `P` seguidas: secuencia de home.
                    self.x = 0;
                    self.y = 0;
                    self.dist_x = 0;
                    self.dist_y = 0;
                    self.finished = true;
                    self.paused = false;
                    out.push(Event::Home);
                } else {
                    self.execute_distance(out);
                    self.paused = true;
                    if let Some(m) = self.mode {
                        match m {
                            1 => out.push(Event::Rail { locked: true }),
                            2 => out.push(Event::Rail { locked: false }),
                            _ => {}
                        }
                    }
                }
            }
            b'N' => {
                self.execute_distance(out);
                self.laser = false;
                self.compact = false;
            }
            b'M' => {
                self.x_on = true;
                self.y_on = true;
            }
            b'W' => self.digit_mode = Digits::Power,
            _ => {}
        }
    }

    fn execute_state(&mut self, out: &mut Vec<Event>) {
        match self.mode {
            None => {
                if self.returning_compact {
                    self.compact = true;
                }
                if self.returning_finished {
                    self.finished = true;
                    out.push(Event::Finished);
                }
                if self.horizontal_major {
                    self.left = !self.left;
                    self.x_on = true;
                    self.y_on = false;
                } else {
                    self.top = !self.top;
                    self.x_on = false;
                    self.y_on = true;
                }
            }
            Some(0) => {}
            Some(1) => {
                self.compact = true;
                self.horizontal_major = self.x_on;
            }
            Some(_) => self.compact = true,
        }
        self.returning_finished = false;
        self.returning_compact = true;
        self.laser = false;
    }

    fn digit(&mut self, b: u8, out: &mut Vec<Event>) {
        match self.digit_mode {
            Digits::Mode => {
                self.mode = Some(b - b'0');
                self.digits.clear();
                self.digit_mode = Digits::Distance;
            }
            Digits::Power => {
                self.digits.push(b as char);
                if self.digits.len() >= 3 {
                    let p: u16 = self.digits.parse().unwrap_or(0);
                    out.push(Event::Power(if p == 0 { 1000 } else { p }));
                    self.digits.clear();
                    self.digit_mode = Digits::Distance;
                }
            }
            Digits::Distance => {
                self.digits.push(b as char);
                if self.digits.len() >= 3 {
                    let v: i64 = self.digits.parse().unwrap_or(0);
                    self.digits.clear();
                    self.append_distance(v);
                }
            }
        }
    }
}

/// Decodifica un flujo completo y devuelve todos los eventos.
pub fn decode_stream(stream: &[u8]) -> Vec<Event> {
    let mut d = Decoder::new();
    let mut out = Vec::new();
    d.feed(stream, &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    type MoveTuple = ((i64, i64), (i64, i64), bool, MoveMode);

    fn moves(events: &[Event]) -> Vec<MoveTuple> {
        events
            .iter()
            .filter_map(|e| match e {
                Event::Move {
                    from,
                    to,
                    laser,
                    mode,
                } => Some((*from, *to, *laser, *mode)),
                _ => None,
            })
            .collect()
    }

    /// Flujo del test `egv_rect` de MeerK40t (`test/test_drivers_lihuiyu.py`, MIT): un rectángulo
    /// de 1 cm en (2 cm, 2 cm) a 15 mm/s.
    const EGV_RECT: &[u8] =
        b"IBzzzvRzzzvS1P\nICV2490731016000027CNLBS1EDz139Rz139Tz139Lz139FNSE-\n";

    #[test]
    fn meerk40t_rectangle_decodes_to_a_square_with_the_laser_on() {
        let ev = decode_stream(EGV_RECT);
        let m = moves(&ev);
        // Primero el salto rápido de (0,0) a (787,787) con el láser apagado.
        assert_eq!(m[0], ((0, 0), (787, 787), false, MoveMode::Rapid));
        // Después los cuatro lados de 394 mil con el láser encendido.
        let sides: Vec<_> = m[1..].to_vec();
        assert_eq!(sides.len(), 4);
        let pts = [
            (787, 787),
            (1181, 787),
            (1181, 1181),
            (787, 1181),
            (787, 787),
        ];
        for (i, s) in sides.iter().enumerate() {
            assert_eq!((s.0, s.1), (pts[i], pts[i + 1]), "lado {i}");
            assert!(s.2, "láser encendido en el lado {i}");
            assert_eq!(s.3, MoveMode::Program);
        }
        assert!(ev.contains(&Event::Finished));
        let speed = ev.iter().find_map(|e| match e {
            Event::Speed(p) => Some(p.clone()),
            _ => None,
        });
        let p = speed.expect("código de velocidad");
        assert!(p.suffix_c && p.accel == 1 && p.step_value == 16);
    }

    #[test]
    fn distance_alphabet() {
        // a=1, y=25, |a=26, |z=51, z=255, tres dígitos literales.
        for (text, want) in [
            ("a", 1),
            ("y", 25),
            ("|a", 26),
            ("|z", 51),
            ("z", 255),
            ("zzzv", 787),
            ("z139", 394),
            ("052", 52),
            ("zz001", 511),
        ] {
            let mut d = Decoder::new();
            let mut out = Vec::new();
            d.feed(format!("IB{text}S1P").as_bytes(), &mut out);
            assert_eq!(d.x, want, "{text}");
        }
    }

    #[test]
    fn direction_letters_set_signs() {
        let mut d = Decoder::new();
        let mut out = Vec::new();
        d.feed(b"ITzzRaS1P", &mut out);
        assert_eq!((d.x, d.y), (-510, 1));
        d.feed(b"ILbBcS1P", &mut out);
        // T queda activa hasta B: L(2) -> y -= 2 ... B(3) -> x += 3
        assert_eq!((d.x, d.y), (-507, -1));
    }

    #[test]
    fn diagonal_moves_both_axes() {
        let mut d = Decoder::new();
        let mut out = Vec::new();
        d.feed(b"ICV2490731016000027CNRBS1E", &mut out);
        out.clear();
        d.feed(b"DTLMeFNSE-", &mut out);
        let m = moves(&out);
        assert_eq!(m.len(), 1);
        assert_eq!(m[0].1, (-5, -5));
        assert!(m[0].2);
    }

    #[test]
    fn home_sequence_is_two_p_in_a_packet() {
        let mut d = Decoder::new();
        let mut out = Vec::new();
        d.feed(b"IBzzzRzzzS1P", &mut out);
        assert_eq!((d.x, d.y), (765, 765));
        out.clear();
        d.feed(b"IPP", &mut out);
        assert!(out.contains(&Event::Home));
        assert_eq!((d.x, d.y), (0, 0));
    }

    #[test]
    fn finished_state_ignores_everything_until_the_next_i() {
        let mut d = Decoder::new();
        let mut out = Vec::new();
        d.feed(b"ICV2490731016000027CNLBS1EDaFNSE", &mut out);
        assert!(d.is_finished());
        let n = out.len();
        d.feed(b"BzzzS1P", &mut out);
        assert_eq!(out.len(), n, "tras FNSE no pasa nada");
        d.feed(b"IBaS1P", &mut out);
        assert_eq!(d.x, 1 + 1, "tras I vuelve a funcionar");
    }

    #[test]
    fn padding_with_f_after_a_rapid_move_is_harmless() {
        let mut d = Decoder::new();
        let mut payload = [b'F'; 30];
        payload[..10].copy_from_slice(b"IBzzRzzS1P");
        let ev = d.feed_payload(&payload);
        assert_eq!(moves(&ev).len(), 1);
        assert_eq!((d.x, d.y), (510, 510));
    }

    /// Flujo del test `egv_image` de MeerK40t (`test/test_drivers_lihuiyu.py`, MIT): una imagen
    /// pequeña en modo trama con paso 2.
    const EGV_IMAGE: &[u8] = b"IB055R091S1P\nIV1552121G002NLBS1EDnUxTtD|aUvBtD|iUtTtD|mUtBtD|sUrTtD|uUtBtD|yUtTtD054UrBtD054UtTtD058UrBtD058UrTtD058UtBtD062UrTtD062UrBtD062UrTtD062UrBtD062UrTtD062UrBtD062UrTvD058UrBtD058UrTtD058UrBvD054UrTtD054UrBvD|yUrTvD|uUrBvD|qUrTvD|mUrBvD|iUrTxD|aUrB|aDlFNSE-\n";

    #[test]
    fn meerk40t_raster_image_decodes_with_the_g_step() {
        let ev = decode_stream(EGV_IMAGE);
        assert_eq!(
            ev.iter()
                .filter(|e| matches!(e, Event::BadSpeed(_)))
                .count(),
            0
        );
        let sp = ev.iter().find_map(|e| match e {
            Event::Speed(p) => Some(p.clone()),
            _ => None,
        });
        let sp = sp.expect("velocidad");
        assert_eq!(sp.raster, crate::speed::RasterStep::Bidirectional(2));
        assert!(ev.contains(&Event::Finished));
        let m = moves(&ev);
        // Primero el salto a (55, 91) mil.
        assert_eq!(m[0].1, (55, 91));
        // La trama quema en pasadas horizontales que avanzan 2 mil en Y al invertir el sentido.
        let burn: Vec<_> = m.iter().filter(|x| x.2).collect();
        assert!(burn.len() > 20, "{} tramos quemados", burn.len());
        let ys: std::collections::BTreeSet<i64> = m.iter().map(|x| x.1.1).collect();
        assert!(ys.len() > 5, "avanza en Y: {ys:?}");
        for w in ys.iter().collect::<Vec<_>>().windows(2) {
            assert_eq!(w[1] - w[0], 2, "paso de trama de 2 mil: {ys:?}");
        }
        // Toda la imagen cabe en unos 150 mil.
        for x in &m {
            assert!(
                x.1.0 >= 0 && x.1.0 < 300 && x.1.1 >= 20 && x.1.1 <= 95,
                "{x:?}"
            );
        }
    }

    #[test]
    fn raster_step_adds_distance_when_the_direction_reverses() {
        // Trama horizontal con paso 2: al pasar de T a B se avanza 2 mil en Y.
        let mut d = Decoder::new();
        let mut out = Vec::new();
        d.feed(b"IV1552121G002NLBS1E", &mut out);
        assert!(d.in_program());
        out.clear();
        // Derecha 5 con láser; `T` invierte (suma el paso en Y, hacia arriba por la `L`) y
        // vuelve 5 a la izquierda.
        d.feed(b"DeUTeFNSE", &mut out);
        let m = moves(&out);
        assert_eq!(m.len(), 3, "{m:?}");
        assert_eq!(m[0].1, (5, 0));
        assert_eq!(m[1].1, (5, -2), "el paso de trama se sumó en Y: {m:?}");
        assert_eq!(m[2].1, (0, -2));
    }
}
