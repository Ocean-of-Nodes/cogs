//! The defining property of incrementality: after any sequence of
//! input changes, propagating gives exactly the graph — ids
//! included — that running every view from scratch over the same
//! inputs gives.

use super::*;

/// The inputs of `g` replayed from its event log into a new
/// database, propagated from scratch.
fn from_scratch(g: &Graph) -> Graph {
    let mut inputs = Graph::default();
    let delta: Delta = g
        .events
        .iter()
        .map(|m| ThinPatch::from(&m.operation))
        .collect();
    inputs.apply_delta(delta).unwrap();
    let mut db = compiler(inputs);
    propagate(&mut db);
    db.into_graph()
}

/// xorshift64: deterministic randomness without dependencies.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }

    fn pick<T: Copy>(&mut self, items: &[T]) -> Option<T> {
        (!items.is_empty()).then(|| items[self.below(items.len())])
    }
}

const NAMES: [&str; 3] = ["a", "b", "c"];
const KINDS: [&str; 3] = ["def", "use", "comment"];

/// Apply one random input change.
fn random_edit(db: &mut Database, rng: &mut Rng) {
    let inputs: Vec<NodeId> = {
        let mut ids: Vec<NodeId> = db
            .graph()
            .iter_nodes()
            .filter(|id| !crate::views::is_derived(id))
            .collect();
        ids.sort();
        ids
    };
    let input_edges: Vec<EdgeId> = {
        let mut ids: Vec<EdgeId> = db
            .graph()
            .iter_edges()
            .filter(|id| !crate::views::is_derived(id))
            .collect();
        ids.sort();
        ids
    };
    let random_obj = |rng: &mut Rng| {
        obj(&[
            ("kind", KINDS[rng.below(KINDS.len())].into()),
            ("name", NAMES[rng.below(NAMES.len())].into()),
        ])
    };
    let g = db.graph_mut();

    match rng.below(8) {
        0 | 1 => {
            g.add_node(random_obj(rng));
        }
        2 => {
            if let Some(id) = rng.pick(&inputs) {
                g.replace_node(&id, random_obj(rng)).unwrap();
            }
        }
        3 => {
            if let Some(id) = rng.pick(&inputs) {
                g.remove_node(&id).unwrap();
            }
        }
        4 => {
            if let (Some(a), Some(b)) = (rng.pick(&inputs), rng.pick(&inputs)) {
                g.add_edge(a, b).unwrap();
            }
        }
        5 => {
            if let Some(e) = rng.pick(&input_edges) {
                g.remove_edge(&e).unwrap();
            }
        }
        _ => {
            g.undo().unwrap();
        }
    }
}

#[test]
fn incremental_equals_from_scratch() {
    for seed in 1..=20u64 {
        let mut rng = Rng(seed.wrapping_mul(0x9e3779b97f4a7c15));
        let mut db = compiler(Graph::default());
        for step in 0..60 {
            random_edit(&mut db, &mut rng);
            // Sometimes several edits pile up before propagating.
            if rng.below(3) == 0 {
                continue;
            }
            propagate(&mut db);
            assert!(
                content(db.graph()) == content(&from_scratch(db.graph())),
                "seed {seed}, step {step}: incremental and from-scratch differ"
            );
        }
    }
}
