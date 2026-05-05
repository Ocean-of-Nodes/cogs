//! Node-side incidence: which entities does `id` connect to?
//!
//! Algorithms over the public read API of [`Graph`] — none of these
//! need privileged field access.

use std::collections::HashSet;

use crate::*;

/// Entities directly connected to `id` — the *other* endpoint of
/// every incident edge or hyperedge member alongside `id`. Direction
/// is ignored; hyperedge co-members are included. Result is
/// deduplicated.
pub fn neighbours(g: &Graph, id: &EntityId) -> Result<Vec<Pointee>, EntityNotFoundError> {
    if !g.is_exist(id) {
        return Err(EntityNotFoundError(*id));
    }

    let me = Pointee::EntityId(*id);
    let mut out: HashSet<Pointee> = HashSet::new();

    for eid in g.iter_edges() {
        if let Ok(t) = g.edge(&eid) {
            if t.source == me {
                out.insert(t.target);
            } else if t.target == me {
                out.insert(t.source);
            }
        }
    }

    for hid in g.iter_hyperedge() {
        if let Some(members) = g.hyperedge_members(&hid) {
            if members.contains(&me) {
                for m in members {
                    if m != &me {
                        out.insert(m.clone());
                    }
                }
            }
        }
    }

    Ok(out.into_iter().collect())
}

/// Endpoints reachable through outgoing edges from `id` — for
/// every edge with `source == id`, the `target`. Hyperedges are
/// undirected and not included.
pub fn out_neighbours(g: &Graph, id: &EntityId) -> Result<Vec<Pointee>, EntityNotFoundError> {
    if !g.is_exist(id) {
        return Err(EntityNotFoundError(*id));
    }

    let me = Pointee::EntityId(*id);
    let mut out: HashSet<Pointee> = HashSet::new();

    for eid in g.iter_edges() {
        if let Ok(t) = g.edge(&eid) {
            if t.source == me {
                out.insert(t.target);
            }
        }
    }

    Ok(out.into_iter().collect())
}

/// Endpoints from which incoming edges arrive at `id` — for every
/// edge with `target == id`, the `source`. Hyperedges are
/// undirected and not included.
pub fn in_neighbours(g: &Graph, id: &EntityId) -> Result<Vec<Pointee>, EntityNotFoundError> {
    if !g.is_exist(id) {
        return Err(EntityNotFoundError(*id));
    }

    let me = Pointee::EntityId(*id);
    let mut inc: HashSet<Pointee> = HashSet::new();

    for eid in g.iter_edges() {
        if let Ok(t) = g.edge(&eid) {
            if t.target == me {
                inc.insert(t.source);
            }
        }
    }

    Ok(inc.into_iter().collect())
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;
    use crate::tests::test_utils;

    /// A simple case
    #[test]
    fn test_neighbours1() {
        // Built graph:
        // ```text
        //  node1 --(edge)--> node2
        //   ^
        //   |
        // (edge2)
        //   |
        //  node3
        // ```
        let mut graph = Graph::default();

        let field1 = test_utils::create_simple_obj("filed1");
        let field2 = test_utils::create_simple_obj("filed2");
        let field3 = test_utils::create_simple_obj("filed3");
        let node_id1 = graph.add_node(field1);
        let node_id2 = graph.add_node(field2);
        let node_id3 = graph.add_node(field3);
        graph.add_edge(node_id1, node_id2).unwrap();
        graph.add_edge(node_id3, node_id1).unwrap();

        let neighbours = neighbours(&graph, &node_id1).unwrap();
        assert_eq!(neighbours.len(), 2);
        assert!(neighbours.contains(&Pointee::EntityId(node_id2)));
        assert!(neighbours.contains(&Pointee::EntityId(node_id3)));
    }

    /// A more complex case with edge beetween edges
    /// Test that get_neighbours will return only nodes,
    /// but not edges, even if edge is beetween two edges
    #[test]
    fn test_neighbours2() {
        // Built graph:
        // ```text
        //  node1 --(edge1)--> node2
        //            |
        //          (edge3) < -- (edge4) -- node5
        //            |
        //            v
        //  node3 --(edge2)--> node4
        // ```
        let mut graph = Graph::default();

        let field1 = test_utils::create_simple_obj("node1");
        let field2 = test_utils::create_simple_obj("node2");
        let node_id1 = graph.add_node(field1);
        let node_id2 = graph.add_node(field2);
        let edge1 = graph.add_edge(node_id1, node_id2).unwrap();

        let field3 = test_utils::create_simple_obj("node3");
        let field4 = test_utils::create_simple_obj("node4");
        let node_id3 = graph.add_node(field3);
        let node_id4 = graph.add_node(field4);
        let edge2 = graph.add_edge(node_id3, node_id4).unwrap();

        let edge3 = graph.add_edge(edge1, edge2).unwrap();

        let field5 = test_utils::create_simple_obj("node5");
        let node_id5 = graph.add_node(field5);
        let _edge4 = graph.add_edge(node_id5, edge3).unwrap();

        let neighbours = neighbours(&graph, &edge3).unwrap();
        assert_eq!(neighbours.len(), 1);
        assert!(neighbours.contains(&Pointee::EntityId(node_id5)));
    }

    /// Test node connected to node\edge\hyperedge
    #[test]
    fn test_neighbours3() {
        let (graph, n1, n2, _, _, e_a, e_b, _, _, h) = test_utils::create_semple_graph2();
        let neighbours: HashSet<_> =
            neighbours(&graph, &n1).unwrap().into_iter().collect();

        let expected: HashSet<_> = [
            Pointee::EntityId(n2),
            Pointee::EntityId(e_b),
            Pointee::EntityId(h),
        ]
        .into_iter()
        .collect();
        assert_eq!(neighbours, expected);

        // Sanity: the bridging edge `e_a` itself is a path,
        // not a destination, so it must not appear.
        assert!(!neighbours.contains(&Pointee::EntityId(e_a)));
    }

    /// `id` must not appear in its own neighbours list when
    /// it is a member of a hyperedge that includes it.
    /// Guards specifically against a regression where the
    /// iteration over hyperedge members forgets the
    /// `m != id` skip.
    #[test]
    fn test_neighbours4() {
        // Built graph:
        // ```text
        //   h = {n1, n2}
        // ```
        let mut graph = Graph::default();
        let obj = test_utils::create_simple_obj("test_field");

        let n1 = graph.add_node(obj.clone());
        let n2 = graph.add_node(obj.clone());

        let mut m  = HashSet::new();
        m.insert(n1.into());
        m.insert(n2.into());

        let _h = graph.create_hyperedge(m);

        let neighbours: HashSet<_> = neighbours(&graph, &n1).unwrap().into_iter().collect();

        assert!(
            !neighbours.contains(&Pointee::EntityId(n1)),
            "n1 must not be listed as its own neighbour"
        );
        let expected: HashSet<_> = [Pointee::EntityId(n2)].into_iter().collect();
        assert_eq!(neighbours, expected);
    }

    #[test]
    fn test_out_neighbours() {
        let (graph, n1, n2, n3, _e1, _e2, _e3, _e4) = test_utils::create_semple_graph3();

        let from_n1: HashSet<_> =
            out_neighbours(&graph, &n1).unwrap().into_iter().collect();
        let from_n2: HashSet<_> =
            out_neighbours(&graph, &n2).unwrap().into_iter().collect();
        let from_n3: HashSet<_> =
            out_neighbours(&graph, &n3).unwrap().into_iter().collect();

        // n1 → n2 only.
        assert_eq!(from_n1, [Pointee::from(n2)].into_iter().collect());
        // n2 has no outgoing.
        assert_eq!(from_n2, HashSet::new());
        // n3 → n1 (via e2), n3 → n2 (via e3 and e4 — dedupes).
        assert_eq!(
            from_n3,
            [Pointee::from(n1), Pointee::from(n2)].into_iter().collect()
        );
    }

    #[test]
    fn test_in_neighbours() {
        let (graph, n1, n2, n3, _e1, _e2, _e3, _e4) = test_utils::create_semple_graph3();

        let into_n1: HashSet<_> =
            in_neighbours(&graph, &n1).unwrap().into_iter().collect();
        let into_n2: HashSet<_> =
            in_neighbours(&graph, &n2).unwrap().into_iter().collect();
        let into_n3: HashSet<_> =
            in_neighbours(&graph, &n3).unwrap().into_iter().collect();

        // n3 → n1.
        assert_eq!(into_n1, [Pointee::from(n3)].into_iter().collect());
        // n1 → n2 (via e1), n3 → n2 (via e3 and e4 — dedupes).
        assert_eq!(
            into_n2,
            [Pointee::from(n1), Pointee::from(n3)].into_iter().collect()
        );
        // n3 has no incoming.
        assert_eq!(into_n3, HashSet::new());
    }
}