//! Geometría: listas de vértices y primitivas de `Path`, y las curvas de `Rect` y `Ellipse`.

use tlaku_core::Point;
use tlaku_core::bezpath::{BezPath, PathSeg};

/// Constante de la aproximación de un cuarto de círculo con una cúbica.
const KAPPA: f64 = 0.552_284_749_830_793_4;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Vert {
    pub p: Point,
    /// Manejador de salida y de entrada; `None` si el vértice no los trae (o solo trae la marca
    /// `c0x1` de "sin manejador").
    pub c0: Option<Point>,
    pub c1: Option<Point>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PrimKind {
    Line,
    Bezier,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Prim {
    pub kind: PrimKind,
    pub a: usize,
    pub b: usize,
}

/// Lista de primitivas: o explícita, o `LineClosed`/`LineOpen` (rectas por todos los vértices, con
/// o sin cierre).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Prims {
    List(Vec<Prim>),
    LineClosed,
    LineOpen,
}

struct Scanner<'a> {
    s: &'a [u8],
    i: usize,
}

impl<'a> Scanner<'a> {
    fn new(s: &'a str) -> Self {
        Self {
            s: s.as_bytes(),
            i: 0,
        }
    }

    fn peek(&self) -> Option<u8> {
        self.s.get(self.i).copied()
    }

    fn skip_ws(&mut self) {
        while self.peek().is_some_and(|c| c.is_ascii_whitespace()) {
            self.i += 1;
        }
    }

    /// Lee un número (con signo y exponente) en la posición actual.
    fn number(&mut self) -> Option<f64> {
        let start = self.i;
        while self
            .peek()
            .is_some_and(|c| matches!(c, b'0'..=b'9' | b'+' | b'-' | b'.' | b'e' | b'E'))
        {
            self.i += 1;
        }
        let text = std::str::from_utf8(&self.s[start..self.i]).ok()?;
        text.parse::<f64>().ok().filter(|v| v.is_finite())
    }
}

/// `V10 20c0x1c1x1V30 5c0x3c0y4c1x2c1y6...`: cada `V` lleva `x y` y, para curvas, los manejadores
/// `c0x c0y c1x c1y`. Un manejador solo cuenta si trae las dos coordenadas (`c0x1` solo es la
/// marca de "sin manejador"). Devuelve los vértices y cuántos no se pudieron leer.
pub(crate) fn parse_verts(text: &str) -> (Vec<Vert>, usize) {
    let mut sc = Scanner::new(text);
    let mut verts = Vec::new();
    let mut bad = 0usize;
    loop {
        sc.skip_ws();
        match sc.peek() {
            None => break,
            Some(b'V') => sc.i += 1,
            Some(_) => {
                sc.i += 1;
                continue;
            }
        }
        sc.skip_ws();
        let x = sc.number();
        sc.skip_ws();
        let y = sc.number();
        let (mut c0x, mut c0y, mut c1x, mut c1y) = (None, None, None, None);
        while let Some(c) = sc.peek() {
            if c == b'V' {
                break;
            }
            if c == b'c' {
                let tag = (sc.s.get(sc.i + 1).copied(), sc.s.get(sc.i + 2).copied());
                let slot = match tag {
                    (Some(b'0'), Some(b'x')) => Some(&mut c0x),
                    (Some(b'0'), Some(b'y')) => Some(&mut c0y),
                    (Some(b'1'), Some(b'x')) => Some(&mut c1x),
                    (Some(b'1'), Some(b'y')) => Some(&mut c1y),
                    _ => None,
                };
                if let Some(slot) = slot {
                    sc.i += 3;
                    *slot = sc.number();
                    continue;
                }
            }
            sc.i += 1;
        }
        let pair = |a: Option<f64>, b: Option<f64>| match (a, b) {
            (Some(x), Some(y)) => Some(Point::new(x, y)),
            _ => None,
        };
        match (x, y) {
            (Some(x), Some(y)) => verts.push(Vert {
                p: Point::new(x, y),
                c0: pair(c0x, c0y),
                c1: pair(c1x, c1y),
            }),
            _ => bad += 1,
        }
    }
    (verts, bad)
}

/// `L0 1B1 2...`, `LineClosed` o `LineOpen`. Devuelve también las letras de primitivas que no se conocen.
pub(crate) fn parse_prims(text: &str) -> (Prims, Vec<char>) {
    match text.trim() {
        "LineClosed" => return (Prims::LineClosed, Vec::new()),
        "LineOpen" => return (Prims::LineOpen, Vec::new()),
        _ => {}
    }
    let mut sc = Scanner::new(text);
    let mut prims = Vec::new();
    let mut unknown = Vec::new();
    loop {
        sc.skip_ws();
        let Some(letter) = sc.peek() else { break };
        sc.i += 1;
        if !letter.is_ascii_alphabetic() {
            continue;
        }
        let mut ints = Vec::new();
        for _ in 0..4 {
            sc.skip_ws();
            let before = sc.i;
            while sc.peek().is_some_and(|c| c.is_ascii_digit()) {
                sc.i += 1;
            }
            if sc.i == before {
                break;
            }
            match std::str::from_utf8(&sc.s[before..sc.i])
                .ok()
                .and_then(|t| t.parse::<usize>().ok())
            {
                Some(v) => ints.push(v),
                None => break,
            }
        }
        let kind = match letter {
            b'L' => Some(PrimKind::Line),
            b'B' => Some(PrimKind::Bezier),
            _ => None,
        };
        match (kind, ints.as_slice()) {
            (Some(kind), &[a, b]) => prims.push(Prim { kind, a, b }),
            _ => {
                let c = char::from(letter);
                if !unknown.contains(&c) {
                    unknown.push(c);
                }
            }
        }
    }
    (Prims::List(prims), unknown)
}

/// Resultado de armar la trayectoria; `degraded` cuenta curvas sin manejadores que se dejaron
/// como rectas y `invalid` las primitivas con un índice fuera de rango.
pub(crate) struct BuiltPath {
    pub path: BezPath,
    pub degraded: usize,
    pub invalid: usize,
}

/// Arma las curvas con los vértices y las primitivas. Una primitiva que no empieza donde terminó
/// la anterior abre un subtrayecto; el que vuelve a su primer vértice se cierra.
pub(crate) fn build_path(verts: &[Vert], prims: &Prims) -> BuiltPath {
    let mut segs: Vec<PathSeg> = Vec::new();
    let (mut degraded, mut invalid) = (0usize, 0usize);
    match prims {
        Prims::LineClosed | Prims::LineOpen => {
            for (i, v) in verts.iter().enumerate() {
                segs.push(if i == 0 {
                    PathSeg::MoveTo { to: v.p }
                } else {
                    PathSeg::LineTo { to: v.p }
                });
            }
            if verts.len() < 2 {
                segs.clear();
            } else if *prims == Prims::LineClosed {
                segs.push(PathSeg::Close);
            }
        }
        Prims::List(list) => {
            let mut cur: Option<usize> = None;
            let mut start: Option<usize> = None;
            for prim in list {
                let (Some(va), Some(vb)) = (verts.get(prim.a), verts.get(prim.b)) else {
                    invalid += 1;
                    continue;
                };
                if prim.a == prim.b {
                    continue;
                }
                if cur != Some(prim.a) {
                    segs.push(PathSeg::MoveTo { to: va.p });
                    start = Some(prim.a);
                }
                let seg = match (prim.kind, va.c0, vb.c1) {
                    (PrimKind::Bezier, None, None) => {
                        degraded += 1;
                        PathSeg::LineTo { to: vb.p }
                    }
                    (PrimKind::Bezier, c0, c1) => PathSeg::CubicTo {
                        ctrl1: c0.unwrap_or(va.p),
                        ctrl2: c1.unwrap_or(vb.p),
                        to: vb.p,
                    },
                    (PrimKind::Line, _, _) => PathSeg::LineTo { to: vb.p },
                };
                segs.push(seg);
                cur = Some(prim.b);
                if Some(prim.b) == start {
                    segs.push(PathSeg::Close);
                    cur = None;
                }
            }
        }
    }
    BuiltPath {
        path: BezPath(segs),
        degraded,
        invalid,
    }
}

/// Rectángulo centrado en el origen, contra el reloj desde la esquina inferior izquierda.
/// El radio se limita a la mitad del lado menor.
pub fn rect_path(w: f64, h: f64, corner_radius: f64) -> BezPath {
    let (hw, hh) = (w.abs() / 2.0, h.abs() / 2.0);
    let r = corner_radius.max(0.0).min(hw).min(hh);
    let pt = Point::new;
    let mut s = Vec::new();
    if r <= 1e-9 {
        s.push(PathSeg::MoveTo { to: pt(-hw, -hh) });
        s.push(PathSeg::LineTo { to: pt(hw, -hh) });
        s.push(PathSeg::LineTo { to: pt(hw, hh) });
        s.push(PathSeg::LineTo { to: pt(-hw, hh) });
        s.push(PathSeg::Close);
        return BezPath(s);
    }
    let k = r * KAPPA;
    s.push(PathSeg::MoveTo {
        to: pt(-hw + r, -hh),
    });
    s.push(PathSeg::LineTo {
        to: pt(hw - r, -hh),
    });
    s.push(PathSeg::CubicTo {
        ctrl1: pt(hw - r + k, -hh),
        ctrl2: pt(hw, -hh + r - k),
        to: pt(hw, -hh + r),
    });
    s.push(PathSeg::LineTo { to: pt(hw, hh - r) });
    s.push(PathSeg::CubicTo {
        ctrl1: pt(hw, hh - r + k),
        ctrl2: pt(hw - r + k, hh),
        to: pt(hw - r, hh),
    });
    s.push(PathSeg::LineTo {
        to: pt(-hw + r, hh),
    });
    s.push(PathSeg::CubicTo {
        ctrl1: pt(-hw + r - k, hh),
        ctrl2: pt(-hw, hh - r + k),
        to: pt(-hw, hh - r),
    });
    s.push(PathSeg::LineTo {
        to: pt(-hw, -hh + r),
    });
    s.push(PathSeg::CubicTo {
        ctrl1: pt(-hw, -hh + r - k),
        ctrl2: pt(-hw + r - k, -hh),
        to: pt(-hw + r, -hh),
    });
    s.push(PathSeg::Close);
    BezPath(s)
}

/// Elipse centrada en el origen con cuatro cúbicas, contra el reloj desde el punto de la derecha.
pub fn ellipse_path(rx: f64, ry: f64) -> BezPath {
    let (rx, ry) = (rx.abs(), ry.abs());
    let (kx, ky) = (rx * KAPPA, ry * KAPPA);
    let pt = Point::new;
    BezPath(vec![
        PathSeg::MoveTo { to: pt(rx, 0.0) },
        PathSeg::CubicTo {
            ctrl1: pt(rx, ky),
            ctrl2: pt(kx, ry),
            to: pt(0.0, ry),
        },
        PathSeg::CubicTo {
            ctrl1: pt(-kx, ry),
            ctrl2: pt(-rx, ky),
            to: pt(-rx, 0.0),
        },
        PathSeg::CubicTo {
            ctrl1: pt(-rx, -ky),
            ctrl2: pt(-kx, -ry),
            to: pt(0.0, -ry),
        },
        PathSeg::CubicTo {
            ctrl1: pt(kx, -ry),
            ctrl2: pt(rx, -ky),
            to: pt(rx, 0.0),
        },
        PathSeg::Close,
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verts_with_and_without_handles() {
        let (v, bad) = parse_verts("V10 20c0x1c1x1V-1.5 2e1c0x3c0y4c1x5c1y6");
        assert_eq!(bad, 0);
        assert_eq!(v.len(), 2);
        assert_eq!(v[0].p, Point::new(10.0, 20.0));
        assert_eq!((v[0].c0, v[0].c1), (None, None));
        assert_eq!(v[1].p, Point::new(-1.5, 20.0));
        assert_eq!(v[1].c0, Some(Point::new(3.0, 4.0)));
        assert_eq!(v[1].c1, Some(Point::new(5.0, 6.0)));
    }

    #[test]
    fn prims_and_unknown_letters() {
        let (p, unknown) = parse_prims("L0 1B1 2Q2 3");
        assert_eq!(unknown, vec!['Q']);
        let Prims::List(list) = p else { panic!() };
        assert_eq!(list.len(), 2);
        assert_eq!(list[1].kind, PrimKind::Bezier);
        assert_eq!(parse_prims("LineClosed").0, Prims::LineClosed);
        assert_eq!(parse_prims(" LineOpen ").0, Prims::LineOpen);
    }

    #[test]
    fn closes_subpath_that_returns_to_start() {
        let (v, _) = parse_verts("V0 0V10 0V10 10");
        let (p, _) = parse_prims("L0 1L1 2L2 0");
        let built = build_path(&v, &p);
        assert_eq!(built.path.0.len(), 5);
        assert!(matches!(built.path.0[4], PathSeg::Close));
    }

    #[test]
    fn rounded_rect_bounds() {
        let b = rect_path(10.0, 6.0, 2.0).bounds().unwrap();
        assert!((b.max.x - 5.0).abs() < 1e-9 && (b.min.y + 3.0).abs() < 1e-9);
        let b = ellipse_path(4.0, 2.0).bounds().unwrap();
        assert!((b.max.x - 4.0).abs() < 1e-6 && (b.max.y - 2.0).abs() < 1e-6);
    }
}
