//! Casos comunes con la maqueta de la interfaz (`apps/desktop/src/api/mock/`): el ajuste de
//! Print & Cut y el plan de tiling. Los mismos archivos JSON los lee vitest, de modo que las dos
//! implementaciones se comprueban con los mismos datos. El motor es la fuente de verdad: si un
//! caso falla aquí, el esperado está mal o el motor cambió; la maqueta se ajusta a él.

use serde_json::Value;
use tlaku_core::{Bounds, Point};
use tlaku_pipeline::{
    FitChoice, FitError, MarkPair, TileError, TileOrder, fit_registration, plan_grid,
};

fn load(name: &str) -> Value {
    let path = format!("{}/tests/cross/{name}.json", env!("CARGO_MANIFEST_DIR"));
    serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap()
}

fn num(v: &Value) -> f64 {
    v.as_f64().unwrap()
}

fn pair2(v: &Value) -> [f64; 2] {
    [num(&v[0]), num(&v[1])]
}

fn near(a: f64, b: f64, tol: f64, what: &str) {
    assert!((a - b).abs() <= tol, "{what}: {a} != {b} (tol {tol})");
}

#[test]
fn registration_cases_match_the_shared_expectations() {
    let file = load("registration");
    let tol = num(&file["tolerance"]);
    for case in file["cases"].as_array().unwrap() {
        let name = case["name"].as_str().unwrap();
        let pairs: Vec<MarkPair> = serde_json::from_value(case["pairs"].clone()).unwrap();
        let mode = serde_json::from_value(case["mode"].clone()).unwrap();
        let got = fit_registration(&pairs, mode);
        let want = &case["expect"];
        if want["ok"] == false {
            let err: FitError = serde_json::from_value(want["error"].clone()).unwrap();
            assert_eq!(got.unwrap_err(), err, "{name}");
            continue;
        }
        let fit = got.unwrap_or_else(|e| panic!("{name}: {e}"));
        if let Some(t) = want.get("transform") {
            for (i, v) in t.as_array().unwrap().iter().enumerate() {
                near(
                    fit.transform[i],
                    num(v),
                    tol,
                    &format!("{name}: transform[{i}]"),
                );
            }
        }
        for (field, value) in [
            ("rotationDeg", fit.rotation_deg),
            ("scale", fit.scale),
            ("residualMax", fit.residual_max),
        ] {
            if let Some(v) = want.get(field) {
                near(value, num(v), tol, &format!("{name}: {field}"));
            }
        }
        if let Some(r) = want.get("residuals") {
            for (i, v) in r.as_array().unwrap().iter().enumerate() {
                near(
                    fit.residuals[i],
                    num(v),
                    tol,
                    &format!("{name}: residual {i}"),
                );
            }
        }
        // Sin la clave `warnings` el caso no fija los avisos.
        if let Some(expected) = want.get("warnings") {
            let kinds: Vec<String> = fit
                .warnings
                .iter()
                .map(|w| {
                    serde_json::to_value(w).unwrap()["kind"]
                        .as_str()
                        .unwrap()
                        .to_string()
                })
                .collect();
            let expected: Vec<&str> = expected
                .as_array()
                .unwrap()
                .iter()
                .map(|w| w["kind"].as_str().unwrap())
                .collect();
            assert_eq!(kinds, expected, "{name}: avisos");
        }
    }
}

#[test]
fn mode_resolution_matches_the_shared_expectations() {
    let file = load("registration");
    for case in file["choice"].as_array().unwrap() {
        let choice: FitChoice = serde_json::from_value(case["choice"].clone()).unwrap();
        let design: Vec<[f64; 2]> = case["design"]
            .as_array()
            .unwrap()
            .iter()
            .map(pair2)
            .collect();
        let want: tlaku_pipeline::FitMode = serde_json::from_value(case["expect"].clone()).unwrap();
        assert_eq!(choice.resolve(&design), want, "{}", case["name"]);
    }
}

fn bounds(v: &Value) -> Bounds {
    let (a, b) = (pair2(&v[0]), pair2(&v[1]));
    Bounds {
        min: Point::new(a[0], a[1]),
        max: Point::new(b[0], b[1]),
    }
}

#[test]
fn tiling_cases_match_the_shared_expectations() {
    let file = load("tiling");
    let tol = num(&file["tolerance"]);
    for case in file["cases"].as_array().unwrap() {
        let name = case["name"].as_str().unwrap();
        let order: TileOrder = serde_json::from_value(case["order"].clone()).unwrap();
        let plan = plan_grid(
            bounds(&case["design"]),
            pair2(&case["size"]),
            num(&case["overlap"]),
            order,
            num(&case["margin"]),
        )
        .unwrap_or_else(|e| panic!("{name}: {e}"));
        let want = &case["expect"];
        assert_eq!(
            plan.cols as u64,
            want["cols"].as_u64().unwrap(),
            "{name}: columnas"
        );
        assert_eq!(
            plan.rows as u64,
            want["rows"].as_u64().unwrap(),
            "{name}: filas"
        );
        for (got, key) in [
            (&plan.col_ranges, "colRanges"),
            (&plan.row_ranges, "rowRanges"),
        ] {
            let want = want[key].as_array().unwrap();
            assert_eq!(got.len(), want.len(), "{name}: {key}");
            for (g, w) in got.iter().zip(want) {
                let w = pair2(w);
                near(g[0], w[0], tol, &format!("{name}: {key}"));
                near(g[1], w[1], tol, &format!("{name}: {key}"));
            }
        }
        if let Some(tiles) = want.get("tiles") {
            let tiles = tiles.as_array().unwrap();
            assert_eq!(plan.tiles.len(), tiles.len(), "{name}: tramos");
            for (g, w) in plan.tiles.iter().zip(tiles) {
                assert_eq!(g.row as u64, w["row"].as_u64().unwrap(), "{name}: fila");
                assert_eq!(g.col as u64, w["col"].as_u64().unwrap(), "{name}: columna");
                for k in 0..2 {
                    let (lo, hi) = (pair2(&w["rect"][0]), pair2(&w["rect"][1]));
                    near(g.spec.rect[0][k], lo[k], tol, &format!("{name}: rect"));
                    near(g.spec.rect[1][k], hi[k], tol, &format!("{name}: rect"));
                    near(
                        g.spec.offset[k],
                        pair2(&w["offset"])[k],
                        tol,
                        &format!("{name}: offset"),
                    );
                }
            }
        }
        if let Some(marks) = want.get("marks") {
            let marks = marks.as_array().unwrap();
            assert_eq!(plan.marks.len(), marks.len(), "{name}: marcas");
            for (g, w) in plan.marks.iter().zip(marks) {
                for (gp, wp) in g.iter().zip(w.as_array().unwrap()) {
                    let wp = pair2(wp);
                    near(gp[0], wp[0], tol, &format!("{name}: marca"));
                    near(gp[1], wp[1], tol, &format!("{name}: marca"));
                }
            }
        }
        if let Some(n) = want.get("marksCount") {
            assert_eq!(
                plan.marks.len() as u64,
                n.as_u64().unwrap(),
                "{name}: marcas"
            );
        }
    }
    for case in file["errors"].as_array().unwrap() {
        let err = plan_grid(
            bounds(&case["design"]),
            pair2(&case["size"]),
            num(&case["overlap"]),
            TileOrder::Rows,
            0.0,
        )
        .unwrap_err();
        let kind = serde_json::to_value(&err).unwrap()["kind"]
            .as_str()
            .unwrap()
            .to_string();
        assert_eq!(
            kind,
            case["expectError"].as_str().unwrap(),
            "{}",
            case["name"]
        );
        assert!(matches!(
            err,
            TileError::Size
                | TileError::Overlap { .. }
                | TileError::OverlapNegative
                | TileError::Empty
                | TileError::TooMany { .. }
        ));
    }
}
