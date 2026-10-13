//! Capas de LightBurn (`CutSetting`) a `LayerSettings`, con la tabla índice -> color.
//!
//! Unidades de LightBurn en el archivo: velocidad en mm/s (Tlaku usa mm/min), potencias en %,
//! intervalos y distancias en mm. Los campos sin equivalente se avisan solo si están activos.
//!
//! # Tabla de mapeo de `CutSetting` a `LayerSettings`
//!
//! Los nombres de elemento son datos del formato (ver `THIRD_PARTY.md`: lbrnts y lbrn2-to-svg, MIT;
//! escritores públicos de archivos `.lbrn2`). Lo marcado "(inferido)" no está documentado.
//!
//! | Elemento de LightBurn | Ajuste de Tlaku |
//! |---|---|
//! | `type` (`Cut`, `Scan`, `Scan+Cut`, `Image`, `Tool`) | `mode` (`Tool` = guía sin salida) |
//! | `speed` (mm/s) | `speed` (mm/min, x60) |
//! | `maxPower`, `minPower` | `power`, `min_power` |
//! | `numPasses`, `zOffset`, `zPerPass`, `kerf`, `runBlower` | `passes`, `z_offset`, `z_step`, `kerf`, `air` |
//! | `interval` o `dpi`, `angle`, `crossHatch`, `bidir` | `interval`, `angle`, `crosshatch`, `bidirectional` |
//! | `overscan`, `overscanPercent` | `overscan` (% de la velocidad, inferido) |
//! | `tabsEnabled`, `tabSize`, `tabCount`, `tabSpacing`, `skipInnerTabs` | `tabs` |
//! | `perforate`, `perfLen`, `perfSkip` | `perforation` (`cut_mm`, `skip_mm`) |
//! | `dotMode`, `dotTime`, `dotSpacing` | `dot_mode` (`dwell_ms`, `spacing_mm`; `dotTime` en ms, inferido) |
//! | `floodFill` (en `Scan`) | `fill_style` = `flood` |
//! | `overcut` (mm) | `overcut` |
//! | `enablePowerRamp` (o `rampLength` > 0), `rampLength`, `rampOuter` | `ramp` (`length_mm`, `outer_only`; arranca en 0 %, inferido) |
//! | `startDelay`, `endDelay` (ms) | `start_delay_ms`, `end_delay_ms` |
//! | `enableCutThrough`, `enableCutThroughStart/End`, `throughPower` | `cut_through` (potencia; el tiempo sale de `startDelay` o `endDelay`, que dejan de ser pausas; inferido) |
//! | otra `CutSetting` con el mismo `index` | `sub_layers` (hasta 11, en orden) |
//! | `index`, `name`, `priority`, `doOutput`, `hide` | color, nombre, `order`, `output`, `visible` |
//!
//! Sin equivalente (se avisan si están activos): PPI, frecuencia, segundo láser, potencia constante,
//! velocidad del soplador y el relleno por objeto. LightBurn no guarda entradas ni salidas en la
//! capa (no se conoce un elemento para ellas): `lead_in` y `lead_out` quedan apagados.

use std::collections::{BTreeMap, HashMap, HashSet};
use tlaku_core::Rgb;
use tlaku_doc::{
    CutThrough, DotMode, FillStyle, ImageMode, LayerMode, LayerSettings, Overscan, Perforation,
    Ramp, TabSettings,
};
use tlaku_lbrn::{Layer, Project, Shape, ShapeKind, TOOL_LAYER_FIRST};

/// Color de Tlaku de cada capa de LightBurn `C00`..`C29`.
///
/// LightBurn usa una paleta fija de 30 colores (más T1 y T2 para herramientas) y su RGB exacto no
/// está publicado: los de abajo son aproximados (contrastados con la miniatura que guarda un
/// proyecto público y con la lista MIT de `lbrnts`; ver `THIRD_PARTY.md`). Donde hay un color
/// equivalente en la paleta de la app (`apps/desktop/src/lib/palette.ts`) se usa el de la app; el
/// resto conserva un RGB propio. Cada índice recibe un color distinto (la capa se identifica por
/// su color) y ninguno coincide con el de las guías.
const COLORS: [(&str, Rgb); 30] = [
    ("negro", Rgb(0x00, 0x00, 0x00)),
    ("azul -> Azul", Rgb(0x2f, 0x7b, 0xff)),
    ("rojo -> Rojo", Rgb(0xff, 0x3b, 0x4a)),
    ("verde -> Verde", Rgb(0x14, 0xc2, 0x8a)),
    ("amarillo -> Amarillo", Rgb(0xe6, 0xe6, 0x00)),
    ("naranja -> Naranja", Rgb(0xff, 0x7a, 0x1a)),
    ("cian -> Cian", Rgb(0x00, 0xc2, 0xe0)),
    ("magenta -> Violeta", Rgb(0xb0, 0x4d, 0xff)),
    ("gris claro -> Blanco", Rgb(0xf2, 0xf2, 0xf2)),
    ("azul marino -> Marino", Rgb(0x1d, 0x3d, 0x8f)),
    ("granate", Rgb(0xa0, 0x00, 0x00)),
    ("verde oscuro", Rgb(0x00, 0xa0, 0x00)),
    ("oliva", Rgb(0xa0, 0xa0, 0x00)),
    ("marrón -> Cobre", Rgb(0xc8, 0x79, 0x3c)),
    ("azul cielo", Rgb(0x00, 0xa0, 0xff)),
    ("púrpura", Rgb(0xa0, 0x00, 0xa0)),
    ("gris -> Gris", Rgb(0x9a, 0xa4, 0xb2)),
    ("azul grisáceo", Rgb(0x80, 0x80, 0xc0)),
    ("rosa apagado", Rgb(0xc0, 0x76, 0x8a)),
    ("azul real", Rgb(0x40, 0x68, 0xe1)),
    ("rosa fuerte -> Rosa", Rgb(0xff, 0x4f, 0xb0)),
    ("verde claro", Rgb(0x55, 0xc8, 0x55)),
    ("lavanda grisácea", Rgb(0xaa, 0xb0, 0xd1)),
    ("rosa claro", Rgb(0xf0, 0xa0, 0xd0)),
    ("rosa intenso", Rgb(0xfb, 0x80, 0xc8)),
    // C25-C29: su color real en LightBurn no se pudo verificar; se asignan colores libres de la
    // paleta de la app.
    ("(sin verificar) -> Lima", Rgb(0x7f, 0xd1, 0x1f)),
    ("(sin verificar) -> Índigo", Rgb(0x6a, 0x5c, 0xff)),
    ("(sin verificar) -> Ámbar", Rgb(0xff, 0xb0, 0x20)),
    ("(sin verificar) -> Turquesa", Rgb(0x2f, 0xd6, 0xc8)),
    ("(sin verificar)", Rgb(0xd0, 0xd0, 0x30)),
];

/// T1 y T2 (índices 30 y 31): capas de herramienta, equivalen a las guías (no salen a la máquina).
const TOOL_COLORS: [Rgb; 2] = [Rgb(0x7e, 0x8a, 0xa0), Rgb(0x8b, 0x7e, 0xa0)];

/// Color de una capa de índice cualquiera: el de la tabla o uno generado y distinto de `used`.
fn pick_color(index: i32, used: &HashSet<Rgb>) -> Rgb {
    let base = usize::try_from(index).ok().and_then(|i| {
        COLORS.get(i).map(|c| c.1).or_else(|| {
            i.checked_sub(TOOL_LAYER_FIRST as usize)
                .and_then(|t| TOOL_COLORS.get(t).copied())
        })
    });
    if let Some(c) = base.filter(|c| !used.contains(c)) {
        return c;
    }
    // Fuera de la paleta (o repetida): matiz por ángulo áureo hasta dar con uno libre.
    let mut n = index.unsigned_abs();
    loop {
        let hue = (f64::from(n) * 137.508) % 360.0;
        let c = hsv(hue, 0.65, 0.85);
        if !used.contains(&c) {
            return c;
        }
        n = n.wrapping_add(1);
    }
}

fn hsv(h: f64, s: f64, v: f64) -> Rgb {
    let (c, x) = (v * s, v * s * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs()));
    let m = v - c;
    let (r, g, b) = match (h / 60.0) as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let ch = |v: f64| ((v + m) * 255.0).round().clamp(0.0, 255.0) as u8;
    Rgb(ch(r), ch(g), ch(b))
}

/// Índices de capa que usan formas vectoriales (y texto) y mapas de bits.
#[derive(Default)]
pub(super) struct Usage {
    vector: HashSet<i32>,
    bitmap: HashSet<i32>,
}

impl Usage {
    pub(super) fn scan(shapes: &[Shape]) -> Self {
        let mut u = Self::default();
        u.walk(shapes);
        u
    }

    fn walk(&mut self, shapes: &[Shape]) {
        for s in shapes {
            let idx = s.cut_index.unwrap_or(0);
            match &s.kind {
                ShapeKind::Group(kids) => self.walk(kids),
                ShapeKind::Bitmap(_) => {
                    self.bitmap.insert(idx);
                }
                ShapeKind::Unknown(_) => {}
                _ => {
                    self.vector.insert(idx);
                }
            }
        }
    }
}

/// Tabla de capas del proyecto: convierte las definidas y crea las que solo se citan.
pub(super) struct Layers {
    /// Capa, índice de LightBurn y prioridad, en el orden del archivo.
    entries: Vec<(LayerSettings, i32, i32)>,
    by_index: HashMap<i32, usize>,
    used: HashSet<Rgb>,
    usage: Usage,
    pub warnings: Vec<String>,
    /// Aviso -> nombres de las capas a las que aplica (se resumen en `finish`).
    notes: BTreeMap<String, Vec<String>>,
    /// Capa común para imágenes que están en una capa que también lleva vectores.
    image_fallback: Option<Rgb>,
}

impl Layers {
    pub(super) fn new(project: &Project, usage: Usage) -> Self {
        let mut me = Self {
            entries: Vec::new(),
            by_index: HashMap::new(),
            used: HashSet::new(),
            usage,
            warnings: Vec::new(),
            notes: BTreeMap::new(),
            image_fallback: None,
        };
        for layer in &project.layers {
            let color = pick_color(layer.index, &me.used);
            me.used.insert(color);
            let (settings, priority, notes) = convert(layer, color);
            for note in notes {
                me.notes.entry(note).or_default().push(layer.name.clone());
            }
            me.by_index.insert(layer.index, me.entries.len());
            me.entries.push((settings, layer.index, priority));
        }
        me
    }

    /// Capa de `index` (o de la `C00` si la forma no trae `CutIndex`); si el archivo no la define
    /// se crea con los ajustes por defecto.
    pub(super) fn ensure(&mut self, index: i32) -> Rgb {
        if let Some(&k) = self.by_index.get(&index) {
            return self.entries[k].0.color;
        }
        let color = pick_color(index, &self.used);
        self.used.insert(color);
        let mut settings = LayerSettings::for_color(color);
        settings.name = format!("C{index:02}");
        if index >= TOOL_LAYER_FIRST {
            settings.name = format!("T{}", index - TOOL_LAYER_FIRST + 1);
            settings.output = false;
        } else {
            self.warnings.push(format!(
                "La capa C{index:02} no está definida en el archivo; se usan los ajustes por defecto"
            ));
        }
        self.by_index.insert(index, self.entries.len());
        self.entries.push((settings, index, 0));
        color
    }

    /// Capa para una imagen: la suya si es de modo imagen (o si solo lleva imágenes, y entonces
    /// se pasa a modo imagen); si comparte capa con vectores, la capa común de imágenes.
    pub(super) fn image_layer(&mut self, index: i32) -> Rgb {
        let color = self.ensure(index);
        let k = self.by_index[&index];
        if self.entries[k].0.mode == LayerMode::Image {
            return color;
        }
        if !self.usage.vector.contains(&index) {
            let e = &mut self.entries[k].0;
            e.mode = LayerMode::Image;
            e.image = tlaku_doc::ImageSettings::default();
            self.warnings.push(format!(
                "La capa «{}» solo lleva imágenes: se importa en modo imagen",
                self.entries[k].0.name
            ));
            return color;
        }
        if let Some(c) = self.image_fallback {
            return c;
        }
        let mut layer = LayerSettings::for_images();
        let mut c = layer.color;
        if self.used.contains(&c) {
            c = pick_color(10_000, &self.used);
            layer.color = c;
        }
        self.used.insert(c);
        self.image_fallback = Some(c);
        self.warnings.push(format!(
            "Hay imágenes en la capa «{}», que también lleva vectores: van a la capa «{}»",
            self.entries[k].0.name, layer.name
        ));
        self.entries.push((layer, 10_000, 0));
        c
    }

    /// Las capas en el orden en que LightBurn las ejecuta (por prioridad y luego por índice), con
    /// la herramienta (guías) al final.
    pub(super) fn finish(self) -> (Vec<LayerSettings>, Vec<String>) {
        let mut tool = Vec::new();
        let mut work: Vec<(LayerSettings, i32, i32)> = Vec::new();
        for e in self.entries {
            if e.1 >= TOOL_LAYER_FIRST && e.1 < 10_000 {
                tool.push(e.0);
            } else {
                work.push(e);
            }
        }
        work.sort_by_key(|e| (e.2, e.1));
        let distinct = work.iter().map(|e| e.2).collect::<HashSet<_>>().len() > 1;
        let mut out: Vec<LayerSettings> = work
            .into_iter()
            .enumerate()
            .map(|(rank, (mut l, _, _))| {
                if distinct {
                    l.order = Some(rank as u32 + 1);
                }
                l
            })
            .collect();
        out.extend(tool);
        let mut warnings = self.warnings;
        for (note, names) in self.notes {
            let shown: Vec<String> = names.iter().take(4).map(|n| format!("«{n}»")).collect();
            let more = names.len().saturating_sub(4);
            warnings.push(format!(
                "{note} (capas: {}{})",
                shown.join(", "),
                if more > 0 {
                    format!(" y {more} más")
                } else {
                    String::new()
                }
            ));
        }
        (out, warnings)
    }
}

/// `ditherMode` de LightBurn a modo de imagen de Tlaku.
fn dither_mode(name: &str) -> Option<ImageMode> {
    Some(match name.trim().to_ascii_lowercase().as_str() {
        "threshold" => ImageMode::Threshold,
        "ordered" => ImageMode::Bayer4,
        "atkinson" => ImageMode::Atkinson,
        // "Dither" de LightBurn es difusión de error; en el archivo aparece como `dithered`.
        "dither" | "dithered" => ImageMode::Floyd,
        "stucki" => ImageMode::Stucki,
        "jarvis" => ImageMode::Jarvis,
        "newsprint" => ImageMode::Newsprint,
        "halftone" => ImageMode::Halftone,
        "sketch" => ImageMode::Sketch,
        "grayscale" | "greyscale" => ImageMode::Grayscale,
        _ => return None,
    })
}

/// Máximo de sub-capas de una capa (el del motor, `place.rs`).
const MAX_SUB_LAYERS: usize = 11;

/// Convierte una capa con sus sub-capas (las `CutSetting` siguientes con el mismo índice). Devuelve
/// los ajustes, la prioridad de LightBurn y los avisos (frases sin el nombre de la capa, que `Layers`
/// agrupa).
fn convert(layer: &Layer, color: Rgb) -> (LayerSettings, i32, Vec<String>) {
    let (mut main, priority, mut warns) = convert_one(layer, color);
    if main.output && !layer.subs.is_empty() && !layer.is_tool() {
        for sub in &layer.subs {
            let (mut s, _, w) = convert_one(sub, color);
            let n = main.sub_layers.len() + 1;
            if sub.is_tool() || (s.mode == LayerMode::Image) != (main.mode == LayerMode::Image) {
                warns.push(format!(
                    "la sub-capa {n} es de un tipo ({}) que no se puede combinar con la capa; se ignora",
                    sub.kind
                ));
                continue;
            }
            if main.sub_layers.len() >= MAX_SUB_LAYERS {
                warns.push(format!(
                    "hay más de {MAX_SUB_LAYERS} sub-capas; se ignoran las sobrantes"
                ));
                break;
            }
            warns.extend(w.into_iter().map(|w| format!("sub-capa {n}: {w}")));
            // Una sub-capa no lleva color, orden ni sub-capas propias.
            s.color = main.color;
            s.order = None;
            s.sub_layers.clear();
            main.sub_layers.push(s);
        }
    }
    (main, priority, warns)
}

/// Convierte una sola `CutSetting`, sin mirar sus sub-capas.
fn convert_one(layer: &Layer, color: Rgb) -> (LayerSettings, i32, Vec<String>) {
    let mut s = LayerSettings::for_color(color);
    let mut warns: Vec<String> = Vec::new();
    s.name = layer.name.clone();
    let kind = layer.kind.to_ascii_lowercase();

    // Modo.
    let tool = layer.is_tool();
    if tool {
        s.output = false;
        s.mode = LayerMode::Line;
    } else if kind == "image" {
        s.mode = LayerMode::Image;
    } else if kind.contains("offset") {
        s.mode = LayerMode::Fill;
        s.fill_style = FillStyle::Offset;
    } else if kind == "scan+cut" || kind.contains("fill+line") || kind.contains("scan+") {
        s.mode = LayerMode::Fill;
        warns.push(
            "«Relleno + línea» no existe en una sola capa; se importa solo el relleno (crea otra \
             capa de línea si también quieres el contorno)"
                .into(),
        );
    } else if kind == "scan" || kind == "fill" {
        s.mode = LayerMode::Fill;
    } else if kind == "cut" || kind == "line" {
        s.mode = LayerMode::Line;
    } else {
        warns.push(format!(
            "modo de capa «{}» desconocido; se importa como línea",
            layer.kind
        ));
    }
    if tool {
        // T1/T2 no llevan ajustes de corte.
        let priority = layer.num("priority").map_or(0, |v| v as i32);
        return (s, priority, warns);
    }

    // Velocidad (mm/s en el archivo) y potencias.
    let speed_mm_s = layer.num("speed").filter(|v| *v > 0.0);
    if let Some(v) = speed_mm_s {
        s.speed = v * 60.0;
    }
    let speed_mm_s = speed_mm_s.unwrap_or(s.speed / 60.0);
    if let Some(v) = layer.num("maxPower") {
        s.power = v.clamp(0.0, 100.0);
    }
    if let Some(v) = layer.num("minPower") {
        s.min_power = v.clamp(0.0, 100.0);
    }
    if let Some(v) = layer.num("numPasses") {
        s.passes = (v.round() as i64).clamp(1, i64::from(u32::MAX)) as u32;
    }

    // Relleno e imagen.
    if let Some(v) = layer.num("interval").filter(|v| *v > 0.0) {
        s.interval = v;
    } else if let Some(dpi) = layer.num("dpi").filter(|v| *v > 0.0) {
        s.interval = 25.4 / dpi;
    }
    if let Some(v) = layer.num("angle") {
        s.angle = v;
    }
    if let Some(v) = layer.flag("crossHatch") {
        s.crosshatch = v;
    }
    if let Some(v) = layer.flag("bidir") {
        s.bidirectional = v;
    }
    // Overscan: LightBurn lo da como % de la velocidad (docs de LightBurn: «Overscanning distance is
    // calculated as a percentage of Speed»); con la velocidad en mm/s sale la distancia en mm.
    // (inferido: la unidad exacta no está documentada)
    s.overscan = if layer.flag("overscan").unwrap_or(false) {
        let pct = layer.num("overscanPercent").unwrap_or(2.5);
        Overscan::Mm((speed_mm_s * pct / 100.0).max(0.0))
    } else {
        Overscan::Mm(0.0)
    };
    if let Some(v) = layer.num("kerf") {
        s.kerf = v;
    }
    if let Some(v) = layer.flag("runBlower") {
        s.air = v;
    }
    if let Some(v) = layer.num("zOffset") {
        s.z_offset = v;
    }
    if let Some(v) = layer.num("zPerPass") {
        s.z_step = v;
    }
    if let Some(v) = layer.flag("doOutput") {
        s.output = v;
    }
    if let Some(v) = layer.flag("hide") {
        s.visible = !v;
    }

    // Puentes.
    if layer.flag("tabsEnabled").unwrap_or(false) {
        let d = TabSettings::default();
        s.tabs = TabSettings {
            enabled: true,
            width: layer.num("tabSize").filter(|v| *v > 0.0).unwrap_or(d.width),
            count: layer
                .num("tabCount")
                .filter(|v| *v >= 1.0)
                .map_or(d.count, |v| v as u32),
            spacing: if layer.flag("tabsUseSpacing").unwrap_or(false) {
                layer.num("tabSpacing").filter(|v| *v > 0.0).unwrap_or(0.0)
            } else {
                0.0
            },
            skip_inner: layer.flag("skipInnerTabs").unwrap_or(d.skip_inner),
            power: d.power,
        };
        if layer.flag("manualTabs").unwrap_or(false) {
            // Con puentes manuales LightBurn coloca cada puente a mano; aquí se reparten solos.
            warns.push(
                "los puentes de LightBurn se colocan a mano y aquí se reparten solos (revisa su \
                 posición)"
                    .into(),
            );
        }
    }

    // Ajustes avanzados de corte (solo cuentan en capas de línea; en las demás el motor no los usa).
    let on = |k: &str| layer.flag(k).unwrap_or(false);
    let nonzero = |k: &str| layer.num(k).is_some_and(|v| v != 0.0);
    let positive = |k: &str| layer.num(k).filter(|v| *v > 0.0);
    if s.mode == LayerMode::Line {
        let d = Perforation::default();
        s.perforation = Perforation {
            enabled: on("perforate"),
            cut_mm: positive("perfLen").unwrap_or(d.cut_mm),
            skip_mm: positive("perfSkip").unwrap_or(d.skip_mm),
        };
        let d = DotMode::default();
        s.dot_mode = DotMode {
            enabled: on("dotMode"),
            dwell_ms: positive("dotTime").unwrap_or(d.dwell_ms),
            spacing_mm: layer
                .num("dotSpacing")
                .filter(|v| *v >= 0.01)
                .unwrap_or(d.spacing_mm),
        };
        s.overcut = layer.num("overcut").filter(|v| *v > 0.0).unwrap_or(0.0);
        // `enablePowerRamp` (si el archivo lo trae) manda; si no, basta un largo de rampa.
        let ramp_on = layer
            .flag("enablePowerRamp")
            .unwrap_or_else(|| nonzero("rampLength"));
        let d = Ramp::default();
        s.ramp = Ramp {
            enabled: ramp_on && positive("rampLength").is_some(),
            length_mm: positive("rampLength").unwrap_or(d.length_mm),
            start_pct: d.start_pct,
            outer_only: on("rampOuter"),
        };
        if ramp_on && positive("rampLength").is_none() {
            warns.push("la rampa de potencia no tiene largo; se ignora".into());
        }
        let delay = |k: &str| layer.num(k).filter(|v| *v > 0.0).unwrap_or(0.0);
        let (start, end) = (delay("startDelay"), delay("endDelay"));
        if on("enableCutThrough") || on("enableCutThroughStart") || on("enableCutThroughEnd") {
            // El tiempo del pulso de atravesado sale de la pausa de inicio (o de fin): en Tlaku
            // es parte del corte pasante y ya no es una pausa aparte (inferido).
            let d = CutThrough::default();
            let ms = if start > 0.0 { start } else { end };
            s.cut_through = CutThrough {
                enabled: true,
                power: layer
                    .num("throughPower")
                    .map_or(d.power, |v| v.clamp(0.0, 100.0)),
                ms: if ms > 0.0 { ms } else { d.ms },
            };
            if ms <= 0.0 {
                warns.push(format!(
                    "el corte pasante no trae su tiempo; se usan {} ms",
                    d.ms
                ));
            }
        } else {
            s.start_delay_ms = start;
            s.end_delay_ms = end;
        }
    } else if s.mode == LayerMode::Fill && s.fill_style == FillStyle::Scan && on("floodFill") {
        s.fill_style = FillStyle::Flood;
    }

    // Modo imagen.
    if s.mode == LayerMode::Image {
        if let Some(mode) = layer.text("ditherMode") {
            match dither_mode(mode) {
                Some(m) => s.image.mode = m,
                None => warns.push(format!(
                    "modo de imagen «{mode}» desconocido; se usa Stucki"
                )),
            }
        }
        if let Some(cpi) = layer.num("cellsPerInch").filter(|v| *v > 0.0) {
            s.image.halftone_cell = 25.4 / cpi;
        }
        if let Some(a) = layer.num("halftoneAngle") {
            s.image.halftone_angle = a;
        }
    }

    // Lo que no tiene equivalente, solo si está activo.
    let mut ignored: Vec<&str> = Vec::new();
    if on("enablePPI") {
        ignored.push("pulsos por pulgada (PPI)");
    }
    if on("overrideFrequency") {
        ignored.push("frecuencia del láser");
    }
    if on("enableLaser2") {
        ignored.push("segundo láser");
    }
    if on("forceConstantPower") {
        ignored.push("potencia constante");
    }
    if on("blowerSpeedOverride") {
        ignored.push("velocidad del soplador");
    }
    if layer.text("scanOpt").is_some_and(|v| v == "individual") && s.mode == LayerMode::Fill {
        ignored.push("relleno por objeto (scanOpt individual)");
    }
    warns.extend(
        ignored
            .into_iter()
            .map(|what| format!("«{what}» no tiene equivalente en Tlaku y se ignora")),
    );
    let priority = layer.num("priority").map_or(0, |v| v as i32);
    (s, priority, warns)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn palette_colors_are_distinct_and_never_the_guide_color() {
        let mut seen = HashSet::new();
        for (_, c) in COLORS {
            assert!(seen.insert(c), "color repetido {c}");
        }
        for c in TOOL_COLORS {
            assert!(seen.insert(c), "color repetido {c}");
        }
        // El color de las guías de la app (`GUIDE_COLOR`) es el de T1 y de ninguna capa de trabajo.
        assert_eq!(TOOL_COLORS[0], Rgb(0x7e, 0x8a, 0xa0));
        assert!(!COLORS.iter().any(|(_, c)| *c == TOOL_COLORS[0]));
    }

    #[test]
    fn out_of_palette_indices_get_unique_colors() {
        let mut used: HashSet<Rgb> = (0..32).map(|i| pick_color(i, &HashSet::new())).collect();
        for index in [32, 33, 99, 255, -1] {
            let c = pick_color(index, &used);
            assert!(used.insert(c), "índice {index}: color repetido {c}");
        }
    }

    #[test]
    fn dither_names() {
        assert_eq!(dither_mode("threshold"), Some(ImageMode::Threshold));
        assert_eq!(dither_mode("Dithered"), Some(ImageMode::Floyd));
        assert_eq!(dither_mode("ordered"), Some(ImageMode::Bayer4));
        assert_eq!(dither_mode("???"), None);
    }
}
