use super::*;
use serde_json::{Value, json};
use std::path::PathBuf;

/// Aporte de ejemplo (inventado para las pruebas; no es un ajuste real).
fn sample(issue: u32) -> Value {
    json!({
        "id": format!("community.basswood.3.cut.diode_20.{issue}"),
        "material": "basswood",
        "thickness": 3,
        "role": "cut",
        "machine_class": "diode_20",
        "laser_watts": 22,
        "machine": "Máquina de prueba 22 W",
        "machine_model": "maquina-de-prueba",
        "speed": 500,
        "power": 100,
        "passes": 2,
        "air": true,
        "notes": "Ejemplo de prueba; no es un ajuste real.",
        "source": format!("https://github.com/jahziel1/tlaku/issues/{issue}"),
        "confidence": "forum",
        "license": "CC0-1.0",
        "contributor": "alguien",
        "date": "2026-10-01",
        "tlaku_version": "0.1.0",
        "confirmations": [
            {"by": "otra-persona", "machine": "Otra máquina 20 W", "machine_class": "diode_20",
             "date": "2026-10-05", "source": format!("https://github.com/jahziel1/tlaku/issues/{issue}#issuecomment-123")}
        ]
    })
}

fn file(issue: u32, v: &Value) -> (String, String) {
    (format!("{issue}.json"), v.to_string())
}

fn issue_of(v: &Value) -> u32 {
    v["source"]
        .as_str()
        .unwrap()
        .rsplit('/')
        .next()
        .unwrap()
        .parse()
        .unwrap()
}

fn built(entries: &[Value]) -> CatalogIndex {
    let files: Vec<_> = entries.iter().map(|v| file(issue_of(v), v)).collect();
    build_index(&files, "2026-10-15").expect("válido")
}

fn index_json(patch: impl FnOnce(&mut Value)) -> String {
    let mut v = serde_json::to_value(built(&[sample(7)])).unwrap();
    patch(&mut v);
    v.to_string()
}

#[test]
fn el_indice_se_arma_con_los_campos_calculados() {
    let index = built(&[sample(9), sample(7)]);
    assert_eq!(index.schema, CATALOG_SCHEMA);
    assert_eq!(index.version, 1);
    assert_eq!(index.generated_at, "2026-10-15");
    // Ordenado por id.
    assert_eq!(
        index
            .entries
            .iter()
            .map(|e| e.id.as_str())
            .collect::<Vec<_>>(),
        [
            "community.basswood.3.cut.diode_20.7",
            "community.basswood.3.cut.diode_20.9"
        ]
    );
    let e = &index.entries[0];
    assert_eq!(e.operation, CatalogOperation::Cut);
    assert_eq!(e.mode, LayerMode::Line);
    assert_eq!(e.verified_by, 1);
    assert_eq!(e.machine_model.as_deref(), Some("maquina-de-prueba"));
    assert_eq!(e.contributor.as_deref(), Some("alguien"));
    assert_eq!(e.date.as_deref(), Some("2026-10-01"));
}

#[test]
fn el_indice_publicado_lo_acepta_el_validador_y_va_y_vuelve_igual() {
    let index = built(&[sample(7)]);
    let text = serde_json::to_string(&index).unwrap();
    assert_eq!(validate_index(&text).unwrap(), index);
}

#[test]
fn la_operacion_sale_del_rol_y_del_modo() {
    let cases = [
        (MaterialRole::Cut, LayerMode::Line, CatalogOperation::Cut),
        (
            MaterialRole::Score,
            LayerMode::Line,
            CatalogOperation::Score,
        ),
        (
            MaterialRole::Engrave,
            LayerMode::Line,
            CatalogOperation::Engrave,
        ),
        (
            MaterialRole::Engrave,
            LayerMode::Fill,
            CatalogOperation::Fill,
        ),
        (
            MaterialRole::Engrave,
            LayerMode::Image,
            CatalogOperation::Image,
        ),
    ];
    for (role, mode, op) in cases {
        assert_eq!(CatalogOperation::of(role, mode), op, "{role:?} {mode:?}");
    }
    // Grabar sin decir el modo es relleno.
    let mut v = sample(8);
    v["role"] = "engrave".into();
    v["id"] = "community.basswood.3.engrave.diode_20.8".into();
    assert_eq!(
        built(std::slice::from_ref(&v)).entries[0].operation,
        CatalogOperation::Fill
    );
    v["mode"] = "image".into();
    assert_eq!(built(&[v]).entries[0].operation, CatalogOperation::Image);
}

#[test]
fn un_archivo_de_aporte_invalido_o_mal_nombrado_no_arma_el_indice() {
    let ok = sample(7);
    assert!(
        build_index(&[file(8, &ok)], "2026-10-15").is_err(),
        "nombre distinto al issue"
    );
    assert!(build_index(&[file(7, &ok), file(7, &ok)], "2026-10-15").is_err());
    assert!(build_index(&[("7.json".into(), "no es json".into())], "2026-10-15").is_err());
    assert!(build_index(&[file(7, &ok)], "15/10/2026").is_err());
    let mut bad = sample(7);
    bad["power"] = 150.into();
    assert!(build_index(&[file(7, &bad)], "2026-10-15").is_err());
    assert!(build_index(&[], "2026-10-15").unwrap().entries.is_empty());
}

#[test]
fn se_rechaza_el_indice_que_incumple_el_esquema_o_los_limites() {
    type Patch = Box<dyn Fn(&mut Value)>;
    let set = |field: &'static str, value: Value| -> Patch {
        Box::new(move |v| v["entries"][0][field] = value.clone())
    };
    let bad: Vec<(&str, Patch)> = vec![
        ("schema ajeno", Box::new(|v| v["schema"] = "otro".into())),
        (
            "versión mayor desconocida",
            Box::new(|v| v["version"] = 2.into()),
        ),
        (
            "sin versión",
            Box::new(|v| {
                v.as_object_mut().unwrap().remove("version");
            }),
        ),
        (
            "fecha mala",
            Box::new(|v| v["generated_at"] = "ayer".into()),
        ),
        (
            "campo de la raíz mal tipado",
            Box::new(|v| v["entries"] = "x".into()),
        ),
        ("potencia sobre 100", set("power", 101.into())),
        ("potencia cero", set("power", 0.into())),
        ("potencia negativa", set("power", (-5).into())),
        ("velocidad cero", set("speed", 0.into())),
        ("velocidad negativa", set("speed", (-100).into())),
        ("velocidad enorme", set("speed", 9_999_999.into())),
        ("pasadas cero", set("passes", 0.into())),
        ("pasadas de más", set("passes", 1000.into())),
        ("grosor negativo", set("thickness", (-3).into())),
        ("watts que no son de la clase", set("laser_watts", 5.into())),
        ("licencia ajena", set("license", "MIT".into())),
        (
            "fuente que no es un issue",
            set("source", "https://example.com/x".into()),
        ),
        ("operación incoherente", set("operation", "image".into())),
        ("verificado incoherente", set("verified_by", 5.into())),
        ("material sin nombre", set("material", "felt".into())),
        ("rol desconocido", set("role", "burn".into())),
        (
            "clase desconocida",
            set("machine_class", "diode_999".into()),
        ),
        ("notas con control", set("notes", "a\u{7}b".into())),
        ("notas larguísimas", set("notes", "x".repeat(401).into())),
        ("alias con símbolos", set("contributor", "a/b".into())),
        ("fecha de aporte mala", set("date", "2026-13-40".into())),
        ("versión de Tlaku mala", set("tlaku_version", "x.y".into())),
        (
            "modelo con mayúsculas",
            set("machine_model", "Falcon".into()),
        ),
        ("imagen que corta", set("mode", "image".into())),
        (
            "confirma quien aportó",
            Box::new(|v| v["entries"][0]["confirmations"][0]["by"] = "Alguien".into()),
        ),
        (
            "confirmación repetida",
            Box::new(|v| {
                let c = v["entries"][0]["confirmations"][0].clone();
                v["entries"][0]["confirmations"] = json!([c.clone(), c]);
                v["entries"][0]["verified_by"] = 2.into();
            }),
        ),
        (
            "confirmación sin máquina",
            Box::new(|v| v["entries"][0]["confirmations"][0]["machine"] = " ".into()),
        ),
        (
            "confirmación con fuente ajena",
            Box::new(|v| {
                v["entries"][0]["confirmations"][0]["source"] = "https://evil.example/x".into()
            }),
        ),
        (
            "id repetido",
            Box::new(|v| {
                let e = v["entries"][0].clone();
                v["entries"] = json!([e.clone(), e]);
            }),
        ),
    ];
    for (why, patch) in bad {
        assert!(
            validate_index(&index_json(patch)).is_err(),
            "debió rechazarse: {why}"
        );
    }
    assert!(validate_index("no es json").is_err());
    assert!(validate_index("").is_err());
    assert!(validate_index("[]").is_err());
}

#[test]
fn el_texto_o_la_lista_demasiado_grandes_se_descartan() {
    let big = format!(
        r#"{{"schema":"{CATALOG_SCHEMA}","version":1,"generated_at":"2026-10-15","entries":[],"relleno":"{}"}}"#,
        "x".repeat(MAX_INDEX_BYTES)
    );
    let err = validate_index(&big).unwrap_err();
    assert!(err.contains(&MAX_INDEX_BYTES.to_string()), "{err}");

    let one = serde_json::to_value(built(&[sample(7)]).entries.remove(0)).unwrap();
    let many: Vec<Value> = (0..=MAX_ENTRIES)
        .map(|i| {
            let mut e = one.clone();
            e["id"] = format!("community.basswood.3.cut.diode_20.{}", i + 1).into();
            e["confirmations"] = json!([]);
            e["verified_by"] = 0.into();
            e
        })
        .collect();
    let text = json!({"schema": CATALOG_SCHEMA, "version": 1, "generated_at": "2026-10-15", "entries": many}).to_string();
    assert!(
        text.len() < MAX_INDEX_BYTES,
        "la prueba debe topar con el número de entradas, no con los bytes"
    );
    assert!(
        validate_index(&text)
            .unwrap_err()
            .contains(&MAX_ENTRIES.to_string())
    );
}

#[test]
fn campos_nuevos_de_la_misma_version_se_ignoran() {
    // Una versión posterior del sitio puede añadir campos opcionales sin romper a las apps viejas.
    let text = index_json(|v| v["entries"][0]["campo_futuro"] = "x".into());
    assert!(validate_index(&text).is_ok());
}

#[test]
fn el_indice_real_del_repositorio_se_arma_y_es_valido() {
    let dir: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../community/materials");
    let index =
        build_index_from_dir(&dir, "2026-10-15").expect("los aportes reales arman el índice");
    let text = serde_json::to_string(&index).unwrap();
    assert!(text.len() < MAX_INDEX_BYTES);
    assert_eq!(validate_index(&text).unwrap(), index);
}
