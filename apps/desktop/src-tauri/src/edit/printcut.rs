//! Comandos de Print & Cut (M37) y tiling (M39): el cálculo vive en `tlaku-pipeline` y la
//! interfaz solo lo llama, así que hay una única implementación (antes había otra en TypeScript).
//!
//! - `fit_registration(pairs, mode)` calcula la transformación del diseño a la cama a partir de
//!   las marcas medidas. `mode` es `auto`, `rigid`, `similarity` o `affine`; `auto` (y una afín
//!   que no se puede resolver) elige el modo según las marcas.
//! - `plan_tiles(design, size, overlap, order, margin)` reparte la caja del diseño en tramos y
//!   da las cruces de alineación de las zonas de solape.
//!
//! Los fallos de validación no son errores del comando: vuelven como `{ ok: false, error }` con
//! el tipo del motivo, para que la interfaz lo muestre en su idioma.

use serde::Serialize;
use tlaku_core::{Bounds, Point};
use tlaku_pipeline::{
    FitChoice, FitError, MarkPair, RegistrationFit, TileError, TileOrder, TilePlan,
    fit_with_choice, plan_grid,
};

/// Respuesta de `fit_registration`: `fit` si `ok`, `error` si no.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FitResponse {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fit: Option<RegistrationFit>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<FitError>,
}

pub fn run_fit(pairs: &[MarkPair], mode: FitChoice) -> FitResponse {
    match fit_with_choice(pairs, mode) {
        Ok(fit) => FitResponse {
            ok: true,
            fit: Some(fit),
            error: None,
        },
        Err(error) => FitResponse {
            ok: false,
            fit: None,
            error: Some(error),
        },
    }
}

/// Calcula el ajuste de Print & Cut de las marcas medidas.
#[tauri::command]
pub fn fit_registration(pairs: Vec<MarkPair>, mode: FitChoice) -> FitResponse {
    run_fit(&pairs, mode)
}

/// Respuesta de `plan_tiles`: `plan` si `ok`, `error` si no.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PlanResponse {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan: Option<TilePlan>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<TileError>,
}

pub fn run_plan(
    design: [[f64; 2]; 2],
    size: [f64; 2],
    overlap: f64,
    order: TileOrder,
    margin: f64,
) -> PlanResponse {
    let bounds = Bounds {
        min: Point::new(design[0][0], design[0][1]),
        max: Point::new(design[1][0], design[1][1]),
    };
    match plan_grid(bounds, size, overlap, order, margin) {
        Ok(plan) => PlanResponse {
            ok: true,
            plan: Some(plan),
            error: None,
        },
        Err(error) => PlanResponse {
            ok: false,
            plan: None,
            error: Some(error),
        },
    }
}

/// Reparte la caja `design` (`[[x0, y0], [x1, y1]]`, mm) en tramos de `size` con `overlap` mm de
/// solape; `margin` mm se suman al desplazamiento de todos los tramos.
#[tauri::command]
pub fn plan_tiles(
    design: [[f64; 2]; 2],
    size: [f64; 2],
    overlap: f64,
    order: TileOrder,
    margin: f64,
) -> PlanResponse {
    run_plan(design, size, overlap, order, margin)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pair(d: [f64; 2], p: [f64; 2]) -> MarkPair {
        MarkPair {
            design: d,
            physical: p,
        }
    }

    #[test]
    fn fit_returns_the_translation_and_picks_the_mode() {
        let pairs = [
            pair([0.0, 0.0], [3.0, -2.0]),
            pair([100.0, 0.0], [103.0, -2.0]),
        ];
        let r = run_fit(&pairs, FitChoice::Auto);
        assert!(r.ok && r.error.is_none());
        let fit = r.fit.unwrap();
        assert_eq!(fit.mode, tlaku_pipeline::FitMode::Similarity);
        assert!((fit.transform[4] - 3.0).abs() < 1e-9);
        assert!((fit.transform[5] + 2.0).abs() < 1e-9);
    }

    #[test]
    fn fit_failures_come_back_as_data_not_as_a_command_error() {
        let r = run_fit(&[pair([0.0, 0.0], [1.0, 1.0])], FitChoice::Auto);
        assert!(!r.ok && r.fit.is_none());
        assert_eq!(r.error, Some(FitError::Few { need: 2, have: 1 }));
        let v = serde_json::to_value(&r).unwrap();
        assert_eq!(
            v,
            serde_json::json!({"ok": false, "error": {"kind": "few", "need": 2, "have": 1}})
        );
    }

    #[test]
    fn fit_reads_the_arguments_as_the_interface_sends_them() {
        // Lo que manda `api.ts`: pares con `design`/`physical` y el modo en minúsculas.
        let pairs: Vec<MarkPair> = serde_json::from_value(serde_json::json!([
            {"design": [0, 0], "physical": [1, 2]},
            {"design": [50, 0], "physical": [51, 2]},
            {"design": [0, 80], "physical": [1, 82]},
        ]))
        .unwrap();
        let mode: FitChoice = serde_json::from_value(serde_json::json!("affine")).unwrap();
        let r = run_fit(&pairs, mode);
        assert_eq!(r.fit.unwrap().mode, tlaku_pipeline::FitMode::Affine);
    }

    #[test]
    fn plan_splits_the_design_and_adds_the_margin() {
        let r = run_plan(
            [[0.0, 0.0], [100.0, 60.0]],
            [55.0, 35.0],
            10.0,
            TileOrder::Rows,
            5.0,
        );
        assert!(r.ok);
        let plan = r.plan.unwrap();
        assert_eq!((plan.cols, plan.rows, plan.tiles.len()), (2, 2, 4));
        assert_eq!(plan.tiles[1].spec.offset, [-40.0, -20.0]);
        assert!(!plan.marks.is_empty());
    }

    #[test]
    fn plan_errors_come_back_as_data() {
        let r = run_plan(
            [[0.0, 0.0], [100.0, 100.0]],
            [50.0, 50.0],
            50.0,
            TileOrder::Rows,
            0.0,
        );
        assert!(!r.ok && r.plan.is_none());
        let v = serde_json::to_value(&r).unwrap();
        assert_eq!(v["error"]["kind"], "overlap");
        let r = run_plan(
            [[0.0, 0.0], [100.0, 100.0]],
            [0.0, 50.0],
            0.0,
            TileOrder::Rows,
            0.0,
        );
        assert_eq!(r.error, Some(TileError::Size));
    }

    #[test]
    fn plan_reads_the_arguments_as_the_interface_sends_them() {
        let order: TileOrder = serde_json::from_value(serde_json::json!("serpentine")).unwrap();
        let r = run_plan([[0.0, 0.0], [100.0, 60.0]], [55.0, 35.0], 10.0, order, 0.0);
        let plan = r.plan.unwrap();
        // Serpentina: la segunda fila va de derecha a izquierda.
        assert_eq!((plan.tiles[2].row, plan.tiles[2].col), (1, 1));
    }
}
