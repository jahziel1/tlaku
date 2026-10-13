//! Explicaciones: repite el orden final paso a paso y comprueba qué regla lo decidió.
//! Así la razón siempre corresponde al plan que salió, aunque la mejora lo haya movido.

use laser_core::Point;

use crate::grid::Grid;
use crate::planner::Ctx;
use crate::sched::{DONE, READY, complete, init_marks};
use crate::shapes::push_entries;
use crate::{Blocked, Level, Reason};

/// Tolerancia (mm) para considerar empatadas dos distancias.
const TIE: f64 = 1e-9;

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

/// Contenedor pendiente de `c` (en su cubo) que estaba estrictamente más cerca que `c`.
/// Si hay varios, el más cercano. Es lo que explica "interiores primero" aunque otra figura
/// bloqueada no relacionada estuviera aún más cerca.
fn closest_container(
    ctx: &Ctx,
    c: usize,
    pos: Point,
    own_best: f64,
    buf: &mut Vec<(usize, usize, Point)>,
) -> Option<usize> {
    let mut best: Option<(f64, usize)> = None;
    let mut cur = ctx.bparent[c];
    while let Some(g) = cur {
        if ctx.mark[g] != DONE {
            let d = entry_dist(ctx, g, pos, buf);
            if d < own_best - TIE && best.is_none_or(|(bd, _)| d < bd) {
                best = Some((d, g));
            }
        }
        cur = ctx.bparent[g];
    }
    best.map(|(_, g)| g)
}

/// Razón de nivel (primera figura del cubo) enriquecida con la figura más cercana que se
/// saltó, según la razón de distancia u orden `base`.
fn with_blocked(level: &Reason, base: &Reason) -> Reason {
    let blocked = match *base {
        Reason::InnerFirst { container } => Some(Blocked {
            item: container,
            contains_this: true,
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
                    let ready = grid.nearest(pos, &dead, &|i| mark[i] == READY);
                    let all = if has_edges {
                        grid.nearest(pos, &dead, &|_| true)
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
                            match closest_container(ctx, c, pos, own_best, &mut cands) {
                                Some(container) => Reason::InnerFirst { container },
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
                } else if is_inside(ctx, c, preferred) {
                    Reason::InnerFirst {
                        container: preferred,
                    }
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
