//! Ofuscación por byte (swizzle) de los datos Ruida. No es seguridad: es una biyección sin
//! estado entre bytes. Fuentes: ruida.md sección 2 (EduTech Wiki, FabLab Nürnberg); el algoritmo
//! coincide con MeerK40t (MIT) y ruida-pa (MIT), y la tabla de magics de `probe` con la prueba
//! `test_magic_keys` de MeerK40t.
//!
//! Al enviar, para cada byte:
//! 1. se intercambian los bits 7 y 0;
//! 2. `b ^= magic`;
//! 3. `b = (b + 1) mod 256`.
//!
//! Al recibir se hace lo inverso: restar 1, `XOR magic` e intercambiar los bits 7 y 0.

/// Magic por defecto (644XG, 644XS, 320, 633X, 654XG y todo ID de tarjeta no listado).
pub const MAGIC_DEFAULT: u8 = 0x88;
/// 634XG. EduTech habla de "altered scrambling" sin detallar; MeerK40t (MIT) solo le atribuye
/// este magic y ruida-pa no lo menciona: se asume que no cambia nada más (sin hardware).
pub const MAGIC_634XG: u8 = 0x11;
/// RDL9635.
pub const MAGIC_RDL9635: u8 = 0x38;

/// Clave `magic` del XOR. Depende del modelo o del ID de tarjeta.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Magic(pub u8);

impl Default for Magic {
    fn default() -> Self {
        Magic(MAGIC_DEFAULT)
    }
}

/// Intercambia el bit 7 y el bit 0.
fn swap_ends(b: u8) -> u8 {
    (b & 0x7E) | ((b & 0x01) << 7) | (b >> 7)
}

impl Magic {
    /// Un byte del flujo en claro al byte del cable.
    pub fn swizzle(self, b: u8) -> u8 {
        (swap_ends(b) ^ self.0).wrapping_add(1)
    }

    /// Inversa de [`Magic::swizzle`].
    pub fn unswizzle(self, b: u8) -> u8 {
        swap_ends(b.wrapping_sub(1) ^ self.0)
    }

    pub fn swizzle_all(self, data: &[u8]) -> Vec<u8> {
        data.iter().map(|&b| self.swizzle(b)).collect()
    }

    pub fn unswizzle_all(self, data: &[u8]) -> Vec<u8> {
        data.iter().map(|&b| self.unswizzle(b)).collect()
    }

    /// Magic de un modelo por su nombre (sin distinguir mayúsculas), según ruida.md sección 2.
    /// `None` si el nombre no está en la tabla (se debería usar el de la tarjeta o el defecto).
    pub fn for_model(name: &str) -> Option<Magic> {
        let n = name.trim().to_ascii_uppercase();
        let m = match n.as_str() {
            "644XG" | "644XS" | "320" | "633X" | "654XG" => MAGIC_DEFAULT,
            "634XG" => MAGIC_634XG,
            "RDL9635" => MAGIC_RDL9635,
            _ => return None,
        };
        Some(Magic(m))
    }

    /// Magic que usó la controladora a partir de **su byte de ACK en el cable**. La respuesta
    /// `CC` va swizzleada con el magic propio de la máquina, así que `magic = (b - 1) ^ swap(CC)`.
    /// Como el checksum del paquete se calcula sobre los bytes del cable, la máquina acepta el
    /// paquete aunque el magic de quien envía sea otro: un solo sondeo revela el magic real.
    /// (Deducción propia; coincide con `C6` para 0x88. La máquina real no se ha probado.)
    pub fn from_wire_ack(wire: u8) -> Magic {
        Magic(wire.wrapping_sub(1) ^ swap_ends(0xCC))
    }

    /// Magic para la respuesta de 32 bits de la tarjeta (`DA 00 05 7E`). Las tarjetas reales
    /// repiten el ID de 16 bits en las dos mitades (RDC6442S: `0x65106510`, RDC8445S:
    /// `0x90109010`, ruida-pa y MeerK40t); la tabla de EduTech usa el ID de 16 bits.
    pub fn for_card_reply(reply: u32) -> Magic {
        let (hi, lo) = (reply >> 16, reply & 0xFFFF);
        Magic::for_card_id(if hi == lo { hi } else { reply })
    }

    /// Magic de un ID de tarjeta de 16 bits (la mitad de lo que se lee de la dirección 0x2FE).
    /// Todo ID que no esté en la tabla usa 0x88. La tabla viene de una sola fuente (EduTech):
    /// confianza media.
    pub fn for_card_id(id: u32) -> Magic {
        let m = match id {
            0x6320 => 0x11,
            0x6302 => 0x83,
            0x6301 => 0x88,
            0x6300 | 0x630B => 0x33,
            0x2210 => 0x16,
            0x6501 => 0x34,
            0x6511 | 0x6811 => 0x77,
            0x6512 => 0x75,
            0x6513 => 0x99,
            0x6514 => 0x13,
            0x651E => 0x60,
            0x651F => 0x76,
            0x6530 | 0x6532 => 0x10,
            0x6920 | 0x690F..=0x6911 => 0xA1,
            0x6912 => 0x88,
            0x6930 | 0x693F => 0x1A,
            0x7230 | 0x7235 | 0x7236 | 0x7430 | 0x7930 => 0x10,
            _ => MAGIC_DEFAULT,
        };
        Magic(m)
    }
}

/// Lista de magics a probar al sondear una máquina desconocida (ruida.md sección 2): primero el
/// defecto, luego los de modelo y por último los distintos de la tabla de tarjetas.
pub fn probe_candidates() -> Vec<Magic> {
    let mut v = vec![
        Magic(MAGIC_DEFAULT),
        Magic(MAGIC_634XG),
        Magic(MAGIC_RDL9635),
    ];
    for m in [
        0x83, 0x33, 0x16, 0x34, 0x77, 0x75, 0x99, 0x13, 0x60, 0x76, 0x10, 0xA1, 0x1A,
    ] {
        if !v.contains(&Magic(m)) {
            v.push(Magic(m));
        }
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    const MAGICS: [u8; 3] = [MAGIC_DEFAULT, MAGIC_634XG, MAGIC_RDL9635];

    #[test]
    fn bijection_for_every_byte_and_known_magic() {
        for m in MAGICS.into_iter().chain(0..=255u8) {
            let magic = Magic(m);
            let mut seen = [false; 256];
            for b in 0..=255u8 {
                let s = magic.swizzle(b);
                assert_eq!(magic.unswizzle(s), b, "magic {m:#x} byte {b:#x}");
                assert!(!seen[s as usize], "colisión con magic {m:#x}");
                seen[s as usize] = true;
            }
        }
    }

    #[test]
    fn independent_cross_check_with_fablab_ack_bytes() {
        // V1/V2: CC y CD swizzleados dan C6 y 46, los bytes de ACK/error de FabLab Nürnberg.
        let m = Magic::default();
        assert_eq!(m.swizzle(0xCC), 0xC6);
        assert_eq!(m.swizzle(0xCD), 0x46);
        // V2b: CF -> 48 (posible NAK de EduTech visto en el cable).
        assert_eq!(m.swizzle(0xCF), 0x48);
        assert_eq!(m.unswizzle(0xC6), 0xCC);
        assert_eq!(m.unswizzle(0x46), 0xCD);
    }

    #[test]
    fn computed_vectors() {
        let m = Magic::default();
        // V7: consulta del ID de tarjeta.
        assert_eq!(
            m.swizzle_all(&[0xDA, 0x00, 0x05, 0x7E]),
            [0xD4, 0x89, 0x0D, 0xF7]
        );
        // V8: cabecera del ejemplo 1.
        let plain = [0xD8, 0x10, 0xF0, 0xE6, 0x01, 0xF1, 0x02, 0x00, 0xD8, 0x00];
        let wire = [0xD2, 0x99, 0xFA, 0xF0, 0x09, 0x7A, 0x8B, 0x89, 0xD2, 0x89];
        assert_eq!(m.swizzle_all(&plain), wire);
        assert_eq!(m.unswizzle_all(&wire), plain);
        // V9: fin de archivo.
        assert_eq!(m.swizzle(0xD7), 0x60);
    }

    #[test]
    fn swap_matches_the_three_xor_formulation_of_edutech() {
        for b in 0..=255u8 {
            let mut x = b;
            x ^= x >> 7;
            x ^= x << 7;
            x ^= x >> 7;
            assert_eq!(swap_ends(b), x);
        }
    }

    #[test]
    fn card_id_table() {
        assert_eq!(Magic::for_card_id(0x6320), Magic(0x11));
        assert_eq!(Magic::for_card_id(0x6301), Magic(0x88));
        assert_eq!(Magic::for_card_id(0x690F), Magic(0xA1));
        assert_eq!(Magic::for_card_id(0x6910), Magic(0xA1));
        assert_eq!(Magic::for_card_id(0x6911), Magic(0xA1));
        assert_eq!(Magic::for_card_id(0x6912), Magic(0x88));
        assert_eq!(Magic::for_card_id(0x693F), Magic(0x1A));
        assert_eq!(Magic::for_card_id(0x7930), Magic(0x10));
        // Lo no listado (incluida la 8445S) usa el defecto.
        assert_eq!(Magic::for_card_id(0x1234), Magic(0x88));
        assert_eq!(Magic::for_card_id(0x9010_9010), Magic(0x88));
        // Respuestas reales de tarjeta: el ID de 16 bits repetido (RDC6442S, RDC8445S).
        assert_eq!(Magic::for_card_reply(0x6510_6510), Magic(0x88));
        assert_eq!(Magic::for_card_reply(0x9010_9010), Magic(0x88));
        assert_eq!(Magic::for_card_reply(0x6320_6320), Magic(0x11));
        assert_eq!(Magic::for_card_reply(0x6512_6512), Magic(0x75));
    }

    #[test]
    fn magic_is_recovered_from_the_wire_ack() {
        for m in 0..=255u8 {
            let wire = Magic(m).swizzle(0xCC);
            assert_eq!(Magic::from_wire_ack(wire), Magic(m));
        }
        assert_eq!(Magic::from_wire_ack(0xC6), Magic(0x88));
    }

    #[test]
    fn meerk40t_magic_key_vectors() {
        // Consulta `DA 00 05 7E` swizzleada con cada magic. Vectores de `test/test_ruida.py` de
        // MeerK40t (https://github.com/meerk40t/meerk40t, licencia MIT, Copyright (c) 2021
        // meerk40t); ver THIRD_PARTY.md.
        let q = [0xDA, 0x00, 0x05, 0x7E];
        for (wire, magic) in [
            ([0xD4, 0x89, 0x0D, 0xF7], 0x88),
            ([0xD9, 0x84, 0x08, 0xFE], 0x83),
            ([0x69, 0x34, 0xB8, 0x4E], 0x33),
            ([0x49, 0x14, 0x98, 0x6E], 0x13),
            ([0x7A, 0x23, 0xA7, 0x5D], 0x22),
            ([0x4B, 0x12, 0x96, 0x70], 0x11),
            ([0x2D, 0x78, 0xF4, 0x0A], 0x77),
            ([0xB6, 0xEF, 0x6B, 0x91], 0xEE),
        ] {
            assert_eq!(Magic(magic).swizzle_all(&q), wire, "magic {magic:#x}");
        }
    }

    #[test]
    fn model_table() {
        assert_eq!(Magic::for_model("644xg"), Some(Magic(0x88)));
        assert_eq!(Magic::for_model("634XG"), Some(Magic(0x11)));
        assert_eq!(Magic::for_model("RDL9635"), Some(Magic(0x38)));
        assert_eq!(Magic::for_model("otra"), None);
    }

    #[test]
    fn probe_list_starts_with_default_and_has_no_duplicates() {
        let c = probe_candidates();
        assert_eq!(c[0], Magic(0x88));
        for (i, m) in c.iter().enumerate() {
            assert!(!c[..i].contains(m));
        }
    }
}
