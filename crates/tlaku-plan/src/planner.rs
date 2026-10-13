//! Orquestación: niveles, contención, orden por cubo, explicaciones y recorrido.

use std::cmp::Reverse;
use std::collections::BinaryHeap;

use tlaku_core::{Point, Polyline};

use crate::contain::find_parents;
use crate::explain::explain;
use crate::improve::{improve, polish};
use crate::overlap::{Extra, find_overlaps};
use crate::sched::{input_order, nearest_neighbor};
use crate::shapes::Shapes;
use crate::{Level, Plan, PlanItem, PlanOptions, PlanStep, Reason, Warning};

/// Estado compartido entre las etapas. Los vectores son del tamaño de la entrada.
pub(crate) struct Ctx<'a> {
    pub items: &'a [PlanItem],
    pub opts: &'a PlanOptions,
    pub sh: Shapes,
    /// Contenedor inmediato (global).
    pub gparent: Vec<Option<usize>>,
    /// Precedencias por solape parcial (global): figuras que deben salir antes que `outer`.
    pub extras: Vec<Vec<Extra>>,
    /// Todas las figuras del mismo cubo que deben cortarse después de esta: el contenedor más
    /// cercano que está en el cubo y las de solape parcial. Es un grafo acíclico, no un árbol.
    pub bouter: Vec<Vec<usize>>,
    /// Inverso de `bouter`: las figuras que deben cortarse antes que ésta.
    pub bchildren: Vec<Vec<usize>>,
    /// Marcas de visita reutilizables (ver `explain`).
    pub seen: Vec<u32>,
    pub seen_gen: u32,
    pub mark: Vec<u8>,
    pub pending: Vec<u32>,
    pub local: Vec<u32>,
}

type Key = (usize, Option<usize>, i32);

fn is_valid(path: &Polyline) -> bool {
    !path.points.is_empty()
        && path
            .points
            .iter()
            .all(|p| p.x.is_finite() && p.y.is_finite())
}

fn travel_in_order(items: &[PlanItem], order: impl Iterator<Item = usize>, start: Point) -> f64 {
    let mut pos = start;
    let mut total = 0.0;
    for i in order {
        total += pos.dist(items[i].path.start());
        pos = items[i].path.end();
    }
    total
}

/// Orden original hecho válido: dentro de cada cubo, topológico estable (siempre el de menor
/// índice entre los listos), con los cubos en el orden de niveles.
fn valid_input_order(
    buckets: &[Vec<usize>],
    bouter: &[Vec<usize>],
    bchildren: &[Vec<usize>],
) -> Vec<Vec<usize>> {
    let mut pending: Vec<usize> = bchildren.iter().map(Vec::len).collect();
    buckets
        .iter()
        .map(|members| {
            let mut ready: BinaryHeap<Reverse<usize>> = members
                .iter()
                .copied()
                .filter(|&i| pending[i] == 0)
                .map(Reverse)
                .collect();
            let mut order = Vec::with_capacity(members.len());
            while let Some(Reverse(i)) = ready.pop() {
                order.push(i);
                for &p in &bouter[i] {
                    pending[p] -= 1;
                    if pending[p] == 0 {
                        ready.push(Reverse(p));
                    }
                }
            }
            order
        })
        .collect()
}

/// Primer nivel activo en el que `key` difiere de `prev` (o el primero activo si no hay
/// anterior).
fn changed_level(opts: &PlanOptions, prev: Option<&Key>, key: &Key) -> Option<Level> {
    let active = [
        (
            opts.order_by_layer,
            Level::Layer,
            prev.is_none_or(|p| p.0 != key.0),
        ),
        (
            opts.order_by_group,
            Level::Group,
            prev.is_none_or(|p| p.1 != key.1),
        ),
        (
            opts.order_by_priority,
            Level::Priority,
            prev.is_none_or(|p| p.2 != key.2),
        ),
    ];
    active
        .into_iter()
        .find(|&(on, _, differs)| on && differs)
        .map(|(_, level, _)| level)
}

fn level_reason(level: Level, key: &Key) -> Reason {
    match level {
        Level::Layer => Reason::FirstOfLayer {
            layer: key.0,
            blocked: None,
        },
        Level::Group => Reason::FirstOfGroup {
            group: key.1,
            blocked: None,
        },
        Level::Priority => Reason::FirstOfPriority {
            priority: key.2,
            blocked: None,
        },
    }
}

fn build_path(ctx: &Ctx, i: usize) -> Polyline {
    let src = &ctx.items[i].path;
    let mut points = src.points.clone();
    if src.closed {
        points.rotate_left(ctx.sh.vertex[i]);
    } else if ctx.sh.flip[i] {
        points.reverse();
    }
    Polyline {
        points,
        closed: src.closed,
    }
}

/// Ordena cada cubo y arma los pasos. Con `fixed` usa esos órdenes en vez de optimizar.
fn run(
    ctx: &mut Ctx,
    buckets: &[Vec<usize>],
    keys: &[Key],
    overrides: &[Option<(usize, Level)>],
    fixed: Option<&[Vec<usize>]>,
) -> (Vec<PlanStep>, f64) {
    let options = ctx.opts;
    let mut steps = Vec::with_capacity(buckets.iter().map(Vec::len).sum());
    let mut travel = 0.0;
    let mut pos = options.start;
    for (b, members) in buckets.iter().enumerate() {
        let order = if let Some(f) = fixed {
            f[b].clone()
        } else {
            let mut order = if options.reduce_travel {
                nearest_neighbor(ctx, members, pos)
            } else {
                input_order(ctx, members, pos)
            };
            if options.improve_passes > 0 {
                if options.reduce_travel && members.len() > 2 {
                    improve(ctx, members, &mut order, pos, options.improve_passes);
                }
                polish(ctx, &order, pos);
            }
            order
        };
        let first = changed_level(options, b.checked_sub(1).map(|p| &keys[p]), &keys[b])
            .map(|level| level_reason(level, &keys[b]));
        let reasons = explain(ctx, members, &order, pos, first.as_ref(), overrides);
        for (&i, reason) in order.iter().zip(reasons) {
            travel += pos.dist(ctx.sh.entry(i));
            pos = ctx.sh.exit(i);
            steps.push(PlanStep {
                item: i,
                path: build_path(ctx, i),
                reason,
            });
        }
    }
    (steps, travel)
}

/// Planifica el orden de corte. Ver la documentación del crate para las reglas.
pub fn plan(items: &[PlanItem], options: &PlanOptions) -> Plan {
    let n = items.len();
    let mut warnings = Vec::new();
    let mut valid = vec![true; n];
    for (i, it) in items.iter().enumerate() {
        if !is_valid(&it.path) {
            valid[i] = false;
            warnings.push(Warning::SkippedInvalid { item: i });
        }
    }
    // Cubos: misma clave de niveles activos.
    let key_of = |i: usize| -> Key {
        (
            if options.order_by_layer {
                items[i].layer_order
            } else {
                0
            },
            if options.order_by_group {
                items[i].group
            } else {
                None
            },
            if options.order_by_priority {
                items[i].priority
            } else {
                0
            },
        )
    };
    let mut sorted: Vec<usize> = (0..n).filter(|&i| valid[i]).collect();
    sorted.sort_by(|&a, &b| key_of(a).cmp(&key_of(b)).then(a.cmp(&b)));
    let mut buckets: Vec<Vec<usize>> = Vec::new();
    let mut keys: Vec<Key> = Vec::new();
    let mut bucket_of = vec![usize::MAX; n];
    for &i in &sorted {
        let k = key_of(i);
        if keys.last() != Some(&k) {
            keys.push(k);
            buckets.push(Vec::new());
        }
        bucket_of[i] = buckets.len() - 1;
        buckets.last_mut().unwrap().push(i);
    }

    // Contención global y restricciones dentro de cada cubo.
    let mut gparent = vec![None; n];
    let mut extras: Vec<Vec<Extra>> = vec![Vec::new(); n];
    if options.inner_first {
        let ids: Vec<usize> = sorted.clone();
        let paths: Vec<&Polyline> = ids.iter().map(|&i| &items[i].path).collect();
        let parents = find_parents(&paths);
        for (local, &parent) in parents.iter().enumerate() {
            gparent[ids[local]] = parent.map(|p| ids[p]);
        }
        let overlaps = find_overlaps(&paths, &parents);
        for (local, list) in overlaps.extras.into_iter().enumerate() {
            extras[ids[local]] = list
                .into_iter()
                .map(|e| Extra {
                    outer: ids[e.outer],
                    ..e
                })
                .collect();
        }
        if overlaps.truncated {
            warnings.push(Warning::OverlapAnalysisLimited);
        }
    }
    let mut bparent = vec![None; n];
    let mut bchildren = vec![Vec::new(); n];
    let mut overrides: Vec<Option<(usize, Level)>> = vec![None; n];
    for i in (0..n).filter(|&i| valid[i]) {
        let mut cur = gparent[i];
        let mut warned = false;
        while let Some(c) = cur {
            if bucket_of[c] == bucket_of[i] {
                if bparent[i].is_none() {
                    bparent[i] = Some(c);
                    bchildren[c].push(i);
                }
            } else if bucket_of[c] < bucket_of[i] && !warned {
                warned = true;
                let level = changed_level(options, Some(&keys[bucket_of[c]]), &keys[bucket_of[i]])
                    .unwrap_or(Level::Layer);
                warnings.push(Warning::InnerOverriddenByLevel {
                    inner: i,
                    container: c,
                    level,
                });
                if overrides[c].is_none() {
                    overrides[c] = Some((i, level));
                }
            }
            cur = gparent[c];
        }
    }

    // Solapes parciales: solo restringen dentro del mismo cubo (entre cubos manda el nivel).
    let mut bouter: Vec<Vec<usize>> = (0..n).map(|i| bparent[i].into_iter().collect()).collect();
    for i in (0..n).filter(|&i| valid[i]) {
        for e in &extras[i] {
            if bucket_of[e.outer] == bucket_of[i] {
                bouter[i].push(e.outer);
                bchildren[e.outer].push(i);
            }
        }
    }

    // Referencia: el orden de entrada hecho válido (niveles y precedencias), sin rotar el inicio
    // ni invertir el sentido de ninguna figura.
    let base_orders = valid_input_order(&buckets, &bouter, &bchildren);
    let baseline = travel_in_order(items, base_orders.iter().flatten().copied(), options.start);

    let mut ctx = Ctx {
        items,
        opts: options,
        sh: Shapes::new(items),
        gparent,
        extras,
        bouter,
        bchildren,
        seen: vec![0; n],
        seen_gen: 0,
        mark: vec![0; n],
        pending: vec![0; n],
        local: vec![0; n],
    };

    let (mut steps, mut travel) = run(&mut ctx, &buckets, &keys, &overrides, None);
    // Garantía: el plan nunca es más largo que la referencia válida. Si las heurísticas la
    // empeoran, se usa la referencia (con las figuras sin rotar ni invertir).
    if travel > baseline {
        ctx.sh = Shapes::new(items);
        (steps, travel) = run(&mut ctx, &buckets, &keys, &overrides, Some(&base_orders));
    }

    Plan {
        steps,
        travel_mm: travel,
        baseline_travel_mm: baseline,
        warnings,
    }
}
