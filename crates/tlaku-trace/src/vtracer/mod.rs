// Adaptado de vtracer 0.6.5 (https://github.com/visioncortex/vtracer), src/lib.rs.
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
// Cambios de Tlaku: se conserva solo la fachada del módulo (sin la enlazadura de Python); la biblioteca visioncortex se usa directamente.

//! Vectorizador binario de imágenes sobre `visioncortex`, adaptado de la biblioteca de vtracer.
//!
//! Tlaku solo usa el modo binario (tinta negra sobre blanco), así que no se incluyen el modo de
//! color, la jerarquía de capas, la lectura de archivos ni el programa de línea de comandos.

mod config;
mod converter;
mod svg;

pub(crate) use config::Config;
pub(crate) use converter::convert;
pub(crate) use visioncortex::ColorImage;
