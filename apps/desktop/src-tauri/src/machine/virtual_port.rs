//! Máquinas virtuales: puertos `virtual:<perfil>[@<velocidad>]` que, en lugar del puerto serie,
//! abren un [`tlaku_emu::VirtualMachine`] con reloj de pared. Sirven para probar la interfaz y el
//! flujo completo (conectar, enviar, pausar, cancelar, fallas) sin una máquina ni riesgo de
//! grabar nada.
//!
//! El identificador tiene un prefijo que ningún puerto real usa (`/dev/...`, `COM3`). La
//! velocidad opcional (`@5`) acelera el tiempo simulado; por omisión es ×1 (tiempo real).

use tlaku_doc::SimulatedFault;
use tlaku_emu::{Fault, Handle, Profile, ScaledClock, Trigger, VirtualMachine};
use tlaku_grbl::serial::Transport;

/// Prefijo de los puertos virtuales.
pub const PREFIX: &str = "virtual:";

/// Velocidad máxima aceptada (veces el tiempo real).
pub const MAX_SPEED: f64 = 100.0;

struct Kind {
    id: &'static str,
    profile: fn() -> Profile,
}

/// Perfiles que se ofrecen en la lista de puertos.
const KINDS: [Kind; 6] = [
    Kind {
        id: "falcon2",
        profile: Profile::falcon2_pro,
    },
    Kind {
        id: "grbl11",
        profile: Profile::grbl_1_1h,
    },
    Kind {
        id: "grblhal",
        profile: Profile::grbl_hal,
    },
    Kind {
        id: "fluidnc",
        profile: Profile::fluidnc,
    },
    Kind {
        id: "marlin",
        profile: Profile::marlin,
    },
    Kind {
        id: "smoothie",
        profile: Profile::smoothie,
    },
];

/// Identificadores de los puertos virtuales, para añadirlos al final de la lista de puertos.
pub fn ports() -> Vec<String> {
    KINDS.iter().map(|k| format!("{PREFIX}{}", k.id)).collect()
}

/// ¿Es un puerto virtual?
pub fn is_virtual(port: &str) -> bool {
    port.starts_with(PREFIX)
}

/// `virtual:falcon2@5` -> (`falcon2`, 5.0). Sin `@` la velocidad es 1.
fn parse(port: &str) -> Result<(&'static Kind, f64), String> {
    let rest = port
        .strip_prefix(PREFIX)
        .ok_or_else(|| format!("'{port}' no es un puerto virtual."))?;
    let (id, speed) = match rest.split_once('@') {
        Some((id, s)) => {
            let v: f64 = s.parse().map_err(|_| {
                format!("La velocidad '{s}' de la máquina virtual no es un número.")
            })?;
            if !(v.is_finite() && v > 0.0 && v <= MAX_SPEED) {
                return Err(format!(
                    "La velocidad de la máquina virtual debe estar entre 0 y {MAX_SPEED} veces el \
                     tiempo real."
                ));
            }
            (id, v)
        }
        None => (rest, 1.0),
    };
    let kind = KINDS
        .iter()
        .find(|k| k.id == id)
        .ok_or_else(|| format!("No existe la máquina virtual '{id}'."))?;
    Ok((kind, speed))
}

/// Crea la máquina virtual de `port` con reloj de pared (escalado por la velocidad pedida).
/// Devuelve el transporte, ya sin el saludo de arranque (como `Grbl::open`), y el mando que
/// permite inyectar fallas.
pub fn open(port: &str) -> Result<(Box<dyn Transport>, Handle), String> {
    let (kind, speed) = parse(port)?;
    // Arranca referenciada y sin alarma: una máquina lista para trabajar.
    let (mut vm, handle) = VirtualMachine::builder((kind.profile)())
        .clock(ScaledClock::new(speed))
        .homed(true)
        .build();
    // Como al abrir un puerto real: el saludo del arranque se descarta.
    vm.clear_input()
        .map_err(|e| format!("No se pudo preparar la máquina virtual: {e}"))?;
    Ok((Box::new(vm), handle))
}

/// Falla de la interfaz -> falla del emulador, inmediata.
pub fn inject(handle: &Handle, fault: SimulatedFault) {
    let f = match fault {
        // `halt: true`: la máquina se detiene, como con una llama real.
        SimulatedFault::Flame => Fault::CrealityError {
            code: 5,
            halt: true,
        },
        SimulatedFault::Tilt => Fault::CrealityError {
            code: 1,
            halt: true,
        },
        SimulatedFault::Alarm3 => Fault::Alarm(3),
        SimulatedFault::Disconnect => Fault::Disconnect,
    };
    handle.inject(Trigger::Now, f);
}
