// Adaptado de vtracer 0.6.5 (https://github.com/visioncortex/vtracer), src/config.rs.
// Copyright (c) 2023 Tsang Hao Fung.
//
// Licencia MIT OR Apache-2.0 (a elección). Se usa bajo la licencia MIT:
//
//   Permission is hereby granted, free of charge, to any person obtaining a copy of this
//   software and associated documentation files (the "Software"), to deal in the Software
//   without restriction, including without limitation the rights to use, copy, modify, merge,
//   publish, distribute, sublicense, and/or sell copies of the Software, and to permit persons
//   to whom the Software is furnished to do so, subject to the following conditions:
//
//   The above copyright notice and this permission notice shall be included in all copies or
//   substantial portions of the Software.
//
//   THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED,
//   INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR
//   PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE
//   FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR
//   OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
//   DEALINGS IN THE SOFTWARE.
//
// Cambios de Tlaku: se quitan los modos de color, las jerarquías, los preajustes y los parámetros que solo usa el modo de color; solo queda lo que necesita el modo binario.

use visioncortex::PathSimplifyMode;

/// Parámetros del vectorizado binario.
#[derive(Debug, Clone)]
pub(crate) struct Config {
    /// Lado (en píxeles) de las manchas que se descartan; el área mínima es su cuadrado.
    pub filter_speckle: usize,
    pub mode: PathSimplifyMode,
    /// Ángulo mínimo (grados) para considerar una esquina.
    pub corner_threshold: i32,
    pub length_threshold: f64,
    pub max_iterations: usize,
    /// Ángulo (grados) para unir segmentos de la spline.
    pub splice_threshold: i32,
    /// Decimales de las coordenadas del SVG; `None` es la precisión completa.
    pub path_precision: Option<u32>,
}

#[derive(Debug, Clone)]
pub(super) struct ConverterConfig {
    pub filter_speckle_area: usize,
    pub mode: PathSimplifyMode,
    pub corner_threshold: f64,
    pub length_threshold: f64,
    pub max_iterations: usize,
    pub splice_threshold: f64,
    pub path_precision: Option<u32>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            mode: PathSimplifyMode::Spline,
            filter_speckle: 4,
            corner_threshold: 60,
            length_threshold: 4.0,
            splice_threshold: 45,
            max_iterations: 10,
            path_precision: Some(2),
        }
    }
}

impl Config {
    pub(super) fn into_converter_config(self) -> ConverterConfig {
        ConverterConfig {
            filter_speckle_area: self.filter_speckle * self.filter_speckle,
            mode: self.mode,
            corner_threshold: deg2rad(self.corner_threshold),
            length_threshold: self.length_threshold,
            max_iterations: self.max_iterations,
            splice_threshold: deg2rad(self.splice_threshold),
            path_precision: self.path_precision,
        }
    }
}

fn deg2rad(deg: i32) -> f64 {
    deg as f64 / 180.0 * std::f64::consts::PI
}
