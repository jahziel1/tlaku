//! Reporte de estado (`<Idle|...>`) y línea de parámetros `$#`.

// Reporte de estado
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MachineState {
    Idle,
    Run,
    /// Subcódigo: 0 = detención completa, 1 = deteniéndose.
    Hold(Option<u8>),
    Jog,
    Alarm,
    /// Subcódigo: 0 = puerta cerrada y lista para reanudar; 1 = máquina detenida con la puerta
    /// aún entreabierta (no se puede reanudar); 2 = puerta abierta, con hold o retracción en
    /// curso (un reset lanzaría alarma); 3 = puerta cerrada y reanudando (un reset lanzaría
    /// alarma).
    Door(Option<u8>),
    Check,
    Home,
    Sleep,
    /// Estado que Tlaku no conoce (p. ej. `Tool` de grblHAL). El reporte no se descarta: el
    /// nombre original queda en [`StatusReport::other_state`].
    Other,
}

impl MachineState {
    /// `true` si, con este estado, la máquina no se está moviendo y un soft reset conserva la
    /// posición. `Hold` sin subcódigo, `Hold:1`, `Door:2`/`Door:3`, `Run`, `Jog`, `Home` y los
    /// estados desconocidos cuentan como "puede estar en movimiento".
    pub fn is_stationary(self) -> bool {
        matches!(
            self,
            Self::Idle
                | Self::Alarm
                | Self::Check
                | Self::Sleep
                | Self::Hold(Some(0))
                | Self::Door(Some(0 | 1))
        )
    }
}

/// Coordenadas de la máquina. `z` vale 0 si el reporte solo trae dos ejes;
/// los ejes adicionales (A, B, ...) van en `extra`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Position {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub extra: Vec<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BufferState {
    /// Bloques libres en el planificador de movimiento.
    pub planner_blocks: u32,
    /// Bytes libres en el búfer de recepción serie.
    pub rx_bytes: u32,
}

/// Porcentajes de override activos.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Overrides {
    pub feed: u32,
    pub rapid: u32,
    pub spindle: u32,
}

/// Reporte de estado (`<Idle|MPos:...|...>`). Los campos ausentes quedan en `None`.
#[derive(Debug, Clone, PartialEq)]
pub struct StatusReport {
    pub state: MachineState,
    /// Nombre original del estado cuando es [`MachineState::Other`].
    pub other_state: Option<String>,
    pub mpos: Option<Position>,
    pub wpos: Option<Position>,
    pub wco: Option<Position>,
    pub buffer: Option<BufferState>,
    pub line_number: Option<u32>,
    /// Velocidad de avance actual (de `FS:` o `F:`).
    pub feed: Option<f64>,
    /// Velocidad/potencia del husillo (solo en `FS:`).
    pub spindle: Option<f64>,
    pub overrides: Option<Overrides>,
    /// Pines activos (`Pn:`), p. ej. `XYZPDHRS`.
    pub pins: Option<String>,
    /// Accesorios activos (`A:`), p. ej. `SFM`.
    pub accessories: Option<String>,
}

/// Número finito. `nan`, `-nan` e `inf` se leen bien con `parse::<f64>()`, pero no son una
/// posición ni un valor válido: el firmware de Creality llega a imprimir `-nan` en G54 y
/// `$#`. Se tratan como desconocidos.
fn finite(s: &str) -> Option<f64> {
    s.trim().parse::<f64>().ok().filter(|v| v.is_finite())
}

/// Lista de números finitos separados por comas; falla entera si alguno no lo es.
fn finite_list(s: &str) -> Option<Vec<f64>> {
    s.split(',').map(finite).collect()
}

fn parse_position(s: &str) -> Option<Position> {
    let v = finite_list(s)?;
    if v.len() < 2 {
        return None;
    }
    Some(Position {
        x: v[0],
        y: v[1],
        z: v.get(2).copied().unwrap_or(0.0),
        extra: v.iter().skip(3).copied().collect(),
    })
}

/// Estado y nombre original si es desconocido. Un subcódigo que no sea un número se ignora
/// en lugar de descartar el reporte.
fn parse_state(s: &str, has_fields: bool) -> Option<(MachineState, Option<String>)> {
    let (name, sub) = match s.split_once(':') {
        Some((n, c)) => (n.trim(), c.trim().parse::<u8>().ok()),
        None => (s.trim(), None),
    };
    let known = match name {
        "Idle" => MachineState::Idle,
        "Run" => MachineState::Run,
        "Hold" => MachineState::Hold(sub),
        "Jog" => MachineState::Jog,
        "Alarm" => MachineState::Alarm,
        "Door" => MachineState::Door(sub),
        "Check" => MachineState::Check,
        "Home" => MachineState::Home,
        "Sleep" => MachineState::Sleep,
        // Un nombre desconocido pero con forma de estado y seguido de campos (p. ej.
        // `<Tool|MPos:...>`) no tira el reporte; la basura sí (vacío, símbolos, demasiado
        // largo, o un `<Nope>` sin campos).
        other
            if has_fields
                && (1..=16).contains(&other.len())
                && other.bytes().all(|b| b.is_ascii_alphabetic()) =>
        {
            return Some((MachineState::Other, Some(other.to_string())));
        }
        _ => return None,
    };
    Some((known, None))
}

impl StatusReport {
    /// Interpreta una línea `<...>`. Devuelve `None` si no es un reporte o no trae un estado
    /// con forma válida; los campos desconocidos o mal formados se ignoran, y los valores
    /// `nan`/`inf` cuentan como mal formados (campo desconocido, nunca una posición válida).
    pub fn parse(line: &str) -> Option<Self> {
        let inner = line.trim().strip_prefix('<')?.strip_suffix('>')?;
        let mut fields = inner.split('|');
        let (state, other_state) = parse_state(fields.next()?, inner.contains('|'))?;
        let mut report = StatusReport {
            state,
            other_state,
            mpos: None,
            wpos: None,
            wco: None,
            buffer: None,
            line_number: None,
            feed: None,
            spindle: None,
            overrides: None,
            pins: None,
            accessories: None,
        };
        for field in fields {
            let Some((key, value)) = field.split_once(':') else {
                continue;
            };
            match key {
                "MPos" => report.mpos = parse_position(value),
                "WPos" => report.wpos = parse_position(value),
                "WCO" => report.wco = parse_position(value),
                "Bf" => {
                    if let Some(&[a, b]) = finite_list(value).as_deref() {
                        report.buffer = Some(BufferState {
                            planner_blocks: a as u32,
                            rx_bytes: b as u32,
                        });
                    }
                }
                "Ln" => report.line_number = value.trim().parse().ok(),
                // grblHAL manda tres valores (`FS:avance,S programada,S real`); aquí
                // interesan los dos primeros.
                "FS" => {
                    if let Some(&[f, s] | &[f, s, _]) = finite_list(value).as_deref() {
                        report.feed = Some(f);
                        report.spindle = Some(s);
                    }
                }
                "F" => report.feed = finite(value),
                "Ov" => {
                    if let Some(&[f, r, s]) = finite_list(value).as_deref() {
                        report.overrides = Some(Overrides {
                            feed: f as u32,
                            rapid: r as u32,
                            spindle: s as u32,
                        });
                    }
                }
                "Pn" => report.pins = Some(value.trim().to_string()),
                "A" => report.accessories = Some(value.trim().to_string()),
                _ => {}
            }
        }
        Some(report)
    }
}

impl StatusReport {
    /// Interpreta el informe de estado con comas de GRBL 0.9 y de Smoothieware:
    /// `<Idle,MPos:0.0000,0.0000,0.0000,WPos:0.0000,0.0000,0.0000>`. Solo trae estado y
    /// posiciones. El formato de `?` de Smoothie no está confirmado por su documentación
    /// (`dialectos-gcode.md`, 3.2, y 7 para GRBL 0.9): contrástalo con una máquina real.
    pub fn parse_legacy(line: &str) -> Option<Self> {
        let inner = line.trim().strip_prefix('<')?.strip_suffix('>')?;
        let mut tokens = inner.split(',');
        let (state, other_state) = parse_state(tokens.next()?, inner.contains(','))?;
        let mut report = StatusReport {
            state,
            other_state,
            mpos: None,
            wpos: None,
            wco: None,
            buffer: None,
            line_number: None,
            feed: None,
            spindle: None,
            overrides: None,
            pins: None,
            accessories: None,
        };
        // Los valores sin clave pertenecen al campo `clave:` anterior.
        let mut fields: Vec<(&str, Vec<&str>)> = Vec::new();
        for t in tokens {
            match t.split_once(':') {
                Some((k, v)) => fields.push((k, vec![v])),
                None => {
                    if let Some(f) = fields.last_mut() {
                        f.1.push(t);
                    }
                }
            }
        }
        for (key, vals) in fields {
            let pos = parse_position(&vals.join(","));
            match key {
                "MPos" => report.mpos = pos,
                "WPos" => report.wpos = pos,
                _ => {}
            }
        }
        Some(report)
    }
}

/// Línea de `$#` (`[G54:x,y,z]`, `[TLO:z]`, `[PRB:x,y,z:1]`...). Devuelve el nombre y los
/// valores, o `None` en los valores si alguno es `nan`/`inf`/ilegible: el firmware de la
/// Falcon2 llega a imprimir `[G54:-nan,1.579,1.619]`, que no es una posición válida.
pub fn parse_parameter_line(line: &str) -> Option<(String, Option<Vec<f64>>)> {
    let inner = line.trim().strip_prefix('[')?.strip_suffix(']')?;
    let (name, rest) = inner.split_once(':')?;
    if name.is_empty() || !name.bytes().all(|b| b.is_ascii_alphanumeric()) {
        return None;
    }
    // `PRB` añade `:éxito` tras las coordenadas.
    let values = rest.split(':').next().unwrap_or("");
    Some((name.to_string(), finite_list(values)))
}
