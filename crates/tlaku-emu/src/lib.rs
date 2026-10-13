//! Emulador de controladoras láser para probar Tlaku sin máquina física.
//!
//! [`VirtualMachine`] implementa [`tlaku_grbl::serial::Transport`]: se le pasa al `Streamer`, a
//! `Grbl` o a cualquier código que hable con un puerto serie, y contesta como la controladora
//! que describe su [`Profile`]. El [`Handle`] que acompaña a la máquina sirve a las pruebas para
//! inyectar fallas y consultar posición, estado, rastro del láser y registro de lo recibido.
//!
//! ```
//! use tlaku_emu::{Profile, VirtualMachine};
//! use tlaku_grbl::serial::Transport;
//! use std::time::Duration;
//!
//! let (mut vm, handle) = VirtualMachine::new(Profile::grbl_1_1h());
//! vm.write_all(b"G90\n").unwrap();
//! let mut buf = [0u8; 256];
//! let n = vm.read_timeout(&mut buf, Duration::from_millis(5)).unwrap();
//! let text = String::from_utf8_lossy(&buf[..n]).into_owned();
//! assert!(text.starts_with("Grbl 1.1h ['$' for help]\r\n"));
//! assert!(text.ends_with("ok\r\n"));
//! assert_eq!(handle.received_lines(), ["G90"]);
//! ```
//!
//! # Qué emula
//!
//! - **Enlace**: banner al conectar y tras `0x18` (según el perfil), búfer RX de `rx_size - 1`
//!   bytes que se llena de verdad cuando el intérprete está bloqueado, y descarte (contado) de
//!   los bytes que no caben; `ok`/`error:N` en orden; líneas vacías con `ok`; `\r` y `\n` como
//!   terminadores (cada uno cierra una línea, como GRBL); bytes en tiempo real fuera del búfer.
//!   `max_in_flight()` mide los bytes sin respuesta, que con conteo de caracteres correcto no
//!   pasan del búfer.
//! - **Planificador** de `planner_blocks` bloques: una línea de movimiento recibe `ok` al
//!   entrar; con el planificador lleno el intérprete se bloquea. `M3/M4/M5` con cambio de estado,
//!   `G4`, `M0`, `M2/M30`, refrigerante y cambios de `S` sin movimiento sincronizan (el `ok` llega
//!   al vaciarse), según `grbl.md` sección 4.
//! - **Estados**: `Idle`, `Run`, `Hold:1/0`, `Jog`, `Alarm`, `Door:0-3`, `Home`, `Check`, `Sleep`
//!   y, en grblHAL, `Tool` y `Alarm:N`. Soft reset con `ALARM:3` si había movimiento (`ALARM:6`
//!   durante el homing) y sin alarma si estaba quieta o en `Hold:0`.
//! - **Tiempo real**: `? ! ~ 0x18 0x84 0x85`, overrides `0x90-0x9E`, `0xA0/0xA1`, y los de
//!   grblHAL (`0x80-0x83`, `0x87`).
//! - **Reporte de estado** con `$10`, `Bf`, `FS`, `Pn`, `WCO` y `Ov` periódicos, `A:`, `Ln`
//!   (grblHAL), `FS` de tres valores y campos extra (grblHAL).
//! - **Comandos `$`**: `$ $$ $x=v $# $G $I $I=texto $N $Nx= $C $X $H $J= $SLP $RST=`, con los
//!   errores de la tabla de GRBL 1.1 (`error:3/4/6/8/9/10/15/16/22`...).
//! - **G-code**: el subconjunto de `src/parse.rs` (movimientos, arcos troceados, offsets
//!   `G10/G92/G54-59`, unidades, `G93`...); lo demás da `error:20` y los fallos de
//!   sintaxis dan `error:1/2/4/21/22/23/24/25/26/27/28/30/31/32/33/34/35/36`.
//! - **Movimiento**: perfil trapezoidal con la aceleración `min($120,$121)`, avance programado
//!   (con override), rápidos a `min($110,$111)` y velocidad de paso por esquinas según el ángulo.
//!   No hay precisión física: sirve para que el tiempo avance, `Run`/`Idle` se alternen y la
//!   posición cambie.
//! - **Láser**: encendido según `M3/M4/M5`, `S`, `$30` (saturación) y `$32`. En modo láser
//!   los `G0` nunca queman; `M3` quema parado (también en hold, a menos que el perfil diga
//!   `laser_off_in_hold`); `M4` escala con la velocidad y se apaga parado. Con `$32=0` el
//!   husillo arde también en `G0`. Todo queda en el [`LaserTrace`].
//! - **Fallas** ([`Fault`], [`Trigger`]): ruido serie, `ERROR:NN` de Creality (con o sin
//!   detención), `ALARM:N`, `ALARM:3` con banner, reinicio, silencio (perdiendo o reteniendo lo
//!   dicho), desconexión, `-nan` en offsets, `\r\n` o `\n`, y respuesta cambiada o perdida de
//!   una línea concreta.
//!
//! # Fidelidad
//!
//! Los formatos (banners, `$I`, `$$`, reportes, errores) salen de `docs/hardware/`; lo que esos
//! documentos marcan como inferido sigue siéndolo aquí: tamaños de búfer de grblHAL y FluidNC,
//! límite de línea (80 en GRBL clásico), `Pn`, orden de `ALARM:3` y banner (aquí alarma primero),
//! efecto de `!` sobre `M3` (el láser sigue encendido: el riesgo del punto 20 de `grbl.md`).
//! El vector E-1 usa `XYZZY` y espera `error:20`; esta implementación da `error:2` (valor
//! numérico ausente, que es lo que dice la tabla), así que las pruebas usan `G99` para `error:20`.
//!
//! # Qué no emula
//!
//! - Velocidad en baudios (los bytes llegan al instante) ni la pérdida de caracteres durante
//!   escrituras en EEPROM.
//! - Física real: junction deviation, look-ahead completo, pasos, pérdida de pasos.
//! - Sondas (`G38.x`), ejes A/B/C, compensación de herramienta, arcos fuera del plano XY, SD,
//!   WiFi, parking, `$SLP` más allá del estado, líneas de arranque con `Ln`.
//! - **Marlin y Smoothie** (protocolo RepRap) solo cubren: banner, `ok` por línea, `M115` (con
//!   `Cap:`), `M114`, `M400`, movimiento, láser con `S` en línea (modo inline sin disparo en
//!   parado; modo estándar con `G0` que apaga hasta el siguiente `M3`), `M106/M107` (con
//!   `Profile::marlin_fan_laser` el ventilador manda sobre el láser y `G0` no lo apaga),
//!   `M410` (encolada, o inmediata con `EMERGENCY_PARSER`), `M112` (paro hasta reiniciar; en
//!   Smoothie hasta `M999`), `M0`/`M108` con `EMERGENCY_PARSER`, líneas numeradas con checksum
//!   y `Resend:` (Marlin, con la falla `CorruptLine`) y el `?` de Smoothie. Pendientes:
//!   `M600/M601`, `M154`, el módulo `switch`. Los textos exactos de `Error:`/`Resend:` y el
//!   formato de `?` de Smoothie (`<Idle,MPos:...,WPos:...>`) son **inferidos**, no medidos.
//! - El `ok` anticipado de `G1` de Smoothie ni la cola corta de su planificador (se trata como
//!   Marlin).
//!
//! # Relojes
//!
//! El emulador no tiene hilos: avanza a demanda hasta el instante del [`Clock`] inyectado cada
//! vez que el anfitrión escribe, lee o consulta. [`AutoClock`] (por defecto) hace correr el
//! tiempo simulado mientras el anfitrión espera, sin dormir; [`ManualClock`] solo avanza con
//! `advance`; [`ScaledClock`] sigue al reloj de pared multiplicado por un factor (úsalo cuando
//! el anfitrión tiene plazos reales, como pausas o silencio, y pon un factor de 20 a 50 para
//! que los trabajos terminen rápido).

pub mod clock;
pub mod fault;
pub mod machine;
mod model;
mod parse;
pub mod profile;
pub mod trace;
pub mod vm;

pub use clock::{AutoClock, Clock, ManualClock, ScaledClock};
pub use fault::{Fault, Scheduled, Trigger};
pub use machine::{LogEntry, Machine, Options, Received};
pub use model::StateKind;
pub use profile::{Family, LineEnding, Profile, Protocol, Setting};
pub use trace::{LaserSegment, LaserTrace, MoveKind, SpindleMode};
pub use vm::{Builder, Handle, VirtualMachine};
