//! Edge-side incidence: which edges touch `id`?
//!
//! Algorithms over the public read API of [`Graph`] — none of these
//! need privileged field access.

use crate::*;

/* 
/// All edges incident to `id` — every edge that has
/// `Pointee::EntityId(id)` as its `source` or `target`. Hyperedges
/// are not included; iterate [`Graph::iter_hyperedge`] separately
/// if you want them.
///
/// Edges whose endpoint is a *subobject* of `id` (i.e.
/// `Pointee::Path { entity: id, .. }`) are **not** returned — the
/// match is on `Pointee::EntityId(id)` exactly.
pub fn edges(g: &Graph, id: &EntityId) -> Result<Vec<EdgeID>, EntityNotFoundError> {
    if !g.is_exist(id) {
        return Err(EntityNotFoundError { id: *id });
    }
    let me = Pointee::EntityId(*id);
    Ok(g.iter_edges()
        .filter(|eid| {
            g.edge(eid)
                .map(|t| t.source == me || t.target == me)
                .unwrap_or(false)
        })
        .collect())
}

/// Edges going *out* of `id` — those with `source == id`. The
/// directional split of [`edges`].
pub fn out_edges(g: &Graph, id: &EntityId) -> Result<Vec<EdgeID>, EntityNotFoundError> {
    if !g.is_exist(id) {
        return Err(EntityNotFoundError { id: *id });
    }
    let me = Pointee::EntityId(*id);
    Ok(g.iter_edges()
        .filter(|eid| g.edge(eid).map(|t| t.source == me).unwrap_or(false))
        .collect())
}

/// Edges coming *in* to `id` — those with `target == id`. The
/// directional split of [`edges`].
pub fn in_edges(g: &Graph, id: &EntityId) -> Result<Vec<EdgeID>, EntityNotFoundError> {
    if !g.is_exist(id) {
        return Err(EntityNotFoundError { id: *id });
    }
    let me = Pointee::EntityId(*id);
    Ok(g.iter_edges()
        .filter(|eid| g.edge(eid).map(|t| t.target == me).unwrap_or(false))
        .collect())
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;
    use crate::tests::test_utils;

    /// Test all kind of edges
    #[test]
    fn test_get_edges_1() {
        let (graph, n1, _n2, _n3, _n4, e_a, _e_b, meta_edge, edge_to_h, _h) =
            test_utils::create_sample_graph2();

        // n1 is endpoint of three edges:
        //   e_a       (n1 ↔ n2)        — edge to a node
        //   meta_edge (n1 ↔ e_b)       — edge to an edge
        //   edge_to_h (n1 ↔ h)         — edge to a hyperedge
        let edges: HashSet<_> = edges(&graph, &n1).unwrap().into_iter().collect();
        let expected: HashSet<_> = [e_a, meta_edge, edge_to_h].into_iter().collect();
        assert_eq!(edges, expected);
    }

    // The next two tests use `create_sample_graph3` and exercise
    // the directional split of `edges`. The reversed
    // `e2 = (n3 → n1)` is what makes them informative.

    #[test]
    fn test_out_edges() {
        let (graph, n1, n2, n3, e1, e2, e3, e4) = test_utils::create_sample_graph3();

        let from_n1: HashSet<_> = out_edges(&graph, &n1).unwrap().into_iter().collect();
        let from_n2: HashSet<_> = out_edges(&graph, &n2).unwrap().into_iter().collect();
        let from_n3: HashSet<_> = out_edges(&graph, &n3).unwrap().into_iter().collect();

        assert_eq!(from_n1, [e1].into_iter().collect());
        assert_eq!(from_n2, HashSet::new());
        assert_eq!(from_n3, [e2, e3, e4].into_iter().collect());
    }

    #[test]
    fn test_in_edges() {
        let (graph, n1, n2, n3, e1, e2, e3, e4) = test_utils::create_sample_graph3();

        let into_n1: HashSet<_> = in_edges(&graph, &n1).unwrap().into_iter().collect();
        let into_n2: HashSet<_> = in_edges(&graph, &n2).unwrap().into_iter().collect();
        let into_n3: HashSet<_> = in_edges(&graph, &n3).unwrap().into_iter().collect();

        assert_eq!(into_n1, [e2].into_iter().collect());
        assert_eq!(into_n2, [e1, e3, e4].into_iter().collect());
        assert_eq!(into_n3, HashSet::new());
    }
}
*/