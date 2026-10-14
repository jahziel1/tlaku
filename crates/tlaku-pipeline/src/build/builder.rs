//! `Builder`: agrega bloques y pasadas al trabajo en construcción.

use crate::job::{LayerReport, block_bounds};
use crate::zaxis::LayerZ;
use tlaku_core::toolpath::ToolpathBlock;

use super::*;

impl Builder<'_> {
    fn push(&mut self, mut block: ToolpathBlock, mut report: LayerReport) {
        if let Some(g) = &self.galvo {
            g.finish(&mut block, &mut self.warnings);
        }
        if let Some((n, total)) = self.sub {
            report.sub = Some((n, total));
            report.name = format!("{} · {n}/{total}", report.name);
        }
        report.bounds = block_bounds(&block);
        report.laser2 = self.laser2;
        report.u_offset_mm = self.u_offset_mm;
        if let Some(last) = block.moves.last() {
            self.head = last.to();
        }
        self.toolpath.blocks.push(block);
        self.layers.push(report);
    }

    /// Agrega el bloque de una capa con sus `passes` pasadas. `block` trae los movimientos de
    /// una sola pasada. Sin `z_step` (o sin eje Z) las pasadas se repiten dentro del mismo
    /// bloque; con `z_step` cada pasada es su propio bloque con su Z y su propio informe, para
    /// conservar la correspondencia 1:1 entre bloques e informes (ver `zaxis`).
    pub(super) fn push_passes(
        &mut self,
        mut block: ToolpathBlock,
        mut report: LayerReport,
        passes: u32,
        z: LayerZ,
        label_passes: bool,
    ) {
        if z.splits(passes) {
            for k in 0..passes {
                let mut b = block.clone();
                b.z_mm = z.z_at(k);
                if let Some(zk) = b.z_mm {
                    b.label
                        .push_str(&format!(" pasada {}/{passes} z={zk}", k + 1));
                }
                let mut r = report.clone();
                r.passes = 1;
                r.pass = Some((k + 1, passes));
                r.z_mm = b.z_mm;
                if k > 0 {
                    // El plan y sus explicaciones se muestran una sola vez.
                    r.plan = None;
                }
                self.push(b, r);
            }
            return;
        }
        if label_passes {
            repeat_passes(&mut block, passes);
        } else {
            repeat_moves(&mut block, passes);
        }
        block.z_mm = z.z_at(0);
        report.z_mm = block.z_mm;
        self.push(block, report);
    }

    /// Como [`Self::push_passes`] cuando cada pasada tiene sus propios movimientos (por
    /// ejemplo, con incremento de ángulo): `blocks` trae una pasada por elemento. Se reparte
    /// igual que las pasadas con Z: si `z` pide Z distinta en cada pasada, cada una es su propio
    /// bloque con su informe; si no, se concatenan en un solo bloque.
    pub(super) fn push_series(
        &mut self,
        blocks: Vec<ToolpathBlock>,
        mut report: LayerReport,
        z: LayerZ,
    ) {
        let passes = blocks.len() as u32;
        if z.splits(passes) {
            for (k, mut b) in (0u32..).zip(blocks) {
                b.z_mm = z.z_at(k);
                if let Some(zk) = b.z_mm {
                    b.label
                        .push_str(&format!(" pasada {}/{passes} z={zk}", k + 1));
                }
                let mut r = report.clone();
                r.passes = 1;
                r.pass = Some((k + 1, passes));
                r.z_mm = b.z_mm;
                r.count = travel_count(&b);
                self.push(b, r);
            }
            return;
        }
        let mut all = blocks.into_iter();
        let Some(mut block) = all.next() else { return };
        for b in all {
            block.moves.extend(b.moves);
        }
        block.label.push_str(&format!(" pasadas={passes}"));
        block.z_mm = z.z_at(0);
        report.z_mm = block.z_mm;
        self.push(block, report);
    }
}
