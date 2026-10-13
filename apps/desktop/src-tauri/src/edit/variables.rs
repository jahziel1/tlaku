//! Comandos del área `variables` (ola 3): lectura del CSV del texto variable.
//!
//! La interfaz resuelve las plantillas (`{serie}`, `{csv:Nombre}`...); aquí solo se lee el
//! archivo de datos. El formato que se acepta:
//! - UTF-8 con o sin BOM (cualquier otra codificación se rechaza con un mensaje claro);
//! - separador `,`, `;` o tabulador, detectado en la fila de encabezados;
//! - campos entre comillas dobles, con `""` para una comilla y saltos de línea dentro;
//! - la primera fila son los encabezados; las filas totalmente vacías se ignoran.

use serde::Serialize;
use std::path::Path;

/// Más filas que esto no son una lista de etiquetas, son un error de archivo.
pub const MAX_ROWS: usize = 100_000;
/// Tamaño máximo del archivo (bytes).
pub const MAX_BYTES: u64 = 50 * 1024 * 1024;

/// Misma forma que `CsvTable` de `src/api/types.ts`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CsvTable {
    /// Nombres de columna de la primera fila (sin espacios en los extremos).
    pub headers: Vec<String>,
    /// Filas de datos; cada una tiene al menos tantas celdas como encabezados.
    pub rows: Vec<Vec<String>>,
    /// Separador detectado: `,`, `;` o tabulador.
    pub delimiter: String,
}

impl CsvTable {
    /// Posición de la columna `name`: coincidencia exacta y, si no, sin distinguir mayúsculas.
    pub fn column(&self, name: &str) -> Option<usize> {
        let name = name.trim();
        let lower = name.to_lowercase();
        self.headers
            .iter()
            .position(|h| h == name)
            .or_else(|| self.headers.iter().position(|h| h.to_lowercase() == lower))
    }

    /// Nombres de `required` que no son una columna (ni un número de columna válido, 1 a N).
    pub fn missing(&self, required: &[String]) -> Vec<String> {
        required
            .iter()
            .filter(|name| {
                self.column(name).is_none()
                    && !name
                        .trim()
                        .parse::<usize>()
                        .is_ok_and(|n| n >= 1 && n <= self.headers.len())
            })
            .cloned()
            .collect()
    }
}

/// Separador más probable según la primera línea (fuera de comillas). Empata a favor de `,`.
fn detect_delimiter(text: &str) -> char {
    let (mut comas, mut puntos, mut tabs) = (0usize, 0usize, 0usize);
    let mut in_quotes = false;
    for c in text.chars() {
        match c {
            '"' => in_quotes = !in_quotes,
            '\n' | '\r' if !in_quotes => break,
            ',' if !in_quotes => comas += 1,
            ';' if !in_quotes => puntos += 1,
            '\t' if !in_quotes => tabs += 1,
            _ => {}
        }
    }
    if puntos > comas && puntos >= tabs {
        ';'
    } else if tabs > comas && tabs > puntos {
        '\t'
    } else {
        ','
    }
}

/// Registros del texto con las reglas de RFC 4180. Devuelve el error con el número de línea.
fn records(text: &str, delim: char) -> Result<Vec<Vec<String>>, String> {
    let mut out: Vec<Vec<String>> = Vec::new();
    let mut row: Vec<String> = Vec::new();
    let mut field = String::new();
    let mut in_quotes = false;
    // El campo tuvo comillas: `""` es un campo vacío legítimo y no una fila vacía.
    let mut quoted = false;
    let mut line = 1usize;
    let mut quote_line = 1usize;
    let mut chars = text.chars().peekable();

    let mut end_row = |row: &mut Vec<String>, field: &mut String, quoted: &mut bool| {
        row.push(std::mem::take(field));
        let empty = row.len() == 1 && row[0].is_empty() && !*quoted;
        *quoted = false;
        let done = std::mem::take(row);
        if !empty {
            out.push(done);
        }
    };

    while let Some(c) = chars.next() {
        if in_quotes {
            match c {
                '"' if chars.peek() == Some(&'"') => {
                    chars.next();
                    field.push('"');
                }
                '"' => in_quotes = false,
                '\n' => {
                    line += 1;
                    field.push('\n');
                }
                _ => field.push(c),
            }
            continue;
        }
        match c {
            '"' if field.is_empty() && !quoted => {
                in_quotes = true;
                quoted = true;
                quote_line = line;
            }
            c if c == delim => {
                row.push(std::mem::take(&mut field));
                quoted = false;
            }
            '\r' => {
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
                line += 1;
                end_row(&mut row, &mut field, &mut quoted);
            }
            '\n' => {
                line += 1;
                end_row(&mut row, &mut field, &mut quoted);
            }
            _ => field.push(c),
        }
    }
    if in_quotes {
        return Err(format!(
            "Comillas sin cerrar (empiezan en la línea {quote_line})."
        ));
    }
    if !field.is_empty() || !row.is_empty() || quoted {
        end_row(&mut row, &mut field, &mut quoted);
    }
    Ok(out)
}

/// Lee un CSV desde sus bytes (UTF-8 con o sin BOM).
pub fn parse_csv(bytes: &[u8]) -> Result<CsvTable, String> {
    let bytes = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes);
    let text = std::str::from_utf8(bytes)
        .map_err(|_| "El CSV debe estar guardado como UTF-8.".to_string())?;
    let delim = detect_delimiter(text);
    let mut recs = records(text, delim)?.into_iter();
    let Some(head) = recs.next() else {
        return Err("El CSV está vacío.".into());
    };
    let headers: Vec<String> = head.iter().map(|h| h.trim().to_string()).collect();
    let width = headers.len();
    let mut rows = Vec::new();
    for mut r in recs {
        if rows.len() >= MAX_ROWS {
            return Err(format!("El CSV tiene más de {MAX_ROWS} filas."));
        }
        if r.len() < width {
            r.resize(width, String::new());
        }
        rows.push(r);
    }
    Ok(CsvTable {
        headers,
        rows,
        delimiter: delim.to_string(),
    })
}

/// Lee el archivo y comprueba que existan las columnas de `required`.
pub fn read_csv_file(path: &Path, required: &[String]) -> Result<CsvTable, String> {
    let meta = std::fs::metadata(path)
        .map_err(|e| format!("No se pudo abrir el CSV «{}»: {e}", path.display()))?;
    if meta.len() > MAX_BYTES {
        return Err("El CSV pesa más de 50 MB.".into());
    }
    let bytes = std::fs::read(path)
        .map_err(|e| format!("No se pudo leer el CSV «{}»: {e}", path.display()))?;
    let table = parse_csv(&bytes)?;
    let missing = table.missing(required);
    if !missing.is_empty() {
        return Err(format!(
            "Al CSV le faltan columnas que usa el diseño: {}. Columnas disponibles: {}.",
            missing.join(", "),
            table.headers.join(", ")
        ));
    }
    Ok(table)
}

/// Lee el CSV del texto variable. `required` son las columnas que usan las plantillas del
/// diseño (`{csv:Nombre}`); si falta alguna, falla con un mensaje que las nombra.
#[tauri::command]
pub async fn read_csv(path: String, required: Option<Vec<String>>) -> Result<CsvTable, String> {
    tauri::async_runtime::spawn_blocking(move || {
        read_csv_file(Path::new(&path), &required.unwrap_or_default())
    })
    .await
    .map_err(|e| format!("La tarea se interrumpió: {e}"))?
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table(s: &str) -> CsvTable {
        parse_csv(s.as_bytes()).unwrap()
    }

    #[test]
    fn lee_encabezados_y_filas_con_coma() {
        let t = table("Nombre,Cargo\nAna,Jefa\nLuis,Técnico\n");
        assert_eq!(t.headers, ["Nombre", "Cargo"]);
        assert_eq!(t.rows, [["Ana", "Jefa"], ["Luis", "Técnico"]]);
        assert_eq!(t.delimiter, ",");
    }

    #[test]
    fn detecta_punto_y_coma_y_tabulador() {
        assert_eq!(table("a;b;c\n1;2;3").delimiter, ";");
        assert_eq!(table("a\tb\tc\n1\t2\t3").delimiter, "\t");
        // Decimales con coma dentro de un CSV de punto y coma no confunden la detección.
        let t = table("precio;nombre\n1,5;Tornillo");
        assert_eq!(t.delimiter, ";");
        assert_eq!(t.rows[0], ["1,5", "Tornillo"]);
    }

    #[test]
    fn comillas_con_separador_comilla_doble_y_salto() {
        let t = table("a,b\n\"x, y\",\"di \"\"hola\"\"\"\n\"dos\nlíneas\",z\n");
        assert_eq!(t.rows[0], ["x, y", "di \"hola\""]);
        assert_eq!(t.rows[1], ["dos\nlíneas", "z"]);
    }

    #[test]
    fn acepta_bom_y_saltos_de_windows() {
        let t = parse_csv(b"\xEF\xBB\xBFa,b\r\n1,2\r\n3,4\r\n").unwrap();
        assert_eq!(t.headers, ["a", "b"]);
        assert_eq!(t.rows.len(), 2);
        assert_eq!(t.rows[1], ["3", "4"]);
    }

    #[test]
    fn ignora_filas_vacias_y_completa_las_cortas() {
        let t = table("a,b,c\n\n1\n\n2,3,4\n");
        assert_eq!(t.rows, [["1", "", ""], ["2", "3", "4"]]);
    }

    #[test]
    fn campo_vacio_entre_comillas_no_es_fila_vacia() {
        let t = table("a\n\"\"\nx\n");
        assert_eq!(t.rows, [[""], ["x"]]);
    }

    #[test]
    fn rechaza_lo_que_no_es_utf8_ni_comillas_abiertas() {
        assert!(parse_csv(b"a,b\n\xE9,2").unwrap_err().contains("UTF-8"));
        assert!(parse_csv(b"a,b\n\"x,2").unwrap_err().contains("Comillas"));
        assert!(parse_csv(b"").unwrap_err().contains("vacío"));
    }

    #[test]
    fn columna_por_nombre_exacto_o_sin_mayusculas() {
        let t = table("Nombre,Cargo\nA,B");
        assert_eq!(t.column("Cargo"), Some(1));
        assert_eq!(t.column(" cargo "), Some(1));
        assert_eq!(t.column("Edad"), None);
    }

    #[test]
    fn valida_columnas_requeridas_por_nombre_o_numero() {
        let t = table("Nombre,Cargo\nA,B");
        let req = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert!(t.missing(&req(&["Nombre", "2"])).is_empty());
        assert_eq!(t.missing(&req(&["Nombre", "Edad", "3"])), ["Edad", "3"]);
    }

    #[test]
    fn read_csv_file_nombra_las_columnas_que_faltan() {
        let dir = std::env::temp_dir().join(format!("tlaku-csv-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("datos.csv");
        std::fs::write(&path, "Nombre;Cargo\nAna;Jefa\n").unwrap();
        let ok = read_csv_file(&path, &["Nombre".into()]).unwrap();
        assert_eq!(ok.rows.len(), 1);
        let err = read_csv_file(&path, &["Edad".into()]).unwrap_err();
        assert!(
            err.contains("Edad") && err.contains("Nombre, Cargo"),
            "{err}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
