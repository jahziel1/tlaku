use super::*;
use std::sync::atomic::{AtomicU32, Ordering};

static COUNTER: AtomicU32 = AtomicU32::new(0);

/// Carpeta temporal que contiene `org.tlaku.app/` (antigua) e `io.github.jahziel1.tlaku/` (nueva).
struct Sandbox(PathBuf);

impl Sandbox {
    fn new() -> Self {
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir =
            std::env::temp_dir().join(format!("tlaku-id-migration-{}-{n}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }
    fn old(&self) -> PathBuf {
        self.0.join(OLD_ID)
    }
    fn dest(&self) -> PathBuf {
        self.0.join("io.github.jahziel1.tlaku")
    }
    /// La carpeta antigua con datos de todo tipo, incluidas subcarpetas.
    fn with_old_data(self) -> Self {
        let old = self.old();
        fs::create_dir_all(old.join("versiones/proyecto")).unwrap();
        fs::create_dir_all(old.join("interrupted-job")).unwrap();
        fs::write(old.join("devices.json"), b"[\"laser\"]").unwrap();
        fs::write(old.join("preferences.json"), b"{\"version\":1}").unwrap();
        fs::write(old.join("job-history.jsonl"), b"{\"id\":\"a\"}\n").unwrap();
        fs::write(old.join("versiones/proyecto/1.meta.json"), b"{}").unwrap();
        fs::write(old.join("interrupted-job/meta.json"), b"{}").unwrap();
        self
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn read(path: &Path) -> String {
    fs::read_to_string(path).unwrap()
}

#[test]
fn copia_la_carpeta_vieja_a_la_nueva_inexistente() {
    let sb = Sandbox::new().with_old_data();
    let out = migrate_dir(&sb.old(), &sb.dest());
    assert_eq!(out, Outcome::Migrated { files: 5 });
    assert_eq!(read(&sb.dest().join("devices.json")), "[\"laser\"]");
    assert_eq!(
        read(&sb.dest().join("versiones/proyecto/1.meta.json")),
        "{}"
    );
    assert!(sb.dest().join("interrupted-job/meta.json").exists());
    assert!(sb.dest().join(DONE).exists());
    assert!(!sb.dest().join(PROGRESS).exists());
    // Copia, no mueve: la vieja queda intacta y sin marcadores ni temporales.
    assert_eq!(read(&sb.old().join("devices.json")), "[\"laser\"]");
    assert!(!sb.old().join(DONE).exists());
}

#[test]
fn copia_a_una_carpeta_nueva_vacia() {
    let sb = Sandbox::new().with_old_data();
    fs::create_dir_all(sb.dest()).unwrap();
    assert_eq!(
        migrate_dir(&sb.old(), &sb.dest()),
        Outcome::Migrated { files: 5 }
    );
    assert!(sb.dest().join("preferences.json").exists());
}

#[test]
fn una_carpeta_nueva_con_solo_webview_cuenta_como_vacia_y_no_se_copia_el_webview() {
    let sb = Sandbox::new().with_old_data();
    fs::create_dir_all(sb.dest().join("EBWebView")).unwrap();
    fs::write(sb.dest().join("EBWebView/lock"), b"x").unwrap();
    fs::create_dir_all(sb.old().join("EBWebView")).unwrap();
    fs::write(sb.old().join("EBWebView/Local Storage"), b"viejo").unwrap();
    assert_eq!(
        migrate_dir(&sb.old(), &sb.dest()),
        Outcome::Migrated { files: 5 }
    );
    assert_eq!(read(&sb.dest().join("EBWebView/lock")), "x");
    assert!(!sb.dest().join("EBWebView/Local Storage").exists());
}

/// Lo que añadió el respaldo a carpeta y las curvas de tono (viven en la carpeta de configuración):
/// la copia genérica y recursiva los lleva sin lista de archivos.
#[test]
fn copia_el_respaldo_y_los_tonos() {
    let sb = Sandbox::new();
    let old = sb.old();
    fs::create_dir_all(old.join("respaldos-locales")).unwrap();
    fs::write(old.join("backup.json"), b"{\"install_id\":\"x\"}").unwrap();
    fs::write(old.join("materials.json"), b"{\"tones\":[]}").unwrap();
    fs::write(old.join("respaldos-locales/antes-1.tlakubundle"), b"{}").unwrap();
    let out = migrate_dir(&sb.old(), &sb.dest());
    assert_eq!(out, Outcome::Migrated { files: 3 });
    assert_eq!(
        read(&sb.dest().join("backup.json")),
        "{\"install_id\":\"x\"}"
    );
    assert_eq!(read(&sb.dest().join("materials.json")), "{\"tones\":[]}");
    assert!(
        sb.dest()
            .join("respaldos-locales/antes-1.tlakubundle")
            .exists()
    );
}

#[test]
fn ya_migrado_no_repite() {
    let sb = Sandbox::new().with_old_data();
    assert!(matches!(
        migrate_dir(&sb.old(), &sb.dest()),
        Outcome::Migrated { .. }
    ));
    // La persona borra un archivo migrado y la vieja cambia: no debe reaparecer ni pisarse nada.
    fs::remove_file(sb.dest().join("devices.json")).unwrap();
    fs::write(sb.old().join("nuevo.json"), b"x").unwrap();
    assert_eq!(migrate_dir(&sb.old(), &sb.dest()), Outcome::AlreadyDone);
    assert!(!sb.dest().join("devices.json").exists());
    assert!(!sb.dest().join("nuevo.json").exists());
}

#[test]
fn la_nueva_con_datos_no_se_pisa() {
    let sb = Sandbox::new().with_old_data();
    fs::create_dir_all(sb.dest()).unwrap();
    fs::write(sb.dest().join("devices.json"), b"[\"propio\"]").unwrap();
    assert_eq!(migrate_dir(&sb.old(), &sb.dest()), Outcome::KeptNew);
    assert_eq!(read(&sb.dest().join("devices.json")), "[\"propio\"]");
    assert!(!sb.dest().join("preferences.json").exists());
    // Queda el marcador: tampoco después se mezcla.
    assert_eq!(migrate_dir(&sb.old(), &sb.dest()), Outcome::AlreadyDone);
}

#[test]
fn sin_carpeta_vieja_no_hace_nada() {
    let sb = Sandbox::new();
    assert_eq!(migrate_dir(&sb.old(), &sb.dest()), Outcome::NoOldDir);
    assert!(!sb.dest().exists());
}

#[test]
fn una_copia_interrumpida_se_retoma() {
    let sb = Sandbox::new().with_old_data();
    // Quedó a medias: marcador de progreso, un archivo copiado y un temporal.
    fs::create_dir_all(sb.dest()).unwrap();
    fs::write(sb.dest().join(PROGRESS), b"").unwrap();
    fs::write(sb.dest().join("devices.json"), b"[\"laser\"]").unwrap();
    fs::write(sb.dest().join("preferences.json.migrando"), b"par").unwrap();
    assert!(matches!(
        migrate_dir(&sb.old(), &sb.dest()),
        Outcome::Migrated { .. }
    ));
    assert_eq!(read(&sb.dest().join("preferences.json")), "{\"version\":1}");
    assert!(sb.dest().join(DONE).exists());
    assert!(!sb.dest().join(PROGRESS).exists());
}

#[cfg(unix)]
#[test]
fn si_la_copia_falla_se_deshace_y_se_puede_reintentar() {
    use std::os::unix::fs::PermissionsExt;
    let sb = Sandbox::new().with_old_data();
    let locked = sb.old().join("zzz-ilegible.json");
    fs::write(&locked, b"x").unwrap();
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();
    // Como superusuario los permisos no impiden leer: la prueba no aplica.
    if fs::read(&locked).is_ok() {
        return;
    }
    let out = migrate_dir(&sb.old(), &sb.dest());
    assert!(matches!(out, Outcome::Failed(_)), "{out:?}");
    // La nueva queda como estaba: ni siquiera existe, y la vieja intacta.
    assert!(!sb.dest().exists());
    assert_eq!(read(&sb.old().join("devices.json")), "[\"laser\"]");
    // Al arreglar el problema, el siguiente arranque migra.
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o644)).unwrap();
    assert!(matches!(
        migrate_dir(&sb.old(), &sb.dest()),
        Outcome::Migrated { .. }
    ));
}

#[test]
fn si_la_nueva_no_se_puede_crear_sigue_sin_entrar_en_panico() {
    let sb = Sandbox::new().with_old_data();
    // Un archivo ocupa el sitio de la carpeta nueva.
    fs::write(sb.dest(), b"estorbo").unwrap();
    let out = migrate_dir(&sb.old(), &sb.dest());
    assert!(matches!(out, Outcome::Failed(_)), "{out:?}");
    assert_eq!(read(&sb.dest()), "estorbo");
}

#[test]
fn plan_deduplica_y_descarta_lo_ajeno() {
    let base = PathBuf::from("/datos");
    let nueva = base.join("io.github.jahziel1.tlaku");
    let dirs = vec![
        nueva.clone(),
        nueva.clone(),
        PathBuf::from("/otro/sitio-raro"),
        PathBuf::from("/local").join("io.github.jahziel1.tlaku"),
    ];
    let plan = plan(&dirs, "io.github.jahziel1.tlaku");
    assert_eq!(
        plan,
        vec![
            (base.join(OLD_ID), nueva),
            (
                PathBuf::from("/local").join(OLD_ID),
                PathBuf::from("/local/io.github.jahziel1.tlaku")
            ),
        ]
    );
    // Con el id antiguo (otra compilación) no hay nada que migrar.
    assert!(super::plan(&[base.join(OLD_ID)], OLD_ID).is_empty());
}

#[test]
fn migrate_all_une_configuracion_y_datos_distintos() {
    let sb = Sandbox::new().with_old_data();
    // Como en Linux: configuración y datos en carpetas distintas con el mismo nombre de id.
    let cfg_root = sb.0.join("config");
    let data_root = sb.0.join("data");
    fs::create_dir_all(cfg_root.join(OLD_ID)).unwrap();
    fs::write(cfg_root.join(OLD_ID).join("devices.json"), b"[1]").unwrap();
    fs::create_dir_all(data_root.join(OLD_ID)).unwrap();
    fs::write(data_root.join(OLD_ID).join("job-history.jsonl"), b"{}\n").unwrap();
    let id = "io.github.jahziel1.tlaku";
    let dirs = [cfg_root.join(id), data_root.join(id), data_root.join(id)];
    let out = migrate_all(&dirs, id);
    assert_eq!(
        out,
        vec![
            Outcome::Migrated { files: 1 },
            Outcome::Migrated { files: 1 }
        ]
    );
    assert_eq!(read(&cfg_root.join(id).join("devices.json")), "[1]");
    assert_eq!(read(&data_root.join(id).join("job-history.jsonl")), "{}\n");
}
