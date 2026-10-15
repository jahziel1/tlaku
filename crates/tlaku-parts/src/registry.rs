//! Registro de generadores: lo que la app necesita saber de cada uno (formulario, valores de
//! fábrica) y la función que genera, todo por nombre y con parámetros JSON.
//!
//! **Para añadir un generador** (engranajes, bandejas, cola de milano...):
//! 1. módulo nuevo con su struct de parámetros (`#[serde(default)]`, `Default`), su función
//!    `params -> Result<Generated, PartsError>` y su `schema()` (campos del formulario);
//! 2. una entrada en [`GENERATORS`] con `id`, `schema`, `defaults` y `run` (usa [`run`]);
//! 3. textos `parts.<id>.*` en la interfaz (`i18n/{es,en}/parts.ts`) y mensajes en
//!    `locales/{es,en}/parts.txt`.
//!
//! El comando de Tauri, el diálogo, la vista previa, «Insertar» y «Regenerar» no cambian.

use crate::boxes::{BoxParams, box_parts};
use crate::error::PartsError;
use crate::fit_test::{FitTestParams, fit_test_parts};
use crate::gears::{GearParams, gears_parts};
use crate::hinge::{HingeParams, hinge_parts};
use crate::joints::{JointsParams, joints_parts};
use crate::layout::pack;
use crate::part::{Generated, Part};
use crate::rounded_box::{RoundedBoxParams, rounded_box_parts};
use crate::tray::{TrayParams, tray_parts};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tlaku_core::Polyline;
use tlaku_i18n::tr;
use ts_rs::TS;

/// Tipo de un campo del formulario.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
#[ts(export)]
pub enum ParamKind {
    /// Longitud en mm (la interfaz la muestra en la unidad elegida).
    Length {
        min: f64,
        max: f64,
        step: f64,
        default: f64,
    },
    /// Número sin unidad de longitud.
    Number {
        min: f64,
        max: f64,
        step: f64,
        default: f64,
    },
    /// Entero.
    Int {
        min: i32,
        max: i32,
        default: i32,
    },
    Bool {
        default: bool,
    },
    /// Texto corto libre (una lista de medidas, por ejemplo).
    Text {
        default: String,
    },
    /// Una opción de una lista cerrada (los valores son los del JSON).
    Choice {
        options: Vec<String>,
        default: String,
    },
}

/// Solo se muestra el campo si otro campo vale alguno de estos valores.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub struct ParamCondition {
    pub key: String,
    pub any_of: Vec<String>,
}

/// Un campo del formulario de un generador.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub struct ParamSpec {
    /// Nombre del campo en el JSON de parámetros. La interfaz busca su texto en
    /// `parts.<generador>.<key>` y el de cada opción en `parts.<generador>.<key>.<opción>`.
    pub key: String,
    pub kind: ParamKind,
    /// Grupo del formulario (`size`, `joints`, `options`...): texto `parts.section.<section>`.
    pub section: String,
    #[ts(optional = nullable)]
    pub when: Option<ParamCondition>,
}

impl ParamSpec {
    fn new(key: &str, section: &str, kind: ParamKind) -> Self {
        Self {
            key: key.to_string(),
            kind,
            section: section.to_string(),
            when: None,
        }
    }

    pub(crate) fn length(
        key: &str,
        section: &str,
        min: f64,
        max: f64,
        step: f64,
        default: f64,
    ) -> Self {
        Self::new(
            key,
            section,
            ParamKind::Length {
                min,
                max,
                step,
                default,
            },
        )
    }

    pub(crate) fn int(key: &str, section: &str, min: i32, max: i32, default: i32) -> Self {
        Self::new(key, section, ParamKind::Int { min, max, default })
    }

    pub(crate) fn boolean(key: &str, section: &str, default: bool) -> Self {
        Self::new(key, section, ParamKind::Bool { default })
    }

    pub(crate) fn text(key: &str, section: &str, default: &str) -> Self {
        Self::new(
            key,
            section,
            ParamKind::Text {
                default: default.to_string(),
            },
        )
    }

    pub(crate) fn choice(key: &str, section: &str, options: &[&str], default: &str) -> Self {
        Self::new(
            key,
            section,
            ParamKind::Choice {
                options: options.iter().map(|s| s.to_string()).collect(),
                default: default.to_string(),
            },
        )
    }

    /// Solo se muestra si `key` vale alguno de `values`.
    pub(crate) fn when(mut self, key: &str, values: &[&str]) -> Self {
        self.when = Some(ParamCondition {
            key: key.to_string(),
            any_of: values.iter().map(|s| s.to_string()).collect(),
        });
        self
    }
}

/// Un generador registrado.
pub struct GeneratorDef {
    pub id: &'static str,
    pub schema: fn() -> Vec<ParamSpec>,
    pub defaults: fn() -> Value,
    pub run: fn(&Value) -> Result<Generated, PartsError>,
}

/// Lee los parámetros JSON de un generador y lo ejecuta. Los campos que falten toman su valor de
/// fábrica y los que sobren se ignoran (guardados de versiones distintas siguen abriendo).
pub fn run<P: for<'de> Deserialize<'de>>(
    params: &Value,
    f: fn(&P) -> Result<Generated, PartsError>,
) -> Result<Generated, PartsError> {
    let parsed: P = serde_json::from_value(params.clone())
        .map_err(|e| PartsError::general(tr!("parts.registry.bad_params", error = e)))?;
    f(&parsed)
}

fn to_value<T: Serialize + Default>() -> Value {
    serde_json::to_value(T::default()).unwrap_or(Value::Null)
}

/// Todos los generadores, en el orden en que se ofrecen.
pub const GENERATORS: &[GeneratorDef] = &[
    GeneratorDef {
        id: "box",
        schema: crate::boxes::schema,
        defaults: to_value::<BoxParams>,
        run: |v| run::<BoxParams>(v, box_parts),
    },
    GeneratorDef {
        id: "tray",
        schema: crate::tray::schema,
        defaults: to_value::<TrayParams>,
        run: |v| run::<TrayParams>(v, tray_parts),
    },
    GeneratorDef {
        id: "hinge",
        schema: crate::hinge::schema,
        defaults: to_value::<HingeParams>,
        run: |v| run::<HingeParams>(v, hinge_parts),
    },
    GeneratorDef {
        id: "fit_test",
        schema: crate::fit_test::schema,
        defaults: to_value::<FitTestParams>,
        run: |v| run::<FitTestParams>(v, fit_test_parts),
    },
    GeneratorDef {
        id: "gears",
        schema: crate::gears::schema,
        defaults: to_value::<GearParams>,
        run: |v| run::<GearParams>(v, gears_parts),
    },
    GeneratorDef {
        id: "joints",
        schema: crate::joints::schema,
        defaults: to_value::<JointsParams>,
        run: |v| run::<JointsParams>(v, joints_parts),
    },
    GeneratorDef {
        id: "rounded_box",
        schema: crate::rounded_box::schema,
        defaults: to_value::<RoundedBoxParams>,
        run: |v| run::<RoundedBoxParams>(v, rounded_box_parts),
    },
];

/// Ficha de un generador para el formulario.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub struct GeneratorInfo {
    pub id: String,
    pub params: Vec<ParamSpec>,
    /// Parámetros de fábrica (el JSON que entiende `generate`).
    #[ts(type = "Record<string, unknown>")]
    pub defaults: Value,
}

/// Lista de generadores con su formulario.
pub fn list() -> Vec<GeneratorInfo> {
    GENERATORS
        .iter()
        .map(|g| GeneratorInfo {
            id: g.id.to_string(),
            params: (g.schema)(),
            defaults: (g.defaults)(),
        })
        .collect()
}

/// Trayectoria lista para el contrato con la interfaz: puntos `[x, y]` en mm.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub struct PathDto {
    pub points: Vec<[f64; 2]>,
    pub closed: bool,
}

impl From<&Polyline> for PathDto {
    fn from(p: &Polyline) -> Self {
        Self {
            points: p.points.iter().map(|q| [q.x, q.y]).collect(),
            closed: p.closed,
        }
    }
}

/// Una pieza ya colocada dentro del conjunto: los trazos vienen en las coordenadas del conjunto
/// (esquina inferior izquierda de todo en (0, 0)).
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub struct PlacedPart {
    pub key: String,
    pub name: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub cuts: Vec<PathDto>,
    pub engraves: Vec<PathDto>,
}

/// Resultado de generar. Los parámetros inválidos no son un fallo del comando: vienen en
/// `error` (con el campo) para que el formulario los marque mientras se edita.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub struct GenerateResult {
    pub parts: Vec<PlacedPart>,
    pub warnings: Vec<String>,
    pub width: f64,
    pub height: f64,
    #[ts(optional = nullable)]
    pub error: Option<PartsError>,
}

fn place(parts: &[Part], max_width: f64, gap: f64) -> (Vec<PlacedPart>, f64, f64) {
    let layout = pack(parts, max_width, gap);
    let placed = parts
        .iter()
        .zip(&layout.places)
        .map(|(p, at)| {
            let (cuts, engraves) = p.placed(at.x, at.y);
            PlacedPart {
                key: p.key.clone(),
                name: p.name.clone(),
                x: at.x,
                y: at.y,
                width: p.width,
                height: p.height,
                cuts: cuts.iter().map(PathDto::from).collect(),
                engraves: engraves.iter().map(PathDto::from).collect(),
            }
        })
        .collect();
    (placed, layout.width, layout.height)
}

/// Genera las piezas del generador `id` con `params` y las acomoda en filas de a lo más
/// `max_width_mm` (sin tope si es `None`), separadas `gap_mm`. `Err` solo si el generador no
/// existe; los parámetros inválidos van en `error`.
pub fn generate(
    id: &str,
    params: &Value,
    max_width_mm: Option<f64>,
    gap_mm: f64,
) -> Result<GenerateResult, String> {
    let def = GENERATORS
        .iter()
        .find(|g| g.id == id)
        .ok_or_else(|| tr!("parts.registry.unknown", id = id))?;
    let gap = if gap_mm.is_finite() {
        gap_mm.clamp(0.0, 100.0)
    } else {
        5.0
    };
    Ok(match (def.run)(params) {
        Ok(g) => {
            let max = max_width_mm
                .filter(|w| w.is_finite() && *w > 0.0)
                .unwrap_or(f64::INFINITY);
            let (parts, width, height) = place(&g.parts, max, gap);
            GenerateResult {
                parts,
                warnings: g.warnings,
                width,
                height,
                error: None,
            }
        }
        Err(e) => GenerateResult {
            parts: Vec::new(),
            warnings: Vec::new(),
            width: 0.0,
            height: 0.0,
            error: Some(e),
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tlaku_i18n::{Locale, with_locale};

    #[test]
    fn lists_every_generator_with_consistent_schemas() {
        let all = list();
        let ids: Vec<&str> = all.iter().map(|g| g.id.as_str()).collect();
        assert_eq!(
            ids,
            [
                "box",
                "tray",
                "hinge",
                "fit_test",
                "gears",
                "joints",
                "rounded_box"
            ]
        );
        for g in &all {
            let defaults = g
                .defaults
                .as_object()
                .expect("los valores de fábrica son un objeto");
            // Cada campo del formulario existe en los parámetros y su valor de fábrica coincide.
            for spec in &g.params {
                let v = defaults
                    .get(&spec.key)
                    .unwrap_or_else(|| panic!("{}.{} no está en los parámetros", g.id, spec.key));
                match &spec.kind {
                    ParamKind::Length {
                        min,
                        max,
                        default,
                        step,
                    }
                    | ParamKind::Number {
                        min,
                        max,
                        default,
                        step,
                    } => {
                        assert_eq!(v.as_f64(), Some(*default), "{}.{}", g.id, spec.key);
                        assert!(min < max && *step > 0.0 && (min..=max).contains(&default));
                    }
                    ParamKind::Int { min, max, default } => {
                        assert_eq!(v.as_i64(), Some(i64::from(*default)));
                        assert!(min < max);
                    }
                    ParamKind::Bool { default } => assert_eq!(v.as_bool(), Some(*default)),
                    ParamKind::Text { default } => assert_eq!(v.as_str(), Some(default.as_str())),
                    ParamKind::Choice { options, default } => {
                        assert_eq!(v.as_str(), Some(default.as_str()));
                        assert!(options.contains(default));
                    }
                }
                if let Some(c) = &spec.when {
                    let other = g
                        .params
                        .iter()
                        .find(|p| p.key == c.key)
                        .expect("condición sobre un campo que no existe");
                    let ParamKind::Choice { options, .. } = &other.kind else {
                        panic!("la condición va sobre una lista")
                    };
                    assert!(c.any_of.iter().all(|v| options.contains(v)));
                }
            }
            // Y todos los parámetros están en el formulario (nada oculto).
            for key in defaults.keys() {
                assert!(
                    g.params.iter().any(|p| &p.key == key),
                    "{}.{key} falta en el formulario",
                    g.id
                );
            }
        }
    }

    #[test]
    fn every_generator_works_with_its_defaults_and_at_range_limits() {
        for g in list() {
            let r = generate(&g.id, &g.defaults, Some(400.0), 5.0).unwrap();
            assert!(r.error.is_none(), "{}: {:?}", g.id, r.error);
            assert!(!r.parts.is_empty() && r.width > 0.0 && r.height > 0.0);
            // En los límites del formulario no hay pánico (puede dar error de validación).
            for spec in &g.params {
                let (lo, hi) = match &spec.kind {
                    ParamKind::Length { min, max, .. } | ParamKind::Number { min, max, .. } => {
                        (json!(min), json!(max))
                    }
                    ParamKind::Int { min, max, .. } => (json!(min), json!(max)),
                    _ => continue,
                };
                for v in [lo, hi] {
                    let mut p = g.defaults.clone();
                    p[&spec.key] = v;
                    let r = generate(&g.id, &p, Some(400.0), 5.0).unwrap();
                    assert!(r.error.is_some() || !r.parts.is_empty());
                }
            }
        }
    }

    #[test]
    fn placed_parts_fit_the_width_and_do_not_overlap() {
        let r = generate("box", &json!({}), Some(300.0), 5.0).unwrap();
        assert_eq!(r.parts.len(), 6);
        assert!(r.width <= 300.0 + 1e-9);
        for (i, a) in r.parts.iter().enumerate() {
            for b in &r.parts[i + 1..] {
                let apart = a.x + a.width + 5.0 <= b.x + 1e-9
                    || b.x + b.width + 5.0 <= a.x + 1e-9
                    || a.y + a.height + 5.0 <= b.y + 1e-9
                    || b.y + b.height + 5.0 <= a.y + 1e-9;
                assert!(apart, "{} y {} se tocan", a.key, b.key);
            }
            for path in &a.cuts {
                for q in &path.points {
                    assert!(
                        q[0] >= a.x - 1e-9
                            && q[0] <= a.x + a.width + 1e-9
                            && q[1] >= a.y - 1e-9
                            && q[1] <= a.y + a.height + 1e-9
                    );
                }
            }
        }
        // Sin tope de ancho: una sola fila.
        let one = generate("box", &json!({}), None, 5.0).unwrap();
        assert!(one.parts.iter().all(|p| p.y == 0.0));
    }

    #[test]
    fn invalid_parameters_are_data_not_command_failures() {
        let r = generate("box", &json!({"thickness_mm": -1}), None, 5.0).unwrap();
        assert_eq!(
            r.error.as_ref().and_then(|e| e.field.as_deref()),
            Some("thickness_mm")
        );
        assert!(r.parts.is_empty());
        let r = generate("box", &json!({"thickness_mm": "mucho"}), None, 5.0).unwrap();
        assert!(r.error.is_some());
        assert!(generate("nope", &json!({}), None, 5.0).is_err());
        let es = generate("nope", &json!({}), None, 5.0).unwrap_err();
        let en = with_locale(Locale::En, || {
            generate("nope", &json!({}), None, 5.0).unwrap_err()
        });
        assert_ne!(es, en);
    }

    #[test]
    fn partial_and_future_parameters_still_load() {
        // Un guardado viejo (faltan campos) y uno nuevo (sobra un campo) siguen generando.
        let r = generate(
            "hinge",
            &json!({"width_mm": 80, "campo_del_futuro": true}),
            None,
            5.0,
        )
        .unwrap();
        assert!(r.error.is_none());
        assert!((r.width - 80.0).abs() < 1e-9);
    }

    /// El mock de la interfaz lee `apps/desktop/src/api/mock/partsSchema.json`: esta prueba lo
    /// mantiene al día (`UPDATE_FIXTURES=1 cargo test -p tlaku-parts schema_fixture` lo reescribe).
    #[test]
    fn schema_fixture_for_the_mock_is_current() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../apps/desktop/src/api/mock/partsSchema.json"
        );
        let fresh = serde_json::to_string_pretty(&list()).unwrap() + "\n";
        if std::env::var_os("UPDATE_FIXTURES").is_some() {
            std::fs::write(path, &fresh).unwrap();
        }
        let saved = std::fs::read_to_string(path).unwrap_or_default();
        assert_eq!(
            saved, fresh,
            "partsSchema.json está desactualizado: UPDATE_FIXTURES=1 cargo test -p tlaku-parts schema_fixture"
        );
    }
}
