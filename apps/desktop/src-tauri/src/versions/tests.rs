//! Pruebas del historial de versiones: almacén, duplicados, atomicidad, retención y restauración.

use super::config::{Location, VersionsConfig};
use super::project::{self, BESIDE_DIR};
use super::retention::{DAY_MS, HOUR_MS, Policy, plan};
use super::store::{Kind, NewVersion, Store, VersionInfo};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use tlaku_core::bezpath::BezPath;
use tlaku_core::{Point, Polyline, Rgb};
use tlaku_doc::affine::IDENTITY;
use tlaku_doc::{DesignObject, Document, LayerSettings, ObjectKind, VectorObject};

static COUNTER: AtomicU32 = AtomicU32::new(0);

struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("tlaku-versions-{}-{n}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }
    fn root(&self) -> PathBuf {
        self.0.join("versiones")
    }
    fn project(&self, name: &str) -> String {
        self.0.join(name).to_string_lossy().into_owned()
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn square(id: &str, size: f64) -> DesignObject {
    let red = Rgb(255, 0, 0);
    DesignObject {
        id: id.into(),
        layer: red,
        transform: IDENTITY,
        kind: ObjectKind::Vector(VectorObject::new(vec![BezPath::from(&Polyline {
            points: vec![
                Point::new(10.0, 10.0),
                Point::new(10.0 + size, 10.0),
                Point::new(10.0 + size, 10.0 + size),
                Point::new(10.0, 10.0 + size),
            ],
            closed: true,
        })])),
        locked: false,
        hidden: false,
        group: None,
        priority: None,
        power_scale: None,
        shape: None,
        start: None,
        manual_tabs: Vec::new(),
        array: None,
    }
}

fn doc_with(n: usize) -> Document {
    Document {
        objects: (0..n)
            .map(|i| square(&format!("o{i}"), 10.0 + i as f64))
            .collect(),
        layers: vec![LayerSettings::for_color(Rgb(255, 0, 0))],
        ..Document::default()
    }
}

fn new_version(text: &str, kind: Kind, at: u64) -> NewVersion<'_> {
    NewVersion {
        kind,
        label: None,
        created_ms: at,
        text,
        objects: 0,
        layers: 0,
        thumb: None,
    }
}

fn info(id: &str, kind: Kind, at: u64, bytes: u64) -> VersionInfo {
    VersionInfo {
        id: id.into(),
        kind,
        label: (kind == Kind::Manual).then(|| "hito".to_string()),
        created_ms: at,
        size_bytes: bytes,
        stored_bytes: bytes,
        hash: id.into(),
        objects: 0,
        layers: 0,
        has_thumb: false,
    }
}

fn cfg() -> VersionsConfig {
    VersionsConfig::default()
}

fn files_in(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = fs::read_dir(dir)
        .map(|r| {
            r.flatten()
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    v.sort();
    v
}

// ---------------------------------------------------------------- almacén

#[test]
fn guarda_lee_comprime_y_lista_de_nueva_a_vieja() {
    let tmp = TempDir::new();
    let store = Store::new(tmp.root().join("p"));
    let big = "x".repeat(50_000);
    let a = store
        .add(new_version(&big, Kind::Saved, 1_000))
        .unwrap()
        .unwrap();
    let b = store
        .add(new_version("segundo", Kind::Auto, 2_000))
        .unwrap()
        .unwrap();
    assert!(
        a.stored_bytes < a.size_bytes / 10,
        "gzip debe comprimir texto repetitivo"
    );
    assert_eq!(store.read_text(&a.id).unwrap(), big);
    assert_eq!(store.read_text(&b.id).unwrap(), "segundo");
    let ids: Vec<String> = store.list().into_iter().map(|v| v.id).collect();
    assert_eq!(ids, vec![b.id, a.id]);
}

#[test]
fn no_repite_el_mismo_contenido_que_la_ultima() {
    let tmp = TempDir::new();
    let store = Store::new(tmp.root().join("p"));
    assert!(
        store
            .add(new_version("igual", Kind::Auto, 1_000))
            .unwrap()
            .is_some()
    );
    assert!(
        store
            .add(new_version("igual", Kind::Auto, 5_000))
            .unwrap()
            .is_none()
    );
    assert!(
        store
            .add(new_version("igual", Kind::Saved, 6_000))
            .unwrap()
            .is_none()
    );
    assert_eq!(store.list().len(), 1);
    // Otro contenido sí, y volver al anterior también (ya no es la última).
    assert!(
        store
            .add(new_version("otro", Kind::Auto, 7_000))
            .unwrap()
            .is_some()
    );
    assert!(
        store
            .add(new_version("igual", Kind::Auto, 8_000))
            .unwrap()
            .is_some()
    );
    assert_eq!(store.list().len(), 3);
}

#[test]
fn un_guardado_repetido_del_mismo_archivo_no_duplica_ni_con_otra_en_medio() {
    // El mismo archivo (misma fecha y contenido) capturado dos veces tiene el mismo id.
    let tmp = TempDir::new();
    let store = Store::new(tmp.root().join("p"));
    store
        .add(new_version("a", Kind::Saved, 1_000))
        .unwrap()
        .unwrap();
    store
        .add(new_version("b", Kind::Auto, 2_000))
        .unwrap()
        .unwrap();
    assert!(
        store
            .add(new_version("a", Kind::Saved, 1_000))
            .unwrap()
            .is_none()
    );
    assert_eq!(store.list().len(), 2);
}

#[test]
fn la_manual_con_el_mismo_contenido_nombra_a_la_ultima_en_vez_de_duplicar() {
    let tmp = TempDir::new();
    let store = Store::new(tmp.root().join("p"));
    store
        .add(new_version("doc", Kind::Auto, 1_000))
        .unwrap()
        .unwrap();
    let mut manual = new_version("doc", Kind::Manual, 2_000);
    manual.label = Some("Entrega".into());
    let named = store.add(manual).unwrap().unwrap();
    assert_eq!(named.kind, Kind::Manual);
    assert_eq!(named.label.as_deref(), Some("Entrega"));
    let list = store.list();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].kind, Kind::Manual);
}

#[test]
fn una_version_a_medias_no_existe_y_la_limpieza_la_borra() {
    let tmp = TempDir::new();
    let dir = tmp.root().join("p");
    let store = Store::new(dir.clone());
    let ok = store
        .add(new_version("buena", Kind::Auto, 1_000))
        .unwrap()
        .unwrap();
    // Corte justo antes de escribir el meta: queda el documento, sin meta.
    fs::write(dir.join("0000000009999-deadbeef.tlaku.gz"), b"basura").unwrap();
    fs::write(dir.join("0000000009999-deadbeef.thumb.svg.gz"), b"basura").unwrap();
    fs::write(dir.join("algo.meta.json.123.4.tmp"), b"{").unwrap();
    // Meta ilegible: tampoco existe.
    fs::write(dir.join("0000000008888-cafe.meta.json"), b"{no es json").unwrap();
    let listed = store.list();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, ok.id);

    store.prune(10_000, cfg().policy());
    let names = files_in(&dir);
    assert!(
        names.iter().all(|n| !n.starts_with("0000000009999")),
        "{names:?}"
    );
    assert!(names.iter().all(|n| !n.ends_with(".tmp")), "{names:?}");
    assert_eq!(store.read_text(&ok.id).unwrap(), "buena");
}

#[test]
fn una_escritura_exitosa_no_deja_temporales() {
    let tmp = TempDir::new();
    let dir = tmp.root().join("p");
    let store = Store::new(dir.clone());
    store.add(new_version("uno", Kind::Auto, 1_000)).unwrap();
    assert!(files_in(&dir).iter().all(|n| !n.ends_with(".tmp")));
}

#[test]
fn si_el_disco_falla_devuelve_error_y_no_deja_una_version_a_medias() {
    let tmp = TempDir::new();
    // La «carpeta» del proyecto es un archivo: no se puede escribir dentro.
    let blocker = tmp.root().join("p");
    fs::create_dir_all(tmp.root()).unwrap();
    fs::write(&blocker, b"soy un archivo").unwrap();
    let store = Store::new(blocker.clone());
    assert!(store.add(new_version("x", Kind::Auto, 1_000)).is_err());
    assert_eq!(fs::read(&blocker).unwrap(), b"soy un archivo");
    assert!(store.list().is_empty());
}

#[test]
fn rechaza_ids_con_rutas() {
    let tmp = TempDir::new();
    let store = Store::new(tmp.root().join("p"));
    assert!(store.read_text("../secreto").is_err());
    assert!(store.delete(&["a/b".to_string()]).is_err());
}

#[test]
fn borrar_quita_la_version_y_solo_esa() {
    let tmp = TempDir::new();
    let store = Store::new(tmp.root().join("p"));
    let a = store
        .add(new_version("a", Kind::Auto, 1_000))
        .unwrap()
        .unwrap();
    let b = store
        .add(new_version("b", Kind::Auto, 2_000))
        .unwrap()
        .unwrap();
    assert_eq!(store.delete(std::slice::from_ref(&a.id)).unwrap(), 1);
    assert_eq!(store.delete(std::slice::from_ref(&a.id)).unwrap(), 0);
    let ids: Vec<String> = store.list().into_iter().map(|v| v.id).collect();
    assert_eq!(ids, vec![b.id]);
}

// ---------------------------------------------------------------- retención

fn policy() -> Policy {
    Policy {
        recent_hours: 24,
        daily_days: 30,
        weekly_weeks: 26,
        max_bytes: u64::MAX,
        max_versions: 10_000,
    }
}

const NOW: u64 = 400 * DAY_MS + 12 * HOUR_MS;

#[test]
fn conserva_todas_las_de_24_horas() {
    let v: Vec<VersionInfo> = (0..50)
        .map(|i| info(&format!("r{i}"), Kind::Auto, NOW - i * 10 * 60_000, 10))
        .collect();
    assert!(plan(&v, NOW, policy()).is_empty());
}

#[test]
fn una_por_dia_despues_de_24_horas_y_la_mas_nueva_de_cada_dia() {
    // Tres versiones el mismo día, hace 5 días.
    let day = NOW - 5 * DAY_MS;
    let base = day - day % DAY_MS;
    let v = vec![
        info("d1", Kind::Auto, base + HOUR_MS, 10),
        info("d2", Kind::Auto, base + 5 * HOUR_MS, 10),
        info("d3", Kind::Saved, base + 9 * HOUR_MS, 10),
        info("hoy", Kind::Auto, NOW - HOUR_MS, 10),
    ];
    let mut doomed = plan(&v, NOW, policy());
    doomed.sort();
    assert_eq!(
        doomed,
        vec!["d1", "d2"],
        "queda d3, la más nueva de ese día"
    );
}

#[test]
fn una_por_semana_despues_de_30_dias_y_nada_despues_del_horizonte() {
    let weeks_ago = |w: u64, h: u64| {
        let t = NOW - w * 7 * DAY_MS;
        t - t % (7 * DAY_MS) + h * HOUR_MS
    };
    let v = vec![
        info("s1a", Kind::Auto, weeks_ago(8, 1), 10),
        info("s1b", Kind::Auto, weeks_ago(8, 30), 10),
        info("s2", Kind::Auto, weeks_ago(12, 1), 10),
        info("vieja", Kind::Auto, NOW - 27 * 7 * DAY_MS, 10),
        info("hoy", Kind::Auto, NOW, 10),
    ];
    let mut doomed = plan(&v, NOW, policy());
    doomed.sort();
    assert_eq!(doomed, vec!["s1a", "vieja"]);
}

#[test]
fn las_manuales_con_nombre_y_la_mas_nueva_nunca_se_borran() {
    let v = vec![
        info("hito", Kind::Manual, NOW - 300 * DAY_MS, 10),
        info("hito2", Kind::Manual, NOW - 300 * DAY_MS + 1, 10),
        info("unica", Kind::Auto, NOW - 200 * DAY_MS, 10),
    ];
    assert!(
        plan(&v, NOW, policy()).is_empty(),
        "la más nueva es `unica`"
    );
    let tiny = Policy {
        max_bytes: 1,
        max_versions: 10,
        ..policy()
    };
    assert!(
        plan(&v, NOW, tiny).is_empty(),
        "ni el tope de tamaño toca lo protegido"
    );
}

#[test]
fn el_tope_de_tamano_borra_las_mas_viejas_primero() {
    let v: Vec<VersionInfo> = (0..6)
        .map(|i| info(&format!("v{i}"), Kind::Auto, NOW - (6 - i) * 60_000, 100))
        .collect();
    let p = Policy {
        max_bytes: 250,
        ..policy()
    };
    assert_eq!(plan(&v, NOW, p), vec!["v0", "v1", "v2", "v3"]);
}

#[test]
fn el_tope_de_numero_borra_las_mas_viejas_primero() {
    let v: Vec<VersionInfo> = (0..5)
        .map(|i| info(&format!("v{i}"), Kind::Auto, NOW - (5 - i) * 60_000, 1))
        .collect();
    let p = Policy {
        max_versions: 3,
        ..policy()
    };
    assert_eq!(plan(&v, NOW, p), vec!["v0", "v1"]);
}

#[test]
fn sin_escalon_diario_pasa_directo_al_semanal() {
    let p = Policy {
        daily_days: 0,
        ..policy()
    };
    let t = NOW - 3 * DAY_MS;
    let v = vec![
        info("a", Kind::Auto, t, 1),
        info("b", Kind::Auto, t + 1_000, 1),
        info("hoy", Kind::Auto, NOW, 1),
    ];
    // Misma semana: queda la más nueva.
    assert_eq!(plan(&v, NOW, p), vec!["a"]);
}

#[test]
fn la_poda_del_disco_aplica_el_plan() {
    let tmp = TempDir::new();
    let store = Store::new(tmp.root().join("p"));
    let old = store
        .add(new_version("viejo-1", Kind::Auto, NOW - 5 * DAY_MS))
        .unwrap()
        .unwrap();
    store
        .add(new_version("viejo-2", Kind::Auto, NOW - 5 * DAY_MS + 1_000))
        .unwrap()
        .unwrap();
    store
        .add(new_version("nuevo", Kind::Auto, NOW - 1_000))
        .unwrap()
        .unwrap();
    let removed = store.prune(NOW, policy());
    assert_eq!(removed, vec![old.id.clone()]);
    assert_eq!(store.list().len(), 2);
    assert!(store.read_text(&old.id).is_err());
}

#[test]
fn la_configuracion_se_acota() {
    let c = VersionsConfig {
        auto_minutes: 9_999,
        recent_hours: 0,
        max_mb: 0,
        max_versions: 1,
        daily_days: 99_999,
        weekly_weeks: 99_999,
        ..VersionsConfig::default()
    }
    .clamped();
    assert_eq!(c.auto_minutes, 240);
    assert_eq!(c.recent_hours, 1);
    assert_eq!(c.max_mb, 1);
    assert_eq!(c.max_versions, 10);
    assert_eq!(c.daily_days, 365);
    assert_eq!(c.weekly_weeks, 520);
}

#[test]
fn la_configuracion_se_guarda_y_un_archivo_danado_vuelve_a_fabrica() {
    let tmp = TempDir::new();
    let dir = tmp.0.join("config");
    assert_eq!(super::config::load(&dir), VersionsConfig::default());
    let c = VersionsConfig {
        auto_minutes: 0,
        location: Location::Beside,
        ..VersionsConfig::default()
    };
    super::config::save(&dir, &c).unwrap();
    assert_eq!(super::config::load(&dir), c);
    fs::write(dir.join("versions.json"), b"{roto").unwrap();
    assert_eq!(super::config::load(&dir), VersionsConfig::default());
}

// ---------------------------------------------------------------- proyecto

#[test]
fn captura_el_archivo_antes_de_sobrescribirlo() {
    let tmp = TempDir::new();
    let path = tmp.project("mesa.tlaku");
    let old = doc_with(2);
    fs::write(&path, old.to_json().unwrap()).unwrap();

    let v = project::capture_file(&tmp.root(), &cfg(), &path, 5_000)
        .unwrap()
        .expect("se guarda");
    assert_eq!(v.kind, Kind::Saved);
    assert_eq!((v.objects, v.layers), (2, 1));
    assert!(v.has_thumb);
    // La fecha de la versión es la del archivo, no la de «ahora».
    assert_ne!(v.created_ms, 5_000);

    // Sobrescribir el archivo no cambia la versión guardada.
    fs::write(&path, doc_with(5).to_json().unwrap()).unwrap();
    let restored = project::read_doc(&tmp.root(), &cfg(), &path, &v.id).unwrap();
    assert_eq!(restored.objects.len(), 2);
    assert_eq!(restored, old);
}

#[test]
fn capturar_dos_veces_el_mismo_archivo_no_duplica() {
    let tmp = TempDir::new();
    let path = tmp.project("mesa.tlaku");
    fs::write(&path, doc_with(1).to_json().unwrap()).unwrap();
    assert!(
        project::capture_file(&tmp.root(), &cfg(), &path, 1)
            .unwrap()
            .is_some()
    );
    assert!(
        project::capture_file(&tmp.root(), &cfg(), &path, 2)
            .unwrap()
            .is_none()
    );
    assert_eq!(
        project::list(&tmp.root(), &cfg(), &path)
            .unwrap()
            .versions
            .len(),
        1
    );
}

#[test]
fn sin_archivo_o_con_el_historial_apagado_no_captura() {
    let tmp = TempDir::new();
    let path = tmp.project("nuevo.tlaku");
    assert!(
        project::capture_file(&tmp.root(), &cfg(), &path, 1)
            .unwrap()
            .is_none()
    );
    fs::write(&path, doc_with(1).to_json().unwrap()).unwrap();
    let off = VersionsConfig {
        enabled: false,
        ..cfg()
    };
    assert!(
        project::capture_file(&tmp.root(), &off, &path, 1)
            .unwrap()
            .is_none()
    );
    assert!(
        project::list(&tmp.root(), &off, &path)
            .unwrap()
            .versions
            .is_empty()
    );
}

#[test]
fn instantaneas_automaticas_manuales_y_de_restauracion() {
    let tmp = TempDir::new();
    let path = tmp.project("mesa.tlaku");
    let d1 = doc_with(1);
    let d2 = doc_with(2);
    let r = tmp.root();
    let c = cfg();

    let auto = project::snapshot(&r, &c, &path, &d1, Kind::Auto, None, 1_000)
        .unwrap()
        .unwrap();
    assert_eq!(auto.kind, Kind::Auto);
    // Sin cambios: nada.
    assert!(
        project::snapshot(&r, &c, &path, &d1, Kind::Auto, None, 2_000)
            .unwrap()
            .is_none()
    );

    // La manual necesita nombre.
    assert!(project::snapshot(&r, &c, &path, &d2, Kind::Manual, None, 3_000).is_err());
    assert!(project::snapshot(&r, &c, &path, &d2, Kind::Manual, Some("  ".into()), 3_000).is_err());
    let manual = project::snapshot(
        &r,
        &c,
        &path,
        &d2,
        Kind::Manual,
        Some(" Entrega ".into()),
        3_000,
    )
    .unwrap()
    .unwrap();
    assert_eq!(manual.label.as_deref(), Some("Entrega"));

    // `Saved` no se acepta desde la interfaz.
    assert!(project::snapshot(&r, &c, &path, &d2, Kind::Saved, None, 4_000).is_err());

    // Auto apagado: las automáticas no se guardan, las manuales sí.
    let no_auto = VersionsConfig {
        auto_minutes: 0,
        ..c.clone()
    };
    assert!(
        project::snapshot(&r, &no_auto, &path, &doc_with(3), Kind::Auto, None, 5_000)
            .unwrap()
            .is_none()
    );
    assert!(
        project::snapshot(
            &r,
            &no_auto,
            &path,
            &doc_with(3),
            Kind::Manual,
            Some("tres".into()),
            5_000
        )
        .unwrap()
        .is_some()
    );

    assert_eq!(project::list(&r, &c, &path).unwrap().versions.len(), 3);
}

#[test]
fn restaurar_conserva_el_estado_actual_para_deshacer() {
    // Flujo de «Restaurar»: se guarda el estado actual (Restore) y luego se lee la versión elegida.
    let tmp = TempDir::new();
    let path = tmp.project("mesa.tlaku");
    let r = tmp.root();
    let c = cfg();
    let before = doc_with(1);
    let current = doc_with(4);
    let target = project::snapshot(&r, &c, &path, &before, Kind::Auto, None, 1_000)
        .unwrap()
        .unwrap();

    let safety = project::snapshot(&r, &c, &path, &current, Kind::Restore, None, 2_000)
        .unwrap()
        .unwrap();
    assert_eq!(safety.kind, Kind::Restore);
    let got = project::read_doc(&r, &c, &path, &target.id).unwrap();
    assert_eq!(got, before);
    // Deshacer la restauración = restaurar la versión de seguridad.
    assert_eq!(
        project::read_doc(&r, &c, &path, &safety.id).unwrap(),
        current
    );
}

#[test]
fn las_rutas_de_datos_se_guardan_relativas_y_se_restauran_absolutas() {
    let tmp = TempDir::new();
    let path = tmp.project("mesa.tlaku");
    let csv = tmp.0.join("datos.csv").to_string_lossy().into_owned();
    let mut doc = doc_with(1);
    doc.variable_text.csv_path = Some(csv.clone());
    let v = project::snapshot(
        &tmp.root(),
        &cfg(),
        &path,
        &doc,
        Kind::Manual,
        Some("csv".into()),
        1,
    )
    .unwrap()
    .unwrap();
    let store_dir = project::app_dir(&tmp.root(), Path::new(&path));
    let raw = Store::new(store_dir).read_text(&v.id).unwrap();
    assert!(raw.contains("datos.csv") && !raw.contains(&csv), "{raw}");
    let back = project::read_doc(&tmp.root(), &cfg(), &path, &v.id).unwrap();
    assert_eq!(back.variable_text.sources[0].path, csv);
}

#[test]
fn la_miniatura_es_un_svg_y_un_diseno_vacio_no_la_tiene() {
    let svg = project::thumbnail(&doc_with(2)).expect("miniatura");
    assert!(svg.contains("<svg"));
    assert!(!svg.contains("<image"));
    assert!(project::thumbnail(&Document::default()).is_none());
}

#[test]
fn borrar_listar_y_limpiar_el_historial_de_un_proyecto() {
    let tmp = TempDir::new();
    let path = tmp.project("mesa.tlaku");
    let r = tmp.root();
    let c = cfg();
    let a = project::snapshot(&r, &c, &path, &doc_with(1), Kind::Auto, None, 1_000)
        .unwrap()
        .unwrap();
    project::snapshot(&r, &c, &path, &doc_with(2), Kind::Auto, None, 2_000)
        .unwrap()
        .unwrap();

    let projects = project::projects(&r);
    assert_eq!(projects.len(), 1);
    assert_eq!(projects[0].name, "mesa.tlaku");
    assert_eq!(projects[0].versions, 2);
    assert!(!projects[0].exists);

    assert_eq!(
        project::delete(&r, &c, &path, std::slice::from_ref(&a.id)).unwrap(),
        1
    );
    assert_eq!(project::list(&r, &c, &path).unwrap().versions.len(), 1);
    assert!(project::read_doc(&r, &c, &path, &a.id).is_err());

    assert_eq!(project::clear(&r, &c, &path).unwrap(), 1);
    assert!(project::list(&r, &c, &path).unwrap().versions.is_empty());
    assert!(project::projects(&r).is_empty());
}

#[test]
fn el_historial_junto_al_archivo_vive_en_una_carpeta_oculta_y_se_ve_desde_ambos_modos() {
    let tmp = TempDir::new();
    let path = tmp.project("mesa.tlaku");
    let r = tmp.root();
    let beside = VersionsConfig {
        location: Location::Beside,
        ..cfg()
    };

    let v = project::snapshot(
        &r,
        &beside,
        &path,
        &doc_with(1),
        Kind::Manual,
        Some("junto".into()),
        1_000,
    )
    .unwrap()
    .unwrap();
    assert!(tmp.0.join(BESIDE_DIR).is_dir());
    assert!(!r.exists(), "nada en la carpeta de datos de la app");

    // Cambiar la preferencia a «carpeta de la app» sigue viendo y leyendo lo de junto al archivo.
    let app = cfg();
    assert_eq!(project::list(&r, &app, &path).unwrap().versions.len(), 1);
    assert_eq!(
        project::read_doc(&r, &app, &path, &v.id).unwrap(),
        doc_with(1)
    );
    // …y lo nuevo ya va a la carpeta de la app.
    project::snapshot(&r, &app, &path, &doc_with(2), Kind::Auto, None, 2_000)
        .unwrap()
        .unwrap();
    assert!(r.is_dir());
    assert_eq!(project::list(&r, &app, &path).unwrap().versions.len(), 2);

    assert_eq!(project::clear(&r, &app, &path).unwrap(), 2);
    assert!(!tmp.0.join(BESIDE_DIR).exists());
}

#[test]
fn dos_proyectos_con_el_mismo_nombre_en_carpetas_distintas_no_se_mezclan() {
    let tmp = TempDir::new();
    let r = tmp.root();
    let a = tmp.project("a/mesa.tlaku");
    let b = tmp.project("b/mesa.tlaku");
    project::snapshot(&r, &cfg(), &a, &doc_with(1), Kind::Auto, None, 1)
        .unwrap()
        .unwrap();
    assert!(project::list(&r, &cfg(), &b).unwrap().versions.is_empty());
}

#[test]
fn la_poda_corre_despues_de_cada_escritura() {
    let tmp = TempDir::new();
    let path = tmp.project("mesa.tlaku");
    let r = tmp.root();
    let c = VersionsConfig {
        max_versions: 10,
        ..cfg()
    };
    for i in 1..=14u64 {
        project::snapshot(
            &r,
            &c,
            &path,
            &doc_with(i as usize),
            Kind::Auto,
            None,
            i * 1_000,
        )
        .unwrap();
    }
    let list = project::list(&r, &c, &path).unwrap().versions;
    assert_eq!(list.len(), 10);
    assert_eq!(list[0].created_ms, 14_000);
}
