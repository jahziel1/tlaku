//! Documentos de la memoria de la controladora: borrar, ejecutar y buscar por nombre (M23, M24).
//!
//! **EXPERIMENTAL: sin probar con una controladora real.** Las fuentes públicas no coinciden en
//! la forma de estas órdenes (ver `docs/hardware/ruida.md` sección 16):
//!
//! - `E8 00` borrar: MeerK40t (`emulator.py`, MIT) lo lee como dos números de 14 bits tras el
//!   subcódigo (`E8 00 n(2) m(2)`, el primero es el número del documento y 0 borra todos);
//!   EduTech y ruida-pa lo describen con dos valores de 35 bits sin decir qué significan.
//! - `E8 03` seleccionar/ejecutar: MeerK40t lo lee con un número de 14 bits (`E8 03 n(2)`),
//!   como `E8 01`; EduTech y ruida-pa dicen un solo byte (`E8 03 n`).
//!
//! Tlaku usa la forma de MeerK40t porque es la única que dice qué significa cada campo. Como un
//! índice mal interpretado borraría o ejecutaría otro documento, **antes de cada operación se
//! lee el nombre del documento n y debe ser el que el usuario confirmó**, y nunca se envía el
//! "borrar todos" (`n = 0`).

use super::*;
use crate::link::PacketLink;
use crate::packet;
use std::time::Duration;

/// Longitud máxima del nombre de un documento que pone Tlaku (los de RDWorks y los que genera
/// Tlaku, `TLAKUnnn`, tienen 8 caracteres).
pub const MAX_DOC_NAME: usize = 8;

/// ¿Es un nombre que Tlaku acepta para un documento? 1 a 8 caracteres entre letras ASCII,
/// cifras, `_` y `-`.
pub fn valid_doc_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= MAX_DOC_NAME
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

/// Resultado de borrar un documento.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeleteReport {
    pub index: u32,
    pub name: String,
    pub count_before: u32,
    /// Documentos tras borrar; `None` si no contestó.
    pub count_after: Option<u32>,
}

impl DeleteReport {
    /// ¿La cuenta bajó en uno? Si no, la orden no tuvo el efecto esperado.
    pub fn verified(&self) -> bool {
        self.count_after == Some(self.count_before.saturating_sub(1))
    }
}

/// Resultado de pedir la ejecución de un documento.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunReport {
    pub index: u32,
    pub name: String,
    /// La máquina pasó a trabajo en marcha o movimiento tras la orden.
    pub started: bool,
}

fn same_name(a: &str, b: &str) -> bool {
    a.eq_ignore_ascii_case(b)
}

impl<L: PacketLink> RuidaSession<L> {
    /// Número del documento con ese nombre (sin distinguir mayúsculas), si existe. Solo lectura.
    pub fn find_document(&mut self, name: &str) -> Result<Option<u32>, SessionError> {
        Ok(self
            .list_documents()?
            .into_iter()
            .find(|f| f.name.as_deref().is_some_and(|n| same_name(n, name)))
            .map(|f| f.index))
    }

    /// Antes de guardar un documento con `name`: si ya existe, o se falla con
    /// [`SessionError::NameExists`] o, con `overwrite`, se borra el anterior (con las
    /// comprobaciones de [`Self::delete_document`]). Devuelve el número del documento reemplazado.
    pub fn prepare_store(
        &mut self,
        name: &str,
        overwrite: bool,
    ) -> Result<Option<u32>, SessionError> {
        if !valid_doc_name(name) {
            return Err(SessionError::Protocol(tr!("ruida.session.bad_doc_name")));
        }
        let Some(index) = self.find_document(name)? else {
            return Ok(None);
        };
        if !overwrite {
            return Err(SessionError::NameExists {
                name: name.to_string(),
                index,
            });
        }
        self.delete_document(index, name, true)?;
        Ok(Some(index))
    }

    /// Lee el nombre del documento `index` y comprueba que es `expected`. Devuelve la cuenta.
    fn check_document(&mut self, index: u32, expected: &str) -> Result<u32, SessionError> {
        if index == 0 || index > MAX_LISTED_DOCS {
            return Err(SessionError::NoSuchDocument { index });
        }
        let count = u32::try_from(self.read_mem(addr::DOC_COUNT)?).unwrap_or(u32::MAX);
        if index > count {
            return Err(SessionError::NoSuchDocument { index });
        }
        let found = self.read_document_name(index).ok();
        match found {
            Some(n) if same_name(&n, expected) => Ok(count),
            other => Err(SessionError::DocumentMismatch {
                index,
                expected: expected.to_string(),
                found: other,
            }),
        }
    }

    /// Borra el documento `index` (1 en adelante) si su nombre es `expected_name`. Exige
    /// `confirmed`, la máquina en reposo y comprueba al final que la cuenta bajó en uno. Nunca
    /// envía el "borrar todos".
    pub fn delete_document(
        &mut self,
        index: u32,
        expected_name: &str,
        confirmed: bool,
    ) -> Result<DeleteReport, SessionError> {
        if !confirmed {
            return Err(SessionError::NotConfirmed);
        }
        self.ensure_idle()?;
        let count_before = self.check_document(index, expected_name)?;
        let n = idx14(index);
        // `E8 00 n(2) 00 00`: el segundo número (sin significado conocido) va en 0.
        self.raw_command(&[0xE8, 0x00, n[0], n[1], 0x00, 0x00], self.cfg.ack_timeout)?;
        let count_after = self
            .read_mem(addr::DOC_COUNT)
            .ok()
            .map(|c| u32::try_from(c).unwrap_or(u32::MAX));
        Ok(DeleteReport {
            index,
            name: expected_name.to_string(),
            count_before,
            count_after,
        })
    }

    /// Pide a la controladora que ejecute el documento `index` (`E8 03`) si su nombre es
    /// `expected_name`. Exige `confirmed` y la máquina en reposo; **quien llama debe haber pasado
    /// antes la lista de comprobación de seguridad** (la interfaz lo hace igual que con Iniciar).
    /// Después observa unos segundos si la máquina empezó.
    pub fn run_document(
        &mut self,
        index: u32,
        expected_name: &str,
        confirmed: bool,
    ) -> Result<RunReport, SessionError> {
        if !confirmed {
            return Err(SessionError::NotConfirmed);
        }
        self.ensure_idle()?;
        self.check_document(index, expected_name)?;
        let n = idx14(index);
        self.raw_command(&[0xE8, 0x03, n[0], n[1]], self.cfg.ack_timeout)?;
        let mut started = false;
        for _ in 0..10 {
            if let Ok(st) = self.status()
                && (st.running || st.moving)
            {
                started = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        Ok(RunReport {
            index,
            name: expected_name.to_string(),
            started,
        })
    }

    /// Envía un mensaje en claro (un paquete) y espera su ACK.
    fn raw_command(&mut self, body: &[u8], timeout: Duration) -> Result<(), SessionError> {
        let d = packet::build_packet(body, &self.pcfg());
        self.link.drain()?;
        let (mut stash, mut resends) = (Vec::new(), 0);
        self.send_acked(&d, 0, timeout, false, &mut stash, &mut resends)
    }
}

/// Número de documento en dos bytes de 7 bits (como `E8 01`).
fn idx14(index: u32) -> [u8; 2] {
    [((index >> 7) & 0x7F) as u8, (index & 0x7F) as u8]
}
