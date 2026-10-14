//! Lo que mide el trabajador por archivo y cómo se vuelca a CSV.

use serde::{Deserialize, Serialize};

/// Todo lo medido de un archivo y un modo. El trabajador imprime una copia acumulada (una línea
/// JSON) al terminar cada fase, así que si el proceso muere el padre conserva hasta dónde llegó.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Report {
    pub file: String,
    pub category: String,
    pub mode: String,
    pub file_bytes: u64,
    /// Fase en curso (vacío = terminó).
    pub running: String,
    /// `ok`, `error`, `panic`, `crash` o `timeout` (lo decide el padre salvo ok/error/panic).
    pub status: String,
    pub error_phase: String,
    pub error: String,

    // importar
    pub import_s: f64,
    pub objects: usize,
    pub vector_objects: usize,
    pub image_objects: usize,
    pub text_objects: usize,
    pub paths: usize,
    pub subpaths: usize,
    pub nodes: usize,
    pub layers: usize,
    pub import_warnings: usize,
    pub image_px: u64,
    pub import_rss_mb: f64,
    /// Factor con el que se escaló el diseño para que quepa en la cama (1 = no hizo falta).
    pub fit_scale: f64,

    // calcular
    pub compute_s: f64,
    pub est_s: f64,
    pub blocks: usize,
    pub moves: usize,
    pub cut_moves: usize,
    pub cut_len_mm: f64,
    pub compute_warnings: usize,
    pub compute_rss_mb: f64,

    // G-code
    pub gcode_s: f64,
    pub gcode_bytes: usize,
    pub gcode_lines: usize,
    pub gcode_rss_mb: f64,

    // Ruida
    pub rd_s: f64,
    pub rd_bytes: usize,
    pub rd_rss_mb: f64,

    // emulador
    /// `ok`, `omitido: ...` o `fallo: ...`.
    pub emu_status: String,
    pub emu_real_s: f64,
    pub emu_sim_s: f64,
    pub emu_speed: f64,
    pub emu_diff_pct: f64,
    pub emu_rapid_burns: usize,
    pub emu_stationary_burns: usize,
    pub emu_burn_mm: f64,
    pub emu_len_diff_pct: f64,
    pub emu_errors: usize,
    /// Porcentaje de muestras (mientras quedaban líneas por enviar) con el planificador vacío:
    /// si es alto, el anfitrión no alimentó a la máquina y el tiempo simulado no vale.
    pub emu_starved_pct: f64,
    pub emu_rss_mb: f64,

    pub peak_rss_mb: f64,
    pub total_s: f64,
    /// Fases que pasaron del umbral, separadas por `+`.
    pub slow: String,
}

pub const CSV_HEADER: &[&str] = &[
    "file",
    "category",
    "mode",
    "file_bytes",
    "status",
    "error_phase",
    "error",
    "slow",
    "import_s",
    "objects",
    "vector_objects",
    "image_objects",
    "text_objects",
    "paths",
    "subpaths",
    "nodes",
    "layers",
    "import_warnings",
    "image_px",
    "import_rss_mb",
    "fit_scale",
    "compute_s",
    "est_s",
    "blocks",
    "moves",
    "cut_moves",
    "cut_len_mm",
    "compute_warnings",
    "compute_rss_mb",
    "gcode_s",
    "gcode_bytes",
    "gcode_lines",
    "gcode_rss_mb",
    "rd_s",
    "rd_bytes",
    "rd_rss_mb",
    "emu_status",
    "emu_real_s",
    "emu_sim_s",
    "emu_speed",
    "emu_diff_pct",
    "emu_rapid_burns",
    "emu_stationary_burns",
    "emu_burn_mm",
    "emu_len_diff_pct",
    "emu_errors",
    "emu_starved_pct",
    "emu_rss_mb",
    "peak_rss_mb",
    "total_s",
];

fn quote(s: &str) -> String {
    let flat: String = s.chars().map(|c| if c == '\n' { ' ' } else { c }).collect();
    if flat.contains([',', '"']) {
        format!("\"{}\"", flat.replace('"', "\"\""))
    } else {
        flat
    }
}

fn f(v: f64) -> String {
    format!("{v:.3}")
}

impl Report {
    pub fn csv_row(&self) -> String {
        let cols: Vec<String> = vec![
            quote(&self.file),
            quote(&self.category),
            self.mode.clone(),
            self.file_bytes.to_string(),
            self.status.clone(),
            self.error_phase.clone(),
            quote(&self.error),
            self.slow.clone(),
            f(self.import_s),
            self.objects.to_string(),
            self.vector_objects.to_string(),
            self.image_objects.to_string(),
            self.text_objects.to_string(),
            self.paths.to_string(),
            self.subpaths.to_string(),
            self.nodes.to_string(),
            self.layers.to_string(),
            self.import_warnings.to_string(),
            self.image_px.to_string(),
            f(self.import_rss_mb),
            f(self.fit_scale),
            f(self.compute_s),
            f(self.est_s),
            self.blocks.to_string(),
            self.moves.to_string(),
            self.cut_moves.to_string(),
            f(self.cut_len_mm),
            self.compute_warnings.to_string(),
            f(self.compute_rss_mb),
            f(self.gcode_s),
            self.gcode_bytes.to_string(),
            self.gcode_lines.to_string(),
            f(self.gcode_rss_mb),
            f(self.rd_s),
            self.rd_bytes.to_string(),
            f(self.rd_rss_mb),
            quote(&self.emu_status),
            f(self.emu_real_s),
            f(self.emu_sim_s),
            f(self.emu_speed),
            f(self.emu_diff_pct),
            self.emu_rapid_burns.to_string(),
            self.emu_stationary_burns.to_string(),
            f(self.emu_burn_mm),
            f(self.emu_len_diff_pct),
            self.emu_errors.to_string(),
            f(self.emu_starved_pct),
            f(self.emu_rss_mb),
            f(self.peak_rss_mb),
            f(self.total_s),
        ];
        debug_assert_eq!(cols.len(), CSV_HEADER.len());
        cols.join(",")
    }
}
