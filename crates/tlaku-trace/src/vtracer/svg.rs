// Adaptado de vtracer 0.6.5 (https://github.com/visioncortex/vtracer), src/svg.rs.
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
// Cambios de Tlaku: el comentario del generador conserva la versión de vtracer de origen y no la del crate; `SvgFile`, `SvgPath` y su formato son iguales.

use std::fmt;
use visioncortex::{Color, CompoundPath, PointF64};

#[derive(Debug, Clone)]
pub(crate) struct SvgFile {
    pub paths: Vec<SvgPath>,
    pub width: usize,
    pub height: usize,
    pub path_precision: Option<u32>,
}

#[derive(Debug, Clone)]
pub(crate) struct SvgPath {
    pub path: CompoundPath,
    pub color: Color,
}

impl SvgFile {
    pub fn new(width: usize, height: usize, path_precision: Option<u32>) -> Self {
        SvgFile {
            paths: vec![],
            width,
            height,
            path_precision,
        }
    }

    pub fn add_path(&mut self, path: CompoundPath, color: Color) {
        self.paths.push(SvgPath { path, color })
    }
}

impl fmt::Display for SvgFile {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, r#"<?xml version="1.0" encoding="UTF-8"?>"#)?;
        writeln!(f, r#"<!-- Generator: visioncortex VTracer 0.6.5 -->"#)?;
        writeln!(
            f,
            r#"<svg version="1.1" xmlns="http://www.w3.org/2000/svg" width="{}" height="{}">"#,
            self.width, self.height
        )?;

        for path in &self.paths {
            path.fmt_with_precision(f, self.path_precision)?;
        }

        writeln!(f, "</svg>")
    }
}

impl fmt::Display for SvgPath {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.fmt_with_precision(f, None)
    }
}

impl SvgPath {
    fn fmt_with_precision(&self, f: &mut fmt::Formatter, precision: Option<u32>) -> fmt::Result {
        let (string, offset) = self
            .path
            .to_svg_string(true, PointF64::default(), precision);
        writeln!(
            f,
            "<path d=\"{}\" fill=\"{}\" transform=\"translate({},{})\"/>",
            string,
            self.color.to_hex_string(),
            offset.x,
            offset.y
        )
    }
}
