//! Rejilla uniforme para vecino más cercano y k vecinos, con borrado perezoso.

use tlaku_core::Point;

#[derive(Clone, Copy)]
struct Entry {
    x: f64,
    y: f64,
    item: u32,
    pt: u32,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Hit {
    pub item: usize,
    pub pt: usize,
    pub dist2: f64,
}

pub(crate) struct Grid {
    ox: f64,
    oy: f64,
    cs: f64,
    inv: f64,
    gx: usize,
    gy: usize,
    cells: Vec<Vec<Entry>>,
}

impl Grid {
    /// `entries`: (elemento, índice de punto dentro del elemento, coordenada).
    pub fn new(entries: &[(usize, usize, Point)]) -> Grid {
        let (mut min_x, mut min_y) = (f64::INFINITY, f64::INFINITY);
        let (mut max_x, mut max_y) = (f64::NEG_INFINITY, f64::NEG_INFINITY);
        for &(_, _, p) in entries {
            min_x = min_x.min(p.x);
            min_y = min_y.min(p.y);
            max_x = max_x.max(p.x);
            max_y = max_y.max(p.y);
        }
        if entries.is_empty() {
            (min_x, min_y, max_x, max_y) = (0.0, 0.0, 0.0, 0.0);
        }
        let (w, h) = (max_x - min_x, max_y - min_y);
        let ext = w.max(h);
        // Unas 3 entradas por celda; las trayectorias casi colineales no deben dar celdas
        // gigantes en un eje.
        let target = (entries.len() / 3).clamp(1, 1 << 20) as f64;
        let cs = if ext > 0.0 {
            let (w2, h2) = (w.max(ext / target), h.max(ext / target));
            (w2 * h2 / target).sqrt().max(ext / 4000.0)
        } else {
            1.0
        };
        let gx = (w / cs).floor() as usize + 1;
        let gy = (h / cs).floor() as usize + 1;
        let mut grid = Grid {
            ox: min_x,
            oy: min_y,
            cs,
            inv: 1.0 / cs,
            gx,
            gy,
            cells: vec![Vec::new(); gx * gy],
        };
        for &(item, pt, p) in entries {
            let (cx, cy) = grid.cell_of(p);
            grid.cells[cy * gx + cx].push(Entry {
                x: p.x,
                y: p.y,
                item: item as u32,
                pt: pt as u32,
            });
        }
        grid
    }

    fn cell_of(&self, p: Point) -> (usize, usize) {
        let cx = ((p.x - self.ox) * self.inv).floor().max(0.0) as usize;
        let cy = ((p.y - self.oy) * self.inv).floor().max(0.0) as usize;
        (cx.min(self.gx - 1), cy.min(self.gy - 1))
    }

    /// Celdas del anillo de Chebyshev `r` alrededor de (cx, cy), recortadas a la rejilla.
    fn ring(&self, cx: usize, cy: usize, r: usize, out: &mut Vec<usize>) {
        out.clear();
        let (cx, cy, r) = (cx as isize, cy as isize, r as isize);
        let (gx, gy) = (self.gx as isize, self.gy as isize);
        if r == 0 {
            out.push((cy * gx + cx) as usize);
            return;
        }
        let (x0, x1, y0, y1) = (cx - r, cx + r, cy - r, cy + r);
        for x in x0.max(0)..=x1.min(gx - 1) {
            if y0 >= 0 {
                out.push((y0 * gx + x) as usize);
            }
            if y1 < gy {
                out.push((y1 * gx + x) as usize);
            }
        }
        for y in (y0 + 1).max(0)..=(y1 - 1).min(gy - 1) {
            if x0 >= 0 {
                out.push((y * gx + x0) as usize);
            }
            if x1 < gx {
                out.push((y * gx + x1) as usize);
            }
        }
    }

    /// Distancia de `p` (en la celda `cx, cy`) al borde del bloque de celdas a distancia de
    /// Chebyshev menor que `r`. Si el bloque se sale de la rejilla por un lado, ese lado no
    /// limita (no hay entradas más allá): pero los puntos de entrada se recortan a la rejilla,
    /// así que se usa el mismo borde y sigue siendo una cota válida.
    fn block_margin(&self, p: Point, cx: usize, cy: usize, r: usize) -> f64 {
        if r == 0 {
            return 0.0;
        }
        let (lo_x, hi_x) = (
            self.ox + (cx + 1 - r.min(cx + 1)) as f64 * self.cs,
            self.ox + (cx + r) as f64 * self.cs,
        );
        let (lo_y, hi_y) = (
            self.oy + (cy + 1 - r.min(cy + 1)) as f64 * self.cs,
            self.oy + (cy + r) as f64 * self.cs,
        );
        let mut m = f64::INFINITY;
        // Un lado sin celdas más allá de la rejilla no puede esconder entradas.
        if cx + 1 > r {
            m = m.min(p.x - lo_x);
        }
        if cx + r < self.gx {
            m = m.min(hi_x - p.x);
        }
        if cy + 1 > r {
            m = m.min(p.y - lo_y);
        }
        if cy + r < self.gy {
            m = m.min(hi_y - p.y);
        }
        m
    }

    fn max_ring(&self) -> usize {
        self.gx.max(self.gy)
    }

    /// Punto más cercano a `p` entre los elementos que `accept` admite. `dead` marca
    /// elementos eliminados (sus entradas se descartan al pasar). Desempata por elemento y
    /// luego por índice de punto, así que es determinista.
    pub fn nearest(
        &mut self,
        p: Point,
        dead: impl Fn(usize) -> bool,
        accept: impl Fn(usize) -> bool,
    ) -> Option<Hit> {
        let (cx, cy) = self.cell_of(p);
        let mut best: Option<Hit> = None;
        let mut buf = Vec::new();
        for r in 0..=self.max_ring() {
            if let Some(b) = &best {
                // Todo lo que queda fuera del bloque de anillos 0..r está al menos a la
                // distancia de `p` al borde de ese bloque (con holgura por el redondeo).
                let lb = (self.block_margin(p, cx, cy, r) - 1e-9 * self.cs).max(0.0);
                if lb * lb > b.dist2 {
                    break;
                }
            }
            self.ring(cx, cy, r, &mut buf);
            for &idx in &buf {
                let cell = &mut self.cells[idx];
                let mut k = 0;
                while k < cell.len() {
                    let e = cell[k];
                    let item = e.item as usize;
                    if dead(item) {
                        cell.swap_remove(k);
                        continue;
                    }
                    k += 1;
                    if !accept(item) {
                        continue;
                    }
                    let (dx, dy) = (e.x - p.x, e.y - p.y);
                    let d2 = dx * dx + dy * dy;
                    let better = match &best {
                        None => true,
                        Some(b) => (d2, item, e.pt as usize) < (b.dist2, b.item, b.pt),
                    };
                    if better {
                        best = Some(Hit {
                            item,
                            pt: e.pt as usize,
                            dist2: d2,
                        });
                    }
                }
            }
        }
        best
    }

    /// Elementos vivos con algún punto a distancia (euclídea) menor o igual que `r` de `p`,
    /// sin repetir y sin orden. Devuelve `false` (sin llenar `out`) si el disco toca más de
    /// `max_cells` celdas: quien llama debe usar otro camino. Descarta de paso los muertos.
    pub fn items_within(
        &mut self,
        p: Point,
        r: f64,
        max_cells: usize,
        dead: impl Fn(usize) -> bool,
        out: &mut Vec<usize>,
    ) -> bool {
        out.clear();
        let span = |lo: f64, hi: f64, o: f64, n: usize| -> (usize, usize) {
            let f = |v: f64| (((v - o) * self.inv).floor().max(0.0) as usize).min(n - 1);
            (f(lo), f(hi))
        };
        let (x0, x1) = span(p.x - r, p.x + r, self.ox, self.gx);
        let (y0, y1) = span(p.y - r, p.y + r, self.oy, self.gy);
        if (x1 - x0 + 1) * (y1 - y0 + 1) > max_cells {
            return false;
        }
        let r2 = r * r;
        for y in y0..=y1 {
            for x in x0..=x1 {
                let cell = &mut self.cells[y * self.gx + x];
                let mut k = 0;
                while k < cell.len() {
                    let e = cell[k];
                    if dead(e.item as usize) {
                        cell.swap_remove(k);
                        continue;
                    }
                    k += 1;
                    let (dx, dy) = (e.x - p.x, e.y - p.y);
                    if dx * dx + dy * dy <= r2 {
                        out.push(e.item as usize);
                    }
                }
            }
        }
        out.sort_unstable();
        out.dedup();
        true
    }

    /// Los `k` elementos distintos más cercanos a `p` (por su punto más próximo), sin
    /// `exclude`, de menor a mayor distancia. No usa borrado.
    pub fn knn(&self, p: Point, k: usize, exclude: usize, out: &mut Vec<(f64, usize)>) {
        out.clear();
        let (cx, cy) = self.cell_of(p);
        let mut buf = Vec::new();
        for r in 0..=self.max_ring() {
            if out.len() == k {
                let lb = r.saturating_sub(1) as f64 * self.cs;
                if lb * lb > out[k - 1].0 {
                    break;
                }
            }
            self.ring(cx, cy, r, &mut buf);
            for &idx in &buf {
                for e in &self.cells[idx] {
                    let item = e.item as usize;
                    if item == exclude {
                        continue;
                    }
                    let (dx, dy) = (e.x - p.x, e.y - p.y);
                    let d2 = dx * dx + dy * dy;
                    if let Some(at) = out.iter().position(|&(_, it)| it == item) {
                        if d2 >= out[at].0 {
                            continue;
                        }
                        out.remove(at);
                    } else if out.len() == k && (d2, item) >= (out[k - 1].0, out[k - 1].1) {
                        continue;
                    }
                    let at = out.partition_point(|&(d, it)| (d, it) < (d2, item));
                    out.insert(at, (d2, item));
                    out.truncate(k);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Lcg(u64);
    impl Lcg {
        fn next(&mut self) -> f64 {
            self.0 = self
                .0
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            (self.0 >> 11) as f64 / (1u64 << 53) as f64
        }
    }

    fn cloud(n: usize, seed: u64) -> Vec<(usize, usize, Point)> {
        let mut r = Lcg(seed);
        (0..n)
            .map(|i| (i, 0, Point::new(r.next() * 100.0, r.next() * 50.0)))
            .collect()
    }

    #[test]
    fn nearest_matches_brute_force_with_deletions() {
        let pts = cloud(500, 7);
        let mut grid = Grid::new(&pts);
        let mut dead = vec![false; pts.len()];
        let mut r = Lcg(99);
        for _ in 0..400 {
            let q = Point::new(r.next() * 140.0 - 20.0, r.next() * 90.0 - 20.0);
            let hit = grid.nearest(q, |i| dead[i], |_| true).unwrap();
            let want = pts
                .iter()
                .filter(|(i, _, _)| !dead[*i])
                .map(|&(i, _, p)| ((p.x - q.x).powi(2) + (p.y - q.y).powi(2), i))
                .fold(
                    (f64::INFINITY, usize::MAX),
                    |a, b| if b < a { b } else { a },
                );
            assert_eq!(hit.item, want.1);
            dead[hit.item] = true;
        }
    }

    #[test]
    fn items_within_matches_brute_force() {
        let pts = cloud(400, 21);
        let mut grid = Grid::new(&pts);
        let dead: Vec<bool> = (0..pts.len()).map(|i| i % 7 == 0).collect();
        let mut r = Lcg(5);
        let mut out = Vec::new();
        for _ in 0..300 {
            let q = Point::new(r.next() * 160.0 - 30.0, r.next() * 100.0 - 25.0);
            let rad = r.next() * 30.0;
            assert!(grid.items_within(q, rad, usize::MAX, |i| dead[i], &mut out));
            let want: Vec<usize> = pts
                .iter()
                .filter(|(i, _, p)| {
                    !dead[*i] && (p.x - q.x).powi(2) + (p.y - q.y).powi(2) <= rad * rad
                })
                .map(|&(i, _, _)| i)
                .collect();
            assert_eq!(out, want);
        }
        assert!(!grid.items_within(Point::new(50.0, 25.0), 1e6, 4, |_| false, &mut out));
    }

    #[test]
    fn nearest_respects_accept_filter() {
        let pts = cloud(100, 3);
        let mut grid = Grid::new(&pts);
        let hit = grid
            .nearest(Point::new(0.0, 0.0), |_| false, |i| i % 2 == 1)
            .unwrap();
        assert_eq!(hit.item % 2, 1);
        assert!(
            grid.nearest(Point::new(0.0, 0.0), |_| false, |_| false)
                .is_none()
        );
    }

    #[test]
    fn knn_sorted_distinct_and_exact() {
        let pts = cloud(300, 11);
        let grid = Grid::new(&pts);
        let q = pts[5].2;
        let mut out = Vec::new();
        grid.knn(q, 8, 5, &mut out);
        assert_eq!(out.len(), 8);
        let mut want: Vec<(f64, usize)> = pts
            .iter()
            .filter(|(i, _, _)| *i != 5)
            .map(|&(i, _, p)| ((p.x - q.x).powi(2) + (p.y - q.y).powi(2), i))
            .collect();
        want.sort_by(|a, b| a.partial_cmp(b).unwrap());
        assert_eq!(out, want[..8]);
    }

    #[test]
    fn knn_dedupes_items_with_several_points() {
        let pts = vec![
            (0, 0, Point::new(0.0, 0.0)),
            (1, 0, Point::new(1.0, 0.0)),
            (1, 1, Point::new(1.1, 0.0)),
            (2, 0, Point::new(5.0, 0.0)),
        ];
        let grid = Grid::new(&pts);
        let mut out = Vec::new();
        grid.knn(Point::new(0.0, 0.0), 3, 0, &mut out);
        assert_eq!(out.iter().map(|x| x.1).collect::<Vec<_>>(), vec![1, 2]);
    }

    #[test]
    fn degenerate_layouts() {
        let empty = Grid::new(&[]);
        let mut e = empty;
        assert!(
            e.nearest(Point::new(1.0, 1.0), |_| false, |_| true)
                .is_none()
        );
        let same: Vec<_> = (0..5).map(|i| (i, 0, Point::new(2.0, 2.0))).collect();
        let mut g = Grid::new(&same);
        assert_eq!(
            g.nearest(Point::new(9.0, 9.0), |_| false, |_| true)
                .unwrap()
                .item,
            0
        );
        let line: Vec<_> = (0..50).map(|i| (i, 0, Point::new(i as f64, 3.0))).collect();
        let mut g = Grid::new(&line);
        assert_eq!(
            g.nearest(Point::new(20.2, 0.0), |_| false, |_| true)
                .unwrap()
                .item,
            20
        );
    }
}
