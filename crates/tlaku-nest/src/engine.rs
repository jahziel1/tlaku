//! Motor de colocación: variantes de cada pieza (rotación y espejo), caché de NFP e IFP y la
//! colocación voraz sobre la región libre.
//!
//! Para colocar una pieza B se calcula la región libre de su punto de referencia (la esquina
//! inferior izquierda de su caja): el IFP del contenedor menos el NFP de B con cada pieza ya
//! colocada. Los candidatos son los vértices de esa región; gana el de mejor puntaje
//! ("gravedad" o "abajo e izquierda"). La región libre de cada (zona, variante) se conserva y
//! solo se le restan las piezas colocadas desde la última vez, que es lo que hace manejable
//! tener cientos de piezas. Con `use_holes`, cada hueco de una pieza colocada se vuelve una
//! zona nueva donde caben otras piezas; las piezas de una zona solo chocan con las de esa zona.

use crate::geom::{Pt, Ring, bounds, make_ccw, signed_area};
use crate::ops::{difference, ifp_general, negate, nfp};
use crate::prep::{Container, ContainerData, PieceType, TOL_MM};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gravity {
    /// Minimiza la caja del conjunto con más peso en el ancho (compacta hacia una esquina).
    Gravity,
    /// La posición más abajo y, a igualdad, la más a la izquierda.
    BottomLeft,
}

/// Interrupción cooperativa: la orden de parar o el tope de tiempo.
pub struct Ctl<'a> {
    pub stop: &'a AtomicBool,
    pub deadline: Instant,
}

impl Ctl<'_> {
    pub fn should_stop(&self) -> bool {
        self.stop.load(Ordering::Relaxed) || Instant::now() >= self.deadline
    }
}

/// Una pieza con una rotación y un espejo concretos, en un marco con la esquina inferior
/// izquierda de su caja en el origen.
pub(crate) struct Variant {
    pub parts: Vec<Ring>,
    pub neg_parts: Vec<Ring>,
    pub holes: Vec<Ring>,
    pub hole_dims: Vec<(f64, f64)>,
    pub w: f64,
    pub h: f64,
    /// Parte lineal `[a b c d]` del giro (y espejo) respecto a `c0`.
    pub lin: [f64; 4],
    /// Esquina inferior izquierda que se restó al transformar.
    pub lo: Pt,
    pub angle: f64,
    pub mirror: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct Gene {
    pub piece: usize,
    pub copy: u32,
    /// Índice en `angles`; `None` = probar varias rotaciones y quedarse con la mejor.
    pub rot: Option<usize>,
    pub mirror: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct Placed {
    pub piece: usize,
    pub copy: u32,
    pub variant: usize,
    pub pos: Pt,
}

#[derive(Debug, Clone)]
pub(crate) struct Solution {
    pub placed: Vec<Placed>,
    pub unplaced: Vec<(usize, u32)>,
    pub placed_area: f64,
    pub cost: f64,
    /// `false` si la corrida se interrumpió antes de intentar todas las piezas.
    pub complete: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum RegionKind {
    Main,
    Hole { owner: usize, hole: usize },
}

#[derive(Debug, Hash, PartialEq, Eq, Clone, Copy)]
enum IfpOwner {
    Main,
    Hole(usize, usize),
}

struct FreeEntry {
    rings: Vec<Ring>,
    applied: usize,
}

struct Region {
    kind: RegionKind,
    placed: Vec<usize>,
    free: HashMap<usize, FreeEntry>,
}

struct Run {
    placed: Vec<Placed>,
    regions: Vec<Region>,
    extent: Pt,
    placed_area: f64,
}

pub(crate) struct Engine {
    pub gravity: Gravity,
    pub use_holes: bool,
    pub allow_mirror: bool,
    pub pieces: Vec<PieceType>,
    pub container: Container,
    pub angles: Vec<f64>,
    try_angles: Vec<usize>,
    min_hole_area: f64,
    variants: Vec<Variant>,
    vkey: HashMap<(usize, i64, bool), usize>,
    nfp_cache: HashMap<(usize, usize), Rc<Vec<Ring>>>,
    ifp_cache: HashMap<(IfpOwner, usize), Rc<Vec<Ring>>>,
}

fn shift(rings: &[Ring], by: Pt) -> Vec<Ring> {
    rings
        .iter()
        .map(|r| r.iter().map(|p| [p[0] + by[0], p[1] + by[1]]).collect())
        .collect()
}

impl Engine {
    pub fn new(
        pieces: Vec<PieceType>,
        container: Container,
        angles: Vec<f64>,
        gravity: Gravity,
        use_holes: bool,
        allow_mirror: bool,
    ) -> Self {
        let try_angles = if angles.len() <= 8 {
            (0..angles.len()).collect()
        } else {
            (0..8).map(|i| i * angles.len() / 8).collect()
        };
        let min_hole_area = pieces
            .iter()
            .map(|p| p.inflated_area)
            .fold(f64::INFINITY, f64::min);
        Self {
            gravity,
            use_holes,
            allow_mirror,
            pieces,
            container,
            angles,
            try_angles,
            min_hole_area,
            variants: Vec::new(),
            vkey: HashMap::new(),
            nfp_cache: HashMap::new(),
            ifp_cache: HashMap::new(),
        }
    }

    pub fn variant(&self, id: usize) -> &Variant {
        &self.variants[id]
    }

    fn variant_id(&mut self, piece: usize, angle: f64, mirror: bool) -> usize {
        let key = (piece, (angle * 100.0).round() as i64, mirror);
        if let Some(&v) = self.vkey.get(&key) {
            return v;
        }
        let (s, c) = angle.to_radians().sin_cos();
        let mx = if mirror { -1.0 } else { 1.0 };
        let lin = [c * mx, s * mx, -s, c];
        let tf = |p: &Pt| -> Pt { [lin[0] * p[0] + lin[2] * p[1], lin[1] * p[0] + lin[3] * p[1]] };
        let place = |r: &Ring| -> Ring {
            let t: Ring = r.iter().map(tf).collect();
            if mirror { make_ccw(t) } else { t }
        };
        let pt = &self.pieces[piece];
        let parts: Vec<Ring> = pt.parts.iter().map(place).collect();
        let holes: Vec<Ring> = pt.holes.iter().map(place).collect();
        let (lo, hi) = bounds(&parts).unwrap_or(([0.0; 2], [0.0; 2]));
        let norm = |rs: Vec<Ring>| -> Vec<Ring> {
            rs.into_iter()
                .map(|r| {
                    r.into_iter()
                        .map(|p| [p[0] - lo[0], p[1] - lo[1]])
                        .collect()
                })
                .collect()
        };
        let parts = norm(parts);
        let holes = norm(holes);
        let hole_dims = holes
            .iter()
            .map(|r| {
                let (a, b) = bounds(std::iter::once(r)).unwrap_or(([0.0; 2], [0.0; 2]));
                (b[0] - a[0], b[1] - a[1])
            })
            .collect();
        let id = self.variants.len();
        self.variants.push(Variant {
            neg_parts: parts.iter().map(|r| negate(r)).collect(),
            parts,
            holes,
            hole_dims,
            w: hi[0] - lo[0],
            h: hi[1] - lo[1],
            lin,
            lo,
            angle,
            mirror,
        });
        self.vkey.insert(key, id);
        id
    }

    /// Transformación afín del documento para la pieza `piece` en la variante `vid` con la
    /// esquina inferior izquierda de su caja en `pos`.
    pub fn transform(&self, piece: usize, vid: usize, pos: Pt) -> [f64; 6] {
        let v = &self.variants[vid];
        let c0 = self.pieces[piece].c0;
        let [a, b, c, d] = v.lin;
        [
            a,
            b,
            c,
            d,
            -(a * c0[0] + c * c0[1]) - v.lo[0] + pos[0],
            -(b * c0[0] + d * c0[1]) - v.lo[1] + pos[1],
        ]
    }

    fn nfp(&mut self, a: usize, b: usize) -> Rc<Vec<Ring>> {
        if let Some(r) = self.nfp_cache.get(&(a, b)) {
            return r.clone();
        }
        let r = Rc::new(nfp(&self.variants[a].parts, &self.variants[b].neg_parts));
        self.nfp_cache.insert((a, b), r.clone());
        r
    }

    fn ifp(&mut self, owner: IfpOwner, vid: usize) -> Rc<Vec<Ring>> {
        if let Some(r) = self.ifp_cache.get(&(owner, vid)) {
            return r.clone();
        }
        let v = &self.variants[vid];
        let rings = match owner {
            IfpOwner::Main => match &self.container.data {
                ContainerData::Region(free) => ifp_general(free, &v.neg_parts, v.w, v.h),
                ContainerData::Rect { .. } => Vec::new(),
            },
            IfpOwner::Hole(va, hole) => ifp_general(
                &[self.variants[va].holes[hole].clone()],
                &v.neg_parts,
                v.w,
                v.h,
            ),
        };
        let r = Rc::new(rings);
        self.ifp_cache.insert((owner, vid), r.clone());
        r
    }

    /// Región libre inicial (IFP) del punto de referencia de `vid` en la zona `rid`.
    fn initial_free(&mut self, run: &Run, rid: usize, vid: usize) -> Vec<Ring> {
        let (w, h) = (self.variants[vid].w, self.variants[vid].h);
        match run.regions[rid].kind {
            RegionKind::Main => match &self.container.data {
                ContainerData::Rect { lo, hi } => {
                    let (x1, y1) = (hi[0] - w, hi[1] - h);
                    if x1 <= lo[0] || y1 <= lo[1] {
                        Vec::new()
                    } else {
                        vec![vec![*lo, [x1, lo[1]], [x1, y1], [lo[0], y1]]]
                    }
                }
                ContainerData::Region(_) => self.ifp(IfpOwner::Main, vid).as_ref().clone(),
            },
            RegionKind::Hole { owner, hole } => {
                let po = &run.placed[owner];
                let (hw, hh) = self.variants[po.variant].hole_dims[hole];
                if w > hw + TOL_MM || h > hh + TOL_MM {
                    return Vec::new();
                }
                let rings = self.ifp(IfpOwner::Hole(po.variant, hole), vid);
                shift(&rings, po.pos)
            }
        }
    }

    /// Pone al día la región libre de (`rid`, `vid`) restándole las piezas nuevas de la zona.
    fn ensure_free(&mut self, run: &mut Run, rid: usize, vid: usize) {
        let mut entry = match run.regions[rid].free.remove(&vid) {
            Some(e) => e,
            None => FreeEntry {
                rings: self.initial_free(run, rid, vid),
                applied: 0,
            },
        };
        let total = run.regions[rid].placed.len();
        if entry.applied < total {
            if !entry.rings.is_empty() {
                let fb = bounds(&entry.rings);
                let mut clip: Vec<Ring> = Vec::new();
                for k in entry.applied..total {
                    let pi = run.regions[rid].placed[k];
                    let (pv, ppos) = (run.placed[pi].variant, run.placed[pi].pos);
                    let n = self.nfp(pv, vid);
                    let moved = shift(&n, ppos);
                    if let (Some((flo, fhi)), Some((nlo, nhi))) = (fb, bounds(&moved))
                        && (nhi[0] < flo[0]
                            || nlo[0] > fhi[0]
                            || nhi[1] < flo[1]
                            || nlo[1] > fhi[1])
                    {
                        continue;
                    }
                    clip.extend(moved);
                }
                if !clip.is_empty() {
                    entry.rings = difference(&entry.rings, &clip);
                }
            }
            entry.applied = total;
        }
        run.regions[rid].free.insert(vid, entry);
    }

    fn score(&self, run: &Run, p: Pt, w: f64, h: f64, in_hole: bool) -> f64 {
        let o = self.container.origin;
        let base = match self.gravity {
            Gravity::Gravity => {
                let ww = run.extent[0].max(p[0] + w) - o[0];
                let hh = run.extent[1].max(p[1] + h) - o[1];
                2.0 * ww + hh + 1e-3 * ((p[0] - o[0]) + (p[1] - o[1]))
            }
            Gravity::BottomLeft => (p[1] - o[1]) + 1e-3 * (p[0] - o[0]),
        };
        // Una pieza que cabe en un hueco no gasta material del contenedor: siempre se prefiere.
        if in_hole { base - 1e9 } else { base }
    }

    fn metric(&self, extent: Pt) -> f64 {
        let (w, h) = (
            extent[0] - self.container.origin[0],
            extent[1] - self.container.origin[1],
        );
        match self.gravity {
            Gravity::Gravity => 2.0 * w + h,
            Gravity::BottomLeft => h + 0.1 * w,
        }
    }

    fn gene_variants(&mut self, g: &Gene) -> Vec<usize> {
        let mut out = Vec::new();
        match g.rot {
            Some(k) => {
                let a = self.angles[k];
                out.push(self.variant_id(g.piece, a, g.mirror && self.allow_mirror));
            }
            None => {
                let mirrors: &[bool] = if self.allow_mirror {
                    &[false, true]
                } else {
                    &[false]
                };
                for k in self.try_angles.clone() {
                    for &m in mirrors {
                        let a = self.angles[k];
                        out.push(self.variant_id(g.piece, a, m));
                    }
                }
            }
        }
        out
    }

    /// Intenta colocar una pieza; `true` si se colocó.
    fn place_gene(&mut self, run: &mut Run, g: &Gene) -> bool {
        let mut best: Option<(f64, usize, Pt)> = None; // (puntaje, variante, posición)
        let mut best_region = 0usize;
        for vid in self.gene_variants(g) {
            let (w, h) = (self.variants[vid].w, self.variants[vid].h);
            for rid in 0..run.regions.len() {
                self.ensure_free(run, rid, vid);
                let in_hole = matches!(run.regions[rid].kind, RegionKind::Hole { .. });
                let clamp = match (&self.container.data, run.regions[rid].kind) {
                    (ContainerData::Rect { lo, hi }, RegionKind::Main) => Some((
                        [lo[0] + TOL_MM, lo[1] + TOL_MM],
                        [hi[0] - TOL_MM - w, hi[1] - TOL_MM - h],
                    )),
                    _ => None,
                };
                let entry = &run.regions[rid].free[&vid];
                for ring in &entry.rings {
                    for &q in ring {
                        let mut p = q;
                        if let Some((cl, ch)) = clamp
                            && ch[0] >= cl[0]
                            && ch[1] >= cl[1]
                        {
                            p = [p[0].clamp(cl[0], ch[0]), p[1].clamp(cl[1], ch[1])];
                        }
                        let s = self.score(run, p, w, h, in_hole);
                        if best.is_none_or(|b| s < b.0 - 1e-9) {
                            best = Some((s, vid, p));
                            best_region = rid;
                        }
                    }
                }
            }
        }
        let Some((_, vid, pos)) = best else {
            return false;
        };
        let idx = run.placed.len();
        run.placed.push(Placed {
            piece: g.piece,
            copy: g.copy,
            variant: vid,
            pos,
        });
        run.regions[best_region].placed.push(idx);
        let (w, h) = (self.variants[vid].w, self.variants[vid].h);
        run.extent = [run.extent[0].max(pos[0] + w), run.extent[1].max(pos[1] + h)];
        run.placed_area += self.pieces[g.piece].true_area;
        if self.use_holes {
            for hole in 0..self.variants[vid].holes.len() {
                if signed_area(&self.variants[vid].holes[hole]) >= self.min_hole_area * (1.0 - 1e-6)
                {
                    run.regions.push(Region {
                        kind: RegionKind::Hole { owner: idx, hole },
                        placed: Vec::new(),
                        free: HashMap::new(),
                    });
                }
            }
        }
        true
    }

    /// Colocación voraz en el orden dado. Si `ctl` pide parar, devuelve lo colocado hasta
    /// ahora con `complete = false`.
    pub fn greedy(&mut self, order: &[Gene], ctl: &Ctl) -> Solution {
        let mut run = Run {
            placed: Vec::with_capacity(order.len()),
            regions: vec![Region {
                kind: RegionKind::Main,
                placed: Vec::new(),
                free: HashMap::new(),
            }],
            extent: self.container.origin,
            placed_area: 0.0,
        };
        let mut unplaced = Vec::new();
        let mut complete = true;
        for (i, g) in order.iter().enumerate() {
            if ctl.should_stop() {
                complete = false;
                unplaced.extend(order[i..].iter().map(|g| (g.piece, g.copy)));
                break;
            }
            if !self.place_gene(&mut run, g) {
                unplaced.push((g.piece, g.copy));
            }
        }
        let unplaced_area: f64 = unplaced
            .iter()
            .map(|&(p, _)| self.pieces[p].true_area)
            .sum();
        let cost = unplaced_area * 1e3 + self.metric(run.extent);
        Solution {
            placed: run.placed,
            unplaced,
            placed_area: run.placed_area,
            cost,
            complete,
        }
    }
}
