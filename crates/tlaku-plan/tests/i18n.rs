//! Las razones del orden y los avisos que ve el usuario en la vista previa salen del catálogo, en
//! español (idénticos a los de siempre) y en inglés.

use tlaku_i18n::{Locale, with_locale};
use tlaku_plan::{Blocked, Level, Reason, Warning};

fn blocked(contains_this: bool, by_overlap: bool) -> Option<Blocked> {
    Some(Blocked {
        item: 7,
        contains_this,
        by_overlap,
    })
}

fn all_reasons() -> Vec<Reason> {
    vec![
        Reason::FirstOfLayer {
            layer: 2,
            blocked: None,
        },
        Reason::FirstOfGroup {
            group: Some(3),
            blocked: blocked(true, false),
        },
        Reason::FirstOfGroup {
            group: None,
            blocked: blocked(true, true),
        },
        Reason::FirstOfPriority {
            priority: -1,
            blocked: blocked(false, false),
        },
        Reason::InnerFirst { container: 4 },
        Reason::OverlapFirst {
            other: 5,
            share_pct: 80,
            with_others: false,
        },
        Reason::OverlapFirst {
            other: 5,
            share_pct: 80,
            with_others: true,
        },
        Reason::Nearest {
            distance_mm: 1.234,
            blocked: None,
        },
        Reason::Nearest {
            distance_mm: 1.234,
            blocked: Some(9),
        },
        Reason::InputOrder { blocked: None },
        Reason::InputOrder { blocked: Some(9) },
        Reason::Improved,
        Reason::LevelOverridesInner {
            inner: 6,
            level: Level::Group,
        },
    ]
}

fn all_warnings() -> Vec<Warning> {
    vec![
        Warning::SkippedInvalid { item: 8 },
        Warning::OverlapAnalysisLimited,
        Warning::InnerOverriddenByLevel {
            inner: 1,
            container: 2,
            level: Level::Priority,
        },
    ]
}

#[test]
fn spanish_texts_are_the_original_ones() {
    let r: Vec<String> = all_reasons().iter().map(ToString::to_string).collect();
    assert_eq!(
        r[0],
        "primera figura de capa 2: las capas se cortan en su orden"
    );
    assert_eq!(
        r[1],
        "primera figura de grupo 3; va antes que la figura #7 (la más cercana) porque está dentro de ella y los interiores se cortan primero"
    );
    assert_eq!(
        r[2],
        "primera figura de figuras sin grupo; va antes que la figura #7 (la más cercana) porque casi toda cae dentro de ella y lo más sujeto se corta primero"
    );
    assert_eq!(
        r[3],
        "primera figura de prioridad -1; la figura #7 estaba más cerca pero debe esperar a sus interiores"
    );
    assert_eq!(
        r[4],
        "va antes que la figura #4, que la contiene: interiores primero"
    );
    assert_eq!(
        r[5],
        "va antes que la figura #5, con la que se solapa: el 80 % de ella cae dentro, así que se corta primero la parte que queda sujeta"
    );
    assert_eq!(
        r[6],
        "va antes que la figura #5, con la que se solapa: el 80 % de ella cae dentro y, junto con otras figuras, queda casi toda sobre material ajeno"
    );
    assert_eq!(
        r[7],
        "la más cercana (1.23 mm) entre las que podían cortarse ahora"
    );
    assert_eq!(
        r[8],
        "la más cercana (1.23 mm) entre las que podían cortarse ahora; la figura #9 estaba más cerca pero debe esperar a sus interiores"
    );
    assert_eq!(
        r[9],
        "sigue el orden original (reducir recorrido está apagado)"
    );
    assert_eq!(
        r[10],
        "sigue el orden original; la figura #9 iba antes pero debe esperar a sus interiores"
    );
    assert_eq!(
        r[11],
        "la mejora de recorrido (2-opt/Or-opt, inicio y sentido) la dejó aquí porque acorta el total, aunque no es la figura más cercana"
    );
    assert_eq!(
        r[12],
        "sale antes que su interior #6 porque el orden por grupo tiene precedencia sobre interiores primero"
    );
    let w: Vec<String> = all_warnings().iter().map(ToString::to_string).collect();
    assert_eq!(
        w[0],
        "la figura #8 está vacía o tiene coordenadas inválidas y se omitió"
    );
    assert!(w[1].starts_with("hay demasiadas figuras grandes que se solapan: no se analizaron"));
    assert_eq!(
        w[2],
        "la figura #2 se corta antes que su interior #1: el orden por prioridad tiene precedencia sobre interiores primero"
    );
}

#[test]
fn english_texts_have_no_spanish_left() {
    with_locale(Locale::En, || {
        let r: Vec<String> = all_reasons().iter().map(ToString::to_string).collect();
        assert_eq!(
            r[0],
            "first shape of layer 2: layers are cut in their own order"
        );
        assert_eq!(
            r[2].split(';').next().unwrap(),
            "first shape of ungrouped shapes"
        );
        assert_eq!(
            r[7],
            "the nearest (1.23 mm) among those that could be cut now"
        );
        assert_eq!(
            r[12],
            "goes before its inner shape #6 because the order by group takes precedence over inner shapes first"
        );
        let w: Vec<String> = all_warnings().iter().map(ToString::to_string).collect();
        assert_eq!(
            w[0],
            "shape #8 is empty or has invalid coordinates and was skipped"
        );
        assert!(w[2].contains("the order by priority"), "{}", w[2]);
        let spanish = [
            "figura",
            "capa",
            "grupo",
            "prioridad",
            "cercana",
            "orden",
            "interior",
            "recorrido",
            "solap",
            "omitió",
        ];
        for text in r.iter().chain(&w) {
            for word in spanish {
                assert!(!text.contains(word), "{word:?} en «{text}»");
            }
            assert!(!text.contains("{"), "marcador sin llenar en «{text}»");
        }
    });
}
