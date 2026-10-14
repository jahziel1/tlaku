//! Comandos en tiempo real de GRBL y el plan de overrides.

use tlaku_doc::OverrideKind;

/// Comandos en tiempo real de GRBL 1.1: un solo byte, sin `\n` y fuera del conteo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Realtime {
    StatusQuery,
    FeedHold,
    CycleStart,
    SoftReset,
    SafetyDoor,
    JogCancel,
    FeedReset,
    FeedPlus10,
    FeedMinus10,
    FeedPlus1,
    FeedMinus1,
    RapidFull,
    RapidHalf,
    RapidQuarter,
    SpindleReset,
    SpindlePlus10,
    SpindleMinus10,
    SpindlePlus1,
    SpindleMinus1,
    SpindleStop,
    ToggleFlood,
    ToggleMist,
}

impl Realtime {
    pub const fn byte(self) -> u8 {
        match self {
            Self::StatusQuery => b'?',
            Self::FeedHold => b'!',
            Self::CycleStart => b'~',
            Self::SoftReset => 0x18,
            Self::SafetyDoor => 0x84,
            Self::JogCancel => 0x85,
            Self::FeedReset => 0x90,
            Self::FeedPlus10 => 0x91,
            Self::FeedMinus10 => 0x92,
            Self::FeedPlus1 => 0x93,
            Self::FeedMinus1 => 0x94,
            Self::RapidFull => 0x95,
            Self::RapidHalf => 0x96,
            Self::RapidQuarter => 0x97,
            Self::SpindleReset => 0x99,
            Self::SpindlePlus10 => 0x9A,
            Self::SpindleMinus10 => 0x9B,
            Self::SpindlePlus1 => 0x9C,
            Self::SpindleMinus1 => 0x9D,
            Self::SpindleStop => 0x9E,
            Self::ToggleFlood => 0xA0,
            Self::ToggleMist => 0xA1,
        }
    }
}

// ---------------------------------------------------------------------------
// Overrides en tiempo real (velocidad y potencia)
// ---------------------------------------------------------------------------

/// Límites de los overrides de GRBL, en porcentaje.
pub const OVERRIDE_MIN: u32 = 10;
pub const OVERRIDE_MAX: u32 = 200;

/// Bytes en tiempo real que llevan el override de `kind` de `current` a `wanted` por ciento.
/// GRBL solo sabe subir o bajar de 10 en 10 y de 1 en 1, o volver al 100 %. `current` es el
/// último valor que informó `Ov:`; sin él se parte de un reinicio al 100 %, así el resultado no
/// depende de lo que hubiera antes. `wanted` se acota a 10..=200. Nunca devuelve un plan vacío:
/// aunque el valor visto ya sea el pedido se reinicia y se reajusta, porque ese valor puede ser de
/// un reporte atrasado.
pub fn override_plan(kind: OverrideKind, current: Option<u32>, wanted: u32) -> Vec<Realtime> {
    let wanted = wanted.clamp(OVERRIDE_MIN, OVERRIDE_MAX);
    let (reset, plus10, minus10, plus1, minus1) = match kind {
        OverrideKind::Feed => (
            Realtime::FeedReset,
            Realtime::FeedPlus10,
            Realtime::FeedMinus10,
            Realtime::FeedPlus1,
            Realtime::FeedMinus1,
        ),
        OverrideKind::Spindle => (
            Realtime::SpindleReset,
            Realtime::SpindlePlus10,
            Realtime::SpindleMinus10,
            Realtime::SpindlePlus1,
            Realtime::SpindleMinus1,
        ),
    };
    let current = current.map(|c| c.clamp(OVERRIDE_MIN, OVERRIDE_MAX));
    let mut plan = Vec::new();
    let base = match current {
        // Si ya está en el valor pedido no se envía nada: el `Ov:` visto puede ser un reporte
        // atrasado, así que se reinicia y se vuelve a ajustar, que cuesta pocos bytes.
        Some(c) if wanted != 100 && c != wanted => c,
        _ => {
            plan.push(reset);
            100
        }
    };
    let (tens, ones, up) = (
        wanted.abs_diff(base) / 10,
        wanted.abs_diff(base) % 10,
        wanted > base,
    );
    let (big, small) = if up {
        (plus10, plus1)
    } else {
        (minus10, minus1)
    };
    plan.extend(std::iter::repeat_n(big, tens as usize));
    plan.extend(std::iter::repeat_n(small, ones as usize));
    plan
}
