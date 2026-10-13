//! Mejora local del orden de un cubo: 2-opt y Or-opt con listas de vecinos, más el ajuste
//! fino de vértices de inicio y sentidos. Todo cambio respeta las precedencias.

use tlaku_core::Point;

use crate::grid::Grid;
use crate::planner::Ctx;

const K: usize = 8;
const NONE: u32 = u32::MAX;
/// Una mejora debe ganar más que esto (mm) para aplicarse; evita ciclos por redondeo.
const GAIN_EPS: f64 = 1e-9;
/// Tope de movimientos repetidos sobre la misma posición en una pasada.
const REPEAT: usize = 8;

/// Problema local con índices 0..m (posición en `members`).
struct Problem {
    m: usize,
    /// Extremos [a, b]; en las cerradas ambos son el vértice elegido.
    ends: Vec<[Point; 2]>,
    flip: Vec<bool>,
    /// Se puede invertir sin romper reglas (cerrada, o abierta con `choose_direction`).
    flippable: Vec<bool>,
    /// Figuras que deben cortarse después de cada una (contenedor y solapes).
    parent: Vec<Vec<u32>>,
    children: Vec<Vec<u32>>,
    constrained: bool,
    order: Vec<u32>,
    pos: Vec<u32>,
    start: Point,
    nbr: Vec<u32>,
}

impl Problem {
    fn entry(&self, l: usize) -> Point {
        self.ends[l][self.flip[l] as usize]
    }

    fn exit(&self, l: usize) -> Point {
        self.ends[l][!self.flip[l] as usize]
    }

    fn prev_exit(&self, p: usize) -> Point {
        if p == 0 {
            self.start
        } else {
            self.exit(self.order[p - 1] as usize)
        }
    }

    fn next_entry(&self, p: usize) -> Option<Point> {
        (p < self.m).then(|| self.entry(self.order[p] as usize))
    }

    fn set_pos_range(&mut self, lo: usize, hi: usize) {
        for p in lo..hi {
            self.pos[self.order[p] as usize] = p as u32;
        }
    }

    fn toggle(&mut self, lo: usize, hi: usize) {
        for p in lo..hi {
            let l = self.order[p] as usize;
            self.flip[l] = !self.flip[l];
        }
    }

    // ---- 2-opt ----

    /// Ganancia de invertir las posiciones i..=j.
    fn rev_gain(&self, i: usize, j: usize) -> f64 {
        let pr = self.prev_exit(i);
        let (first, last) = (self.order[i] as usize, self.order[j] as usize);
        let (en_i, ex_j) = (self.entry(first), self.exit(last));
        let nx = self.next_entry(j + 1);
        let old = pr.dist(en_i) + nx.map_or(0.0, |n| ex_j.dist(n));
        let new = pr.dist(ex_j) + nx.map_or(0.0, |n| en_i.dist(n));
        old - new
    }

    fn rev_feasible(&self, i: usize, j: usize) -> bool {
        for p in i..=j {
            let l = self.order[p] as usize;
            if !self.flippable[l] {
                return false;
            }
            if self.constrained
                && self.parent[l]
                    .iter()
                    .any(|&par| (i..=j).contains(&(self.pos[par as usize] as usize)))
            {
                return false;
            }
        }
        true
    }

    fn apply_rev(&mut self, i: usize, j: usize) {
        self.order[i..=j].reverse();
        self.toggle(i, j + 1);
        self.set_pos_range(i, j + 1);
    }

    fn try_two_opt_at(&mut self, p: usize) -> bool {
        let a = self.order[p] as usize;
        let en = self.flip[a] as usize;
        let ex = 1 - en;
        for (kind, end) in [(0, ex), (1, en)] {
            for k in 0..K {
                let c = self.nbr[(a * 2 + end) * K + k];
                if c == NONE {
                    break;
                }
                let q = self.pos[c as usize] as usize;
                let (i, j) = match (kind, q > p) {
                    (0, true) => (p + 1, q),
                    (0, false) => (q + 1, p),
                    (_, true) => (p, q - 1),
                    (_, false) => (q, p - 1),
                };
                if i > j {
                    continue;
                }
                if self.rev_gain(i, j) > GAIN_EPS && self.rev_feasible(i, j) {
                    self.apply_rev(i, j);
                    return true;
                }
            }
        }
        false
    }

    fn two_opt_sweep(&mut self) -> bool {
        let mut any = false;
        for p in 0..self.m {
            for _ in 0..REPEAT {
                if !self.try_two_opt_at(p) {
                    break;
                }
                any = true;
            }
        }
        any
    }

    // ---- Or-opt ----

    /// Mueve el segmento s..=e al hueco `g` (entre las posiciones g-1 y g del orden actual),
    /// opcionalmente invertido.
    fn try_or_opt(&mut self, s: usize, e: usize) -> bool {
        let len = e - s + 1;
        let (first, last) = (self.order[s] as usize, self.order[e] as usize);
        let pr = self.prev_exit(s);
        let nx = self.next_entry(e + 1);
        let (seg_in, seg_out) = (self.entry(first), self.exit(last));
        let rem =
            pr.dist(seg_in) + nx.map_or(0.0, |n| seg_out.dist(n)) - nx.map_or(0.0, |n| pr.dist(n));
        if rem <= GAIN_EPS {
            return false;
        }
        let all_flippable = (s..=e).all(|p| self.flippable[self.order[p] as usize]);
        let intra = self.constrained
            && len > 1
            && (s..=e).any(|p| {
                self.parent[self.order[p] as usize]
                    .iter()
                    .any(|&par| (s..=e).contains(&(self.pos[par as usize] as usize)))
            });
        let can_rev = all_flippable && !intra;

        let ends = [
            (first, self.flip[first] as usize),
            (last, 1 - self.flip[last] as usize),
        ];
        for (l, end) in ends {
            for k in 0..K {
                let c = self.nbr[(l * 2 + end) * K + k];
                if c == NONE {
                    break;
                }
                let q = self.pos[c as usize] as usize;
                if (s..=e).contains(&q) {
                    continue;
                }
                for g in [q, q + 1] {
                    if g > s && g <= e {
                        continue;
                    }
                    let in_place = g == s || g == e + 1;
                    let (a, b) = if in_place {
                        (pr, nx)
                    } else {
                        let a = if g == 0 {
                            self.start
                        } else {
                            self.exit(self.order[g - 1] as usize)
                        };
                        (a, self.next_entry(g))
                    };
                    let gap_old = b.map_or(0.0, |b| a.dist(b));
                    for rev in [false, true] {
                        if rev && !can_rev {
                            continue;
                        }
                        if in_place && !rev {
                            continue;
                        }
                        let (to_in, to_out) = if rev {
                            (seg_out, seg_in)
                        } else {
                            (seg_in, seg_out)
                        };
                        let ins = a.dist(to_in) + b.map_or(0.0, |b| to_out.dist(b));
                        if rem - (ins - gap_old) <= GAIN_EPS {
                            continue;
                        }
                        if !self.or_feasible(s, e, g) {
                            continue;
                        }
                        self.apply_or(s, e, g, rev);
                        return true;
                    }
                }
            }
        }
        false
    }

    fn or_feasible(&self, s: usize, e: usize, g: usize) -> bool {
        if !self.constrained {
            return true;
        }
        if g > e + 1 {
            // Se adelanta a las figuras e+1..g-1: ninguna debe ser contenedor del segmento.
            (s..=e).all(|p| {
                self.parent[self.order[p] as usize].iter().all(|&par| {
                    let pp = self.pos[par as usize] as usize;
                    pp <= e || pp >= g
                })
            })
        } else if g < s {
            // Se atrasa respecto de g..s-1: ninguna debe estar dentro de una del segmento.
            (s..=e).all(|p| {
                self.children[self.order[p] as usize].iter().all(|&c| {
                    let cp = self.pos[c as usize] as usize;
                    cp >= s || cp < g
                })
            })
        } else {
            true
        }
    }

    fn apply_or(&mut self, s: usize, e: usize, g: usize, rev: bool) {
        let len = e - s + 1;
        let (lo, hi, seg_lo) = if g > e + 1 {
            self.order[s..g].rotate_left(len);
            (s, g, g - len)
        } else if g < s {
            self.order[g..=e].rotate_right(len);
            (g, e + 1, g)
        } else {
            (s, e + 1, s)
        };
        if rev {
            self.order[seg_lo..seg_lo + len].reverse();
            self.toggle(seg_lo, seg_lo + len);
        }
        self.set_pos_range(lo, hi);
    }

    fn or_opt_sweep(&mut self) -> bool {
        let mut any = false;
        for s in 0..self.m {
            for len in 1..=3 {
                let e = s + len - 1;
                if e >= self.m {
                    break;
                }
                if self.try_or_opt(s, e) {
                    any = true;
                    break;
                }
            }
        }
        any
    }
}

/// Mejora `order` (permutación de `members`) con 2-opt y Or-opt, hasta `passes` pasadas o
/// hasta que no haya mejora. Actualiza sentidos en `ctx.sh`; los vértices de inicio de las
/// cerradas no cambian aquí (eso lo hace [`polish`]).
pub(crate) fn improve(
    ctx: &mut Ctx,
    members: &[usize],
    order: &mut [usize],
    start: Point,
    passes: usize,
) {
    let m = members.len();
    for (l, &g) in members.iter().enumerate() {
        ctx.local[g] = l as u32;
    }
    let mut prob = Problem {
        m,
        ends: members
            .iter()
            .map(|&g| [ctx.sh.a[g], ctx.sh.b[g]])
            .collect(),
        flip: members.iter().map(|&g| ctx.sh.flip[g]).collect(),
        flippable: members
            .iter()
            .map(|&g| {
                ctx.items[g].path.closed || (ctx.opts.choose_direction && !ctx.items[g].locked)
            })
            .collect(),
        parent: members
            .iter()
            .map(|&g| ctx.bouter[g].iter().map(|&p| ctx.local[p]).collect())
            .collect(),
        children: members
            .iter()
            .map(|&g| ctx.bchildren[g].iter().map(|&c| ctx.local[c]).collect())
            .collect(),
        constrained: members.iter().any(|&g| !ctx.bouter[g].is_empty()),
        order: order.iter().map(|&g| ctx.local[g]).collect(),
        pos: vec![0; m],
        start,
        nbr: vec![NONE; m * 2 * K],
    };
    prob.set_pos_range(0, m);

    let mut entries = Vec::with_capacity(m * 2);
    for (l, &g) in members.iter().enumerate() {
        entries.push((l, 0, prob.ends[l][0]));
        if !ctx.items[g].path.closed {
            entries.push((l, 1, prob.ends[l][1]));
        }
    }
    let grid = Grid::new(&entries);
    let mut buf = Vec::new();
    for (l, &g) in members.iter().enumerate() {
        let closed = ctx.items[g].path.closed;
        for end in 0..2 {
            if closed && end == 1 {
                let (lo, hi) = (l * 2 * K, (l * 2 + 1) * K);
                prob.nbr.copy_within(lo..lo + K, hi);
                continue;
            }
            grid.knn(prob.ends[l][end], K, l, &mut buf);
            for (k, &(_, c)) in buf.iter().enumerate() {
                prob.nbr[(l * 2 + end) * K + k] = c as u32;
            }
        }
    }

    for _ in 0..passes {
        let a = prob.two_opt_sweep();
        let b = prob.or_opt_sweep();
        if !(a || b) {
            break;
        }
    }

    for (l, &g) in members.iter().enumerate() {
        ctx.sh.flip[g] = prob.flip[l];
    }
    for (p, &l) in prob.order.iter().enumerate() {
        order[p] = members[l as usize];
    }
}

/// Ajuste fino sin tocar el orden: cada cerrada elige el vértice que minimiza
/// entrada + salida respecto de sus vecinas, y cada abierta el sentido que mejor encaja.
pub(crate) fn polish(ctx: &mut Ctx, order: &[usize], start: Point) {
    let m = order.len();
    for _ in 0..2 {
        for p in 0..m {
            let i = order[p];
            let prev = if p == 0 {
                start
            } else {
                ctx.sh.exit(order[p - 1])
            };
            let next = (p + 1 < m).then(|| ctx.sh.entry(order[p + 1]));
            let cost =
                |pin: Point, pout: Point| prev.dist(pin) + next.map_or(0.0, |n| pout.dist(n));
            if ctx.items[i].locked {
                continue;
            }
            if ctx.items[i].path.closed {
                if !ctx.opts.choose_start_point {
                    continue;
                }
                let pts = &ctx.items[i].path.points;
                let mut best = (cost(ctx.sh.a[i], ctx.sh.a[i]), ctx.sh.vertex[i]);
                for (v, &q) in pts.iter().enumerate() {
                    let c = cost(q, q);
                    if c < best.0 - GAIN_EPS {
                        best = (c, v);
                    }
                }
                ctx.sh.apply_entry(ctx.items, i, best.1);
            } else if ctx.opts.choose_direction {
                let (a, b) = (ctx.sh.a[i], ctx.sh.b[i]);
                let (fwd, bwd) = (cost(a, b), cost(b, a));
                if ctx.sh.flip[i] && fwd < bwd - GAIN_EPS {
                    ctx.sh.flip[i] = false;
                } else if !ctx.sh.flip[i] && bwd < fwd - GAIN_EPS {
                    ctx.sh.flip[i] = true;
                }
            }
        }
    }
}
