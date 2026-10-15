//! «Alinear con el cabezal» de punta a punta: la máquina virtual mueve el cabezal con el láser
//! apagado y una cámara simulada genera cada foto según la posición que reporta el emulador.

use super::*;
use crate::edit::camera::{
    AlignRig, FlowEvent, FlowOptions, HeadAlignConfig, run_head_align_flow,
};
use base64::Engine as _;
use image::{ImageFormat, RgbImage};
use std::io::Cursor;
use tlaku_camera::head_align::sim::SimScene;
use tlaku_camera::{HeadAlignIssue, HeadMethod, PlanOptions, UndistortMap};

const BED: [f64; 2] = [400.0, 415.0];

fn png_b64(img: &RgbImage) -> String {
    let mut buf = Cursor::new(Vec::new());
    img.write_to(&mut buf, ImageFormat::Png).unwrap();
    base64::engine::general_purpose::STANDARD.encode(buf.into_inner())
}

/// Máquina virtual + cámara simulada: lo mismo que la app hace con una máquina y una cámara reales.
struct VirtualRig {
    rig: Rig,
    scene: SimScene,
    /// Corrige las fotos con la lente (como la interfaz cuando hay calibración).
    undistort: Option<UndistortMap>,
    shots: u64,
    visited: Vec<[f64; 2]>,
    events: Vec<FlowEvent>,
}

impl VirtualRig {
    fn new(scene: SimScene, corrected: bool) -> Self {
        let rig = connect_virtual("virtual:falcon2@50");
        rig.sink.wait("estado", |s| !s.statuses().is_empty());
        let undistort = corrected
            .then(|| UndistortMap::new(&scene.lens, (scene.lens.width, scene.lens.height)).unwrap());
        Self {
            rig,
            scene,
            undistort,
            shots: 0,
            visited: Vec::new(),
            events: Vec::new(),
        }
    }
}

impl AlignRig for VirtualRig {
    fn move_to(&mut self, mm: [f64; 2]) -> Result<(), String> {
        self.rig.handle.jog_to(mm[0], mm[1], 6000.0)?;
        assert!(emu(&self.rig).wait_idle(Duration::from_secs(30)), "no se detuvo");
        self.visited.push(mm);
        Ok(())
    }

    fn capture(&mut self) -> Result<String, String> {
        // La foto sale de la posición REAL del emulador, no de la pedida.
        let w = emu(&self.rig).wpos();
        self.shots += 1;
        let img = self.scene.render_rgb(Some([w[0], w[1]]), self.shots);
        let img = match &self.undistort {
            Some(m) => m.undistort(&img)?,
            None => img,
        };
        Ok(png_b64(&img))
    }

    fn event(&mut self, e: &FlowEvent) {
        self.events.push(e.clone());
    }
}

fn options(scene: &SimScene, corrected: bool) -> FlowOptions {
    FlowOptions::new(
        PlanOptions {
            bed: BED,
            grid: 3,
            margin_mm: 60.0,
            region: None,
        },
        HeadAlignConfig {
            bed: BED,
            lens: corrected.then(|| scene.lens.clone()),
            ..HeadAlignConfig::default()
        },
    )
}

/// Con el láser apagado todo el tiempo: ni `M3`/`M4`, ni potencia, ni trazo quemado.
fn assert_laser_never_on(rig: &VirtualRig) {
    let e = emu(&rig.rig);
    assert!(!e.laser_on());
    let lines = e.received_lines();
    assert!(
        lines.iter().all(|l| {
            let u = l.to_ascii_uppercase();
            !u.starts_with("M3") && !u.starts_with("M4")
        }),
        "{lines:?}"
    );
    assert!(
        e.trace().segments().iter().all(|s| s.s_peak <= 0.0 && s.energy <= 0.0),
        "el emulador registró un trazo con láser"
    );
}

#[test]
fn the_head_visits_the_grid_with_the_laser_off_and_the_alignment_is_found() {
    let scene = SimScene::fisheye(BED, 330.0);
    let mut rig = VirtualRig::new(scene.clone(), true);
    let r = run_head_align_flow(&mut rig, &options(&scene, true)).unwrap();
    // 3x3 más la referencia, sin rondas de afinado.
    assert_eq!(rig.visited.len(), 10);
    assert_eq!(r.rounds, 0);
    assert_eq!(r.frames.len(), 10);
    assert_eq!(r.report.method, HeadMethod::Led);
    assert_eq!(r.report.detected, 10);
    let fit = r.report.fit.as_ref().expect("ajuste");
    assert!(fit.loo_rms_mm < 1.0 && fit.rms_mm < 1.0, "{fit:?}");
    assert!(r.report.issues.is_empty(), "{:?}", r.report.issues);
    // Las posiciones que se fotografiaron son las que reportó el emulador.
    for (f, v) in r.frames.iter().zip(&rig.visited) {
        assert_eq!(f.machine_mm, *v);
    }
    // Lista para guardar como la alineación de la cámara.
    let a = r.report.alignment.as_ref().expect("alineación");
    assert_eq!(a.image_size, [800, 600]);
    assert_eq!(a.method, crate::edit::camera::AlignmentMethod::HeadSweep);
    assert_eq!(a.samples.len(), 1);
    assert_eq!(a.samples[0].image_px.len(), 10);
    assert!(a.rms_mm < 1.0);
    // Avance: una ronda y 10 fotos.
    assert_eq!(
        rig.events
            .iter()
            .filter(|e| matches!(e, FlowEvent::Shot { .. }))
            .count(),
        10
    );
    assert_laser_never_on(&rig);
}

#[test]
fn a_camera_that_sees_only_the_middle_triggers_a_finer_grid_where_the_head_was_seen() {
    // Cámara baja: solo ve el cabezal en una franja alrededor del centro de la cama, como la de
    // la tapa de una Falcon2 Pro.
    let mut scene = SimScene::fisheye(BED, 85.0);
    scene.camera_mm[0] = 200.0;
    scene.camera_mm[1] = 207.0;
    let mut rig = VirtualRig::new(scene.clone(), true);
    let r = run_head_align_flow(&mut rig, &options(&scene, true)).unwrap();
    assert!(r.rounds >= 1, "sin rondas de afinado");
    assert!(rig.visited.len() > 10);
    assert!(r.report.detected >= 5, "{} detecciones", r.report.detected);
    let fit = r.report.fit.as_ref().expect("ajuste");
    assert!(fit.loo_rms_mm < 1.5, "loo {:.3}", fit.loo_rms_mm);
    // Lo que no se vio queda dicho: fotos sin cabezal y cobertura parcial de la cama.
    assert!(r.report.failed > 0);
    assert!(r
        .report
        .issues
        .iter()
        .any(|i| matches!(i, HeadAlignIssue::NotDetected { .. })));
    assert!(r
        .report
        .issues
        .iter()
        .any(|i| matches!(i, HeadAlignIssue::PartialCoverage { .. })));
    let c = r.report.covered_mm.expect("zona cubierta");
    assert!(c[0] > 60.0 && c[2] < 340.0, "{c:?}");
    assert_laser_never_on(&rig);
}

#[test]
fn without_a_lens_the_flow_estimates_one_and_returns_it_with_the_alignment() {
    let scene = SimScene::barrel(BED, 330.0);
    let mut rig = VirtualRig::new(scene.clone(), false);
    let mut opts = options(&scene, false);
    opts.plan.grid = 4;
    let r = run_head_align_flow(&mut rig, &opts).unwrap();
    assert_eq!(r.report.detected, 17);
    let fit = r.report.fit.as_ref().expect("ajuste");
    assert!(fit.loo_rms_mm < 1.0, "loo {:.3}", fit.loo_rms_mm);
    let lens = r.report.lens.as_ref().expect("lente estimada");
    assert_eq!((lens.width, lens.height), (800, 600));
    assert!(r.report.alignment.is_some());
    assert_laser_never_on(&rig);
}
