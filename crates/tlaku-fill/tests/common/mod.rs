#![allow(dead_code)]

use tlaku_core::toolpath::{Move, ToolpathBlock};
use tlaku_core::{Point, Polyline};

pub fn poly(pts: &[(f64, f64)]) -> Polyline {
    Polyline {
        points: pts.iter().map(|&(x, y)| Point::new(x, y)).collect(),
        closed: true,
    }
}

pub fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Polyline {
    poly(&[(x0, y0), (x1, y0), (x1, y1), (x0, y1)])
}

/// Igual que `rect` pero recorrido en sentido horario.
pub fn rect_cw(x0: f64, y0: f64, x1: f64, y1: f64) -> Polyline {
    poly(&[(x0, y0), (x0, y1), (x1, y1), (x1, y0)])
}

pub fn circle(cx: f64, cy: f64, r: f64, n: usize) -> Polyline {
    Polyline {
        points: (0..n)
            .map(|i| {
                let a = std::f64::consts::TAU * i as f64 / n as f64;
                Point::new(cx + r * a.cos(), cy + r * a.sin())
            })
            .collect(),
        closed: true,
    }
}

/// Tramos con potencia > 0 como (desde, hasta, potencia).
pub fn burns(block: &ToolpathBlock) -> Vec<(Point, Point, f64)> {
    let mut pos = Point::new(0.0, 0.0);
    let mut out = Vec::new();
    for m in &block.moves {
        if let Move::Cut { to, power_pct } = *m
            && power_pct > 0.0
        {
            out.push((pos, to, power_pct));
        }
        pos = m.to();
    }
    out
}

/// Longitud recorrida con `Cut` a potencia 0 (overscan y huecos internos).
pub fn idle_length(block: &ToolpathBlock) -> f64 {
    let mut pos = Point::new(0.0, 0.0);
    let mut total = 0.0;
    for m in &block.moves {
        if let Move::Cut { to, power_pct } = *m
            && power_pct == 0.0
        {
            total += pos.dist(to);
        }
        pos = m.to();
    }
    total
}

pub fn burned_length(block: &ToolpathBlock) -> f64 {
    burns(block).iter().map(|(a, b, _)| a.dist(*b)).sum()
}

/// Número de giro de un punto respecto a un conjunto de polígonos, por suma de ángulos.
/// Independiente del algoritmo de barrido a propósito.
pub fn winding(p: Point, polys: &[Polyline]) -> i32 {
    let mut total = 0.0;
    for poly in polys.iter().filter(|q| q.closed && q.points.len() >= 3) {
        let n = poly.points.len();
        for i in 0..n {
            let a = poly.points[i];
            let b = poly.points[(i + 1) % n];
            let (ax, ay, bx, by) = (a.x - p.x, a.y - p.y, b.x - p.x, b.y - p.y);
            total += (ax * by - ay * bx).atan2(ax * bx + ay * by);
        }
    }
    (total / std::f64::consts::TAU).round() as i32
}

pub fn is_inside(p: Point, polys: &[Polyline], nonzero: bool) -> bool {
    let w = winding(p, polys);
    if nonzero { w != 0 } else { w % 2 != 0 }
}

/// Lista legible del recorrido, para los archivos dorados.
pub fn dump(block: &ToolpathBlock) -> String {
    let f = |v: f64| if v.abs() < 5e-5 { 0.0 } else { v };
    let mut s = format!(
        "# {}\n# F{} aire={} modo={:?}\n",
        block.label, block.speed_mm_min, block.air_assist, block.power_mode
    );
    for m in &block.moves {
        match *m {
            Move::Travel { to } => s += &format!("G0 X{:.4} Y{:.4}\n", f(to.x), f(to.y)),
            Move::Cut { to, power_pct } => {
                s += &format!("G1 X{:.4} Y{:.4} S{:.1}\n", f(to.x), f(to.y), power_pct)
            }
        }
    }
    s
}
