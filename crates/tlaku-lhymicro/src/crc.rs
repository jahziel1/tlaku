//! CRC de 8 bits de los paquetes y su forma en el cable (CH341 en modo EPP).
//!
//! Un paquete lógico mide **32 bytes**: `0x00`, 30 bytes de carga útil y el CRC. El CRC cubre solo
//! los 30 bytes de carga (el `0x00` inicial no cambia el resultado con valor inicial 0). En el
//! cable el CH341 parte el paquete en bloques de hasta 31 bytes de datos, cada uno precedido de
//! `0xA6` (escritura de datos EPP): `A6 00 <30 bytes>` y `A6 <crc>`, **34 bytes** en total.
//! La documentación de EduTech describe 30, 32 o 34 bytes según el texto; las tres cifras son
//! el mismo paquete visto en distintas capas, y el código de MeerK40t (MIT, `lihuiyu/controller.py`
//! y `ch341/libusb.py`) confirma 30 de carga, 32 lógicos y 34 en el cable.
//!
//! El CRC es el CRC-8 de Dallas/Maxim (polinomio `x^8 + x^5 + x^4 + 1`, reflejado `0x8C`), el
//! mismo de los dispositivos 1-Wire. MeerK40t lo calcula con una tabla de 32 entradas (dos
//! nibbles) de Arjen Lentz (BSD-2-Clause, <https://lentz.com.au/blog/calculating-crc-with-a-tiny-32-entry-lookup-table>);
//! aquí las tablas se generan en tiempo de compilación a partir del polinomio y una prueba las
//! compara con las 32 constantes de MeerK40t.

/// Bytes de carga útil de un paquete.
pub const PAYLOAD_LEN: usize = 30;
/// Paquete lógico: `0x00`, la carga y el CRC.
pub const LOGICAL_LEN: usize = 32;
/// Paquete tal como viaja por el endpoint de escritura (con los `0xA6` del CH341).
pub const WIRE_LEN: usize = 34;
/// Comando EPP del CH341: escritura de datos; va delante de cada bloque de hasta 31 bytes.
pub const EPP_WRITE: u8 = 0xA6;
/// Comando EPP del CH341: consulta de estado; la respuesta son 6 bytes por el endpoint de lectura.
pub const EPP_STATUS: u8 = 0xA0;
/// Datos por bloque de escritura del CH341 (los paquetes USB del chip son de 32 bytes).
const EPP_BLOCK: usize = 31;
/// Polinomio reflejado del CRC-8 de Dallas/Maxim.
const POLY: u8 = 0x8C;

const fn crc_byte(mut c: u8) -> u8 {
    let mut i = 0;
    while i < 8 {
        c = if c & 1 != 0 { (c >> 1) ^ POLY } else { c >> 1 };
        i += 1;
    }
    c
}

const fn nibble_table(shift: u32) -> [u8; 16] {
    let mut t = [0u8; 16];
    let mut i = 0;
    while i < 16 {
        t[i] = crc_byte((i as u8) << shift);
        i += 1;
    }
    t
}

/// Tabla de los 4 bits bajos.
const LOW: [u8; 16] = nibble_table(0);
/// Tabla de los 4 bits altos.
const HIGH: [u8; 16] = nibble_table(4);

/// CRC-8 (Dallas/Maxim, valor inicial 0) de `data`.
pub fn crc8(data: &[u8]) -> u8 {
    let mut crc = 0u8;
    for &b in data {
        crc ^= b;
        crc = LOW[(crc & 0x0F) as usize] ^ HIGH[(crc >> 4) as usize];
    }
    crc
}

/// Paquete lógico de 32 bytes: `0x00`, la carga y su CRC.
pub fn logical_packet(payload: &[u8; PAYLOAD_LEN]) -> [u8; LOGICAL_LEN] {
    let mut out = [0u8; LOGICAL_LEN];
    out[1..=PAYLOAD_LEN].copy_from_slice(payload);
    out[LOGICAL_LEN - 1] = crc8(payload);
    out
}

/// Bytes de escritura del CH341 para `data`: bloques de hasta 31 bytes, cada uno con `0xA6` delante.
pub fn epp_write_wire(data: &[u8]) -> Vec<u8> {
    let blocks = data.len().div_ceil(EPP_BLOCK);
    let mut out = Vec::with_capacity(data.len() + blocks);
    for chunk in data.chunks(EPP_BLOCK) {
        out.push(EPP_WRITE);
        out.extend_from_slice(chunk);
    }
    out
}

/// Un paquete completo tal como se escribe en el endpoint `0x02`: 34 bytes.
pub fn wire_packet(payload: &[u8; PAYLOAD_LEN]) -> Vec<u8> {
    epp_write_wire(&logical_packet(payload))
}

/// Resultado de comprobar un paquete lógico recibido (lo usa el emulador).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PacketCheck {
    Ok,
    /// El CRC no coincide con la carga.
    BadCrc {
        expected: u8,
        found: u8,
    },
    /// El primer byte no es `0x00`.
    BadHeader(u8),
}

/// Comprueba un paquete lógico de 32 bytes.
pub fn check_logical(packet: &[u8; LOGICAL_LEN]) -> PacketCheck {
    if packet[0] != 0 {
        return PacketCheck::BadHeader(packet[0]);
    }
    let expected = crc8(&packet[1..=PAYLOAD_LEN]);
    let found = packet[LOGICAL_LEN - 1];
    if expected == found {
        PacketCheck::Ok
    } else {
        PacketCheck::BadCrc { expected, found }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tabla de 32 entradas de MeerK40t (`onewire_crc_lookup`, Arjen Lentz, BSD-2-Clause):
    /// 16 de los nibbles bajos y 16 de los altos.
    const MEERK40T_TABLE: [u8; 32] = [
        0x00, 0x5E, 0xBC, 0xE2, 0x61, 0x3F, 0xDD, 0x83, 0xC2, 0x9C, 0x7E, 0x20, 0xA3, 0xFD, 0x1F,
        0x41, 0x00, 0x9D, 0x23, 0xBE, 0x46, 0xDB, 0x65, 0xF8, 0x8C, 0x11, 0xAF, 0x32, 0xCA, 0x57,
        0xE9, 0x74,
    ];

    /// CRC-8 bit a bit, sin tablas: referencia independiente.
    fn crc8_bitwise(data: &[u8]) -> u8 {
        let mut crc = 0u8;
        for &b in data {
            crc ^= b;
            for _ in 0..8 {
                crc = if crc & 1 != 0 {
                    (crc >> 1) ^ 0x8C
                } else {
                    crc >> 1
                };
            }
        }
        crc
    }

    #[test]
    fn generated_tables_equal_the_meerk40t_constants() {
        assert_eq!(LOW[..], MEERK40T_TABLE[..16]);
        assert_eq!(HIGH[..], MEERK40T_TABLE[16..]);
    }

    #[test]
    fn table_crc_matches_the_bitwise_one() {
        let mut data = Vec::new();
        for n in 0..=64u32 {
            data.clear();
            data.extend((0..n).map(|i| (i * 37 + n * 11) as u8));
            assert_eq!(crc8(&data), crc8_bitwise(&data), "longitud {n}");
        }
    }

    #[test]
    fn dallas_check_value() {
        // Valor de comprobación del CRC-8/MAXIM-DOW: "123456789" -> 0xA1.
        assert_eq!(crc8(b"123456789"), 0xA1);
        assert_eq!(crc8(&[]), 0);
    }

    #[test]
    fn logical_packet_is_32_bytes_with_leading_zero_and_trailing_crc() {
        let mut payload = [b'F'; PAYLOAD_LEN];
        payload[..4].copy_from_slice(b"IBzz");
        let p = logical_packet(&payload);
        assert_eq!(p.len(), 32);
        assert_eq!(p[0], 0);
        assert_eq!(&p[1..31], &payload);
        assert_eq!(p[31], crc8(&payload));
        assert_eq!(check_logical(&p), PacketCheck::Ok);
    }

    #[test]
    fn wire_packet_is_34_bytes_with_two_a6_prefixes() {
        let payload = [b'F'; PAYLOAD_LEN];
        let w = wire_packet(&payload);
        assert_eq!(w.len(), WIRE_LEN);
        assert_eq!(w[0], EPP_WRITE);
        assert_eq!(w[1], 0x00);
        assert_eq!(&w[2..32], &payload);
        assert_eq!(w[32], EPP_WRITE);
        assert_eq!(w[33], crc8(&payload));
    }

    #[test]
    fn epp_blocks_hold_at_most_31_data_bytes() {
        assert_eq!(epp_write_wire(&[]), Vec::<u8>::new());
        assert_eq!(epp_write_wire(&[1, 2, 3]), [0xA6, 1, 2, 3]);
        let w = epp_write_wire(&[7; 62]);
        assert_eq!(w.len(), 64);
        assert_eq!((w[0], w[32]), (0xA6, 0xA6));
        let w = epp_write_wire(&[7; 63]);
        assert_eq!(w.len(), 66, "63 bytes son tres bloques");
        assert_eq!(w[64], 0xA6);
    }

    #[test]
    fn corrupted_packets_are_detected() {
        let payload = [b'x'; PAYLOAD_LEN];
        let mut p = logical_packet(&payload);
        p[5] ^= 0x01;
        assert!(matches!(check_logical(&p), PacketCheck::BadCrc { .. }));
        let mut p = logical_packet(&payload);
        p[0] = 1;
        assert_eq!(check_logical(&p), PacketCheck::BadHeader(1));
    }
}
