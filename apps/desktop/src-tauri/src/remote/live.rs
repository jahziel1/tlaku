//! Vista en vivo para el celular (solo lectura): la ruta del trabajo, simplificada y mandada UNA
//! vez por trabajo, y la ubicación que la ventana informa con cada reporte de estado.
//!
//! La ventana decide qué se dibuja (ver `features/live/remoteRoute.ts`): trazos continuos de
//! corte y grabado con el tiempo de trabajo en cada vértice; los desplazamientos no van (la página
//! mueve la cabeza en línea recta de un trazo al siguiente). Aquí solo se valida que lo recibido
//! sea razonable y quepa en el tope, porque el servidor lo repite a otros dispositivos. Nada de
//! esto envía algo a la máquina.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tlaku_i18n::tr;

/// Tope del JSON de la ruta (bytes).
pub const MAX_LIVE_BYTES: usize = 320 * 1024;
/// Topes de tamaño por conteo: protegen la memoria antes de serializar.
pub const MAX_RUNS: usize = 30_000;
pub const MAX_VERTICES: usize = 60_000;
pub const MAX_LAYERS: usize = 64;
/// Coordenadas y tiempos razonables (mm y s).
const MAX_COORD: f64 = 100_000.0;
const MAX_TIME_S: f64 = 10_000_000.0;
/// Ancho máximo de un trazo (mm).
const MAX_WIDTH_MM: f64 = 50.0;

/// Un trazo continuo con láser.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LiveRun {
    /// 0 = corte, 1 = grabado.
    pub k: u8,
    /// Capa (índice en `layers`).
    pub l: u16,
    /// Ancho con que se dibuja, mm.
    pub w: f64,
    /// `[x, y, t, x, y, t, ...]`: posición (mm del documento, Y hacia arriba) y tiempo de trabajo (s).
    pub p: Vec<f64>,
}

/// La ruta tal como llega de la ventana.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct LiveRouteIn {
    /// Tamaño de la cama, mm.
    pub bed: [f64; 2],
    /// Duración de la ruta (s): el 100 %.
    pub total: f64,
    /// Colores `#rrggbb` de las capas.
    pub layers: Vec<String>,
    pub runs: Vec<LiveRun>,
    /// Hubo que simplificar más de lo normal para caber (solo informativo).
    #[serde(default)]
    pub decimated: bool,
}

/// Ubicación de la máquina en la ruta, por reporte (va dentro del estado que ve el celular).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LiveReport {
    /// Tiempo de trabajo (s) en que se ubicó la posición real.
    pub t: f64,
    /// La máquina está ejecutando (no en pausa).
    pub running: bool,
    /// Sube cada vez que la máquina retrocedió de verdad.
    pub retreat_seq: u32,
    /// De dónde sale el avance: `position`, `progress` o `clock`.
    pub src: String,
}

impl LiveReport {
    /// Acota lo que llega de la ventana.
    pub fn sanitized(mut self) -> Self {
        self.t = if self.t.is_finite() {
            self.t.clamp(0.0, MAX_TIME_S)
        } else {
            0.0
        };
        if !matches!(self.src.as_str(), "position" | "progress" | "clock") {
            self.src = "position".into();
        }
        self
    }
}

/// Una ruta ya validada y serializada.
#[derive(Debug, Clone)]
pub struct LiveRoute {
    pub version: u64,
    json: Value,
}

impl LiveRoute {
    pub fn json(&self) -> Value {
        self.json.clone()
    }
}

fn is_color(s: &str) -> bool {
    s.len() == 7 && s.starts_with('#') && s[1..].chars().all(|c| c.is_ascii_hexdigit())
}

/// Valida la ruta y arma el JSON que se sirve en `GET /api/live`.
pub fn validate(r: &LiveRouteIn, version: u64) -> Result<LiveRoute, String> {
    let bad = || tr!("desktop.remote.live_bad");
    if !(r
        .bed
        .iter()
        .all(|v| v.is_finite() && *v > 0.0 && *v <= MAX_COORD)
        && r.total.is_finite()
        && r.total > 0.0
        && r.total <= MAX_TIME_S)
    {
        return Err(bad());
    }
    if r.layers.len() > MAX_LAYERS || !r.layers.iter().all(|c| is_color(c)) {
        return Err(bad());
    }
    if r.runs.len() > MAX_RUNS {
        return Err(tr!("desktop.remote.live_too_big"));
    }
    let mut vertices = 0usize;
    let mut last_t = 0.0f64;
    for run in &r.runs {
        let n = run.p.len();
        vertices += n / 3;
        if vertices > MAX_VERTICES {
            return Err(tr!("desktop.remote.live_too_big"));
        }
        let ok = n >= 6
            && n % 3 == 0
            && run.k <= 1
            && (usize::from(run.l) < r.layers.len())
            && run.w.is_finite()
            && run.w > 0.0
            && run.w <= MAX_WIDTH_MM;
        if !ok {
            return Err(bad());
        }
        for &[x, y, t] in run.p.as_chunks::<3>().0 {
            // Los tiempos solo suben: la página busca por tiempo con una búsqueda binaria.
            if !(x.is_finite() && y.is_finite() && t.is_finite())
                || x.abs() > MAX_COORD
                || y.abs() > MAX_COORD
                || t < last_t - 1e-6
                || t > MAX_TIME_S
            {
                return Err(bad());
            }
            last_t = t.max(last_t);
        }
    }
    let json = json!({
        "version": version,
        "bed": r.bed,
        "total": r.total,
        "layers": r.layers,
        "runs": r.runs,
        "decimated": r.decimated,
    });
    // Contar los bytes serializados es la única forma exacta; se hace una vez por trabajo.
    if json.to_string().len() > MAX_LIVE_BYTES {
        return Err(tr!("desktop.remote.live_too_big"));
    }
    Ok(LiveRoute { version, json })
}

/// La respuesta cuando no hay ruta.
pub fn empty_json() -> Value {
    json!({ "version": 0, "bed": null, "total": 0, "layers": [], "runs": [], "decimated": false })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(n: usize) -> LiveRun {
        LiveRun {
            k: 0,
            l: 0,
            w: 0.3,
            p: (0..n)
                .flat_map(|i| [i as f64, 2.0 * i as f64, i as f64 * 0.5])
                .collect(),
        }
    }

    fn route(runs: Vec<LiveRun>) -> LiveRouteIn {
        LiveRouteIn {
            bed: [400.0, 300.0],
            total: 100.0,
            layers: vec!["#ff0000".into()],
            runs,
            decimated: false,
        }
    }

    #[test]
    fn a_normal_route_is_accepted_and_serialized() {
        let mut second = run(10);
        for v in second.p.as_chunks_mut::<3>().0 {
            v[2] += 20.0;
        }
        let r = validate(&route(vec![run(4), second]), 7).unwrap();
        let j = r.json();
        assert_eq!(j["version"], 7);
        assert_eq!(j["runs"].as_array().unwrap().len(), 2);
        assert_eq!(j["layers"][0], "#ff0000");
    }

    #[test]
    fn rejects_malformed_runs() {
        // Menos de dos vértices.
        assert!(validate(&route(vec![run(1)]), 1).is_err());
        // Capa fuera de rango.
        let mut r = run(3);
        r.l = 5;
        assert!(validate(&route(vec![r]), 1).is_err());
        // Longitud que no es múltiplo de tres.
        let mut r = run(3);
        r.p.pop();
        assert!(validate(&route(vec![r]), 1).is_err());
        // Tiempo que retrocede entre trazos.
        let mut a = run(3);
        a.p[8] = 50.0;
        assert!(validate(&route(vec![a, run(3)]), 1).is_err());
        // No finitos y ancho inválido.
        let mut r = run(3);
        r.p[0] = f64::NAN;
        assert!(validate(&route(vec![r]), 1).is_err());
        let mut r = run(3);
        r.w = 0.0;
        assert!(validate(&route(vec![r]), 1).is_err());
    }

    #[test]
    fn rejects_bad_colors_and_beds() {
        let mut r = route(vec![run(3)]);
        r.layers = vec!["red".into()];
        assert!(validate(&r, 1).is_err());
        let mut r = route(vec![run(3)]);
        r.layers = vec!["#12345g".into()];
        assert!(validate(&r, 1).is_err());
        let mut r = route(vec![run(3)]);
        r.bed = [0.0, 10.0];
        assert!(validate(&r, 1).is_err());
        let mut r = route(vec![run(3)]);
        r.total = f64::INFINITY;
        assert!(validate(&r, 1).is_err());
    }

    #[test]
    fn size_caps_apply_by_count_and_by_bytes() {
        // Demasiados trazos.
        let many = vec![run(2); MAX_RUNS + 1];
        assert!(validate(&route(many), 1).is_err());
        // Demasiados vértices.
        let big = vec![run(MAX_VERTICES / 2 + 1), run(MAX_VERTICES / 2 + 1)];
        assert!(validate(&route(big), 1).is_err());
        // Cabe por conteo pero no por bytes: coordenadas largas.
        let fat = LiveRun {
            k: 1,
            l: 0,
            w: 0.1,
            p: (0..45_000)
                .flat_map(|i| {
                    [
                        12_345.678_901_234 + f64::from(i) * 1e-7,
                        23_456.789_012_345,
                        f64::from(i) * 0.123_456_789,
                    ]
                })
                .collect(),
        };
        assert!(validate(&route(vec![fat]), 1).is_err());
    }

    #[test]
    fn a_route_of_about_300_kb_fits() {
        // 14 000 vértices con 2 decimales en x, y, t: unos 20 bytes cada uno.
        let r = LiveRun {
            k: 1,
            l: 0,
            w: 0.1,
            p: (0..14_000)
                .flat_map(|i| {
                    let v = f64::from(i);
                    [
                        (v * 0.07).rem_euclid(300.0).round() + 0.25,
                        200.0 + (v * 0.01).round() / 100.0 + 0.5,
                        (v * 0.05 * 100.0).round() / 100.0,
                    ]
                })
                .collect(),
        };
        let ok = validate(&route(vec![r]), 1).unwrap();
        let len = ok.json().to_string().len();
        assert!(len > 200 * 1024 && len <= MAX_LIVE_BYTES, "{len}");
    }

    #[test]
    fn report_is_clamped() {
        let r = LiveReport {
            t: f64::NAN,
            running: true,
            retreat_seq: 0,
            src: "evil<script>".into(),
        }
        .sanitized();
        assert_eq!(r.t, 0.0);
        assert_eq!(r.src, "position");
        let r = LiveReport {
            t: -4.0,
            src: "clock".into(),
            ..LiveReport::default()
        }
        .sanitized();
        assert_eq!(r.t, 0.0);
        assert_eq!(r.src, "clock");
    }
}
