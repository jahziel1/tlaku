use super::*;
use serde_json::json;
use std::sync::atomic::{AtomicU32, Ordering};

static COUNTER: AtomicU32 = AtomicU32::new(0);

struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("tlaku-backup-{}-{n}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        // Por si una prueba dejó la carpeta sin permiso de escritura.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&self.0, fs::Permissions::from_mode(0o755));
        }
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// 2026-10-15 14:30:12 UTC.
const T0: u64 = 1_792_074_612_000;

fn bundle() -> Value {
    json!({
        "format": "tlaku-bundle",
        "version": 2,
        "created": "2026-10-15T14:30:12.000Z",
        "app": "0.1.0",
        "devices": [{"id": "a", "name": "Láser A"}],
        "active_device": "a",
        "materials": {"version": 1, "presets": []},
        "costs": {"machines": {}, "materials": []},
        "remnants": {"version": 1, "remnants": []},
        "templates": {"items": []},
        "prefs": {"format": "tlaku-prefs", "version": 1, "prefs": {"theme": "dark"}, "shortcuts": {}},
    })
}

fn names(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_str().unwrap().to_string())
        .collect();
    v.sort();
    v
}

#[test]
fn el_nombre_lleva_la_fecha_y_se_lee_de_vuelta() {
    assert_eq!(
        stamp(FOLDER_PREFIX, T0),
        "tlaku-respaldo-2026-10-15_14-30-12"
    );
    assert_eq!(
        parse_name(
            FOLDER_PREFIX,
            "tlaku-respaldo-2026-10-15_14-30-12.tlakubundle"
        ),
        Some(T0)
    );
    assert_eq!(
        parse_name(
            FOLDER_PREFIX,
            "tlaku-respaldo-2026-10-15_14-30-12_3.tlakubundle"
        ),
        Some(T0)
    );
    // Año bisiesto y cambio de siglo.
    for ms in [0, 951_782_400_000, 1_709_164_800_000, 4_102_444_799_000] {
        assert_eq!(
            parse_name(
                FOLDER_PREFIX,
                &format!("{}.tlakubundle", stamp(FOLDER_PREFIX, ms))
            ),
            Some(ms)
        );
    }
}

#[test]
fn no_confunde_archivos_ajenos_con_respaldos() {
    for name in [
        "tlaku-respaldo.tlakubundle",
        "tlaku-respaldo-2026-13-15_14-30-12.tlakubundle",
        "tlaku-respaldo-2026-10-15_25-30-12.tlakubundle",
        "tlaku-respaldo-2026-10-15_14-30-12.json",
        "tlaku-respaldo-2026-10-15_14-30-12.tlakubundle.tmp",
        "otro-2026-10-15_14-30-12.tlakubundle",
        "tlaku-respaldo-2026-10-15_14-30-12_x.tlakubundle",
        "tlaku-respaldo-2026-10-15_14-30-12_2_3.tlakubundle",
    ] {
        assert_eq!(parse_name(FOLDER_PREFIX, name), None, "{name}");
    }
}

#[test]
fn escribe_un_respaldo_con_todo_el_contenido() {
    let tmp = TempDir::new();
    let w = write_backup(&tmp.0, FOLDER_PREFIX, &bundle(), "abc123", 10, T0).unwrap();
    assert_eq!(w.name, "tlaku-respaldo-2026-10-15_14-30-12.tlakubundle");
    assert!(w.removed.is_empty());
    let back: Value = serde_json::from_str(&fs::read_to_string(&w.path).unwrap()).unwrap();
    for section in [
        "devices",
        "materials",
        "costs",
        "remnants",
        "templates",
        "prefs",
    ] {
        assert_eq!(back[section], bundle()[section], "{section}");
    }
    assert_eq!(back["install_id"], "abc123");
    assert_eq!(back["version"], 2);
    // Nada de `.tmp` a medias.
    assert_eq!(names(&tmp.0), vec![w.name]);
    // Se puede volver a leer con la misma validación del paquete.
    assert!(check_user_bundle(&back).is_ok());
}

#[test]
fn dos_respaldos_en_el_mismo_segundo_no_se_pisan() {
    let tmp = TempDir::new();
    let a = write_backup(&tmp.0, FOLDER_PREFIX, &bundle(), "x", 10, T0).unwrap();
    let b = write_backup(&tmp.0, FOLDER_PREFIX, &bundle(), "x", 10, T0).unwrap();
    assert_ne!(a.name, b.name);
    let list = list_backups(&tmp.0, FOLDER_PREFIX).unwrap();
    assert_eq!(list.len(), 2);
    // El segundo (`_2`) es el más nuevo.
    assert_eq!(list[0].name, b.name);
}

#[test]
fn la_rotacion_conserva_solo_los_ultimos_y_no_toca_lo_ajeno() {
    let tmp = TempDir::new();
    fs::write(tmp.0.join("mis-notas.txt"), "no tocar").unwrap();
    fs::write(tmp.0.join("tlaku.tlakubundle"), "{}").unwrap(); // paquete exportado a mano
    fs::write(tmp.0.join("otro-2026-01-01_00-00-00.tlakubundle"), "{}").unwrap();
    let mut last = None;
    for i in 0..6u64 {
        last = Some(
            write_backup(
                &tmp.0,
                FOLDER_PREFIX,
                &bundle(),
                "x",
                3,
                T0 + i * 86_400_000,
            )
            .unwrap(),
        );
    }
    let list = list_backups(&tmp.0, FOLDER_PREFIX).unwrap();
    assert_eq!(list.len(), 3);
    // Quedan los tres más nuevos, del más nuevo al más viejo.
    assert_eq!(list[0].created_ms, T0 + 5 * 86_400_000);
    assert_eq!(list[2].created_ms, T0 + 3 * 86_400_000);
    assert_eq!(last.unwrap().removed.len(), 1);
    for ajeno in [
        "mis-notas.txt",
        "tlaku.tlakubundle",
        "otro-2026-01-01_00-00-00.tlakubundle",
    ] {
        assert!(tmp.0.join(ajeno).exists(), "{ajeno}");
    }
}

#[test]
fn conservar_cero_deja_al_menos_el_recien_escrito() {
    let tmp = TempDir::new();
    write_backup(&tmp.0, FOLDER_PREFIX, &bundle(), "x", 0, T0).unwrap();
    write_backup(&tmp.0, FOLDER_PREFIX, &bundle(), "x", 0, T0 + 1000).unwrap();
    assert_eq!(list_backups(&tmp.0, FOLDER_PREFIX).unwrap().len(), 1);
}

#[test]
fn limpia_el_tmp_huerfano_propio_pero_no_uno_reciente() {
    let tmp = TempDir::new();
    let old = tmp
        .0
        .join("tlaku-respaldo-2026-01-01_00-00-00.tlakubundle.tmp");
    let ajeno = tmp.0.join("otra-cosa.tlakubundle.tmp");
    fs::write(&old, "a medias").unwrap();
    fs::write(&ajeno, "ajeno").unwrap();
    // «Ahora» está muy lejos de la fecha de modificación real de los archivos: ambos son viejos.
    let far = modified_ms(&old).unwrap() + 3 * STALE_TMP_MS;
    write_backup(&tmp.0, FOLDER_PREFIX, &bundle(), "x", 10, far).unwrap();
    assert!(!old.exists());
    assert!(ajeno.exists());
    // Con la hora real, un `.tmp` recién escrito (otro proceso escribiendo) se respeta.
    let fresh = tmp
        .0
        .join("tlaku-respaldo-2026-02-02_00-00-00.tlakubundle.tmp");
    fs::write(&fresh, "en curso").unwrap();
    write_backup(
        &tmp.0,
        FOLDER_PREFIX,
        &bundle(),
        "x",
        10,
        modified_ms(&fresh).unwrap(),
    )
    .unwrap();
    assert!(fresh.exists());
}

#[cfg(unix)]
#[test]
fn si_la_escritura_falla_no_se_borra_nada_ni_queda_tmp() {
    use std::os::unix::fs::PermissionsExt;
    let tmp = TempDir::new();
    for i in 0..3u64 {
        write_backup(&tmp.0, FOLDER_PREFIX, &bundle(), "x", 3, T0 + i * 1000).unwrap();
    }
    let before = names(&tmp.0);
    fs::set_permissions(&tmp.0, fs::Permissions::from_mode(0o555)).unwrap();
    // `root` ignora los permisos: en ese caso la prueba no demuestra nada.
    let writable = fs::write(tmp.0.join("sonda"), "x").is_ok();
    let err = write_backup(&tmp.0, FOLDER_PREFIX, &bundle(), "x", 1, T0 + 10_000);
    fs::set_permissions(&tmp.0, fs::Permissions::from_mode(0o755)).unwrap();
    if writable {
        return;
    }
    assert!(err.is_err());
    assert_eq!(names(&tmp.0), before, "ni se rotó ni quedó un .tmp");
}

#[test]
fn una_carpeta_que_no_existe_no_se_crea_y_da_aviso() {
    let tmp = TempDir::new();
    let missing = tmp.0.join("Volumes-USB");
    let err = write_backup(&missing, FOLDER_PREFIX, &bundle(), "x", 5, T0).unwrap_err();
    assert!(err.contains("no está disponible"), "{err}");
    assert!(!missing.exists());
    // El listado de una carpeta ausente se reporta como no disponible, no como error.
    let mut cfg = BackupConfig::default();
    cfg.settings.folder = Some(missing.display().to_string());
    let list = list_configured(&cfg);
    assert!(list.configured && !list.available && list.entries.is_empty());
    assert!(!list_configured(&BackupConfig::default()).configured);
}

#[test]
fn nunca_escribe_secretos() {
    let tmp = TempDir::new();
    let cases = [
        (
            "prefs.remoteToken",
            json!({"prefs": {"remoteToken": "abc"}}),
        ),
        ("token", json!({"token": "abc"})),
        ("cameras.api_key", json!({"cameras": {"api_key": "k"}})),
        (
            "devices.password",
            json!({"devices": [{"id": "a", "name": "A", "password": "p"}]}),
        ),
        (
            "x.Authorization",
            json!({"x": {"Authorization": "Bearer z"}}),
        ),
        ("access-token", json!({"access-token": "1"})),
        (
            "cameras.presets.url",
            json!({"cameras": {"presets": [{"url": "rtsp://admin:1234@192.168.0.5/stream"}]}}),
        ),
        (
            "cameras.url",
            json!({"cameras": {"url": "http://usuario@host/cam"}}),
        ),
    ];
    for (what, extra) in cases {
        let mut b = bundle();
        for (k, v) in extra.as_object().unwrap() {
            b[k] = v.clone();
        }
        let err = write_backup(&tmp.0, FOLDER_PREFIX, &b, "x", 5, T0).unwrap_err();
        assert!(err.contains("secreto"), "{what}: {err}");
    }
    assert!(names(&tmp.0).is_empty(), "no debió quedar ningún archivo");
}

#[test]
fn lo_que_no_es_un_secreto_pasa() {
    let tmp = TempDir::new();
    let mut b = bundle();
    // Un token nulo (sin valor) no es un secreto; una URL sin credenciales tampoco;
    // el texto libre de plantillas e historial puede mencionar lo que quiera.
    b["cameras"] = json!({"presets": [{"url": "rtsp://192.168.0.5:554/stream"}], "token": null});
    b["templates"] = json!({"items": [{"name": "t", "doc": {"text": "mira http://yo@sitio.com"}}]});
    b["history"] = json!({"entries": [{"note": "usuario:clave@host://x"}]});
    b["components"] = json!({"items": [{"name": "c", "data": {"objects": [{"text": "mira http://yo@sitio.com"}]}}]});
    b["prefs"]["prefs"]["keyboardLayout"] = json!("es");
    assert!(write_backup(&tmp.0, FOLDER_PREFIX, &b, "x", 5, T0).is_ok());
}

#[test]
fn la_version_del_formato_se_valida_al_escribir() {
    let tmp = TempDir::new();
    for v in [1, 2] {
        let mut b = bundle();
        b["version"] = json!(v);
        assert!(
            write_backup(&tmp.0, FOLDER_PREFIX, &b, "x", 5, T0 + v * 1000).is_ok(),
            "v{v}"
        );
    }
    for v in [json!(0), json!(3), json!("2"), Value::Null] {
        let mut b = bundle();
        b["version"] = v.clone();
        assert!(
            write_backup(&tmp.0, FOLDER_PREFIX, &b, "x", 5, T0 + 9000).is_err(),
            "{v}"
        );
    }
    let mut ajeno = bundle();
    ajeno["format"] = json!("tlaku-prefs");
    assert!(write_backup(&tmp.0, FOLDER_PREFIX, &ajeno, "x", 5, T0 + 9000).is_err());
    assert_eq!(list_backups(&tmp.0, FOLDER_PREFIX).unwrap().len(), 2);
}

#[test]
fn un_respaldo_enorme_se_rechaza_con_un_mensaje_claro() {
    let tmp = TempDir::new();
    let mut b = bundle();
    b["history"] = json!({"entries": [{"note": "x".repeat(USER_BUNDLE_MAX_BYTES as usize)}]});
    let err = write_backup(&tmp.0, FOLDER_PREFIX, &b, "x", 5, T0).unwrap_err();
    assert!(err.contains("demasiado grande"), "{err}");
    assert!(names(&tmp.0).is_empty());
}

#[test]
fn la_configuracion_se_crea_con_identificador_y_se_acota() {
    let tmp = TempDir::new();
    let cfg = load_config(&tmp.0).unwrap();
    assert_eq!(cfg.install_id.len(), 16);
    assert_eq!(
        load_config(&tmp.0).unwrap().install_id,
        cfg.install_id,
        "el identificador es estable"
    );
    let mut cfg = cfg;
    cfg.settings = BackupSettings {
        folder: Some("  /tmp/x  ".into()),
        on_close: true,
        every_days: 9999,
        keep: 0,
        include_history: true,
    }
    .sanitized();
    assert_eq!(cfg.settings.folder.as_deref(), Some("/tmp/x"));
    assert_eq!(
        (cfg.settings.every_days, cfg.settings.keep),
        (MAX_EVERY_DAYS, 1)
    );
    store_config(&tmp.0, &cfg).unwrap();
    assert_eq!(load_config(&tmp.0).unwrap(), cfg);
    assert_eq!(
        BackupSettings {
            folder: Some("   ".into()),
            ..Default::default()
        }
        .sanitized()
        .folder,
        None
    );
}

#[test]
fn una_configuracion_danada_se_aparta_y_se_parte_de_cero() {
    let tmp = TempDir::new();
    fs::write(tmp.0.join(CONFIG_FILE), "{ roto").unwrap();
    let cfg = load_config(&tmp.0).unwrap();
    assert_eq!(cfg.settings, BackupSettings::default());
    assert!(tmp.0.join("backup.json.dañado").exists());
}

#[test]
fn la_configuracion_no_lleva_secretos_ni_viaja_en_el_paquete() {
    // La ruta de la carpeta es de esta computadora: vive en backup.json, no en el paquete.
    let b = bundle();
    assert!(b.get("folder").is_none());
    let text = serde_json::to_string(&BackupConfig::default()).unwrap();
    assert!(check_no_secrets(&serde_json::from_str(&text).unwrap()).is_ok());
}

fn entry(created_ms: u64) -> BackupEntry {
    BackupEntry {
        name: format!("n{created_ms}"),
        path: String::new(),
        size_bytes: 1,
        created_ms,
    }
}

#[test]
fn detecta_un_respaldo_mas_nuevo_de_otra_instalacion() {
    let cfg = BackupConfig {
        install_id: "yo".into(),
        last_backup_ms: 1000,
        last_seen_ms: 0,
        ..Default::default()
    };
    let other = |_: &BackupEntry| Some("otra".to_string());
    let mine = |_: &BackupEntry| Some("yo".to_string());
    let unknown = |_: &BackupEntry| None;
    // Más nuevo y de otra instalación: se ofrece.
    assert_eq!(
        newer_backup(&[entry(2000), entry(900)], &cfg, other)
            .unwrap()
            .created_ms,
        2000
    );
    // Más nuevo pero mío (p. ej. se perdió el estado): no se ofrece.
    assert!(newer_backup(&[entry(2000)], &cfg, mine).is_none());
    // De una versión que no marcaba la instalación: se ofrece (es de otro lado).
    assert!(newer_backup(&[entry(2000)], &cfg, unknown).is_some());
    // Igual o más viejo que lo último respaldado aquí: no.
    assert!(newer_backup(&[entry(1000)], &cfg, other).is_none());
    assert!(newer_backup(&[entry(500)], &cfg, other).is_none());
    // Sin respaldos: nada.
    assert!(newer_backup(&[], &cfg, other).is_none());
    // Ya visto (importado o descartado): no se vuelve a ofrecer.
    let seen = BackupConfig {
        last_seen_ms: 2000,
        ..cfg.clone()
    };
    assert!(newer_backup(&[entry(2000)], &seen, other).is_none());
    assert!(newer_backup(&[entry(3000)], &seen, other).is_some());
}

#[test]
fn la_deteccion_lee_la_instalacion_del_archivo() {
    let tmp = TempDir::new();
    let a = write_backup(&tmp.0, FOLDER_PREFIX, &bundle(), "compu-b", 5, T0).unwrap();
    assert_eq!(
        install_id_of(Path::new(&a.path)).as_deref(),
        Some("compu-b")
    );
    let entries = list_backups(&tmp.0, FOLDER_PREFIX).unwrap();
    let cfg = BackupConfig {
        install_id: "compu-a".into(),
        ..Default::default()
    };
    let found = newer_backup(&entries, &cfg, |e| install_id_of(Path::new(&e.path)));
    assert_eq!(found.unwrap().name, a.name);
    let own = BackupConfig {
        install_id: "compu-b".into(),
        ..Default::default()
    };
    assert!(newer_backup(&entries, &own, |e| install_id_of(Path::new(&e.path))).is_none());
}

#[test]
fn el_respaldo_previo_a_importar_usa_su_propio_prefijo_y_no_pisa_los_de_la_carpeta() {
    let tmp = TempDir::new();
    write_backup(&tmp.0, FOLDER_PREFIX, &bundle(), "x", 1, T0).unwrap();
    write_backup(
        &tmp.0,
        SAFETY_PREFIX,
        &bundle(),
        "x",
        SAFETY_KEEP,
        T0 + 1000,
    )
    .unwrap();
    assert_eq!(list_backups(&tmp.0, FOLDER_PREFIX).unwrap().len(), 1);
    assert_eq!(list_backups(&tmp.0, SAFETY_PREFIX).unwrap().len(), 1);
    for i in 0..8u64 {
        write_backup(
            &tmp.0,
            SAFETY_PREFIX,
            &bundle(),
            "x",
            SAFETY_KEEP,
            T0 + 2000 + i * 1000,
        )
        .unwrap();
    }
    assert_eq!(
        list_backups(&tmp.0, SAFETY_PREFIX).unwrap().len(),
        SAFETY_KEEP
    );
    assert_eq!(list_backups(&tmp.0, FOLDER_PREFIX).unwrap().len(), 1);
}
