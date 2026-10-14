//! Secuencia de figuras con sentido (orden + posición + invertida o no) para la mejora local.
//!
//! [`ArraySeq`] es el arreglo plano de siempre: cada inversión cuesta el largo del tramo. Con
//! cientos de miles de figuras sueltas eso vuelve cuadrática la mejora, así que [`BlockSeq`]
//! parte la secuencia en bloques con marcas perezosas de "invertido" y "sentido": una
//! inversión larga recorta dos bloques y voltea los de en medio, en O(raíz de n). Ambas dan
//! exactamente las mismas respuestas a las mismas operaciones (las pruebas lo comparan), así
//! que el resultado del planificador no depende de cuál se use.

/// Operaciones que necesita la mejora. Las posiciones son índices del orden actual.
pub(crate) trait Seq {
    #[cfg(test)]
    fn len(&self) -> usize;
    /// Figura en la posición `p`.
    fn at(&self, p: usize) -> usize;
    /// Posición de la figura `l`.
    fn pos(&self, l: usize) -> usize;
    /// Figura que sigue a `l` (si la hay).
    fn next(&self, l: usize) -> Option<usize>;
    /// Figura que precede a `l` (si la hay).
    fn prev(&self, l: usize) -> Option<usize>;
    /// La figura `l` se recorre al revés.
    fn flip(&self, l: usize) -> bool;
    /// Invierte el orden de las posiciones `i..=j`; con `toggle` también el sentido de cada una.
    fn reverse(&mut self, i: usize, j: usize, toggle: bool);
    /// `order[lo..hi].rotate_left(k)`.
    fn rotate_left(&mut self, lo: usize, hi: usize, k: usize);
    /// `order[lo..hi].rotate_right(k)`.
    fn rotate_right(&mut self, lo: usize, hi: usize, k: usize);
}

/// Arreglo plano.
pub(crate) struct ArraySeq {
    order: Vec<u32>,
    pos: Vec<u32>,
    flip: Vec<bool>,
}

impl ArraySeq {
    /// `order` es una permutación de `0..m`; `flip[l]` el sentido inicial de la figura `l`.
    pub fn new(order: Vec<u32>, flip: Vec<bool>) -> ArraySeq {
        let mut pos = vec![0u32; order.len()];
        for (p, &l) in order.iter().enumerate() {
            pos[l as usize] = p as u32;
        }
        ArraySeq { order, pos, flip }
    }

    fn set_pos_range(&mut self, lo: usize, hi: usize) {
        for p in lo..hi {
            self.pos[self.order[p] as usize] = p as u32;
        }
    }
}

impl Seq for ArraySeq {
    #[cfg(test)]
    fn len(&self) -> usize {
        self.order.len()
    }

    #[inline]
    fn at(&self, p: usize) -> usize {
        self.order[p] as usize
    }

    #[inline]
    fn pos(&self, l: usize) -> usize {
        self.pos[l] as usize
    }

    #[inline]
    fn next(&self, l: usize) -> Option<usize> {
        self.order
            .get(self.pos[l] as usize + 1)
            .map(|&x| x as usize)
    }

    #[inline]
    fn prev(&self, l: usize) -> Option<usize> {
        (self.pos[l] as usize)
            .checked_sub(1)
            .map(|p| self.order[p] as usize)
    }

    #[inline]
    fn flip(&self, l: usize) -> bool {
        self.flip[l]
    }

    fn reverse(&mut self, i: usize, j: usize, toggle: bool) {
        self.order[i..=j].reverse();
        for p in i..=j {
            let l = self.order[p] as usize;
            if toggle {
                self.flip[l] = !self.flip[l];
            }
            self.pos[l] = p as u32;
        }
    }

    fn rotate_left(&mut self, lo: usize, hi: usize, k: usize) {
        self.order[lo..hi].rotate_left(k);
        self.set_pos_range(lo, hi);
    }

    fn rotate_right(&mut self, lo: usize, hi: usize, k: usize) {
        self.order[lo..hi].rotate_right(k);
        self.set_pos_range(lo, hi);
    }
}

/// Un bloque de posiciones consecutivas. `elems` está en orden físico; si `rev`, el orden
/// lógico es el inverso. `flip` se suma (xor) al sentido propio de cada figura del bloque.
struct Block {
    elems: Vec<u32>,
    rev: bool,
    flip: bool,
}

/// Secuencia por bloques (raíz de n por operación).
pub(crate) struct BlockSeq {
    m: usize,
    /// Tamaño de bloque al armar.
    size: usize,
    blocks: Vec<Block>,
    /// Ids de bloque en orden lógico.
    list: Vec<u32>,
    /// Posición lógica donde empieza cada bloque de `list` (por índice de `list`).
    starts: Vec<usize>,
    /// Lo mismo, por id de bloque.
    bstart: Vec<usize>,
    block_of: Vec<u32>,
    /// Índice físico dentro del bloque.
    idx_in: Vec<u32>,
    /// Sentido propio; el efectivo es `raw ^ flip del bloque`.
    raw: Vec<bool>,
    /// Con más bloques que esto se rearma todo.
    max_blocks: usize,
    /// Últimos dos bloques (índices de `list`) que respondieron a `at`: las consultas de la
    /// mejora vienen en pares de posiciones vecinas.
    hot: [std::cell::Cell<usize>; 2],
}

/// Tramos hasta este largo se invierten figura a figura, sin partir bloques.
const SMALL: usize = 24;

impl BlockSeq {
    pub fn new(order: Vec<u32>, flip: Vec<bool>, size: usize) -> BlockSeq {
        let m = order.len();
        let size = size.max(4);
        let mut s = BlockSeq {
            m,
            size,
            blocks: Vec::new(),
            list: Vec::new(),
            starts: Vec::new(),
            bstart: Vec::new(),
            block_of: vec![0; m],
            idx_in: vec![0; m],
            raw: flip,
            max_blocks: (m / size + 1) * 4 + 8,
            hot: [std::cell::Cell::new(0), std::cell::Cell::new(0)],
        };
        s.rebuild_from(&order);
        s
    }

    /// Arma los bloques desde un orden lógico con los sentidos efectivos ya en `raw`.
    fn rebuild_from(&mut self, order: &[u32]) {
        self.blocks.clear();
        self.list.clear();
        self.starts.clear();
        self.bstart.clear();
        for (b, chunk) in order.chunks(self.size).enumerate() {
            for (k, &l) in chunk.iter().enumerate() {
                self.block_of[l as usize] = b as u32;
                self.idx_in[l as usize] = k as u32;
            }
            self.starts.push(b * self.size);
            self.bstart.push(b * self.size);
            self.list.push(b as u32);
            self.blocks.push(Block {
                elems: chunk.to_vec(),
                rev: false,
                flip: false,
            });
        }
    }

    /// Arma de nuevo todos los bloques (el orden y los sentidos efectivos no cambian).
    fn rebuild(&mut self) {
        let order: Vec<u32> = (0..self.m).map(|p| self.at(p) as u32).collect();
        for l in 0..self.m {
            self.raw[l] = self.flip(l);
        }
        self.rebuild_from(&order);
    }

    /// Índice en `list` del bloque que contiene la posición `p`.
    #[inline]
    fn locate(&self, p: usize) -> usize {
        self.starts.partition_point(|&s| s <= p) - 1
    }

    /// Como `locate`, pero prueba primero los dos últimos bloques consultados.
    #[inline]
    fn locate_hot(&self, p: usize) -> usize {
        for h in &self.hot {
            let li = h.get();
            if li < self.starts.len() {
                let lo = self.starts[li];
                let hi = lo + self.blocks[self.list[li] as usize].elems.len();
                if lo <= p && p < hi {
                    return li;
                }
            }
        }
        let li = self.locate(p);
        self.hot[1].set(self.hot[0].get());
        self.hot[0].set(li);
        li
    }

    #[inline]
    fn phys(blk: &Block, off: usize) -> usize {
        if blk.rev {
            blk.elems.len() - 1 - off
        } else {
            off
        }
    }

    /// Hace que un bloque empiece justo en la posición lógica `p` (0 < p < m). Siempre pasa a
    /// un bloque nuevo la parte que se corta como sufijo físico, para no desplazar ni
    /// renumerar a las figuras que se quedan.
    fn split(&mut self, p: usize) {
        let li = self.locate(p);
        let start = self.starts[li];
        let off = p - start;
        if off == 0 {
            return;
        }
        let id = self.list[li] as usize;
        let new_id = self.blocks.len() as u32;
        let (rev, flip) = (self.blocks[id].rev, self.blocks[id].flip);
        let len = self.blocks[id].elems.len();
        // Con `rev` el tramo lógico [0, off) es el sufijo físico; si no, lo es [off, len).
        let cut = if rev { len - off } else { off };
        let moved = self.blocks[id].elems.split_off(cut);
        for (k, &l) in moved.iter().enumerate() {
            self.block_of[l as usize] = new_id;
            self.idx_in[l as usize] = k as u32;
        }
        self.blocks.push(Block {
            elems: moved,
            rev,
            flip,
        });
        if rev {
            // El nuevo es la cabeza lógica: va antes y el viejo empieza ahora en `p`.
            self.list.insert(li, new_id);
            self.starts.insert(li, start);
            self.starts[li + 1] = p;
            self.bstart.push(start);
            self.bstart[id] = p;
        } else {
            self.list.insert(li + 1, new_id);
            self.starts.insert(li + 1, p);
            self.bstart.push(p);
        }
    }

    /// Coloca la figura `l` (con sentido efectivo `eff`) en la posición `p`, que ya existe.
    fn put(&mut self, p: usize, l: usize, eff: bool) {
        let li = self.locate(p);
        let id = self.list[li] as usize;
        let ph = Self::phys(&self.blocks[id], p - self.starts[li]);
        self.blocks[id].elems[ph] = l as u32;
        self.block_of[l] = id as u32;
        self.idx_in[l] = ph as u32;
        self.raw[l] = eff ^ self.blocks[id].flip;
    }

    fn reverse_small(&mut self, i: usize, j: usize, toggle: bool) {
        let items: Vec<(usize, bool)> = (i..=j)
            .map(|p| {
                let l = self.at(p);
                (l, self.flip(l))
            })
            .collect();
        for (k, &(l, f)) in items.iter().rev().enumerate() {
            self.put(i + k, l, f ^ toggle);
        }
    }

    fn reverse_blocks(&mut self, i: usize, j: usize, toggle: bool) {
        if i > 0 {
            self.split(i);
        }
        if j + 1 < self.m {
            self.split(j + 1);
        }
        let li = self.locate(i);
        let lj = if j + 1 < self.m {
            self.locate(j + 1) - 1
        } else {
            self.list.len() - 1
        };
        self.list[li..=lj].reverse();
        let mut at = i;
        for k in li..=lj {
            let id = self.list[k] as usize;
            let blk = &mut self.blocks[id];
            blk.rev = !blk.rev;
            if toggle {
                blk.flip = !blk.flip;
            }
            self.starts[k] = at;
            self.bstart[id] = at;
            at += blk.elems.len();
        }
        if self.list.len() > self.max_blocks {
            self.rebuild();
        }
    }
}

impl Seq for BlockSeq {
    #[cfg(test)]
    fn len(&self) -> usize {
        self.m
    }

    fn at(&self, p: usize) -> usize {
        let li = self.locate_hot(p);
        let blk = &self.blocks[self.list[li] as usize];
        blk.elems[Self::phys(blk, p - self.starts[li])] as usize
    }

    fn pos(&self, l: usize) -> usize {
        let id = self.block_of[l] as usize;
        let blk = &self.blocks[id];
        let idx = self.idx_in[l] as usize;
        let off = if blk.rev {
            blk.elems.len() - 1 - idx
        } else {
            idx
        };
        self.bstart[id] + off
    }

    fn next(&self, l: usize) -> Option<usize> {
        let id = self.block_of[l] as usize;
        let blk = &self.blocks[id];
        let idx = self.idx_in[l] as usize;
        // Siguiente lugar físico en el sentido lógico.
        if blk.rev {
            if idx > 0 {
                return Some(blk.elems[idx - 1] as usize);
            }
        } else if idx + 1 < blk.elems.len() {
            return Some(blk.elems[idx + 1] as usize);
        }
        let end = self.bstart[id] + blk.elems.len();
        (end < self.m).then(|| self.at(end))
    }

    fn prev(&self, l: usize) -> Option<usize> {
        let id = self.block_of[l] as usize;
        let blk = &self.blocks[id];
        let idx = self.idx_in[l] as usize;
        if blk.rev {
            if idx + 1 < blk.elems.len() {
                return Some(blk.elems[idx + 1] as usize);
            }
        } else if idx > 0 {
            return Some(blk.elems[idx - 1] as usize);
        }
        let first = self.bstart[id];
        (first > 0).then(|| self.at(first - 1))
    }

    fn flip(&self, l: usize) -> bool {
        self.raw[l] ^ self.blocks[self.block_of[l] as usize].flip
    }

    fn reverse(&mut self, i: usize, j: usize, toggle: bool) {
        if i >= j {
            if i == j && toggle {
                let l = self.at(i);
                self.raw[l] = !self.raw[l];
            }
            return;
        }
        if j - i < SMALL {
            self.reverse_small(i, j, toggle);
        } else {
            self.reverse_blocks(i, j, toggle);
        }
    }

    fn rotate_left(&mut self, lo: usize, hi: usize, k: usize) {
        // [seg | resto] -> [resto | seg]
        let rest = hi - lo - k;
        if k == 0 || rest == 0 {
            return;
        }
        self.reverse(lo, hi - 1, false);
        self.reverse(lo, lo + rest - 1, false);
        self.reverse(lo + rest, hi - 1, false);
    }

    fn rotate_right(&mut self, lo: usize, hi: usize, k: usize) {
        // [resto | seg] -> [seg | resto]
        let rest = hi - lo - k;
        if k == 0 || rest == 0 {
            return;
        }
        self.reverse(lo, hi - 1, false);
        self.reverse(lo, lo + k - 1, false);
        self.reverse(lo + k, hi - 1, false);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Lcg(u64);
    impl Lcg {
        fn next(&mut self, n: usize) -> usize {
            self.0 = self
                .0
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            ((self.0 >> 33) as usize) % n
        }
    }

    fn same(a: &ArraySeq, b: &BlockSeq) {
        for p in 0..a.len() {
            assert_eq!(a.at(p), b.at(p), "at({p})");
        }
        for l in 0..a.len() {
            assert_eq!(a.pos(l), b.pos(l), "pos({l})");
            assert_eq!(a.next(l), b.next(l), "next({l})");
            assert_eq!(a.prev(l), b.prev(l), "prev({l})");
            assert_eq!(a.flip(l), b.flip(l), "flip({l})");
        }
    }

    /// Operaciones al azar: ambas secuencias deben responder igual en cada paso.
    fn run(m: usize, size: usize, seed: u64, ops: usize) {
        let mut r = Lcg(seed);
        let mut order: Vec<u32> = (0..m as u32).collect();
        for k in (1..m).rev() {
            order.swap(k, r.next(k + 1));
        }
        let flip: Vec<bool> = (0..m).map(|_| r.next(2) == 1).collect();
        let mut a = ArraySeq::new(order.clone(), flip.clone());
        let mut b = BlockSeq::new(order, flip, size);
        same(&a, &b);
        for step in 0..ops {
            let i = r.next(m);
            let span = if r.next(3) == 0 {
                r.next(m)
            } else {
                r.next(40)
            };
            let j = (i + span).min(m - 1);
            match r.next(4) {
                0 | 1 => {
                    let t = r.next(2) == 1;
                    a.reverse(i, j, t);
                    b.reverse(i, j, t);
                }
                2 => {
                    let (lo, hi) = (i, j + 1);
                    let k = r.next(hi - lo + 1).min(hi - lo);
                    a.rotate_left(lo, hi, k);
                    b.rotate_left(lo, hi, k);
                }
                _ => {
                    let (lo, hi) = (i, j + 1);
                    let k = r.next(hi - lo + 1).min(hi - lo);
                    a.rotate_right(lo, hi, k);
                    b.rotate_right(lo, hi, k);
                }
            }
            if step % 7 == 0 {
                same(&a, &b);
            }
        }
        same(&a, &b);
    }

    #[test]
    fn block_seq_matches_array_seq() {
        run(1, 4, 1, 20);
        run(2, 4, 2, 50);
        run(10, 4, 3, 300);
        run(100, 4, 4, 2000);
        run(500, 8, 5, 2000);
        run(500, 64, 6, 2000);
        run(2000, 16, 7, 3000);
    }
}
