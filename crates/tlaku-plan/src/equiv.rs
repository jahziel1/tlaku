//! Las versiones rápidas del planificador contra sus referencias lentas, en escenas al azar con
//! semilla fija: el plan completo (orden, rotaciones, razones, avisos y recorrido) debe ser
//! idéntico.

use crate::testgen::random_items;
use crate::{PlanOptions, plan};

fn variants() -> Vec<(&'static str, PlanOptions)> {
    let d = PlanOptions::default();
    vec![
        ("defecto", d.clone()),
        (
            "sin elegir inicio",
            PlanOptions {
                choose_start_point: false,
                ..d.clone()
            },
        ),
        (
            "sin sentido",
            PlanOptions {
                choose_direction: false,
                choose_start_point: false,
                ..d.clone()
            },
        ),
        (
            "sin niveles ni prioridad",
            PlanOptions {
                order_by_layer: false,
                order_by_priority: false,
                ..d.clone()
            },
        ),
        (
            "sin interior primero",
            PlanOptions {
                inner_first: false,
                ..d.clone()
            },
        ),
        (
            "sin recorrido corto",
            PlanOptions {
                reduce_travel: false,
                ..d.clone()
            },
        ),
        (
            "seis pasadas",
            PlanOptions {
                improve_passes: 6,
                ..d
            },
        ),
    ]
}

#[test]
fn blocks_give_the_same_plan_as_the_flat_array() {
    for seed in 0..12u64 {
        let n = 25 + (seed as usize * 37) % 330;
        let items = random_items(seed, n, 1 + (seed as usize % 3));
        for (name, opts) in variants() {
            crate::improve::FORCE_BLOCK.with(|f| f.set(None));
            let want = plan(&items, &opts);
            for size in [4usize, 16] {
                crate::improve::FORCE_BLOCK.with(|f| f.set(Some(size)));
                let got = plan(&items, &opts);
                crate::improve::FORCE_BLOCK.with(|f| f.set(None));
                assert_eq!(got, want, "semilla {seed}, {name}, bloque {size}");
            }
        }
    }
}

#[test]
fn pruned_precedence_search_gives_the_same_reasons() {
    for seed in 0..16u64 {
        let n = 25 + (seed as usize * 29) % 300;
        let items = random_items(seed + 500, n, 1 + (seed as usize % 2));
        for (name, opts) in variants() {
            crate::explain::NO_PRUNE.with(|f| f.set(true));
            let want = plan(&items, &opts);
            crate::explain::NO_PRUNE.with(|f| f.set(false));
            let got = plan(&items, &opts);
            assert_eq!(got, want, "semilla {seed}, {name}");
        }
    }
}
