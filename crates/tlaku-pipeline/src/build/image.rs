//! Capas de imagen: `Builder::image`.

use crate::job::LayerReport;
use crate::layers::ResolvedImage;
use crate::place::PlacedImage;
use crate::registration::warp_outline;
use crate::scan_offset;
use crate::zaxis::LayerZ;
use std::path::Path;
use tlaku_core::Rgb;
use tlaku_doc::Overscan;
use tlaku_raster::{Luma, ProcessOptions, Sampling};

use super::*;

impl Builder<'_> {
    pub(super) fn image(
        &mut self,
        color: Rgb,
        img: &PlacedImage,
        r: &ResolvedImage,
        z: LayerZ,
        angle_increment: f64,
    ) -> Result<(), String> {
        let mut raster = r.raster.clone();
        if self.unidirectional {
            raster.bidirectional = false;
        }
        if r.overscan == Overscan::Auto {
            raster.overscan_mm = tlaku_sim::overscan_mm(raster.speed_mm_min, 0.0, self.profile);
        }
        // Con incremento de ángulo la pasada `k` barre a `angle + k * incremento`: el bitmap es
        // el mismo, pero el recorrido cambia en cada pasada.
        let varied = angle_increment != 0.0 && r.passes > 1;
        // Con giro (Print & Cut) la imagen se carga a su tamaño original y se vuelve a muestrear
        // sobre la caja que ocupa ya girada.
        let (w, h) = img.warp.as_ref().map_or(img.size_mm, |wp| wp.src_size);
        let opts = ProcessOptions {
            adjust: r.adjust,
            sampling: Sampling::Interval(r.interval_mm),
            mode: r.mode,
            ..Default::default()
        };
        // La imagen se reduce a la rejilla de grabado al cargarla, antes de ajustes y tramado.
        let (gray, opts) = tlaku_raster::load_for_process(
            Path::new(&img.source_path),
            Luma::default(),
            w,
            h,
            &opts,
        )
        .map_err(|e| format!("{}: {e}", img.source_path))?;
        let gray = flipped(gray, img.mirror_x, img.mirror_y);
        let mut bitmap = if let Some(wp) = &img.warp {
            let (bw, bh) = img.size_mm;
            let (cols, rows) = tlaku_raster::resample::grid_size(bw, bh, opts.sampling)?;
            // Lo de fuera de la imagen es blanco y además se recorta con su contorno.
            let turned = tlaku_raster::warp::warp_affine(
                &gray,
                [wp.src_origin.x, wp.src_origin.y],
                [w, h],
                &wp.transform,
                [img.origin.x, img.origin.y],
                [bw, bh],
                cols,
                rows,
                1.0,
            )?;
            let mut b = tlaku_raster::process(&turned, bw, bh, &opts)
                .map_err(|e| format!("imagen: {e}"))?;
            tlaku_raster::apply_mask(&mut b, img.origin, &[warp_outline(wp)]);
            b
        } else {
            tlaku_raster::process(&gray, w, h, &opts).map_err(|e| format!("imagen: {e}"))?
        };
        // Máscara de recorte: lo que queda fuera de las formas no se graba.
        tlaku_raster::apply_mask(&mut bitmap, img.origin, &img.mask);
        let mut blocks = Vec::new();
        let mut overscan_mm = raster.overscan_mm;
        for k in 0..if varied { r.passes } else { 1 } {
            let mut pass = raster.clone();
            if varied {
                pass.angle_deg += angle_increment * f64::from(k);
                if r.overscan == Overscan::Auto {
                    pass.overscan_mm = tlaku_sim::overscan_mm(
                        pass.speed_mm_min,
                        pass.angle_deg.to_radians(),
                        self.profile,
                    );
                }
                overscan_mm = overscan_mm.max(pass.overscan_mm);
            }
            let mut block = tlaku_raster::raster_to_block(&bitmap, img.origin, &pass);
            if block.moves.is_empty() {
                self.warnings.push(
                    "imagen: no hay ningún punto que grabar (la imagen resultó en blanco)".into(),
                );
                return Ok(());
            }
            scan_offset::apply(
                &mut block,
                self.scan_offsets,
                self.scan_initial,
                &[pass.angle_deg],
                pass.bidirectional,
            );
            if k == 0
                && pass.bidirectional
                && let Some(w) = scan_offset::overscan_warning(
                    "imagen",
                    self.scan_offsets,
                    pass.speed_mm_min,
                    pass.overscan_mm,
                )
            {
                self.warnings.push(w);
            }
            blocks.push(block);
        }
        let lines = blocks.iter().map(travel_count).sum();
        let report = LayerReport {
            name: "imagen".into(),
            color,
            mode: "image",
            count: lines,
            unit: "líneas",
            speed_mm_min: raster.speed_mm_min,
            power: format!("{}-{}%", raster.min_power_pct, raster.max_power_pct),
            passes: r.passes,
            overscan_mm: Some(overscan_mm),
            plan: None,
            seconds: 0.0,
            bounds: None,
            tabs: 0,
            z_mm: None,
            pass: None,
            sub: None,
            laser2: false,
        };
        if varied {
            self.push_series(blocks, report, z);
        } else {
            let block = blocks.remove(0);
            self.push_passes(block, report, r.passes, z, true);
        }
        Ok(())
    }
}
