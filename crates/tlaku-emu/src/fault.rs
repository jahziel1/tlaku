//! Inyección de fallas: situaciones que un cable, una placa o un firmware real producen y que
//! las pruebas del anfitrión deben soportar.

use std::time::Duration;

use crate::profile::LineEnding;

/// Cuándo se activa una falla.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trigger {
    /// En el siguiente avance de la controladora.
    Now,
    /// Al recibir la línea número `n` (desde 1; solo cuentan las líneas no vacías, igual que el
    /// número que ve el anfitrión en el trabajo si no envía líneas vacías ni comentarios).
    Line(usize),
    /// Al llegar el tiempo simulado de la controladora a este instante.
    At(Duration),
}

/// Una falla a inyectar.
#[derive(Debug, Clone, PartialEq)]
pub enum Fault {
    /// Bytes crudos hacia el anfitrión (ruido de línea, p. ej. `b"ab\xff\xfe\r\n"`).
    Noise(Vec<u8>),
    /// La línea que dispara la falla se contesta con `error:N` y no se ejecuta.
    /// Requiere `Trigger::Line`.
    ErrorReply(u32),
    /// La línea que dispara la falla se ejecuta pero su respuesta se pierde (cuenta mal en el
    /// anfitrión). Requiere `Trigger::Line`.
    DropReply,
    /// La respuesta de la línea que dispara la falla llega pegada a un reporte de estado
    /// cortado, sin salto de línea entre ambos: `<Run|MPos:...|Ov:100,100,10ok`. Es lo que se vio
    /// en una Falcon2 Pro real (el `?` se contestó a medias y el `ok` siguió en la misma línea).
    /// El `ok` (o `error:N`) llega completo. Requiere `Trigger::Line`.
    TruncatedStatusBeforeReply,
    /// Falla propia de Creality: línea `ERROR:NN` (mayúsculas, dos dígitos). Con `halt` la
    /// máquina se detiene (vacía el planificador, apaga el láser, deja de contestar líneas, pero
    /// sigue respondiendo a `?`) hasta el soft-reset: es lo que se espera de `ERROR:05` (llama).
    CrealityError { code: u32, halt: bool },
    /// `ALARM:N` (la máquina entra en Alarm; las líneas siguientes darán `error:9`).
    /// `ALARM:1` además deja de contestar a `?` hasta el reset, como un límite duro.
    Alarm(u32),
    /// Reinicio de la placa en pleno movimiento sin que el anfitrión lo pida: `ALARM:3`
    /// seguido del banner, posición perdida.
    Alarm3WithBanner,
    /// Reinicio espontáneo en reposo (brownout): solo banner.
    Reboot,
    /// Corte de luz: la cabeza se queda donde estaba, la placa pierde todo lo que tenía
    /// (planificador, búfer, offsets `G92`, referencia de homing) y el cable se desconecta. Al
    /// volver (`Handle::reconnect`) arranca con el banner y, si el homing está activado (`$22`),
    /// en alarma hasta un `$H`.
    PowerLoss,
    /// Silencio del enlace durante `duration`: lo que la controladora diga en ese lapso o se
    /// pierde (`drop_output`) o se entrega de golpe al terminar. Lo que el anfitrión envíe sí
    /// se procesa.
    Silence {
        duration: Duration,
        drop_output: bool,
    },
    /// Cable desconectado: lo ya enviado se puede leer, después lectura y escritura fallan
    /// con `BrokenPipe`.
    Disconnect,
    /// Desplazamientos de coordenadas dañados (`-nan`), como los G54 de la Falcon2.
    /// Con `in_status` también el `WCO:` de los reportes de estado.
    NanOffsets { in_status: bool },
    /// Cambia el terminador de las respuestas (`\r\n` o solo `\n`).
    LineEnding(LineEnding),
    /// Un byte de la línea que dispara la falla llega alterado (ruido en el cable). En Marlin con
    /// líneas numeradas el checksum falla: `Error:checksum mismatch` y `Resend: n`. Requiere
    /// `Trigger::Line`.
    CorruptLine,
}

/// Falla programada.
#[derive(Debug, Clone, PartialEq)]
pub struct Scheduled {
    pub trigger: Trigger,
    pub fault: Fault,
}
