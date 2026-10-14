//! Los avisos y errores del importador salen en el idioma activo (`tlaku-i18n`), y los errores que la
//! interfaz reconoce llevan un código estable aunque cambie el idioma.

use std::path::PathBuf;
use tlaku_i18n::{Locale, split_code, with_locale};
use tlaku_pipeline::{ImportExtras, ImportOptions, import_file, import_file_extras};

/// Carpeta temporal propia de esta prueba.
fn dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("tlaku-import-i18n-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn write(dir: &std::path::Path, file: &str, data: &[u8]) -> PathBuf {
    let p = dir.join(file);
    std::fs::write(&p, data).unwrap();
    p
}

fn err_of(path: &std::path::Path) -> String {
    import_file(path).expect_err("debía fallar")
}

#[test]
fn errors_follow_the_active_language() {
    let d = dir("errors");
    let plt = write(&d, "vacio.plt", b"IN;SP1;");
    let unknown = write(&d, "x.xyz", b"hola");
    let rd = write(&d, "mal.rd", &[0u8; 64]);
    let pdf = write(&d, "mal.pdf", b"no es un pdf");

    // Por omisión, español.
    assert!(
        err_of(&plt).contains("no contiene trazos"),
        "{}",
        err_of(&plt)
    );
    assert!(err_of(&unknown).contains("formato de entrada no soportado"));

    with_locale(Locale::En, || {
        let m = err_of(&plt);
        assert!(m.contains("contains no strokes"), "{m}");
        assert!(err_of(&unknown).contains("unsupported input format"));
        assert!(err_of(&rd).contains("does not look like a Ruida .rd file"));
        assert!(err_of(&pdf).contains("does not look like a PDF"));
        for m in [err_of(&plt), err_of(&unknown), err_of(&rd), err_of(&pdf)] {
            assert!(
                !m.contains("no parece") && !m.contains("no contiene"),
                "quedó español: {m}"
            );
        }
    });
    std::fs::remove_dir_all(&d).ok();
}

#[test]
fn warnings_follow_the_active_language() {
    let d = dir("warnings");
    // HPGL válido: siempre avisa de la unidad supuesta.
    let plt = write(&d, "ok.plt", b"IN;SP1;PU0,0;PD400,0,400,400,0,400,0,0;PU;");
    let es = import_file(&plt).unwrap().warnings;
    assert!(
        es.iter()
            .any(|w| w.contains("se supuso 40 unidades por mm")),
        "{es:?}"
    );
    let en = with_locale(Locale::En, || import_file(&plt).unwrap().warnings);
    assert!(
        en.iter().any(|w| w.contains("assumed 40 units per mm")),
        "{en:?}"
    );
    assert!(en.iter().all(|w| !w.contains("se supuso")), "{en:?}");
    std::fs::remove_dir_all(&d).ok();
}

#[test]
fn gcode_with_zero_power_has_a_stable_code_in_every_language() {
    let d = dir("zero-power");
    let nc = write(&d, "cero.nc", b"M3 S0\nG1 X10 Y0 F1000\nG1 X10 Y10\nM5\n");
    let run = || {
        import_file_extras(&nc, &ImportOptions::default(), &ImportExtras::default()).unwrap_err()
    };

    let es = run();
    assert_eq!(split_code(&es).0, Some("gcode_zero_power"), "{es}");
    assert!(split_code(&es).1.contains("potencia 0"));

    let en = with_locale(Locale::En, run);
    assert_eq!(split_code(&en).0, Some("gcode_zero_power"), "{en}");
    assert!(split_code(&en).1.contains("power 0"), "{en}");
    assert!(!split_code(&en).1.contains("potencia"), "{en}");

    // Sin potencia 0 (nada que dibujar de otro modo) no lleva código.
    let none = write(&d, "nada.nc", b"G0 X1 Y1\n");
    let e = import_file(&none).unwrap_err();
    assert_eq!(split_code(&e).0, None, "{e}");
    std::fs::remove_dir_all(&d).ok();
}

#[test]
fn no_catalog_fallback_was_needed_by_the_importers() {
    // Cualquier clave que falte en un idioma queda registrada; las pruebas de arriba ya las usaron.
    let missing: Vec<_> = tlaku_i18n::take_fallbacks()
        .into_iter()
        .filter(|f| f.contains("pipeline.import"))
        .collect();
    assert!(missing.is_empty(), "{missing:?}");
}
