//! Catálogo de fuentes y selección de cara por familia + negrita + cursiva.

use crate::linefont::{LineFont, builtin_line_font, builtin_line_fonts};
use crate::shx;
use fontdb::{Database, ID, Stretch, Style};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::{Arc, OnceLock, RwLock};
use tlaku_i18n::tr;

/// Familia con los estilos disponibles (por ejemplo `["Regular", "Bold", "Italic"]`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FontFamily {
    pub family: String,
    pub styles: Vec<String>,
    /// Fuente de un solo trazo (Hershey o SHX): da trayectorias abiertas, no contornos.
    #[serde(default)]
    pub single_line: bool,
}

/// Resultado de elegir una cara: la elegida y las advertencias si no es exactamente lo pedido.
#[derive(Debug, Clone)]
pub struct FaceChoice {
    pub id: ID,
    /// Nombre de la familia tal como la declara la fuente.
    pub family: String,
    pub weight: u16,
    pub italic: bool,
    pub warnings: Vec<String>,
}

/// Fuentes disponibles. Se carga una vez y se comparte por referencia.
pub struct FontLibrary {
    db: Database,
    /// Fuentes de un trazo cargadas por el usuario (SHX). Las Hershey van siempre incluidas.
    user_line_fonts: RwLock<Vec<Arc<LineFont>>>,
}

impl Default for FontLibrary {
    fn default() -> Self {
        Self::new()
    }
}

impl FontLibrary {
    /// Biblioteca vacía (útil en pruebas: se cargan solo las fuentes necesarias).
    pub fn new() -> Self {
        Self {
            db: Database::new(),
            user_line_fonts: RwLock::new(Vec::new()),
        }
    }

    /// Biblioteca con las fuentes instaladas en el sistema.
    pub fn system() -> Self {
        let mut lib = Self::new();
        lib.load_system_fonts();
        lib
    }

    /// Biblioteca del sistema compartida por todo el proceso; se carga la primera vez que se pide.
    pub fn system_shared() -> &'static FontLibrary {
        static SHARED: OnceLock<FontLibrary> = OnceLock::new();
        SHARED.get_or_init(FontLibrary::system)
    }

    pub fn load_system_fonts(&mut self) {
        self.db.load_system_fonts();
    }

    /// Carga las fuentes de un archivo (.ttf, .otf o colección). Devuelve cuántas caras añadió.
    pub fn load_font_file(&mut self, path: &Path) -> Result<usize, String> {
        let data = std::fs::read(path)
            .map_err(|e| tr!("text.library.read_failed", path = path.display(), error = e))?;
        self.load_font_data(data)
            .map_err(|e| tr!("text.library.with_path", error = e, path = path.display()))
    }

    /// Carga las fuentes desde bytes. Devuelve cuántas caras añadió.
    pub fn load_font_data(&mut self, data: Vec<u8>) -> Result<usize, String> {
        let before = self.db.len();
        self.db.load_font_data(data);
        match self.db.len() - before {
            0 => Err(tr!("text.library.not_a_font")),
            n => Ok(n),
        }
    }

    /// Carga todas las fuentes de una carpeta (recursivo).
    pub fn load_fonts_dir(&mut self, dir: &Path) {
        self.db.load_fonts_dir(dir);
    }

    pub fn is_empty(&self) -> bool {
        self.db.is_empty()
    }

    /// Familias ordenadas sin distinguir mayúsculas, con sus estilos ordenados por peso.
    pub fn families(&self) -> Vec<FontFamily> {
        use std::collections::BTreeMap;
        // Clave en minúsculas para ordenar; guarda el nombre original y (peso, cursiva, nombre).
        type Styles = Vec<(u16, bool, String)>;
        let mut map: BTreeMap<String, (String, Styles)> = BTreeMap::new();
        for face in self.db.faces() {
            let Some(name) = face.families.first().map(|(n, _)| n.clone()) else {
                continue;
            };
            // Las familias con punto inicial son internas del sistema (macOS) y no se ofrecen.
            if name.starts_with('.') {
                continue;
            }
            let entry = map
                .entry(name.to_lowercase())
                .or_insert_with(|| (name, Vec::new()));
            let italic = face.style != Style::Normal;
            let style = style_name(face.weight.0, face.style);
            if !entry.1.iter().any(|(_, _, s)| *s == style) {
                entry.1.push((face.weight.0, italic, style));
            }
        }
        map.into_values()
            .map(|(family, mut styles)| {
                styles.sort();
                FontFamily {
                    family,
                    styles: styles.into_iter().map(|(_, _, s)| s).collect(),
                    single_line: false,
                }
            })
            .collect()
    }

    /// Fuentes de un trazo disponibles: las Hershey incluidas y las SHX cargadas, ordenadas
    /// por nombre dentro de cada grupo (primero las incluidas).
    pub fn line_families(&self) -> Vec<FontFamily> {
        let entry = |f: &Arc<LineFont>| FontFamily {
            family: f.name.clone(),
            styles: vec!["Regular".into()],
            single_line: true,
        };
        let mut out: Vec<FontFamily> = builtin_line_fonts().iter().map(entry).collect();
        let user = self
            .user_line_fonts
            .read()
            .unwrap_or_else(|e| e.into_inner());
        let mut extra: Vec<FontFamily> = user.iter().map(entry).collect();
        extra.sort_by_key(|f| f.family.to_lowercase());
        out.extend(extra);
        out
    }

    /// Fuente de un trazo por nombre (sin distinguir mayúsculas): primero las del usuario, para
    /// que puedan sustituir a una incluida.
    pub fn line_font(&self, name: &str) -> Option<Arc<LineFont>> {
        let wanted = name.trim().to_lowercase();
        if wanted.is_empty() {
            return None;
        }
        let user = self
            .user_line_fonts
            .read()
            .unwrap_or_else(|e| e.into_inner());
        user.iter()
            .find(|f| f.name.to_lowercase() == wanted)
            .cloned()
            .or_else(|| builtin_line_font(&wanted))
    }

    /// Registra una fuente de un trazo; si ya había una con ese nombre la reemplaza.
    pub fn add_line_font(&self, font: LineFont) {
        let wanted = font.name.to_lowercase();
        let mut user = self
            .user_line_fonts
            .write()
            .unwrap_or_else(|e| e.into_inner());
        user.retain(|f| f.name.to_lowercase() != wanted);
        user.push(Arc::new(font));
    }

    /// Carga una fuente SHX de AutoCAD desde un archivo y devuelve el nombre con que queda
    /// registrada (el del archivo sin extensión).
    pub fn load_shx_file(&self, path: &Path) -> Result<String, String> {
        let data = std::fs::read(path)
            .map_err(|e| tr!("text.library.read_failed", path = path.display(), error = e))?;
        let name = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .filter(|s| !s.trim().is_empty())
            .ok_or_else(|| tr!("text.library.shx_no_name"))?;
        self.load_shx_data(&name, &data)
            .map_err(|e| tr!("text.library.with_path", error = e, path = path.display()))
    }

    /// Carga una fuente SHX desde bytes con el nombre dado.
    pub fn load_shx_data(&self, name: &str, data: &[u8]) -> Result<String, String> {
        let font = shx::parse_shx(name, data)?;
        let name = font.name.clone();
        self.add_line_font(font);
        Ok(name)
    }

    /// Elige la cara de `family` más cercana a lo pedido.
    ///
    /// Reglas de sustitución (siempre se informa en `warnings`, nunca se inventa una cursiva
    /// ni una negrita sintética):
    /// - Cursiva pedida y la familia solo tiene caras rectas: se usa la recta.
    /// - Cursiva no pedida y la familia solo tiene cursivas: se usa la cursiva.
    /// - Negrita pedida: se usa el peso disponible más cercano a 700; si queda por debajo de
    ///   600 se advierte de que no hay negrita real. Sin negrita se busca el más cercano a 400.
    ///
    /// La familia se compara sin distinguir mayúsculas. Si no existe, el error propone
    /// familias parecidas.
    pub fn resolve(&self, family: &str, bold: bool, italic: bool) -> Result<FaceChoice, String> {
        let wanted = family.trim().to_lowercase();
        let candidates: Vec<&fontdb::FaceInfo> = self
            .db
            .faces()
            .filter(|f| f.families.iter().any(|(n, _)| n.to_lowercase() == wanted))
            .collect();
        if candidates.is_empty() {
            return Err(self.not_found_message(family));
        }
        let mut warnings = Vec::new();
        let shown = candidates[0].families[0].0.clone();

        let is_italic = |f: &&fontdb::FaceInfo| f.style != Style::Normal;
        let same_slant: Vec<&fontdb::FaceInfo> = candidates
            .iter()
            .copied()
            .filter(|f| is_italic(f) == italic)
            .collect();
        let pool = if same_slant.is_empty() {
            warnings.push(if italic {
                tr!("text.library.no_italic", name = shown)
            } else {
                tr!("text.library.only_italic", name = shown)
            });
            candidates
        } else {
            same_slant
        };

        let target: i32 = if bold { 700 } else { 400 };
        let best = pool
            .iter()
            .min_by_key(|f| {
                let w = i32::from(f.weight.0);
                // Menor distancia al peso pedido; a igualdad, el lado más cercano al estilo
                // pedido (más pesado si es negrita, más ligero si no); prefiere ancho normal.
                let side = if bold { -w } else { w };
                (f.stretch != Stretch::Normal, (w - target).abs(), side, f.id)
            })
            .copied()
            .expect("el conjunto de candidatas no está vacío");

        if bold && best.weight.0 < 600 {
            warnings.push(tr!(
                "text.library.no_bold",
                name = shown,
                weight = best.weight.0
            ));
        } else if !bold && best.weight.0 >= 600 {
            warnings.push(tr!(
                "text.library.only_weight",
                name = shown,
                weight = best.weight.0
            ));
        }
        Ok(FaceChoice {
            id: best.id,
            family: shown,
            weight: best.weight.0,
            italic: is_italic(&best),
            warnings,
        })
    }

    /// Ejecuta `f` con los bytes de la cara y su índice dentro del archivo.
    pub(crate) fn with_face_data<T>(&self, id: ID, f: impl FnOnce(&[u8], u32) -> T) -> Option<T> {
        self.db.with_face_data(id, f)
    }

    fn not_found_message(&self, family: &str) -> String {
        if family.trim().is_empty() {
            return tr!("text.library.no_name");
        }
        if self.db.is_empty() {
            return tr!("text.library.no_fonts", family = family);
        }
        let names: Vec<String> = self.families().into_iter().map(|f| f.family).collect();
        let similar = similar_names(family, &names, 5);
        if similar.is_empty() {
            tr!("text.library.not_installed", family = family)
        } else {
            tr!(
                "text.library.not_installed_similar",
                family = family,
                similar = similar.join(", ")
            )
        }
    }
}

fn style_name(weight: u16, style: Style) -> String {
    let w = match weight {
        0..=149 => "Thin",
        150..=249 => "ExtraLight",
        250..=349 => "Light",
        350..=449 => "Regular",
        450..=549 => "Medium",
        550..=649 => "SemiBold",
        650..=749 => "Bold",
        750..=849 => "ExtraBold",
        _ => "Black",
    };
    match (w, style) {
        (w, Style::Normal) => w.to_string(),
        ("Regular", Style::Italic) => "Italic".into(),
        ("Regular", Style::Oblique) => "Oblique".into(),
        (w, Style::Italic) => format!("{w} Italic"),
        (w, Style::Oblique) => format!("{w} Oblique"),
    }
}

/// Hasta `max` nombres cercanos: contienen el texto (o al revés) o están a pocas ediciones.
fn similar_names(query: &str, names: &[String], max: usize) -> Vec<String> {
    let q = query.trim().to_lowercase();
    let limit = (q.chars().count() / 3).max(2);
    let mut scored: Vec<(bool, usize, &String)> = names
        .iter()
        .filter_map(|n| {
            let l = n.to_lowercase();
            let contains = q.chars().count() >= 3 && (l.contains(&q) || q.contains(&l));
            let dist = edit_distance(&q, &l);
            (contains || dist <= limit).then_some((!contains, dist, n))
        })
        .collect();
    scored.sort();
    scored.into_iter().take(max).map(|t| t.2.clone()).collect()
}

/// Distancia de Levenshtein sobre caracteres.
fn edit_distance(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut cur = vec![i + 1];
        for (j, &cb) in b.iter().enumerate() {
            let cost = usize::from(ca != cb);
            cur.push((prev[j] + cost).min(prev[j + 1] + 1).min(cur[j] + 1));
        }
        prev = cur;
    }
    prev[b.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edit_distance_basic() {
        assert_eq!(edit_distance("arial", "arial"), 0);
        assert_eq!(edit_distance("aria", "arial"), 1);
        assert_eq!(edit_distance("", "abc"), 3);
    }

    #[test]
    fn style_names() {
        assert_eq!(style_name(400, Style::Normal), "Regular");
        assert_eq!(style_name(400, Style::Italic), "Italic");
        assert_eq!(style_name(700, Style::Italic), "Bold Italic");
        assert_eq!(style_name(700, Style::Normal), "Bold");
    }

    #[test]
    fn similar_prefers_containing() {
        let names = vec![
            "Helvetica Neue".to_string(),
            "Arial".into(),
            "Courier".into(),
        ];
        assert_eq!(
            similar_names("helvetica", &names, 5),
            vec!["Helvetica Neue"]
        );
        assert_eq!(similar_names("Ariel", &names, 5), vec!["Arial"]);
        assert!(similar_names("Zzzzzzzz", &names, 5).is_empty());
    }
}
