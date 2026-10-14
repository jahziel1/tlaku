// Adaptado de vtracer 0.6.5 (https://github.com/visioncortex/vtracer), src/converter.rs.
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
// Cambios de Tlaku: se conserva solo la conversión binaria en memoria (`binary_image_to_svg`); se quitan la conversión de color, el enmascarado por color clave (y con ello `fastrand`), la lectura de archivos con `image` 0.23 y la escritura a disco.

use super::config::{Config, ConverterConfig};
use super::svg::SvgFile;
use visioncortex::{Color, ColorImage, ColorName};

/// Convierte una imagen en memoria (ya decodificada) en un SVG en memoria. Binaria: los
/// píxeles con rojo menor que 128 son tinta.
pub(crate) fn convert(img: ColorImage, config: Config) -> Result<SvgFile, String> {
    binary_image_to_svg(img, config.into_converter_config())
}

fn binary_image_to_svg(img: ColorImage, config: ConverterConfig) -> Result<SvgFile, String> {
    let img = img.to_binary_image(|x| x.r < 128);
    let width = img.width;
    let height = img.height;

    let clusters = img.to_clusters(false);

    let mut svg = SvgFile::new(width, height, config.path_precision);
    for i in 0..clusters.len() {
        let cluster = clusters.get_cluster(i);
        if cluster.size() >= config.filter_speckle_area {
            let paths = cluster.to_compound_path(
                config.mode,
                config.corner_threshold,
                config.length_threshold,
                config.max_iterations,
                config.splice_threshold,
            );
            svg.add_path(paths, Color::color(&ColorName::Black));
        }
    }

    Ok(svg)
}
