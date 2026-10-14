//! Explicaciones: repite el orden final paso a paso y comprueba qué regla lo decidió.
//! Así la razón siempre corresponde al plan que salió, aunque la mejora lo haya movido.

use tlaku_core::Point;

use crate::grid::Grid;
use crate::planner::Ctx;
use crate::sched::{DONE, READY, complete, init_marks};
use crate::shapes::push_entries;
use crate::{Blocked, Level, Reason};

/// Tolerancia (mm) para considerar empatadas dos distancias.
const TIE: f64 = 1e-9;
#[cfg(test)]
thread_local! {
    /// Pruebas: desactiva las podas de `closest_container` (referencia sin poda).
    pub(crate) static NO_PRUNE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

#[cfg(test)]
fn pruning() -> bool {
    !NO_PRUNE.with(|f| f.get())
}

#[cfg(not(test))]
fn pruning() -> bool {
    true
}

/// Máximo de celdas de la rejilla que se recorren para hallar las figuras cercanas.
const NEAR_CELLS: usize = 4096;

/// `item` está (a cualquier profundidad) dentro de `container`.
fn is_inside(ctx: &Ctx, item: usize, container: usize) -> bool {
    let mut cur = ctx.gparent[item];
    while let Some(p) = cur {
        if p == container {
            return true;
        }
        cur = ctx.gparent[p];
    }
    false
}

/// Distancia mínima desde `pos` a los puntos de entrada de `i`.
fn entry_dist(ctx: &Ctx, i: usize, pos: Point, buf: &mut Vec<(usize, usize, Point)>) -> f64 {
    buf.clear();
    push_entries(ctx.items, ctx.opts, i, buf);
    buf.iter()
        .map(|&(_, _, p)| p.dist(pos))
        .fold(f64::INFINITY, f64::min)
}

/// Figura pendiente que `c` debe preceder (a cualquier distancia en el grafo de precedencias)
/// y que estaba estrictamente más cerca que `c`. Si hay varias, la más cercana. Devuelve la
/// figura y el primer salto (`bouter` directo de `c`) por el que se llega. Es lo que explica
/// "interiores primero" aunque otra figura bloqueada no relacionada estuviera aún más cerca.
fn closest_container(
    ctx: &mut Ctx,
    c: usize,
    pos: Point,
    own_best: f64,
    grid: &mut Grid,
    buf: &mut Vec<(usize, usize, Point)>,
) -> Option<(usize, usize)> {
    // Solo cuentan las figuras con un punto de entrada más cerca que `own_best - TIE`: el
    // disco de ese radio, en la rejilla, dice cuáles (con holgura por el redondeo; la
    // distancia exacta se calcula después). Si el disco es enorme se prueban todas.
    let prune = pruning();
    let radius = own_best - TIE;
    if prune && (radius <= 0.0 || ctx.bouter[c].is_empty()) {
        return None;
    }
    ctx.seen_gen = ctx.seen_gen.wrapping_add(1);
    if ctx.seen_gen == 0 {
        ctx.seen.fill(0);
        ctx.local.fill(0);
        ctx.seen_gen = 1;
    }
    let gen_ = ctx.seen_gen;
    let mut near = Vec::new();
    let near_known = {
        let mark = &ctx.mark;
        grid.items_within(
            pos,
            radius + 1e-9,
            NEAR_CELLS,
            |i| mark[i] == DONE,
            &mut near,
        )
    };
    for &i in &near {
        ctx.local[i] = gen_;
    }
    let mut best: Option<(f64, usize, usize)> = None;
    let mut stack: Vec<(usize, usize)> = Vec::new();
    for k in (0..ctx.bouter[c].len()).rev() {
        let hop = ctx.bouter[c][k];
        stack.push((hop, hop));
    }
    while let Some((g, hop)) = stack.pop() {
        if ctx.seen[g] == gen_ {
            continue;
        }
        ctx.seen[g] = gen_;
        if ctx.mark[g] != DONE {
            // Cota inferior barata (distancia a la caja de sus entradas, con holgura para el
            // redondeo): si ya no puede ganar, se evita recorrer todos sus vértices.
            let lb = ctx.ebox[g].dist_to(pos) - 1e-9;
            let hopeless = prune
                && ((near_known && ctx.local[g] != gen_)
                    || lb >= own_best - TIE
                    || best.is_some_and(|(bd, _, _)| lb > bd));
            if !hopeless {
                let d = entry_dist(ctx, g, pos, buf);
                if d < own_best - TIE && best.is_none_or(|(bd, bg, _)| (d, g) < (bd, bg)) {
                    best = Some((d, g, hop));
                }
            }
        }
        for k in (0..ctx.bouter[g].len()).rev() {
            stack.push((ctx.bouter[g][k], hop));
        }
    }
    best.map(|(_, g, hop)| (g, hop))
}

/// Razón de precedencia de `c` respecto de `g`, alcanzada por el salto directo `hop`.
fn precedence_reason(ctx: &Ctx, c: usize, g: usize, hop: usize) -> Reason {
    if is_inside(ctx, c, g) {
        return Reason::InnerFirst { container: g };
    }
    match ctx.extras[c].iter().find(|e| e.outer == hop) {
        Some(e) => Reason::OverlapFirst {
            other: hop,
            share_pct: e.share_pct,
            with_others: e.with_others,
        },
        None => Reason::InnerFirst { container: hop },
    }
}

/// `g` está en el grafo de precedencias por encima de `c`; devuelve el primer salto.
fn reaches(ctx: &mut Ctx, c: usize, g: usize) -> Option<usize> {
    ctx.seen_gen = ctx.seen_gen.wrapping_add(1);
    if ctx.seen_gen == 0 {
        ctx.seen.fill(0);
        ctx.seen_gen = 1;
    }
    let gen_ = ctx.seen_gen;
    let mut stack: Vec<(usize, usize)> = ctx.bouter[c].iter().map(|&h| (h, h)).collect();
    while let Some((x, hop)) = stack.pop() {
        if x == g {
            return Some(hop);
        }
        if ctx.seen[x] == gen_ {
            continue;
        }
        ctx.seen[x] = gen_;
        stack.extend(ctx.bouter[x].iter().map(|&o| (o, hop)));
    }
    None
}

/// Razón de nivel (primera figura del cubo) enriquecida con la figura más cercana que se
/// saltó, según la razón de distancia u orden `base`.
fn with_blocked(level: &Reason, base: &Reason) -> Reason {
    let blocked = match *base {
        Reason::InnerFirst { container: item } => Some(Blocked {
            item,
            contains_this: true,
            by_overlap: false,
        }),
        Reason::OverlapFirst { other: item, .. } => Some(Blocked {
            item,
            contains_this: true,
            by_overlap: true,
        }),
        Reason::Nearest {
            blocked: Some(item),
            ..
        }
        | Reason::InputOrder {
            blocked: Some(item),
        } => Some(Blocked {
            item,
            contains_this: false,
            by_overlap: false,
        }),
        _ => None,
    };
    match *level {
        Reason::FirstOfLayer { layer, .. } => Reason::FirstOfLayer { layer, blocked },
        Reason::FirstOfGroup { group, .. } => Reason::FirstOfGroup { group, blocked },
        Reason::FirstOfPriority { priority, .. } => Reason::FirstOfPriority { priority, blocked },
        _ => level.clone(),
    }
}

/// Una razón por paso de `order`. `first`: razón de nivel para el primer paso del cubo.
/// `overrides[i]`: interior de `i` que el nivel obligó a cortar después.
pub(crate) fn explain(
    ctx: &mut Ctx,
    members: &[usize],
    order: &[usize],
    start: Point,
    first: Option<&Reason>,
    overrides: &[Option<(usize, Level)>],
) -> Vec<Reason> {
    init_marks(ctx, members);
    let reduce = ctx.opts.reduce_travel;
    let mut grid = None;
    if reduce {
        let mut entries = Vec::new();
        for &i in members {
            push_entries(ctx.items, ctx.opts, i, &mut entries);
        }
        grid = Some(Grid::new(&entries));
    }
    let has_edges = members.iter().any(|&i| !ctx.bchildren[i].is_empty());
    let mut ptr = 0;
    let mut cands = Vec::new();
    let mut pos = start;
    let mut reasons = Vec::with_capacity(order.len());

    for (k, &c) in order.iter().enumerate() {
        let reason = if let Some((inner, level)) = overrides[c] {
            Reason::LevelOverridesInner { inner, level }
        } else {
            // La razón de distancia/orden se calcula siempre; si es el primer paso del cubo
            // se le suma la del nivel, para no perder una figura más cercana que se saltó.
            let base = if let Some(grid) = grid.as_mut() {
                let (ready, all) = {
                    let mark = &ctx.mark;
                    let dead = |i: usize| mark[i] == DONE;
                    let ready = grid.nearest(pos, dead, |i| mark[i] == READY);
                    let all = if has_edges {
                        grid.nearest(pos, dead, |_| true)
                    } else {
                        ready
                    };
                    (ready, all)
                };
                // Se compara la figura, no el punto de entrada: el ajuste fino puede entrar por
                // otro vértice o extremo sin cambiar qué figura toca.
                let own_best = entry_dist(ctx, c, pos, &mut cands);
                match (ready, all) {
                    (Some(r), Some(a)) => {
                        let ready_d = r.dist2.sqrt();
                        if own_best <= ready_d + TIE {
                            match closest_container(ctx, c, pos, own_best, grid, &mut cands) {
                                Some((g, hop)) => precedence_reason(ctx, c, g, hop),
                                None => Reason::Nearest {
                                    distance_mm: own_best,
                                    blocked: (a.item != r.item && a.dist2.sqrt() < ready_d - TIE)
                                        .then_some(a.item),
                                },
                            }
                        } else {
                            Reason::Improved
                        }
                    }
                    _ => Reason::Improved,
                }
            } else {
                while ctx.mark[members[ptr]] == DONE {
                    ptr += 1;
                }
                let preferred = members[ptr];
                if preferred == c {
                    Reason::InputOrder { blocked: None }
                } else if let Some(hop) = reaches(ctx, c, preferred) {
                    precedence_reason(ctx, c, preferred, hop)
                } else {
                    Reason::InputOrder {
                        blocked: Some(preferred),
                    }
                }
            };
            match (k, first) {
                (0, Some(level)) => with_blocked(level, &base),
                _ => base,
            }
        };
        reasons.push(reason);
        complete(ctx, c);
        pos = ctx.sh.exit(c);
    }
    reasons
}
