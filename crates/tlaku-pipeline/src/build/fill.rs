//! Capas `fill`: partes por escala de potencia y `Builder::fill_layer`.

use crate::job::LayerReport;
use crate::place::PathMeta;
use crate::scan_offset;
use crate::zaxis::LayerZ;
use std::f64::consts::FRAC_PI_2;
use tlaku_core::toolpath::ToolpathBlock;
use tlaku_core::{Polyline, Rgb};
use tlaku_doc::Overscan;
use tlaku_fill::FillPattern;

use super::*;

/// Parte de una capa fill: las trayectorias de los objetos con una misma escala de potencia y la
/// potencia (%) con que se rellenan.
struct FillPart {
    paths: Vec<Polyline>,
    /// Un id de grupo por trayectoria (`tlaku_fill::fill_with_groups`).
    group_ids: Vec<u32>,
    /// Objetos de los que salen (posición en `Document::objects`), sin repetir.
    objects: Vec<usize>,
    power: f64,
}

impl FillPart {
    fn new(power: f64) -> Self {
        FillPart {
            paths: Vec::new(),
            group_ids: Vec::new(),
            objects: Vec::new(),
            power,
        }
    }
}

/// Máximo de pares de cajas que se comparan al buscar formas superpuestas de escalas distintas.
const OVERLAP_CHECK_LIMIT: usize = 4_000_000;

/// ¿Se superponen (por su caja) trayectorias de partes distintas? Con demasiadas no se comprueba.
fn fill_parts_overlap(parts: &[FillPart]) -> bool {
    type Rect = [f64; 4];
    let boxes: Vec<Vec<Rect>> = parts
        .iter()
        .map(|part| {
            part.paths
                .iter()
                .filter_map(|path| {
                    let first = path.points.first()?;
                    let mut b = [first.x, first.y, first.x, first.y];
                    for q in &path.points {
                        b = [b[0].min(q.x), b[1].min(q.y), b[2].max(q.x), b[3].max(q.y)];
                    }
                    Some(b)
                })
                .collect()
        })
        .collect();
    let mut pairs = 0_usize;
    for (i, a) in boxes.iter().enumerate() {
        for b in &boxes[i + 1..] {
            pairs = pairs.saturating_add(a.len().saturating_mul(b.len()));
        }
    }
    if pairs > OVERLAP_CHECK_LIMIT {
        return false;
    }
    boxes.iter().enumerate().any(|(i, a)| {
        boxes[i + 1..].iter().any(|b| {
            a.iter().any(|x| {
                b.iter()
                    .any(|y| x[0] < y[2] && y[0] < x[2] && x[1] < y[3] && y[1] < x[3])
            })
        })
    })
}

impl Builder<'_> {
    /// Capa fill. Con `angle_increment != 0` y varias pasadas, la pasada `k` barre con el ángulo
    /// `angle + k * angle_increment` (la geometría cambia en cada pasada, así que cada una se
    /// calcula aparte); si no, se calcula una vez y se repite, como siempre.
    ///
    /// `meta` describe cada trayectoria (`LayerWork::meta`): su grupo para el relleno por grupos y
    /// la escala de potencia de su objeto. Si los objetos tienen escalas distintas se rellenan por
    /// separado, una vez por escala, y los movimientos de todas van en un solo bloque.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn fill_layer(
        &mut self,
        color: Rgb,
        paths: &[Polyline],
        meta: &[PathMeta],
        settings: tlaku_fill::FillSettings,
        overscan: Overscan,
        passes: u32,
        z: LayerZ,
        angle_increment: f64,
    ) -> Result<(), String> {
        let name = color.to_string();
        let parts = self.fill_parts(&name, paths, meta, settings.power_pct);
        if parts.is_empty() {
            return Ok(());
        }
        let power_text = {
            let lo = parts.iter().map(|p| p.power).fold(f64::INFINITY, f64::min);
            let hi = parts.iter().map(|p| p.power).fold(0.0, f64::max);
            if lo == hi {
                format!("{hi}%")
            } else {
                format!("{lo}-{hi}%")
            }
        };
        let mut varied = angle_increment != 0.0 && passes > 1;
        if varied && settings.pattern == FillPattern::Offset {
            self.warnings.push(format!(
                "capa {name}: angle_increment no aplica al relleno por contornos; se ignora"
            ));
            varied = false;
        }
        let mut blocks = Vec::new();
        let mut overscan_mm = 0.0_f64;
        for k in 0..if varied { passes } else { 1 } {
            let mut s = settings.clone();
            if varied {
                s.angle_deg += angle_increment * f64::from(k);
            }
            if self.unidirectional {
                s.bidirectional = false;
            }
            let angle = s.angle_deg;
            let Some((mut block, mm)) = self.fill_block(&name, &parts, s, overscan, k == 0)? else {
                return Ok(());
            };
            if varied {
                block.label.push_str(&format!(" ángulo={angle}"));
            }
            overscan_mm = overscan_mm.max(mm);
            blocks.push(block);
        }
        let lines = blocks.iter().map(travel_count).sum();
        let report = LayerReport {
            name,
            color,
            mode: "fill",
            count: lines,
            unit: "líneas",
            speed_mm_min: settings.speed_mm_min,
            power: power_text,
            passes,
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
            self.push_passes(block, report, passes, z, true);
        }
        Ok(())
    }

    /// Parte las trayectorias de una capa fill por la escala de potencia de su objeto
    /// (`DesignObject::power_scale`), en orden de aparición, y da a cada parte su potencia (la de
    /// la capa por la escala, limitada a 100 %). Sin objetos con escalas distintas sale una sola
    /// parte con todas las trayectorias. Avisa de lo que se limita, se omite o se superpone.
    fn fill_parts(
        &mut self,
        name: &str,
        paths: &[Polyline],
        meta: &[PathMeta],
        layer_pct: f64,
    ) -> Vec<FillPart> {
        let mut parts: Vec<(f64, FillPart)> = Vec::new();
        for (i, path) in paths.iter().enumerate() {
            let m = meta.get(i).copied().unwrap_or(PathMeta::NONE);
            let at = match parts
                .iter()
                .position(|(s, _)| s.to_bits() == m.power_scale.to_bits())
            {
                Some(at) => at,
                None => {
                    parts.push((
                        m.power_scale,
                        FillPart::new(layer_pct * m.power_scale / 100.0),
                    ));
                    parts.len() - 1
                }
            };
            let part = &mut parts[at].1;
            part.paths.push(path.clone());
            part.group_ids.push(m.fill_group);
            if !part.objects.contains(&m.object) {
                part.objects.push(m.object);
            }
        }
        let mut clamped = 0;
        let mut silent = 0;
        let mut out = Vec::new();
        for (_, mut part) in parts {
            if part.power > 100.0 {
                clamped += part.objects.len();
                part.power = 100.0;
            }
            if part.power <= 0.0 {
                silent += part.objects.len();
                continue;
            }
            out.push(part);
        }
        if clamped > 0 {
            self.warnings.push(format!(
                "capa {name}: la escala de potencia de {clamped} objeto(s) pasa de 100 %: se limita a 100 %"
            ));
        }
        if silent > 0 {
            self.warnings.push(format!(
                "capa {name}: {silent} objeto(s) con potencia 0 (escala de potencia 0 %) no se rellenan"
            ));
        }
        if out.len() > 1 {
            self.warnings.push(format!(
                "capa {name}: los objetos tienen {} escalas de potencia distintas; se rellenan por separado (una vez por escala)",
                out.len()
            ));
            if fill_parts_overlap(&out) {
                self.warnings.push(format!(
                    "capa {name}: hay objetos con escalas de potencia distintas que se superponen; \
                     cada escala se rellena por su cuenta, así que lo común se graba dos veces \
                     (un hueco de otra escala no se respeta)"
                ));
            }
        }
        out
    }

    /// Los movimientos de una pasada de relleno con `settings` tal cual (su ángulo incluido) y el
    /// overscan usado; `None` si no produjo ninguna línea. `first` es la primera pasada de la
    /// capa: los avisos que no dependen del ángulo se dan solo una vez. Cada parte se rellena con
    /// su propia potencia y todas se juntan en un bloque.
    fn fill_block(
        &mut self,
        name: &str,
        parts: &[FillPart],
        mut settings: tlaku_fill::FillSettings,
        overscan: Overscan,
        first: bool,
    ) -> Result<Option<(ToolpathBlock, f64)>, String> {
        if overscan == Overscan::Auto {
            let mut mm = tlaku_sim::overscan_mm(
                settings.speed_mm_min,
                settings.angle_deg.to_radians(),
                self.profile,
            );
            if settings.crosshatch {
                let perp = settings.angle_deg.to_radians() + FRAC_PI_2;
                mm = mm.max(tlaku_sim::overscan_mm(
                    settings.speed_mm_min,
                    perp,
                    self.profile,
                ));
            }
            settings.overscan_mm = mm;
        }
        let mut merged: Option<ToolpathBlock> = None;
        let mut ignored = 0;
        for part in parts {
            let mut s = settings.clone();
            s.power_pct = part.power;
            let (block, open) = self.fill_part_block(name, part, &s)?;
            ignored += open;
            if let Some(block) = block {
                match &mut merged {
                    Some(m) => m.moves.extend(block.moves),
                    None => merged = Some(block),
                }
            }
        }
        if first && ignored > 0 {
            self.warnings.push(format!(
                "capa {name}: {ignored} trayectoria(s) abierta(s) ignorada(s): el relleno solo usa trayectorias cerradas"
            ));
        }
        let Some(block) = merged else {
            if first {
                self.warnings
                    .push(format!("capa {name}: el relleno no produjo ninguna línea"));
            }
            return Ok(None);
        };
        if first
            && matches!(settings.pattern, FillPattern::Scan | FillPattern::Flood)
            && settings.bidirectional
            && let Some(w) = scan_offset::overscan_warning(
                &format!("capa {name}"),
                self.scan_offsets,
                settings.speed_mm_min,
                settings.overscan_mm,
            )
        {
            self.warnings.push(w);
        }
        Ok(Some((block, settings.overscan_mm)))
    }

    /// El relleno de una parte (una escala de potencia) con las compensaciones de barrido, y
    /// cuántas trayectorias abiertas ignoró.
    fn fill_part_block(
        &mut self,
        name: &str,
        part: &FillPart,
        settings: &tlaku_fill::FillSettings,
    ) -> Result<(Option<ToolpathBlock>, usize), String> {
        // Los ids de grupo solo cuentan con `FillGrouping::ByGroup`; con otra agrupación el motor
        // los ignora.
        let result = tlaku_fill::fill_with_groups(&part.paths, Some(&part.group_ids), settings)
            .map_err(|e| format!("capa {name}: {e}"))?;
        let ignored = result.ignored_open.len();
        let mut block = result.block;
        if block.moves.is_empty() {
            return Ok((None, ignored));
        }
        // Solo el barrido (y el relleno por regiones, que son las mismas líneas) tiene líneas de
        // ida y vuelta; el relleno por contornos no.
        if matches!(settings.pattern, FillPattern::Scan | FillPattern::Flood) {
            let mut angles = vec![settings.angle_deg];
            if settings.crosshatch {
                angles.push(settings.angle_deg + 90.0);
            }
            scan_offset::apply(
                &mut block,
                self.scan_offsets,
                self.scan_initial,
                &angles,
                settings.bidirectional,
            );
        }
        Ok((Some(block), ignored))
    }
}
