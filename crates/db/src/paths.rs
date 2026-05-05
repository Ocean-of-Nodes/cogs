//! Path queries — algorithms that walk the graph and return a
//! sub-graph (or a yes/no answer once the stubs at the bottom are
//! filled in).
//!
//! Implemented:
//! - [`undirected_paths`] — all simple paths, direction-agnostic.
//! - [`directed_paths`] — all simple paths, direction-respecting.
//!
//! The stubs at the bottom (`reachable`, `any_path`,
//! `shortest_path`, `sheave`) are still TODO; their docs and
//! signatures are kept verbatim as design notes. When you wire one
//! up, follow the same "use only public API + apply_patch"
//! discipline as the implemented pair.
//!
//! ## How the result is built
//!
//! The implemented functions construct their result through
//! [`Graph::apply_patch`] rather than touching `Graph`'s private
//! fields. The algorithm walks the source via `iter_edges` /
//! `edge` / `obj` / `is_exist` and replays the edges-on-path as
//! `Patch::AddNode` / `Patch::AddEdge`, so the result preserves
//! the original ids.
//!
//! ## Meta-edges
//!
//! When an edge's endpoint is itself another edge, that
//! prerequisite edge is added to the result first (recursively),
//! then the dependent edge. So a path that crosses through
//! meta-edges round-trips fully — meta-edges are NOT dropped from
//! the result.
//!
//! ## Hyperedges (current limitation)
//!
//! `apply_patch` doesn't yet handle `CreateHyperEdge`. If a
//! path's traversal somehow involves a hyperedge as an endpoint
//! (currently impossible because path traversal only follows
//! `Graph::edges`, not hyperedges), the hyperedge would be
//! silently dropped. Once `apply_patch` learns
//! `Patch::CreateHyperEdge`, [`ensure_in_result`] should be
//! extended to handle it.

use std::collections::{HashMap, HashSet};

use crate::*;

/// All simple undirected paths from `lhs` to `rhs`, packed into a
/// single sub-graph (union of nodes and edges that lie on at least
/// one path). Direction is ignored during traversal but each edge
/// keeps its original `(source, target)` orientation in the result.
///
/// "Simple" means each node appears at most once on a single path.
/// Parallel edges between the same pair of nodes contribute distinct
/// paths and all show up in the result.
///
/// If `lhs == rhs`, the result is a singleton graph with just
/// `lhs`. If no path exists, the result is empty.
pub fn undirected_paths(g: &Graph, lhs: NodeId, rhs: NodeId) -> Graph {
    let mut result = Graph::default();

    if lhs == rhs {
        if let Some(obj) = g.obj(&lhs) {
            let _ = result.apply_patch(Patch::AddNode {
                id: lhs,
                obj: obj.clone(),
            });
        }
        return result;
    }

    let lhs_p = Pointee::EntityId(lhs);
    let rhs_p = Pointee::EntityId(rhs);

    // Adjacency: each edge contributes BOTH directions so DFS can
    // traverse against the original `source → target`.
    let mut adj: HashMap<Pointee, Vec<(EdgeID, Pointee)>> = HashMap::new();
    for eid in g.iter_edges() {
        if let Ok(t) = g.edge(&eid) {
            adj.entry(t.source.clone())
                .or_default()
                .push((eid, t.target.clone()));
            adj.entry(t.target.clone())
                .or_default()
                .push((eid, t.source.clone()));
        }
    }

    let mut visited: HashSet<Pointee> = HashSet::from([lhs_p.clone()]);
    let mut stack: Vec<EdgeID> = Vec::new();

    fn dfs(
        cur: &Pointee,
        target: &Pointee,
        g: &Graph,
        adj: &HashMap<Pointee, Vec<(EdgeID, Pointee)>>,
        visited: &mut HashSet<Pointee>,
        stack: &mut Vec<EdgeID>,
        result: &mut Graph,
    ) {
        let Some(outgoing) = adj.get(cur) else {
            return;
        };
        for (eid, next) in outgoing {
            if visited.contains(next) {
                continue;
            }
            stack.push(*eid);
            if next == target {
                record_path(stack, g, result);
            } else {
                visited.insert(next.clone());
                dfs(next, target, g, adj, visited, stack, result);
                visited.remove(next);
            }
            stack.pop();
        }
    }

    dfs(
        &lhs_p, &rhs_p, g, &adj, &mut visited, &mut stack, &mut result,
    );

    result
}

/// All simple directed paths from `lhs` to `rhs`, packed into a
/// single sub-graph. Direction is honoured (`source → target` only);
/// hyperedges are ignored.
///
/// "Simple" means no node is visited twice on a single path —
/// cycles never blow up the search. Worst-case complexity is
/// exponential in the number of nodes (this is a heavy operation
/// on dense graphs).
///
/// If `lhs == rhs`, the result is a singleton graph with just
/// `lhs`. If no path exists, the result is empty.
pub fn directed_paths(g: &Graph, lhs: NodeId, rhs: NodeId) -> Graph {
    let mut result = Graph::default();

    if lhs == rhs {
        if let Some(obj) = g.obj(&lhs) {
            let _ = result.apply_patch(Patch::AddNode {
                id: lhs,
                obj: obj.clone(),
            });
        }
        return result;
    }

    let lhs_p = Pointee::EntityId(lhs);
    let rhs_p = Pointee::EntityId(rhs);

    let mut adj: HashMap<Pointee, Vec<(EdgeID, Pointee)>> = HashMap::new();
    for eid in g.iter_edges() {
        if let Ok(t) = g.edge(&eid) {
            adj.entry(t.source.clone())
                .or_default()
                .push((eid, t.target.clone()));
        }
    }

    let mut visited: HashSet<Pointee> = HashSet::from([lhs_p.clone()]);
    let mut stack: Vec<EdgeID> = Vec::new();

    fn dfs(
        cur: &Pointee,
        target: &Pointee,
        g: &Graph,
        adj: &HashMap<Pointee, Vec<(EdgeID, Pointee)>>,
        visited: &mut HashSet<Pointee>,
        stack: &mut Vec<EdgeID>,
        result: &mut Graph,
    ) {
        let Some(outgoing) = adj.get(cur) else {
            return;
        };
        for (eid, next) in outgoing {
            if visited.contains(next) {
                continue;
            }
            stack.push(*eid);
            if next == target {
                record_path(stack, g, result);
            } else {
                visited.insert(next.clone());
                dfs(next, target, g, adj, visited, stack, result);
                visited.remove(next);
            }
            stack.pop();
        }
    }

    dfs(
        &lhs_p, &rhs_p, g, &adj, &mut visited, &mut stack, &mut result,
    );

    result
}

/// Replay the edges in `stack` into `result` via `apply_patch`,
/// preserving original ids. Each edge's endpoints are added first
/// (recursively for meta-edges), then the edge itself.
///
/// `apply_patch` errors are ignored: the only ones we expect are
/// "already exists", which just means another path already added
/// that entity — exactly what we want.
fn record_path(stack: &[EdgeID], g: &Graph, result: &mut Graph) {
    for eid in stack {
        if let Ok(t) = g.edge(eid) {
            ensure_in_result(&t.source, g, result);
            ensure_in_result(&t.target, g, result);
            let _ = result.apply_patch(Patch::AddEdge {
                id: t.id,
                source: t.source,
                target: t.target,
            });
        }
    }
}

/// Make sure `p` is present in `result`. If `p` is itself an edge
/// (a meta-edge endpoint) or a hyperedge, recursively ensure its
/// own dependencies (`source` / `target` for an edge, `members`
/// for a hyperedge) are present first, then add it. That keeps
/// `apply_patch` happy — `is_pointee_exist` checks inside
/// `__add_edge_with_id` pass because the dependency is already
/// in `result`.
///
/// `Pointee::Path` is intentionally a no-op: sub-objects don't
/// have an independent existence in the storage layer; their
/// containing entity carries them.
fn ensure_in_result(p: &Pointee, g: &Graph, result: &mut Graph) {
    let Pointee::EntityId(id) = p else {
        return;
    };

    if result.is_exist(id) {
        return;
    }

    // Edge case: meta-edges have to come in dependency order.
    if let Ok(t) = g.edge(id) {
        ensure_in_result(&t.source, g, result);
        ensure_in_result(&t.target, g, result);
        let _ = result.apply_patch(Patch::AddEdge {
            id: t.id,
            source: t.source,
            target: t.target,
        });
        return;
    }

    // Hyperedge case: ensure each member exists, then create.
    if let Some(members) = g.hyperedge_members(id) {
        let members = members.clone();
        for m in &members {
            ensure_in_result(m, g, result);
        }
        let _ = result.apply_patch(Patch::CreateHyperEdge {
            id: *id,
            members,
        });
        return;
    }

    // Otherwise: a node (or attached object) — add as a node.
    if let Some(obj) = g.obj(id) {
        let _ = result.apply_patch(Patch::AddNode {
            id: *id,
            obj: obj.clone(),
        });
    }
}

/* TODO design notes — kept verbatim from the earlier scratch.
   Implement these on top of the same public-API + apply_patch
   discipline as `directed_paths` / `undirected_paths` above.

/// Returns `true` if a path exists between `source` and `target`
/// when traversal may freely cross between nodes and edges.
///
/// An entity is incident with another whenever an edge — regular
/// or meta — connects them. Unlike [`Graph::is_existing_path`],
/// which stays within entities of one kind, `is_linked` treats
/// nodes and edges uniformly: a node reaches an adjacent edge
/// through that edge's endpoints, and an edge reaches another
/// edge through any meta-edge between them.
///
/// As a consequence, `is_linked` is at least as permissive as
/// `is_existing_path` — every same-kind path is also a cross-kind
/// path, but not every cross-kind path is same-kind.
///
/// # Example
///
/// ```text
///   n1 --(e1)--> n2
///         ^
///         |
///        (e3)
///         |
///   n3 --(e2)--> n4
///         ^
///         |
///        (e5)
///         |
///   n5 --(e4)--> n6 --(e6)--> n7
/// ```
///
/// - `reachable(e1, e4)` → `Ok(true)`  — via `e1 — e3 — e2 — e5 — e4`
///   (same route as `is_existing_path`).
/// - `reachable(n5, n7)` → `Ok(true)`  — via `n5 — e4 — n6 — e6 — n7`
///   (same route as `is_existing_path`).
/// - `reachable(e1, n6)` → `Ok(true)`  — via
///   `e1 — e3 — e2 — e5 — e4 — n6`. The same query is `Ok(false)`
///   for [`Graph::is_existing_path`], which forbids crossing
///   between an edge and a node.
///
/// # Errors
///
/// Returns [`MissingEndpointsError`] if `source` or `target` does
/// not exist anywhere in this graph or its subgraphs.
pub fn reachable(
    g: &Graph,
    source: &EntityId,
    target: &EntityId,
) -> Result<bool, MissingEndpointsError> {
    self.__ensure_endpoints_exist(source, target)?;

    unimplemented!()
}

// pub fn drilling_reachable()
//

/// Returns `true` if a path exists between `source` and `target`
/// while staying within entities of the **same kind** — that is,
/// node-to-node or edge-to-edge, but never crossing between them.
///
/// Two nodes count as connected when an edge has them as its
/// endpoints. Two edges count as connected when a meta-edge — an
/// edge whose endpoints are themselves edges — links them. A
/// mixed query (one node and one edge) therefore always returns
/// `Ok(false)`: there is no same-kind path between them by
/// definition. Use [`Graph::is_linked`] for connectivity that may
/// freely traverse both nodes and edges.
///
/// # Example
///
/// ```text
///   n1 --(e1)--> n2
///         ^
///         |
///        (e3)
///         |
///   n3 --(e2)--> n4
///         ^
///         |
///        (e5)
///         |
///   n5 --(e4)--> n6 --(e6)--> n7
/// ```
///
/// - `any_path(e1, e4)` → `Ok(true)`  — via the meta-edge
///   chain `e1 — e3 — e2 — e5 — e4`.
/// - `any_path(n5, n7)` → `Ok(true)`  — via the node
///   chain `n5 — e4 — n6 — e6 — n7`.
/// - `any_path(e1, n6)` → `Ok(false)` — an edge and a
///   node are never on the same-kind path.
///
/// # Errors
///
/// Returns [`MissingEndpointsError`] if `source` or `target` does
/// not exist anywhere in this graph or its subgraphs.
pub fn any_path(
    &self,
    source: &EntityId,
    target: &EntityId,
) -> Result<bool, MissingEndpointsError> {
    /// Check that `source` and `target` exist.
    /// Returns [`MissingEndpointsError`] with unexist endpoints
    fn __ensure_endpoints_exist(
        &self,
        source: &EntityId,
        target: &EntityId,
    ) -> Result<(), MissingEndpointsError> {
        let mut missing_endpoints = Vec::new();
        if !self.is_exist(source) {
            missing_endpoints.push(*source);
        }
        if !self.is_exist(target) {
            missing_endpoints.push(*target);
        }

        if missing_endpoints.is_empty() {
            Ok(())
        } else {
            Err(MissingEndpointsError { missing_endpoints })
        }
    }

    self.__ensure_endpoints_exist(source, target)?;

    if self.is_node(source) && self.is_node(target) {
        // Check node-to-node path
        unimplemented!()
    } else if self.is_edge(source) && self.is_edge(target) {
        // Check edge-to-edge path
        unimplemented!()
    } else {
        // Mixed query: one node and one edge
        Ok(false)
    }
}

fn shortest_path() {

}

/// `Sheave` is a bunch of `links` between two `Graph`s.
///
/// A sheave bundles cross-graph edges (and any meta-edges built
/// on top of them) into a single object that lives outside the
/// two graphs it connects.
///
/// ```text
/// +---- lhs graph ----+                +---- rhs graph ----+
/// |                   |                |                   |
/// |  n1 ---(a)--- n2  |                |  m1 ---(x)--- m2  |
/// |         |         |                |         |         |
/// |        (b)        |                |        (y)        |
/// |         |         |                |         |         |
/// |         n3        |                |         m3        |
/// |                   |                |                   |
/// +-------------------+                +-------------------+
///          :                                    :
///          :   n2 ----(L1)---------------- m1   :
///          :              ^                     :
///          :             (M)  <- meta-edge      :
///          :              v                     :
///          :   n3 ----(L2)---------------- m3   :
///          :                                    :
///           \-------------- sheave -------------/
/// ```
///
/// In the picture above, `a`, `b`, `x`, `y` are internal edges
/// of `lhs` and `rhs` and stay inside their respective graphs.
/// `L1` and `L2` are regular edges of the sheave that cross the
/// boundary between the two graphs. `M` is a meta-edge whose
/// endpoints are themselves sheave edges (`L1` and `L2`).
pub fn sheave(graph: &Graph, lhs: &Graph, rhs: &Graph) -> &mut Graph {
    unimplemented!()
}
*/

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;
    use crate::tests::test_utils;

    /// Direction is ignored, so even though `e2` is recorded
    /// `n3 → n1`, the undirected walk uses it as `n1 → n3`.
    /// All four edges should appear in the result.
    #[test]
    fn test_undirected_paths() {
        let (graph, n1, n2, n3, e1, e2, e3, e4) = test_utils::create_semple_graph3();

        let result = undirected_paths(&graph, n1, n2);

        let result_nodes: HashSet<_> = result.iter_nodes().collect();
        let result_edges: HashSet<_> = result.iter_edges().collect();

        assert_eq!(result_nodes, [n1, n2, n3].into_iter().collect());
        assert_eq!(result_edges, [e1, e2, e3, e4].into_iter().collect());
    }

    /// Direction matters. The only `n1 → n2` path is `e1`:
    /// `e2` goes `n3 → n1` (wrong way), so we never reach `n3`
    /// from `n1`, and `e3`/`e4` are unreachable.
    #[test]
    fn test_directed_paths() {
        let (graph, n1, n2, _n3, e1, _e2, _e3, _e4) = test_utils::create_semple_graph3();

        let result = directed_paths(&graph, n1, n2);

        let result_nodes: HashSet<_> = result.iter_nodes().collect();
        let result_edges: HashSet<_> = result.iter_edges().collect();

        assert_eq!(result_nodes, [n1, n2].into_iter().collect());
        assert_eq!(result_edges, [e1].into_iter().collect());
    }
}
