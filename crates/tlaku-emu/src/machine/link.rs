//! Enlace: escritura de la salida, búfer RX y recepción de bytes.

use crate::profile::{Family, Protocol};

use super::*;

impl Machine {
    // -----------------------------------------------------------------------
    // Enlace
    // -----------------------------------------------------------------------

    pub(super) fn emit_raw(&mut self, b: &[u8]) {
        if !self.connected {
            return;
        }
        if let Some((until, drop)) = self.silence
            && self.now < until
        {
            if !drop {
                self.held.extend_from_slice(b);
            }
            return;
        }
        self.out.extend(b.iter().copied());
    }

    pub(super) fn emit_line(&mut self, s: &str) {
        let mut v = s.as_bytes().to_vec();
        v.extend_from_slice(self.line_ending.bytes());
        self.emit_raw(&v);
    }

    pub(super) fn rx_capacity(&self) -> usize {
        self.profile.rx_size.saturating_sub(1)
    }

    fn is_realtime(&self, b: u8) -> bool {
        match self.profile.protocol {
            Protocol::Grbl => matches!(b, b'?' | b'!' | b'~' | 0x18) || b >= 0x80,
            // Smoothie atiende `?` al llegar; Marlin no tiene tiempo real.
            Protocol::PingPong => self.profile.family == Family::Smoothie && b == b'?',
        }
    }

    /// Entrega a la controladora los bytes que envía el anfitrión. Los de tiempo real se
    /// atienden al llegar; el resto pasa por el búfer RX y se pierde (contado en
    /// [`rx_overflow_bytes`](Self::rx_overflow_bytes)) si no cabe.
    pub fn receive(&mut self, data: &[u8]) {
        if !self.connected {
            return;
        }
        for &b in data {
            if self.is_realtime(b) {
                self.pump();
                self.realtime(b);
                continue;
            }
            if self.profile.emergency_parser {
                self.emergency_scan(b);
            }
            if self.rx.len() >= self.rx_capacity() {
                // El intérprete va tan rápido como llegan los bytes salvo que esté bloqueado.
                self.pump();
            }
            if self.rx.len() >= self.rx_capacity() {
                self.overflow_bytes += 1;
                continue;
            }
            self.rx.push_back(b);
            self.in_flight += 1;
            self.max_in_flight = self.max_in_flight.max(self.in_flight);
            if b == b'\n' || b == b'\r' {
                self.pump();
            }
        }
        self.pump();
    }
}
