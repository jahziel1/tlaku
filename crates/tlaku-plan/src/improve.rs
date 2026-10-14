//! Mejora local del orden de un cubo: 2-opt y Or-opt con listas de vecinos, más el ajuste
//! fino de vértices de inicio y sentidos. Todo cambio respeta las precedencias.

use tlaku_core::Point;

use crate::grid::Grid;
use crate::planner::Ctx;
use crate::seq::{ArraySeq, BlockSeq, Seq};

const K: usize = 8;
const NONE: u32 = u32::MAX;
/// Una mejora debe ganar más que esto (mm) para aplicarse; evita ciclos por redondeo.
const GAIN_EPS: f64 = 1e-9;
/// Tope de movimientos repetidos sobre la misma posición en una pasada.
const REPEAT: usize = 8;

/// Problema local con índices 0..m (posición en `members`).
struct Problem<S: Seq> {
    m: usize,
    /// Extremos [a, b]; en las cerradas ambos son el vértice elegido.
    ends: Vec<[Point; 2]>,
    /// Orden, posiciones y sentido de cada figura.
    seq: S,
    /// Se puede invertir sin romper reglas (cerrada, o abierta con `choose_direction`).
    flippable: Vec<bool>,
    /// Figuras que deben cortarse después de cada una (contenedor y solapes).
    parent: Vec<Vec<u32>>,
    children: Vec<Vec<u32>>,
    constrained: bool,
    /// Todas se pueden invertir.
    all_flippable: bool,
    start: Point,
    nbr: Vec<u32>,
}

impl<S: Seq> Problem<S> {
    fn entry(&self, l: usize) -> Point {
        self.ends[l][self.seq.flip(l) as usize]
    }

    fn exit(&self, l: usize) -> Point {
        self.ends[l][!self.seq.flip(l) as usize]
    }

    /// Punto de salida de `prev` (o el inicio si no hay anterior).
    fn exit_or_start(&self, prev: Option<usize>) -> Point {
        prev.map_or(self.start, |l| self.exit(l))
    }

    /// Ganancia de invertir un tramo con extremos `en_i` (entrada) y `ex_j` (salida), entre
    /// `pr` (salida de la anterior) y `nx` (entrada de la siguiente). `t1` y `t2` son las
    /// distancias de los dos lados que se quitan si ya se conocen (`pr`-`en_i` y
    /// `ex_j`-`nx`): con los mismos operandos dan el mismo valor que calcularlas aquí.
    fn rev_gain(
        pr: Point,
        en_i: Point,
        ex_j: Point,
        nx: Option<Point>,
        t1: Option<f64>,
        t2: Option<f64>,
    ) -> f64 {
        let t1 = t1.unwrap_or_else(|| pr.dist(en_i));
        let t2 = t2.unwrap_or_else(|| nx.map_or(0.0, |n| ex_j.dist(n)));
        let old = t1 + t2;
        let new = pr.dist(ex_j) + nx.map_or(0.0, |n| en_i.dist(n));
        old - new
    }

    fn rev_feasible(&self, i: usize, j: usize) -> bool {
        if self.all_flippable && !self.constrained {
            return true;
        }
        for p in i..=j {
            let l = self.seq.at(p);
            if !self.flippable[l] {
                return false;
            }
            if self.constrained
                && self.parent[l]
                    .iter()
                    .any(|&par| (i..=j).contains(&(self.seq.pos(par as usize))))
            {
                return false;
            }
        }
        true
    }

    fn apply_rev(&mut self, i: usize, j: usize) {
        self.seq.reverse(i, j, true);
    }

    fn try_two_opt_at(&mut self, p: usize) -> bool {
        let a = self.seq.at(p);
        let (pa, na) = (self.seq.prev(a), self.seq.next(a));
        let en = self.seq.flip(a) as usize;
        let ex = 1 - en;
        // Los lados que unen `a` con sus vecinas se repiten en todos los candidatos.
        let dn = na.map_or(0.0, |n| self.exit(a).dist(self.entry(n)));
        let dp = self.exit_or_start(pa).dist(self.entry(a));
        for (kind, end) in [(0, ex), (1, en)] {
            for k in 0..K {
                let c = self.nbr[(a * 2 + end) * K + k];
                if c == NONE {
                    break;
                }
                let c = c as usize;
                let q = self.seq.pos(c);
                let (i, j) = match (kind, q > p) {
                    (0, true) => (p + 1, q),
                    (0, false) => (q + 1, p),
                    (_, true) => (p, q - 1),
                    (_, false) => (q, p - 1),
                };
                if i > j {
                    continue;
                }
                // Anterior, primera, última y siguiente del tramo i..=j, sin buscar por posición.
                let gain = match (kind, q > p) {
                    (0, true) => {
                        let nx = self.seq.next(c).map(|l| self.entry(l));
                        let en_i = self.entry(na.unwrap_or(c));
                        Self::rev_gain(self.exit(a), en_i, self.exit(c), nx, Some(dn), None)
                    }
                    (0, false) => {
                        let nc = self.seq.next(c).unwrap_or(a);
                        let nx = na.map(|l| self.entry(l));
                        Self::rev_gain(
                            self.exit(c),
                            self.entry(nc),
                            self.exit(a),
                            nx,
                            None,
                            Some(dn),
                        )
                    }
                    (_, true) => {
                        let pc = self.seq.prev(c).unwrap_or(a);
                        let nx = Some(self.entry(c));
                        let pr = self.exit_or_start(pa);
                        Self::rev_gain(pr, self.entry(a), self.exit(pc), nx, Some(dp), None)
                    }
                    (_, false) => {
                        let pc = self.seq.prev(c);
                        let ex_j = self.exit(pa.unwrap_or(c));
                        let nx = Some(self.entry(a));
                        let pr = self.exit_or_start(pc);
                        Self::rev_gain(pr, self.entry(c), ex_j, nx, None, Some(dp))
                    }
                };
                if gain > GAIN_EPS && self.rev_feasible(i, j) {
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
    fn try_or_opt(&mut self, s: usize, e: usize, first: usize, last: usize) -> bool {
        let len = e - s + 1;
        let pr = self.exit_or_start(self.seq.prev(first));
        let nx = self.seq.next(last).map(|l| self.entry(l));
        let (seg_in, seg_out) = (self.entry(first), self.exit(last));
        let rem =
            pr.dist(seg_in) + nx.map_or(0.0, |n| seg_out.dist(n)) - nx.map_or(0.0, |n| pr.dist(n));
        if rem <= GAIN_EPS {
            return false;
        }
        let all_flippable = self.all_flippable || {
            // Las figuras del segmento: `first`, las siguientes y `last`.
            let mut ok = self.flippable[first];
            let mut cur = first;
            while ok && cur != last {
                cur = self.seq.next(cur).unwrap_or(last);
                ok = self.flippable[cur];
            }
            ok
        };
        let intra = self.constrained
            && len > 1
            && (s..=e).any(|p| {
                self.parent[self.seq.at(p)]
                    .iter()
                    .any(|&par| (s..=e).contains(&(self.seq.pos(par as usize))))
            });
        let can_rev = all_flippable && !intra;

        let ends = [
            (first, self.seq.flip(first) as usize),
            (last, 1 - self.seq.flip(last) as usize),
        ];
        for (l, end) in ends {
            for k in 0..K {
                let c = self.nbr[(l * 2 + end) * K + k];
                if c == NONE {
                    break;
                }
                let c = c as usize;
                let q = self.seq.pos(c);
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
                        // `g - 1` y `g` son vecinos de `c` (en la posición `q`).
                        if g == q {
                            let a = self.exit_or_start(self.seq.prev(c));
                            (a, Some(self.entry(c)))
                        } else {
                            let a = self.exit(c);
                            (a, self.seq.next(c).map(|n| self.entry(n)))
                        }
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
                self.parent[self.seq.at(p)].iter().all(|&par| {
                    let pp = self.seq.pos(par as usize);
                    pp <= e || pp >= g
                })
            })
        } else if g < s {
            // Se atrasa respecto de g..s-1: ninguna debe estar dentro de una del segmento.
            (s..=e).all(|p| {
                self.children[self.seq.at(p)].iter().all(|&c| {
                    let cp = self.seq.pos(c as usize);
                    cp >= s || cp < g
                })
            })
        } else {
            true
        }
    }

    fn apply_or(&mut self, s: usize, e: usize, g: usize, rev: bool) {
        let len = e - s + 1;
        let seg_lo = if g > e + 1 {
            self.seq.rotate_left(s, g, len);
            g - len
        } else if g < s {
            self.seq.rotate_right(g, e + 1, len);
            g
        } else {
            s
        };
        if rev {
            self.seq.reverse(seg_lo, seg_lo + len - 1, true);
        }
    }

    fn or_opt_sweep(&mut self) -> bool {
        let mut any = false;
        for s in 0..self.m {
            let first = self.seq.at(s);
            let mut last = first;
            for len in 1..=3 {
                if len > 1 {
                    match self.seq.next(last) {
                        Some(n) => last = n,
                        None => break,
                    }
                }
                if self.try_or_opt(s, s + len - 1, first, last) {
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
    // Las figuras se renumeran por su lugar en el orden de partida ("casilla"): las vecinas en
    // el espacio quedan cerca en memoria y las consultas dejan de saltar por toda la tabla.
    // Los vecinos se siguen buscando y desempatando por el número original, así que el
    // resultado es el mismo que sin renumerar.
    let lid_of: Vec<u32> = order.iter().map(|&g| ctx.local[g]).collect();
    let mut slot_of = vec![0u32; m];
    for (s, &l) in lid_of.iter().enumerate() {
        slot_of[l as usize] = s as u32;
    }
    let ord: Vec<u32> = (0..m as u32).collect();
    let flip: Vec<bool> = lid_of
        .iter()
        .map(|&l| ctx.sh.flip[members[l as usize]])
        .collect();
    let ids = Ids { lid_of, slot_of };
    // Hasta decenas de miles de figuras el arreglo plano es lo más rápido; más allá, invertir
    // tramos largos lo vuelve cuadrático y se pasa a bloques (mismo resultado).
    if m < block_min() {
        solve(
            ctx,
            members,
            order,
            start,
            passes,
            &ids,
            ArraySeq::new(ord, flip),
        );
    } else {
        let size = block_size(m);
        solve(
            ctx,
            members,
            order,
            start,
            passes,
            &ids,
            BlockSeq::new(ord, flip, size),
        );
    }
}

/// Numeración original (posición en `members`) y casilla (lugar en el orden de partida).
struct Ids {
    lid_of: Vec<u32>,
    slot_of: Vec<u32>,
}

/// Tamaño mínimo del problema para usar [`BlockSeq`].
#[cfg(not(test))]
fn block_min() -> usize {
    BLOCK_MIN
}

#[cfg(not(test))]
fn block_size(m: usize) -> usize {
    ((m as f64).sqrt() as usize).max(64)
}

#[cfg(test)]
thread_local! {
    /// Pruebas: fuerza el tamaño de bloque (y el uso de bloques desde 2 figuras).
    pub(crate) static FORCE_BLOCK: std::cell::Cell<Option<usize>> =
        const { std::cell::Cell::new(None) };
}

#[cfg(test)]
fn block_min() -> usize {
    FORCE_BLOCK.with(|f| if f.get().is_some() { 2 } else { BLOCK_MIN })
}

#[cfg(test)]
fn block_size(m: usize) -> usize {
    FORCE_BLOCK
        .with(|f| f.get())
        .unwrap_or(((m as f64).sqrt() as usize).max(64))
}

const BLOCK_MIN: usize = 40_000;

/// Con al menos tantas figuras, las listas de vecinos se calculan en varios hilos.
const PAR_MIN: usize = 50_000;

/// Llena `nbr` (K vecinos por extremo de cada casilla, en casillas). Cada consulta es
/// independiente, así que repartirlas en hilos no cambia el resultado.
fn fill_neighbors(grid: &Grid, ids: &Ids, ends: &[[Point; 2]], closed: &[bool], nbr: &mut [u32]) {
    let m = ends.len();
    let work = |first: usize, out: &mut [u32]| {
        let mut buf = Vec::new();
        for (k, slots) in out.chunks_mut(2 * K).enumerate() {
            let s = first + k;
            for end in 0..2 {
                if closed[s] && end == 1 {
                    slots.copy_within(0..K, K);
                    continue;
                }
                grid.knn(ends[s][end], K, ids.lid_of[s] as usize, &mut buf);
                for (j, &(_, c)) in buf.iter().enumerate() {
                    slots[end * K + j] = ids.slot_of[c];
                }
            }
        }
    };
    let threads = if m >= PAR_MIN {
        std::thread::available_parallelism().map_or(1, |n| n.get().min(8))
    } else {
        1
    };
    if threads <= 1 {
        work(0, nbr);
        return;
    }
    let per = m.div_ceil(threads);
    std::thread::scope(|sc| {
        for (ci, chunk) in nbr.chunks_mut(per * 2 * K).enumerate() {
            let work = &work;
            sc.spawn(move || work(ci * per, chunk));
        }
    });
}

fn solve<S: Seq>(
    ctx: &mut Ctx,
    members: &[usize],
    order: &mut [usize],
    start: Point,
    passes: usize,
    ids: &Ids,
    seq: S,
) {
    let m = members.len();
    // Figura global de cada casilla.
    let glob: Vec<usize> = ids.lid_of.iter().map(|&l| members[l as usize]).collect();
    let to_slot = |g: usize| ids.slot_of[ctx.local[g] as usize];
    let mut prob = Problem {
        m,
        ends: glob.iter().map(|&g| [ctx.sh.a[g], ctx.sh.b[g]]).collect(),
        seq,
        flippable: glob
            .iter()
            .map(|&g| {
                ctx.items[g].path.closed || (ctx.opts.choose_direction && !ctx.items[g].locked)
            })
            .collect(),
        parent: glob
            .iter()
            .map(|&g| ctx.bouter[g].iter().map(|&p| to_slot(p)).collect())
            .collect(),
        children: glob
            .iter()
            .map(|&g| ctx.bchildren[g].iter().map(|&c| to_slot(c)).collect())
            .collect(),
        constrained: members.iter().any(|&g| !ctx.bouter[g].is_empty()),
        all_flippable: false,
        start,
        nbr: vec![NONE; m * 2 * K],
    };
    prob.all_flippable = prob.flippable.iter().all(|&f| f);

    let closed: Vec<bool> = glob.iter().map(|&g| ctx.items[g].path.closed).collect();
    let mut entries = Vec::with_capacity(m * 2);
    for (s, &l) in ids.lid_of.iter().enumerate() {
        entries.push((l as usize, 0, prob.ends[s][0]));
        if !closed[s] {
            entries.push((l as usize, 1, prob.ends[s][1]));
        }
    }
    let grid = Grid::new(&entries);
    fill_neighbors(&grid, ids, &prob.ends, &closed, &mut prob.nbr);

    for _ in 0..passes {
        let a = prob.two_opt_sweep();
        let b = prob.or_opt_sweep();
        if !(a || b) {
            break;
        }
    }

    for (s, &g) in glob.iter().enumerate() {
        ctx.sh.flip[g] = prob.seq.flip(s);
    }
    for (p, o) in order.iter_mut().enumerate() {
        *o = glob[prob.seq.at(p)];
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
