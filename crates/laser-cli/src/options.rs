//! Opciones de capa y de imagen: análisis del texto de la línea de comandos y
//! resolución contra los valores por defecto de los crates del motor.

use clap::ValueEnum;
use laser_core::Rgb;
pub use laser_pipeline::doc::Overscan;
use laser_pipeline::doc::{ImageMode, LayerSettings};

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum LayerMode {
    /// Contorno: sigue las trayectorias.
    Line,
    /// Relleno por barrido de las trayectorias cerradas.
    Fill,
}

impl LayerMode {
    pub fn name(self) -> &'static str {
        match self {
            LayerMode::Line => "line",
            LayerMode::Fill => "fill",
        }
    }

    fn to_doc(self) -> laser_pipeline::doc::LayerMode {
        match self {
            LayerMode::Line => laser_pipeline::doc::LayerMode::Line,
            LayerMode::Fill => laser_pipeline::doc::LayerMode::Fill,
        }
    }
}

/// Cómo se ordenan las capas al armar el trabajo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum LayerOrder {
    /// Grabados (relleno e imagen) primero, líneas después y cortes al final.
    Auto,
    /// El orden en que las capas aparecen en el archivo.
    File,
}

impl From<LayerOrder> for laser_pipeline::doc::LayerOrder {
    fn from(o: LayerOrder) -> Self {
        match o {
            LayerOrder::Auto => Self::Auto,
            LayerOrder::File => Self::File,
        }
    }
}

/// Ajustes de una capa tal como los escribió el usuario; lo que falte toma el valor global
/// y, si tampoco hay, el valor por defecto del crate correspondiente.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct LayerOverride {
    pub mode: Option<LayerMode>,
    pub speed: Option<f64>,
    pub power: Option<f64>,
    pub passes: Option<u32>,
    pub air: Option<bool>,
    pub interval: Option<f64>,
    pub angle: Option<f64>,
    pub crosshatch: Option<bool>,
    pub bidir: Option<bool>,
    pub overscan: Option<Overscan>,
    pub kerf: Option<f64>,
    /// Posición explícita (desde 1) en el orden final; gana sobre `--layer-order`.
    pub order: Option<usize>,
}

/// Valores globales (`--speed`, `--power`, ...) que se aplican a toda capa sin ajuste propio.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Defaults {
    pub mode: Option<LayerMode>,
    pub speed: Option<f64>,
    pub power: Option<f64>,
    pub passes: Option<u32>,
    pub air: Option<bool>,
    pub interval: Option<f64>,
    pub overscan: Option<Overscan>,
}

pub fn parse_f64(s: &str) -> Result<f64, String> {
    s.trim()
        .parse::<f64>()
        .ok()
        .filter(|v| v.is_finite())
        .ok_or_else(|| format!("'{s}' no es un número válido"))
}

pub fn parse_positive(s: &str) -> Result<f64, String> {
    let v = parse_f64(s)?;
    if v > 0.0 {
        Ok(v)
    } else {
        Err(format!("'{s}' debe ser mayor que 0"))
    }
}

pub fn parse_overscan(s: &str) -> Result<Overscan, String> {
    if s.trim().eq_ignore_ascii_case("auto") {
        return Ok(Overscan::Auto);
    }
    match parse_f64(s) {
        Ok(v) if v >= 0.0 => Ok(Overscan::Mm(v)),
        Ok(_) => Err(format!("overscan negativo: '{s}'")),
        Err(_) => Err(format!(
            "overscan inválido '{s}': usa \"auto\" o un número en mm"
        )),
    }
}

/// `ANCHOxALTO` en mm, ambos mayores que 0 (acepta `x`, `X` y `×`).
pub fn parse_size(s: &str) -> Result<(f64, f64), String> {
    let (w, h) = s.split_once(['x', 'X', '×']).ok_or_else(|| {
        format!("formato esperado: ANCHOxALTO (por ejemplo 400x415), recibí '{s}'")
    })?;
    let both = |v: &str| parse_positive(v).map_err(|_| format!("tamaño inválido '{s}'"));
    Ok((both(w)?, both(h)?))
}

/// `X,Y` en mm.
pub fn parse_pos(s: &str) -> Result<(f64, f64), String> {
    let (x, y) = s
        .split_once(',')
        .ok_or_else(|| format!("formato esperado: X,Y (por ejemplo 10,20), recibí '{s}'"))?;
    let c = |v: &str| parse_f64(v).map_err(|_| format!("posición inválida '{s}'"));
    Ok((c(x)?, c(y)?))
}

fn parse_layer_mode(v: &str) -> Result<LayerMode, String> {
    match v {
        "line" => Ok(LayerMode::Line),
        "fill" => Ok(LayerMode::Fill),
        _ => Err(format!("modo '{v}' desconocido: usa line o fill")),
    }
}

/// `"#rrggbb:clave=valor,..."`. Claves con valor: mode, speed, power, passes, interval,
/// angle, overscan, kerf, order. Claves sin valor: air, noair, crosshatch, nocrosshatch, bidir, nobidir.
pub fn parse_layer(s: &str) -> Result<(Rgb, LayerOverride), String> {
    let (color, opts) = s.split_once(':').ok_or(
        "formato esperado: #rrggbb:clave=valor,... (por ejemplo \"#ff0000:speed=300,power=80\")",
    )?;
    let mut o = LayerOverride::default();
    for kv in opts.split(',').map(str::trim).filter(|kv| !kv.is_empty()) {
        let (k, v) = match kv.split_once('=') {
            Some((k, v)) => (k.trim(), Some(v.trim())),
            None => (kv, None),
        };
        let value = || {
            v.ok_or_else(|| format!("la clave '{k}' necesita un valor ({k}=...)"))
                .and_then(|v| parse_f64(v).map_err(|e| format!("valor inválido en '{kv}': {e}")))
        };
        let flag = |on: bool, target: &mut Option<bool>| match v {
            None => {
                *target = Some(on);
                Ok(())
            }
            Some(_) => Err(format!("la clave '{k}' no lleva valor")),
        };
        match k {
            "mode" => {
                let v = v.ok_or("la clave 'mode' necesita un valor (mode=line|fill)")?;
                o.mode = Some(parse_layer_mode(v)?);
            }
            "speed" => o.speed = Some(value()?),
            "power" => o.power = Some(value()?),
            "passes" => {
                let n = value()?;
                if n.fract() != 0.0 || n < 0.0 {
                    return Err(format!("valor inválido en '{kv}': deben ser enteros"));
                }
                o.passes = Some(n.min(f64::from(u32::MAX)) as u32);
            }
            "interval" => o.interval = Some(value()?),
            "angle" => o.angle = Some(value()?),
            "kerf" => o.kerf = Some(value()?),
            "order" => {
                let n = value()?;
                if n.fract() != 0.0 || n < 1.0 {
                    return Err(format!(
                        "valor inválido en '{kv}': la posición debe ser un entero desde 1"
                    ));
                }
                o.order = Some(n.min(f64::from(u32::MAX)) as usize);
            }
            "overscan" => {
                let v = v.ok_or("la clave 'overscan' necesita un valor (mm o auto)")?;
                o.overscan = Some(parse_overscan(v)?);
            }
            "air" => flag(true, &mut o.air)?,
            "noair" => flag(false, &mut o.air)?,
            "crosshatch" => flag(true, &mut o.crosshatch)?,
            "nocrosshatch" => flag(false, &mut o.crosshatch)?,
            "bidir" => flag(true, &mut o.bidir)?,
            "nobidir" => flag(false, &mut o.bidir)?,
            _ => {
                return Err(format!(
                    "clave desconocida '{k}'; claves válidas: mode, speed, power, passes, air, noair, \
                     interval, angle, crosshatch, bidir, nobidir, overscan, kerf, order"
                ));
            }
        }
    }
    Ok((color.trim().parse()?, o))
}

/// Pone sobre los ajustes que dio la importación los valores globales y los de la capa.
/// Solo rechaza claves que no corresponden al modo; los valores los valida el pipeline.
/// Los errores salen con el prefijo `capa #rrggbb:`.
pub fn apply_layer(
    mut s: LayerSettings,
    defaults: &Defaults,
    over: Option<&LayerOverride>,
) -> Result<LayerSettings, String> {
    let none = LayerOverride::default();
    let o = over.unwrap_or(&none);
    let mode = o.mode.or(defaults.mode).unwrap_or(LayerMode::Line);

    // Claves que solo tienen sentido en un modo: se rechazan en vez de ignorarse.
    let only = |key: &str, present: bool, wanted: LayerMode| -> Result<(), String> {
        if present && mode != wanted {
            return Err(format!(
                "capa {}: la clave '{key}' solo aplica en modo {} (esta capa es {})",
                s.color,
                wanted.name(),
                mode.name()
            ));
        }
        Ok(())
    };
    only("kerf", o.kerf.is_some(), LayerMode::Line)?;
    only("interval", o.interval.is_some(), LayerMode::Fill)?;
    only("angle", o.angle.is_some(), LayerMode::Fill)?;
    only("crosshatch", o.crosshatch.is_some(), LayerMode::Fill)?;
    only("bidir", o.bidir.is_some(), LayerMode::Fill)?;
    only("overscan", o.overscan.is_some(), LayerMode::Fill)?;

    s.mode = mode.to_doc();
    s.speed = o.speed.or(defaults.speed).unwrap_or(s.speed);
    s.power = o.power.or(defaults.power).unwrap_or(s.power);
    s.passes = o.passes.or(defaults.passes).unwrap_or(s.passes);
    s.air = o.air.or(defaults.air).unwrap_or(s.air);
    s.order = o.order.map(|n| n.min(u32::MAX as usize) as u32);
    match mode {
        LayerMode::Line => s.kerf = o.kerf.unwrap_or(0.0),
        LayerMode::Fill => {
            s.interval = o.interval.or(defaults.interval).unwrap_or(s.interval);
            s.angle = o.angle.unwrap_or(s.angle);
            s.bidirectional = o.bidir.unwrap_or(s.bidirectional);
            s.crosshatch = o.crosshatch.unwrap_or(s.crosshatch);
            s.overscan = o.overscan.or(defaults.overscan).unwrap_or(Overscan::Auto);
        }
    }
    Ok(s)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ImageModeArg {
    /// Blanco y negro con un umbral (ver --threshold).
    Threshold,
    Floyd,
    Jarvis,
    Stucki,
    Atkinson,
    Bayer4,
    /// Conserva los tonos: potencia proporcional a lo oscuro.
    Grayscale,
}

impl ImageModeArg {
    pub fn to_doc(self) -> ImageMode {
        match self {
            ImageModeArg::Threshold => ImageMode::Threshold,
            ImageModeArg::Floyd => ImageMode::Floyd,
            ImageModeArg::Jarvis => ImageMode::Jarvis,
            ImageModeArg::Stucki => ImageMode::Stucki,
            ImageModeArg::Atkinson => ImageMode::Atkinson,
            ImageModeArg::Bayer4 => ImageMode::Bayer4,
            ImageModeArg::Grayscale => ImageMode::Grayscale,
        }
    }
}

/// Opciones de imagen tal como llegaron; `given` lista las banderas escritas, para avisar si
/// la entrada no es una imagen.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ImageParams {
    /// Tamaño físico en mm.
    pub size: Option<(f64, f64)>,
    /// Esquina inferior izquierda en mm.
    pub pos: Option<(f64, f64)>,
    pub mode: Option<ImageModeArg>,
    pub threshold: Option<f64>,
    pub min_power: Option<f64>,
    pub brightness: Option<f64>,
    pub contrast: Option<f64>,
    pub gamma: Option<f64>,
    pub invert: bool,
    pub dot_width: Option<f64>,
    pub given: Vec<&'static str>,
}

/// Pone los valores globales y las opciones de imagen sobre la capa de imágenes importada.
/// Los valores los valida el pipeline.
pub fn apply_image(
    mut s: LayerSettings,
    defaults: &Defaults,
    img: &ImageParams,
) -> Result<LayerSettings, String> {
    if img.threshold.is_some() && img.mode != Some(ImageModeArg::Threshold) {
        return Err("--threshold solo aplica con --image-mode threshold".into());
    }
    s.speed = defaults.speed.unwrap_or(s.speed);
    s.power = defaults.power.unwrap_or(s.power);
    s.passes = defaults.passes.unwrap_or(s.passes);
    s.air = defaults.air.unwrap_or(s.air);
    s.interval = defaults.interval.unwrap_or(s.interval);
    s.overscan = defaults.overscan.unwrap_or(Overscan::Auto);
    s.min_power = img.min_power.unwrap_or(s.min_power);
    let i = &mut s.image;
    i.mode = img.mode.map_or(i.mode, ImageModeArg::to_doc);
    i.threshold = img.threshold.unwrap_or(i.threshold);
    i.brightness = img.brightness.unwrap_or(i.brightness);
    i.contrast = img.contrast.unwrap_or(i.contrast);
    i.gamma = img.gamma.unwrap_or(i.gamma);
    i.invert = img.invert;
    i.dot_width = img.dot_width.unwrap_or(i.dot_width);
    Ok(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layer_parsing_full() {
        let (c, o) = parse_layer(
            "#ff0000:mode=fill,speed=300,power=100,passes=2,air,interval=0.2,angle=45,crosshatch,nobidir,overscan=auto",
        )
        .unwrap();
        assert_eq!(c, Rgb(255, 0, 0));
        assert_eq!(o.mode, Some(LayerMode::Fill));
        assert_eq!(o.speed, Some(300.0));
        assert_eq!(o.passes, Some(2));
        assert_eq!(o.air, Some(true));
        assert_eq!(o.interval, Some(0.2));
        assert_eq!(o.crosshatch, Some(true));
        assert_eq!(o.bidir, Some(false));
        assert_eq!(o.overscan, Some(Overscan::Auto));
        let (_, o) = parse_layer("#00ff00:kerf=0.1,overscan=1.5,noair").unwrap();
        assert_eq!(o.kerf, Some(0.1));
        assert_eq!(o.overscan, Some(Overscan::Mm(1.5)));
        assert_eq!(o.air, Some(false));
        let (_, o) = parse_layer("#00ff00:order=3").unwrap();
        assert_eq!(o.order, Some(3));
    }

    #[test]
    fn layer_parsing_errors_are_clear() {
        let e = |s: &str| parse_layer(s).unwrap_err();
        assert!(e("#ff0000:foo=1").contains("clave desconocida 'foo'"));
        assert!(e("#ff0000").contains("formato esperado"));
        assert!(e("#ff00:speed=1").contains("color inválido"));
        assert!(e("#ff0000:speed=abc").contains("valor inválido en 'speed=abc'"));
        assert!(e("#ff0000:speed").contains("necesita un valor"));
        assert!(e("#ff0000:air=1").contains("no lleva valor"));
        assert!(e("#ff0000:passes=1.5").contains("enteros"));
        assert!(e("#ff0000:overscan=-1").contains("negativo"));
        assert!(e("#ff0000:mode=x").contains("modo 'x' desconocido"));
        assert!(e("#ff0000:speed=NaN").contains("valor inválido"));
        assert!(e("#ff0000:order=0").contains("entero desde 1"));
        assert!(e("#ff0000:order=1.5").contains("entero desde 1"));
        assert!(e("#ff0000:order").contains("necesita un valor"));
    }

    #[test]
    fn sizes_and_positions() {
        assert_eq!(parse_size("400x415"), Ok((400.0, 415.0)));
        assert_eq!(parse_size("40.5X20"), Ok((40.5, 20.0)));
        assert!(parse_size("400").is_err());
        assert!(parse_size("0x5").is_err());
        assert!(parse_size("ax5").is_err());
        assert_eq!(parse_pos("10,-2.5"), Ok((10.0, -2.5)));
        assert!(parse_pos("10").is_err());
        assert!(parse_pos("a,b").is_err());
    }

    fn layer(color: Rgb) -> LayerSettings {
        LayerSettings::for_color(color)
    }

    #[test]
    fn apply_uses_the_import_defaults() {
        let l = apply_layer(layer(Rgb(0, 0, 0)), &Defaults::default(), None).unwrap();
        assert_eq!(l, layer(Rgb(0, 0, 0)));
        let d = Defaults {
            mode: Some(LayerMode::Fill),
            speed: Some(300.0),
            ..Default::default()
        };
        let l = apply_layer(layer(Rgb(0, 0, 0)), &d, None).unwrap();
        assert_eq!(l.mode, laser_pipeline::doc::LayerMode::Fill);
        assert_eq!(l.speed, 300.0);
        assert_eq!(l.overscan, Overscan::Auto);
    }

    #[test]
    fn apply_layer_override_beats_global() {
        let (c, o) = parse_layer("#ff0000:speed=50,order=2,kerf=0.1").unwrap();
        let d = Defaults {
            speed: Some(300.0),
            power: Some(40.0),
            ..Default::default()
        };
        let l = apply_layer(layer(c), &d, Some(&o)).unwrap();
        assert_eq!(
            (l.speed, l.power, l.kerf, l.order),
            (50.0, 40.0, 0.1, Some(2))
        );
    }

    #[test]
    fn apply_rejects_keys_of_other_mode() {
        let (c, o) = parse_layer("#ff0000:interval=0.1").unwrap();
        let e = apply_layer(layer(c), &Defaults::default(), Some(&o)).unwrap_err();
        assert!(
            e.contains("capa #ff0000") && e.contains("solo aplica en modo fill"),
            "{e}"
        );
        let (c, o) = parse_layer("#ff0000:mode=fill,kerf=0.1").unwrap();
        assert!(apply_layer(layer(c), &Defaults::default(), Some(&o)).is_err());
    }

    #[test]
    fn image_options_are_applied() {
        let base = LayerSettings::for_images();
        let same = apply_image(base.clone(), &Defaults::default(), &ImageParams::default());
        assert_eq!(same.unwrap(), base);
        let img = ImageParams {
            mode: Some(ImageModeArg::Threshold),
            threshold: Some(0.3),
            min_power: Some(5.0),
            invert: true,
            ..Default::default()
        };
        let d = Defaults {
            power: Some(60.0),
            ..Default::default()
        };
        let s = apply_image(base.clone(), &d, &img).unwrap();
        assert_eq!(s.image.mode, ImageMode::Threshold);
        assert_eq!((s.image.threshold, s.min_power, s.power), (0.3, 5.0, 60.0));
        assert!(s.image.invert);
        let img = ImageParams {
            threshold: Some(0.3),
            ..Default::default()
        };
        assert!(apply_image(base, &Defaults::default(), &img).is_err());
    }
}
