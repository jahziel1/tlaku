//! Vista previa por tonos (mejora 8 del plan de usuarios): curva de respuesta de un material.
//!
//! Es un **modelo aproximado**, no una predicción: el resultado real depende de lente, enfoque,
//! aire, humedad y lote del material. La curva convierte una **dosis** en un color:
//!
//! - dosis `D` = (potencia / 100) x segundos de exposición / mm2, o sea
//!   `pasadas x (potencia/100) / (velocidad en mm/s x ancho o intervalo en mm)`;
//! - oscuridad `d = 1 - exp(-(D / dose_char)^gamma)`: crece con la dosis, nunca baja y satura
//!   en 1 (sin NaN);
//! - color = mezcla de `base` (material sin tocar) a `burn` (tono máximo) en proporción `d`.
//!
//! Los valores de partida de cada material viven en `data/tone-presets.json`, por watt del láser
//! (`tone_preset_for`). La calibración
//! con la prueba de materiales ([`ToneCalibration`]) los reemplaza y se guarda en la biblioteca
//! local de materiales. La interfaz tiene una copia de `darkness`/`mix` (`features/tonal/curve.ts`)
//! fijada por prueba con los mismos vectores.

use serde::{Deserialize, Serialize};
use std::sync::OnceLock;
use tlaku_i18n::tr;
use ts_rs::TS;

/// Datos de partida incluidos en el programa.
const DATA: &str = include_str!("../data/tone-presets.json");

/// Dosis característica máxima que se acepta (evita desbordes con datos corruptos).
const MAX_DOSE_CHAR: f64 = 1.0e6;

/// Curva de respuesta de un material.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub struct ToneCurve {
    /// Color del material sin grabar (RGB 0 a 255).
    pub base: [u8; 3],
    /// Color con el grabado más intenso (más oscuro, o más claro en acrílico o anodizado).
    pub burn: [u8; 3],
    /// Dosis con la que se llega al 63 % del tono máximo (`> 0`).
    pub dose_char: f64,
    /// Pendiente de la curva (`0,2 a 4`); 1 = exponencial simple.
    pub gamma: f64,
}

impl ToneCurve {
    /// `Err` con un mensaje si la curva no es usable (valores no finitos o fuera de rango).
    pub fn validate(&self) -> Result<(), String> {
        let ok = self.dose_char.is_finite()
            && self.dose_char > 0.0
            && self.dose_char <= MAX_DOSE_CHAR
            && self.gamma.is_finite()
            && (0.2..=4.0).contains(&self.gamma);
        if ok {
            Ok(())
        } else {
            Err(tr!(
                "doc.tonal.bad_curve",
                dose = self.dose_char,
                gamma = self.gamma
            ))
        }
    }

    /// Oscuridad (0 = material sin tocar, 1 = tono máximo) para una dosis. Monótona, acotada
    /// a 0..1 y sin NaN: una dosis negativa o NaN cuenta como 0 y una infinita como saturada.
    pub fn darkness(&self, dose: f64) -> f64 {
        if dose.is_nan() || dose <= 0.0 {
            return 0.0;
        }
        if dose.is_infinite() {
            return 1.0;
        }
        let x = (dose / self.dose_char).powf(self.gamma);
        (1.0 - (-x).exp()).clamp(0.0, 1.0)
    }

    /// Color RGB para una dosis.
    pub fn color(&self, dose: f64) -> [u8; 3] {
        self.mix(self.darkness(dose))
    }

    /// Color a una oscuridad dada (0..1).
    pub fn mix(&self, darkness: f64) -> [u8; 3] {
        let d = if darkness.is_nan() {
            0.0
        } else {
            darkness.clamp(0.0, 1.0)
        };
        let ch = |i: usize| {
            let a = f64::from(self.base[i]);
            let b = f64::from(self.burn[i]);
            (a + (b - a) * d).round().clamp(0.0, 255.0) as u8
        };
        [ch(0), ch(1), ch(2)]
    }
}

/// Dosis de un trazo: `passes` pasadas a `power_pct` % y `speed_mm_min` sobre un ancho de
/// `width_mm` (ancho del punto en una línea, intervalo en un relleno o una imagen).
pub fn tone_dose(power_pct: f64, speed_mm_min: f64, passes: u32, width_mm: f64) -> f64 {
    let v = speed_mm_min / 60.0;
    if !(power_pct > 0.0 && v > 0.0 && width_mm > 0.0) {
        return 0.0;
    }
    f64::from(passes) * (power_pct / 100.0) / (v * width_mm)
}

#[derive(Debug, Deserialize)]
struct PresetFile {
    /// Watts que se suponen cuando el perfil de la máquina no dice los del láser.
    watts_por_defecto: f64,
    presets: Vec<PresetEntry>,
}

#[derive(Debug, Deserialize)]
struct PresetEntry {
    id: String,
    base: [u8; 3],
    burn: [u8; 3],
    /// Dosis característica por watt (W·s/mm²): `dose_char = dose_char_w / watts`.
    dose_char_w: f64,
    gamma: f64,
}

fn file() -> &'static PresetFile {
    static CELL: OnceLock<PresetFile> = OnceLock::new();
    CELL.get_or_init(|| {
        serde_json::from_str::<PresetFile>(DATA).unwrap_or(PresetFile {
            watts_por_defecto: 10.0,
            presets: Vec::new(),
        })
    })
}

fn presets() -> &'static [PresetEntry] {
    &file().presets
}

/// Watts del láser que se suponen para una curva de partida cuando no se conocen.
pub fn tone_default_watts() -> f64 {
    file().watts_por_defecto
}

/// Identificadores de los materiales de partida (incluye `custom`, el personalizado).
pub fn tone_preset_ids() -> Vec<&'static str> {
    presets().iter().map(|p| p.id.as_str()).collect()
}

/// Curva de partida de un material por su identificador, para un láser de [`tone_default_watts`].
pub fn tone_preset(id: &str) -> Option<ToneCurve> {
    tone_preset_for(id, None)
}

/// Redondea a 6 cifras significativas: la división por los watts no deja decimales de más
/// (0,0924 y no 0,09240000000000001), que además se guardan y leen sin deriva.
fn round_sig(v: f64) -> f64 {
    format!("{v:.5e}").parse().unwrap_or(v)
}

/// Curva de partida de un material para un láser de `watts` (`None` o un valor inválido: los watts
/// por defecto). Los valores de partida van por watt: la dosis (fracción de potencia x s/mm2) con
/// la que el material llega al 63 % de su tono es `dose_char_w / watts`, así que un láser de 22 W
/// llega al mismo tono con menos de la mitad de dosis que uno de 10 W. La curva calibrada por el
/// usuario ya es de su máquina y no se convierte (no pasa por aquí).
pub fn tone_preset_for(id: &str, watts: Option<f64>) -> Option<ToneCurve> {
    let w = watts
        .filter(|w| w.is_finite() && *w > 0.0)
        .unwrap_or_else(tone_default_watts);
    presets().iter().find(|p| p.id == id).map(|p| ToneCurve {
        base: p.base,
        burn: p.burn,
        dose_char: round_sig(p.dose_char_w / w),
        gamma: p.gamma,
    })
}

/// Una observación hecha por el usuario sobre su prueba de materiales: una celda de dosis
/// `dose` se ve igual que la oscuridad `darkness` (0..1) de la curva.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub struct ToneSample {
    pub dose: f64,
    pub darkness: f64,
}

/// Curva calibrada de un material, guardada en la biblioteca local de materiales.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub struct ToneCalibration {
    /// Material de la curva: un identificador de partida (`light_wood`...) o `custom`.
    pub material: String,
    pub curve: ToneCurve,
    /// Observaciones con las que se ajustó (para mostrarlas y volver a ajustar).
    #[serde(default)]
    pub samples: Vec<ToneSample>,
}

impl ToneCalibration {
    /// Comprueba lo que el formato no garantiza: material con nombre, curva usable y muestras
    /// finitas.
    pub fn validate(&self) -> Result<(), String> {
        if self.material.trim().is_empty() {
            return Err(tr!("doc.tonal.no_material"));
        }
        self.curve.validate()?;
        let ok = self.samples.iter().all(|s| {
            s.dose.is_finite()
                && s.dose >= 0.0
                && s.darkness.is_finite()
                && (0.0..=1.0).contains(&s.darkness)
        });
        if ok {
            Ok(())
        } else {
            Err(tr!("doc.tonal.bad_sample", material = self.material))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wood() -> ToneCurve {
        tone_preset("light_wood").unwrap()
    }

    #[test]
    fn presets_scale_with_the_laser_watts() {
        let base = tone_preset("mdf").unwrap();
        // Sin watts, o con watts inválidos, se suponen los de por defecto.
        assert_eq!(tone_preset_for("mdf", None), Some(base));
        assert_eq!(tone_preset_for("mdf", Some(0.0)), Some(base));
        assert_eq!(tone_preset_for("mdf", Some(f64::NAN)), Some(base));
        // MDF medido en una Falcon2 Pro de 22 W: dosis característica 0,042 y gamma 0,74.
        let falcon = tone_preset_for("mdf", Some(22.0)).unwrap();
        assert!((falcon.dose_char - 0.042).abs() < 1e-9, "{falcon:?}");
        assert!((falcon.gamma - 0.74).abs() < 1e-12);
        // Doble de watts, mitad de dosis; los colores y la gamma no cambian.
        let w10 = tone_preset_for("light_wood", Some(10.0)).unwrap();
        let w20 = tone_preset_for("light_wood", Some(20.0)).unwrap();
        assert!((w10.dose_char / w20.dose_char - 2.0).abs() < 1e-12);
        assert_eq!(
            (w10.base, w10.burn, w10.gamma),
            (w20.base, w20.burn, w20.gamma)
        );
        // Los demás siguen siendo estimaciones a los watts por defecto (las cifras de antes).
        assert!((tone_preset("light_wood").unwrap().dose_char - 0.30).abs() < 1e-9);
        assert!(tone_preset_for("no-existe", Some(22.0)).is_none());
        // Todos válidos con cualquier potencia razonable.
        for id in tone_preset_ids() {
            for w in [0.5, 5.0, 22.0, 60.0, 130.0] {
                tone_preset_for(id, Some(w)).unwrap().validate().unwrap();
            }
        }
    }

    #[test]
    fn every_preset_is_valid_and_custom_exists() {
        let ids = tone_preset_ids();
        assert!(ids.contains(&"custom") && ids.len() >= 9);
        for id in ids {
            tone_preset(id).unwrap().validate().unwrap();
        }
        assert!(tone_preset("no-existe").is_none());
    }

    #[test]
    fn darkness_is_monotonic_bounded_and_never_nan() {
        let c = wood();
        let mut prev = 0.0;
        for i in 0..=2000 {
            let d = c.darkness(f64::from(i) * 0.005);
            assert!((0.0..=1.0).contains(&d) && d >= prev);
            prev = d;
        }
        assert_eq!(c.darkness(0.0), 0.0);
        assert_eq!(c.darkness(-3.0), 0.0);
        assert_eq!(c.darkness(f64::NAN), 0.0);
        assert_eq!(c.darkness(f64::INFINITY), 1.0);
        assert!(c.darkness(1.0e6) > 0.999);
    }

    #[test]
    fn more_power_or_less_speed_means_darker() {
        let c = wood();
        let base = c.darkness(tone_dose(40.0, 3000.0, 1, 0.1));
        assert!(c.darkness(tone_dose(60.0, 3000.0, 1, 0.1)) > base);
        assert!(c.darkness(tone_dose(40.0, 1500.0, 1, 0.1)) > base);
        assert!(c.darkness(tone_dose(40.0, 3000.0, 2, 0.1)) > base);
        // Un intervalo mayor reparte la misma energía en más área.
        assert!(c.darkness(tone_dose(40.0, 3000.0, 1, 0.2)) < base);
        assert_eq!(tone_dose(0.0, 3000.0, 1, 0.1), 0.0);
        assert_eq!(tone_dose(50.0, 0.0, 1, 0.1), 0.0);
    }

    #[test]
    fn color_goes_from_base_to_burn() {
        let c = wood();
        assert_eq!(c.color(0.0), c.base);
        assert_eq!(c.color(1.0e9), c.burn);
        // Vectores que la copia de TypeScript (`curve.test.ts`) repite.
        let d = c.darkness(0.30);
        assert!((d - (1.0 - (-1.0f64).exp())).abs() < 1e-12);
        assert_eq!(c.mix(0.5), [148, 114, 79]);
    }

    #[test]
    fn rejects_bad_curves_and_samples() {
        let mut c = wood();
        c.dose_char = 0.0;
        assert!(c.validate().is_err());
        c.dose_char = f64::NAN;
        assert!(c.validate().is_err());
        c = wood();
        c.gamma = 9.0;
        assert!(c.validate().is_err());
        let ok = ToneCalibration {
            material: "light_wood".into(),
            curve: wood(),
            samples: vec![ToneSample {
                dose: 0.2,
                darkness: 0.4,
            }],
        };
        ok.validate().unwrap();
        let mut bad = ok.clone();
        bad.samples[0].darkness = 1.5;
        assert!(bad.validate().is_err());
        bad = ok.clone();
        bad.material = " ".into();
        assert!(bad.validate().is_err());
    }

    #[test]
    fn library_keeps_calibrations_and_old_files_still_open() {
        use crate::materials::MaterialLibrary;
        let old = r#"{"version":1,"presets":[]}"#;
        let lib: MaterialLibrary = serde_json::from_str(old).unwrap();
        assert!(lib.tones.is_empty());
        // Sin curvas no se escribe el campo (los archivos de antes quedan idénticos).
        assert!(!serde_json::to_string(&lib).unwrap().contains("tones"));
        let mut with = lib.clone();
        with.tones.push(ToneCalibration {
            material: "leather".into(),
            curve: tone_preset("leather").unwrap(),
            samples: vec![ToneSample {
                dose: 0.3,
                darkness: 0.5,
            }],
        });
        let json = serde_json::to_string(&with).unwrap();
        assert_eq!(
            serde_json::from_str::<MaterialLibrary>(&json).unwrap(),
            with
        );
    }

    #[test]
    fn calibration_roundtrips_as_json() {
        let cal = ToneCalibration {
            material: "mdf".into(),
            curve: tone_preset("mdf").unwrap(),
            samples: vec![ToneSample {
                dose: 0.1,
                darkness: 0.2,
            }],
        };
        let json = serde_json::to_string(&cal).unwrap();
        assert_eq!(serde_json::from_str::<ToneCalibration>(&json).unwrap(), cal);
    }
}
