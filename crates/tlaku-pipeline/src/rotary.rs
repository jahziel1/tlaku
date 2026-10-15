//! Accesorio rotativo en lugar del eje Y.
//!
//! El documento y el `Toolpath` siempre están en mm de **superficie** de la pieza. La conversión
//! a los mm de Y de la controladora es un factor fijo, `y_scale`:
//!
//! - rodillo: `mm_per_rotation / (π · roller_diameter)`
//! - mandril: `mm_per_rotation / (π · object_diameter)`
//!
//! y se aplica al final, en el G-code y en el encuadre, a la Y **absoluta** (relativa al cero de
//! la controladora, no al inicio del trabajo). Así lo que muestra GRBL como posición Y y lo que
//! el usuario fijó como origen siguen siendo coherentes: no hace falta emitir `G92` ni mover el
//! cero. `current_position` y `user_origin` llegan en coordenadas de la máquina (mm de la
//! controladora) y se pasan a superficie con la inversa antes de colocar el trabajo.
//!
//! La velocidad del bloque se interpreta como velocidad **sobre la superficie** (así la
//! potencia por mm de grabado es la que el usuario pidió); `tlaku-grbl` reescala la F de cada
//! tramo con Y. La aceleración y la velocidad máxima de Y de la controladora se pasan a
//! superficie dividiéndolas por `y_scale`, para que el tiempo estimado y el overscan sean los
//! reales.
//!
//! - `mirror` refleja la Y del trabajo dentro de su caja al generar el G-code (ver `gcode`); la
//!   vista previa y la caja del trabajo no cambian.
//! - Con rodillo el diámetro de la pieza no entra en `y_scale` (solo el del rodillo): se valida
//!   y se usa únicamente para la circunferencia que se avisa y para [`rotary_test`].
//! - La Y de la controladora no tiene final de carrera en el rotativo: si el trabajo la lleva
//!   más allá de `bed[1]`, los límites por software (`$20=1`) darían `ALARM:2`; se avisa.

use crate::gcode::point_to_machine;
use std::f64::consts::PI;
use tlaku_core::bezpath::BezPath;
use tlaku_core::{Bounds, Point, Polyline, Rgb};
use tlaku_doc::affine::IDENTITY;
use tlaku_doc::{
    DOC_VERSION, DesignObject, DeviceKind, DeviceProfile, Document, LayerSettings, ObjectKind,
    RotaryKind, VectorObject,
};
use tlaku_i18n::tr;

/// Factor de mm de la controladora por mm de superficie en Y; 1 si no hay rotativo. Falla si
/// el rotativo está activo y alguno de los valores que usa su tipo no es un número mayor que 0
/// (con rodillo no se mira el diámetro de la pieza).
pub(crate) fn y_scale(device: &DeviceProfile) -> Result<f64, String> {
    let r = &device.rotary;
    // En el galvo el rotativo solo gira entre franjas (ver `split`): el trabajo no escala la Y.
    if !r.enabled || device.kind == DeviceKind::Galvo {
        return Ok(1.0);
    }
    let positive = |name: &str, v: f64| {
        if v.is_finite() && v > 0.0 {
            Ok(())
        } else {
            Err(tr!(
                "pipeline.rotary.must_be_positive",
                machine = device.name,
                name = name
            ))
        }
    };
    positive(&tr!("pipeline.rotary.mm_per_rotation"), r.mm_per_rotation)?;
    let diameter = match r.kind {
        RotaryKind::Roller => {
            positive(&tr!("pipeline.rotary.roller_diameter"), r.roller_diameter)?;
            r.roller_diameter
        }
        RotaryKind::Chuck => {
            positive(&tr!("pipeline.rotary.object_diameter"), r.object_diameter)?;
            r.object_diameter
        }
    };
    Ok(r.mm_per_rotation / (PI * diameter))
}

/// Circunferencia de la pieza, en mm de superficie; `None` sin rotativo o si el diámetro de la
/// pieza no es un número mayor que 0 (con rodillo es un dato opcional).
pub(crate) fn circumference_mm(device: &DeviceProfile) -> Option<f64> {
    let d = device.rotary.object_diameter;
    (device.rotary.enabled && d.is_finite() && d > 0.0).then_some(PI * d)
}

/// Aviso si la Y de la controladora que recorre el trabajo (`area`, en coordenadas del
/// documento) se sale del área de la máquina, donde los límites por software dan `ALARM:2`.
pub(crate) fn soft_limit_warning(device: &DeviceProfile, area: Bounds) -> Option<String> {
    // Los límites por software son de GRBL; el galvo no tiene ALARM:2.
    if device.kind == DeviceKind::Galvo {
        return None;
    }
    let k = y_scale(device).ok().filter(|_| device.rotary.enabled)?;
    let a = point_to_machine(area.min, device.bed, device.origin);
    let c = point_to_machine(area.max, device.bed, device.origin);
    let (lo, hi) = (a.y.min(c.y) * k, a.y.max(c.y) * k);
    let bed_y = device.bed[1];
    if hi <= bed_y + 1e-6 && lo >= -1e-6 {
        return None;
    }
    Some(tr!(
        "pipeline.rotary.soft_limits",
        lo = format!("{lo:.1}"),
        hi = format!("{hi:.1}"),
        machine = device.name,
        bed_y = bed_y
    ))
}

/// Prueba de una vuelta: un documento con una sola línea de exactamente una circunferencia de
/// la pieza (`pi * object_diameter` mm de superficie) en dirección Y, a poca potencia. Con el
/// rotativo bien calibrado el trazo da una vuelta completa y su final cae sobre su principio; si
/// se queda corto o se pasa, hay que ajustar `mm_per_rotation` (más si se queda corto, menos si
/// se pasa). Pasa por el mismo pipeline que cualquier diseño, así que la escala y el espejo del
/// perfil se aplican igual. Con mandril, una circunferencia son exactamente `mm_per_rotation`
/// mm de Y de la controladora.
pub fn rotary_test(
    device: &DeviceProfile,
    speed_mm_min: f64,
    power_pct: f64,
) -> Result<Document, String> {
    if !device.rotary.enabled {
        return Err(tr!(
            "pipeline.rotary.test_needs_rotary",
            machine = device.name
        ));
    }
    y_scale(device)?;
    let circumference = circumference_mm(device)
        .ok_or_else(|| tr!("pipeline.rotary.test_needs_diameter", machine = device.name))?;
    if !(speed_mm_min.is_finite() && speed_mm_min > 0.0) {
        return Err(tr!("pipeline.rotary.test_bad_speed", speed = speed_mm_min));
    }
    if !(power_pct.is_finite() && power_pct > 0.0 && power_pct <= 100.0) {
        return Err(tr!("pipeline.rotary.test_bad_power", power = power_pct));
    }
    let color = Rgb(255, 0, 0);
    let x = 10.0f64.min(device.bed[0] / 2.0);
    let line = Polyline {
        points: vec![Point::new(x, 0.0), Point::new(x, circumference)],
        closed: false,
    };
    Ok(Document {
        version: DOC_VERSION,
        objects: vec![DesignObject {
            id: "prueba-una-vuelta".into(),
            layer: color,
            transform: IDENTITY,
            kind: ObjectKind::Vector(VectorObject::new(vec![BezPath::from(&line)])),
            locked: false,
            hidden: false,
            group: None,
            priority: None,
            power_scale: None,
            shape: None,
            start: None,
            manual_tabs: Vec::new(),
            array: None,
        }],
        layers: vec![LayerSettings {
            name: tr!("pipeline.rotary.test_layer"),
            speed: speed_mm_min,
            power: power_pct,
            ..LayerSettings::for_color(color)
        }],
        device_id: None,
        notes: String::new(),
        guides: Vec::new(),
        generators: Vec::new(),
        variable_text: Default::default(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tlaku_doc::JobOptions;

    fn device(kind: RotaryKind) -> DeviceProfile {
        let mut d = DeviceProfile::default();
        d.rotary.enabled = true;
        d.rotary.kind = kind;
        d.rotary.mm_per_rotation = 360.0;
        d.rotary.roller_diameter = 20.0;
        d.rotary.object_diameter = 80.0;
        d
    }

    #[test]
    fn formulas_follow_the_kind() {
        let roller = y_scale(&device(RotaryKind::Roller)).unwrap();
        assert!((roller - 360.0 / (PI * 20.0)).abs() < 1e-12);
        let chuck = y_scale(&device(RotaryKind::Chuck)).unwrap();
        assert!((chuck - 360.0 / (PI * 80.0)).abs() < 1e-12);
        assert!((circumference_mm(&device(RotaryKind::Chuck)).unwrap() - PI * 80.0).abs() < 1e-12);
    }

    #[test]
    fn real_accessories_give_the_documented_scales() {
        // xTool RA2 Pro: rodillo 32 mm por vuelta con rodillo de 17 mm, y mandril de 128 mm.
        let mut d = device(RotaryKind::Roller);
        d.rotary.mm_per_rotation = 32.0;
        d.rotary.roller_diameter = 17.0;
        assert!((y_scale(&d).unwrap() - 0.5991).abs() < 1e-4);
        d.rotary.kind = RotaryKind::Chuck;
        d.rotary.mm_per_rotation = 128.0;
        assert!((y_scale(&d).unwrap() - 0.5093).abs() < 1e-4);
        // Los valores de fábrica ya no mueven Y casi 6 veces de más.
        let mut d = DeviceProfile::default();
        d.rotary.enabled = true;
        assert!((y_scale(&d).unwrap() - 32.0 / (PI * 17.0)).abs() < 1e-12);
        assert!(y_scale(&d).unwrap() < 1.0);
    }

    #[test]
    fn disabled_rotary_is_the_identity() {
        let d = DeviceProfile::default();
        assert_eq!(y_scale(&d).unwrap(), 1.0);
        assert_eq!(circumference_mm(&d), None);
    }

    #[test]
    fn invalid_values_are_rejected_only_when_enabled_and_only_those_the_kind_uses() {
        for edit in [
            |d: &mut DeviceProfile| d.rotary.mm_per_rotation = 0.0,
            |d: &mut DeviceProfile| d.rotary.roller_diameter = -1.0,
        ] {
            let mut d = device(RotaryKind::Roller);
            edit(&mut d);
            let e = y_scale(&d).unwrap_err();
            assert!(e.contains("rotativo") && e.contains("mayor que 0"), "{e}");
            d.rotary.enabled = false;
            assert_eq!(y_scale(&d).unwrap(), 1.0);
        }
        // Con mandril el diámetro de la pieza es obligatorio; con rodillo no se mira.
        let mut chuck = device(RotaryKind::Chuck);
        chuck.rotary.object_diameter = f64::NAN;
        assert!(y_scale(&chuck).unwrap_err().contains("la pieza"));
        let mut roller = device(RotaryKind::Roller);
        roller.rotary.object_diameter = f64::NAN;
        assert!(y_scale(&roller).is_ok());
        assert_eq!(circumference_mm(&roller), None);
        roller.rotary.object_diameter = 0.0;
        assert!(y_scale(&roller).is_ok());
        // Y en el rodillo el diámetro de la pieza no cambia la escala.
        let (a, mut b) = (device(RotaryKind::Roller), device(RotaryKind::Roller));
        b.rotary.object_diameter = 123.0;
        assert_eq!(y_scale(&a).unwrap(), y_scale(&b).unwrap());
    }

    #[test]
    fn soft_limit_warning_appears_only_beyond_the_bed() {
        let area = |max_y: f64| Bounds {
            min: Point::new(0.0, 0.0),
            max: Point::new(10.0, max_y),
        };
        let d = device(RotaryKind::Roller); // k = 5.73, cama de 415 mm en Y
        assert_eq!(soft_limit_warning(&d, area(50.0)), None);
        let w = soft_limit_warning(&d, area(100.0)).unwrap();
        assert!(
            w.contains("ALARM:2") && w.contains("$20=0") && w.contains("573.0"),
            "{w}"
        );
        // Sin rotativo no hay nada que avisar aquí (lo hace la comprobación de la cama).
        assert_eq!(
            soft_limit_warning(&DeviceProfile::default(), area(1e6)),
            None
        );
    }

    #[test]
    fn one_turn_test_is_a_single_line_of_one_circumference() {
        let mut d = device(RotaryKind::Chuck);
        d.rotary.mm_per_rotation = 40.0;
        d.rotary.object_diameter = 75.0;
        let doc = rotary_test(&d, 1500.0, 5.0).unwrap();
        assert_eq!(doc.objects.len(), 1);
        let b = doc.objects[0].bounds().unwrap();
        assert!((b.max.y - b.min.y - PI * 75.0).abs() < 1e-9);
        assert_eq!(b.max.x, b.min.x);
        assert_eq!((doc.layers[0].speed, doc.layers[0].power), (1500.0, 5.0));
        // Con mandril, una circunferencia son exactamente mm_per_rotation mm de la controladora.
        let job = crate::compute(&doc, &d, &JobOptions::default()).unwrap();
        let g = crate::gcode(&job, &d);
        let top = g
            .lines()
            .filter(|l| l.starts_with("G1"))
            .filter_map(|l| l.split(' ').find(|w| w.starts_with('Y')))
            .map(|w| w[1..].parse::<f64>().unwrap())
            .fold(0.0, f64::max);
        assert!((top - 40.0).abs() < 1e-2, "{g}");
        // Con rodillo: mm_per_rotation * D_pieza / D_rodillo.
        let mut roller = device(RotaryKind::Roller);
        roller.rotary.object_diameter = 60.0;
        let doc = rotary_test(&roller, 1000.0, 10.0).unwrap();
        let job = crate::compute(&doc, &roller, &JobOptions::default()).unwrap();
        let g = crate::gcode(&job, &roller);
        let top = g
            .lines()
            .filter(|l| l.starts_with("G1"))
            .filter_map(|l| l.split(' ').find(|w| w.starts_with('Y')))
            .map(|w| w[1..].parse::<f64>().unwrap())
            .fold(0.0, f64::max);
        assert!((top - 360.0 * 60.0 / 20.0).abs() < 1e-2, "{top}");
    }

    #[test]
    fn one_turn_test_layer_name_follows_the_language() {
        let d = device(RotaryKind::Chuck);
        assert_eq!(
            rotary_test(&d, 1500.0, 5.0).unwrap().layers[0].name,
            "Prueba de una vuelta"
        );
        let en = tlaku_i18n::with_locale(tlaku_i18n::Locale::En, || {
            rotary_test(&d, 1500.0, 5.0).unwrap()
        });
        assert_eq!(en.layers[0].name, "One-turn test");
    }

    #[test]
    fn one_turn_test_rejects_what_it_cannot_draw() {
        let off = DeviceProfile::default();
        assert!(
            rotary_test(&off, 1000.0, 5.0)
                .unwrap_err()
                .contains("activa el rotativo")
        );
        let mut d = device(RotaryKind::Roller);
        d.rotary.object_diameter = 0.0;
        assert!(
            rotary_test(&d, 1000.0, 5.0)
                .unwrap_err()
                .contains("circunferencia")
        );
        d.rotary.object_diameter = 80.0;
        assert!(rotary_test(&d, 0.0, 5.0).is_err());
        assert!(rotary_test(&d, 1000.0, 0.0).is_err());
        assert!(rotary_test(&d, 1000.0, 101.0).is_err());
        assert!(rotary_test(&d, f64::NAN, 5.0).is_err());
    }
}
