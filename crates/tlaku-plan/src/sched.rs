//! Construcción del orden dentro de un cubo: vecino más cercano u orden original.

use std::cmp::Reverse;
use std::collections::BinaryHeap;

use tlaku_core::Point;

use crate::grid::Grid;
use crate::planner::Ctx;
use crate::shapes::push_entries;

pub(crate) const READY: u8 = 1;
pub(crate) const BLOCKED: u8 = 2;
pub(crate) const DONE: u8 = 3;

/// Marca los miembros: listos si no tienen interiores pendientes, bloqueados si sí.
pub(crate) fn init_marks(ctx: &mut Ctx, members: &[usize]) {
    for &i in members {
        let pending = ctx.bchildren[i].len() as u32;
        ctx.pending[i] = pending;
        ctx.mark[i] = if pending == 0 { READY } else { BLOCKED };
    }
}

/// Da `i` por cortada; devuelve las figuras que con esto quedan listas.
pub(crate) fn complete(ctx: &mut Ctx, i: usize) -> Vec<usize> {
    ctx.mark[i] = DONE;
    let mut freed = Vec::new();
    for k in 0..ctx.bouter[i].len() {
        let outer = ctx.bouter[i][k];
        ctx.pending[outer] -= 1;
        if ctx.pending[outer] == 0 {
            ctx.mark[outer] = READY;
            freed.push(outer);
        }
    }
    freed
}

/// Vecino más cercano entre los listos, con vértice de inicio y sentido elegidos.
pub(crate) fn nearest_neighbor(ctx: &mut Ctx, members: &[usize], start: Point) -> Vec<usize> {
    init_marks(ctx, members);
    let mut entries = Vec::new();
    for &i in members {
        push_entries(ctx.items, ctx.opts, i, &mut entries);
    }
    let mut grid = Grid::new(&entries);
    let mut order = Vec::with_capacity(members.len());
    let mut pos = start;
    for _ in 0..members.len() {
        let hit = {
            let mark = &ctx.mark;
            grid.nearest(pos, |i| mark[i] == DONE, |i| mark[i] == READY)
        };
        // Sin listos solo ocurriría con un ciclo de precedencias, que no se construye.
        let (item, pt) = match hit {
            Some(h) => (h.item, h.pt),
            None => (*members.iter().find(|&&i| ctx.mark[i] != DONE).unwrap(), 0),
        };
        ctx.sh.apply_entry(ctx.items, item, pt);
        complete(ctx, item);
        pos = ctx.sh.exit(item);
        order.push(item);
    }
    order
}

/// Orden original respetando precedencias: siempre el de menor índice entre los listos.
pub(crate) fn input_order(ctx: &mut Ctx, members: &[usize], start: Point) -> Vec<usize> {
    init_marks(ctx, members);
    let mut ready: BinaryHeap<Reverse<usize>> = members
        .iter()
        .copied()
        .filter(|&i| ctx.mark[i] == READY)
        .map(Reverse)
        .collect();
    let mut order = Vec::with_capacity(members.len());
    let mut pos = start;
    let mut entries = Vec::new();
    while let Some(Reverse(i)) = ready.pop() {
        entries.clear();
        push_entries(ctx.items, ctx.opts, i, &mut entries);
        let (_, pt, _) = entries
            .iter()
            .copied()
            .min_by(|a, b| {
                let (da, db) = (a.2.dist(pos), b.2.dist(pos));
                da.total_cmp(&db).then(a.1.cmp(&b.1))
            })
            .unwrap();
        ctx.sh.apply_entry(ctx.items, i, pt);
        for p in complete(ctx, i) {
            ready.push(Reverse(p));
        }
        pos = ctx.sh.exit(i);
        order.push(i);
    }
    order
}
