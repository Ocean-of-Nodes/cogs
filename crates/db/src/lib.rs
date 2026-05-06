//! Persistent graph storage for the engine.
//!
//! ## Module layout
//!
//! - [`errors`] — every error type returned by `Graph` operations.
//! - [`types`] — domain taxonomy: `EntityType`, `PointeeKind`,
//!   `Triplet`.
//! - [`object_patch`] — pure helpers over `Object` (apply / diff).
//! - [`graph`] — the `Graph` struct and all its impls, split by
//!   responsibility (queries, cascade, constructors, destructors,
//!   modifiers, replay, listeners, reverse-index).

mod errors;
mod graph;
mod object_patch;
mod types;

// Re-exports for the test module — production code reaches into the
// submodules directly (`use crate::errors::*` etc.).
#[cfg(test)]
#[allow(unused_imports)]
pub(crate) use errors::*;
#[cfg(test)]
#[allow(unused_imports)]
pub(crate) use graph::Graph;
#[cfg(test)]
#[allow(unused_imports)]
pub(crate) use types::{EntityType, PointeeKind, Triplet};

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};

    use uuid::Uuid;

    use common::*;

    use super::*;

    pub(crate) mod test_utils {
        use super::*;

        pub fn create_simple_obj(field_name: &str) -> Object {
            let mut obj = Object::new();
            obj.insert(field_name.into(), Field::Null);
            obj
        }

        /// Cross-check `pointee_uses` and `entity_to_path_pointees`
        /// against `edges` / `hyper_edge`. Panics on the first
        /// inconsistency. Call after any mutation to assert the
        /// reverse-index invariants are intact.
        pub fn check_index_invariant(g: &Graph) {
            // 1) Every Pointee::Path key in pointee_uses must be
            //    tracked in entity_to_path_pointees under its entity.
            for key in g.pointee_uses.keys() {
                if let Pointee::Path(gp) = key {
                    let tracked = g
                        .entity_to_path_pointees
                        .get(&gp.entity())
                        .is_some_and(|s| s.contains(key));
                    assert!(
                        tracked,
                        "path pointee {:?} present in pointee_uses but missing \
                         from entity_to_path_pointees",
                        key
                    );
                }
            }

            // 2) entity_to_path_pointees has no stale or empty entries.
            for (entity, paths) in &g.entity_to_path_pointees {
                assert!(
                    !paths.is_empty(),
                    "entity_to_path_pointees[{}] is empty (should have been removed)",
                    entity
                );
                for p in paths {
                    assert!(
                        g.pointee_uses.contains_key(p),
                        "stale entry in entity_to_path_pointees[{}]: {:?} not in pointee_uses",
                        entity,
                        p
                    );
                }
            }

            // 3) Every edge has both endpoints registered in pointee_uses.
            for (eid, (src, tgt)) in &g.edges {
                let src_ok = g
                    .pointee_uses
                    .get(src)
                    .is_some_and(|b| b.edges_as_source.contains(eid));
                assert!(src_ok, "edge {} not registered in source bucket {:?}", eid, src);
                let tgt_ok = g
                    .pointee_uses
                    .get(tgt)
                    .is_some_and(|b| b.edges_as_target.contains(eid));
                assert!(tgt_ok, "edge {} not registered in target bucket {:?}", eid, tgt);
            }

            // 4) Every hyperedge member has the hyperedge registered.
            for (hid, members) in &g.hyper_edge {
                for m in members {
                    let ok = g
                        .pointee_uses
                        .get(m)
                        .is_some_and(|b| b.hyperedges.contains(hid));
                    assert!(ok, "hyperedge {} not registered in member bucket {:?}", hid, m);
                }
            }

            // 5) No empty buckets — they must have been removed.
            for (k, b) in &g.pointee_uses {
                assert!(
                    !b.is_empty(),
                    "pointee_uses[{:?}] is empty (should have been removed)",
                    k
                );
            }

            // 6) Reverse direction: every (eid, source/target/hyperedge)
            //    in pointee_uses corresponds to a live structural element.
            for (pointee, uses) in &g.pointee_uses {
                for eid in &uses.edges_as_source {
                    let edge = g.edges.get(eid);
                    assert!(
                        edge.is_some_and(|(s, _)| s == pointee),
                        "pointee_uses[{:?}].edges_as_source has stale eid {}",
                        pointee,
                        eid
                    );
                }
                for eid in &uses.edges_as_target {
                    let edge = g.edges.get(eid);
                    assert!(
                        edge.is_some_and(|(_, t)| t == pointee),
                        "pointee_uses[{:?}].edges_as_target has stale eid {}",
                        pointee,
                        eid
                    );
                }
                for hid in &uses.hyperedges {
                    let members = g.hyper_edge.get(hid);
                    assert!(
                        members.is_some_and(|ms| ms.contains(pointee)),
                        "pointee_uses[{:?}].hyperedges has stale hid {}",
                        pointee,
                        hid
                    );
                }
            }
        }

        // Built graph:
        // ```text
        //  ---------
        //  | n1 ---|----e1---n2
        //  |       |    |    |
        //  |       |    |    |
        //  |       |    e3---e4-----
        //  |       |    |          |
        //  | n3 ---|---e2----n4    |
        //  --h------               |
        //    |                     |
        //    |----------------------
        // ````
        pub fn create_sample_graph1() -> (
            Graph,
            NodeId,
            NodeId,
            NodeId,
            NodeId,
            EdgeID,
            EdgeID,
            EdgeID,
            EdgeID,
            EdgeID,
            HyperEdgeId,
        ) {
            let mut g = Graph::default();
            let obj = create_simple_obj("test_field");
            let n1 = g.add_node(obj.clone());
            let n2 = g.add_node(obj.clone());
            let n3 = g.add_node(obj.clone());
            let n4 = g.add_node(obj.clone());

            let e1 = g.add_edge(n1, n2).unwrap();
            let e2 = g.add_edge(n3, n4).unwrap();
            let e3 = g.add_edge(e1, e2).unwrap();
            let e4 = g.add_edge(e3, n2).unwrap();

            let mut m = HashSet::new();
            m.insert(n1.into());
            m.insert(n3.into());

            let h = g.create_hyperedge(m).unwrap();
            let e5 = g.add_edge(h, e4).unwrap();

            (g, n1, n2, n3, n4, e1, e2, e3, e4, e5, h)
        }

        // Built graph:
        // ```text
        //  n1 ---- e_a ----- n2
        //  |\
        //  |  ----------------
        //  |                 |
        //  edge_to_h     meta_edge
        //  |                 |
        //  -------           |     --------
        //  |  n3-|----------e_b----|---n4 |
        //  |     |-----------------|      |
        //  |                              |
        //  |-----------h------------------|
        // ```
        pub fn create_sample_graph2() -> (
            Graph,
            NodeId,
            NodeId,
            NodeId,
            NodeId,
            EdgeID,
            EdgeID,
            EdgeID,
            EdgeID,
            HyperEdgeId,
        ) {
            let mut graph = Graph::default();
            let obj = test_utils::create_simple_obj("test_field");

            let n1 = graph.add_node(obj.clone());
            let n2 = graph.add_node(obj.clone());
            let n3 = graph.add_node(obj.clone());
            let n4 = graph.add_node(obj.clone());

            let e_a = graph.add_edge(n1, n2).unwrap();
            let e_b = graph.add_edge(n3, n4).unwrap();
            let meta_edge = graph.add_edge(n1, e_b).unwrap();

            let mut m = HashSet::new();
            m.insert(n3.into());
            m.insert(n4.into());

            let h = graph.create_hyperedge(m).unwrap();
            let edge_to_h = graph.add_edge(n1, h).unwrap();

            (graph, n1, n2, n3, n4, e_a, e_b, meta_edge, edge_to_h, h)
        }

        /// Built graph (note: `e2` is intentionally directed `n3 → n1`,
        /// not `n1 → n3`):
        ///
        /// ```text
        ///
        ///  n1 ----------- e1 ---------- n2
        ///   ^                          / |
        ///    \         /----- e3 -----   |
        ///     -- e2 - n3 -------- e4 ----
        /// ```
        pub fn create_sample_graph3() -> (
            Graph,
            NodeId,
            NodeId,
            NodeId,
            EdgeID,
            EdgeID,
            EdgeID,
            EdgeID,
        ) {
            let mut graph = Graph::default();
            let obj = test_utils::create_simple_obj("attached");

            let n1 = graph.add_node(obj.clone());
            let n2 = graph.add_node(obj.clone());
            let n3 = graph.add_node(obj.clone());

            let e1 = graph.add_edge(n1, n2).unwrap();
            let e2 = graph.add_edge(n3, n1).unwrap(); // reversed on purpose
            let e3 = graph.add_edge(n3, n2).unwrap();
            let e4 = graph.add_edge(n3, n2).unwrap(); // parallel to e3

            (graph, n1, n2, n3, e1, e2, e3, e4)
        }
    }

    mod test_globals {
        use super::*;

        mod test_iter_entities {
            use super::*;

            /// Iterator yields nodes, edges, and hyperedges all together.
            #[test]
            fn yields_all_entity_kinds() {
                let (g, n1, n2, n3, n4, e1, e2, e3, e4, e5, h) = test_utils::create_sample_graph1();

                let mut expected = HashSet::new();
                expected.insert(n1);
                expected.insert(n2);
                expected.insert(n3);
                expected.insert(n4);
                expected.insert(e1);
                expected.insert(e2);
                expected.insert(e3);
                expected.insert(e4);
                expected.insert(e5);
                expected.insert(h);

                let actual: Vec<_> = g.iter_entities().collect();

                // 1. No duplicates: every entity appears at most once.
                let mut counts: HashMap<EntityId, usize> = HashMap::new();
                for e in &actual {
                    *counts.entry(*e).or_insert(0) += 1;
                }
                let duplicates: Vec<_> = counts
                    .iter()
                    .filter(|(_, c)| **c > 1)
                    .map(|(e, c)| (*e, *c))
                    .collect();
                if !duplicates.is_empty() {
                    panic!("global_entities returned duplicates: {:?}", duplicates);
                }

                // 2. Coverage matches exactly: no missing, no extras.
                let actual_set: HashSet<_> = actual.iter().copied().collect();
                let missing: Vec<_> = expected.difference(&actual_set).copied().collect();
                let unexpected: Vec<_> = actual_set.difference(&expected).copied().collect();
                if !missing.is_empty() || !unexpected.is_empty() {
                    panic!(
                        "global_entities mismatch — missing: {:?}, unexpected: {:?}",
                        missing, unexpected
                    );
                }
            }

            /// Verify deduplication: an id that lives in more than one
            /// storage map must be yielded by `iter_entities` only
            /// once.
            ///
            /// Setup creates two cross-map collisions on purpose:
            /// - `e1` lives as an edge *and*, after `attach_obj`, as
            ///   an entity (object attached to that edge).
            /// - `h`  lives as a hyperedge *and*, after `attach_obj`,
            ///   as an entity.
            ///
            /// If `iter_entities` ever stops deduplicating (e.g.
            /// someone removes the trailing `collect::<HashSet<_>>()`),
            /// `e1` and `h` would each show up twice and this test
            /// catches it.
            #[test]
            fn deduplicates_attached_target_ids() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("test_field");
                let n1 = g.add_node(obj.clone());
                let n2 = g.add_node(obj.clone());

                let e1 = g.add_edge(n1, n2).unwrap();
                g.attach_obj(e1, obj.clone()).unwrap();

                let mut m = HashSet::new();
                m.insert(n1.into());
                m.insert(n2.into());

                let h = g.create_hyperedge(m).unwrap();
                g.attach_obj(h, obj.clone()).unwrap();

                let actual: Vec<_> = g.iter_entities().collect();

                // 1. No id appears more than once — in particular,
                //    e1 and h, which sit in two maps each.
                let mut counts: HashMap<EntityId, usize> = HashMap::new();
                for id in &actual {
                    *counts.entry(*id).or_insert(0) += 1;
                }
                assert_eq!(
                    counts.get(&e1).copied(),
                    Some(1),
                    "e1 yielded {:?} times, expected 1",
                    counts.get(&e1)
                );
                assert_eq!(
                    counts.get(&h).copied(),
                    Some(1),
                    "h yielded {:?} times, expected 1",
                    counts.get(&h)
                );

                // 2. Coverage: exactly the four distinct ids.
                let actual_set: HashSet<_> = actual.iter().copied().collect();
                let expected: HashSet<_> = [n1, n2, e1, h].into_iter().collect();
                assert_eq!(actual_set, expected);
            }
        }

        mod test_iter_edges {
            use super::*;

            #[test]
            fn yields_all_edges_in_sample_graph() {
                let (g, _n1, _n2, _n3, _n4, e1, e2, e3, e4, e5, _h) =
                    test_utils::create_sample_graph1();

                let actual: Vec<_> = g.iter_edges().collect();

                // No duplicates (HashMap keys can't repeat, but we
                // assert anyway in case the impl changes).
                let mut counts: HashMap<EdgeID, usize> = HashMap::new();
                for id in &actual {
                    *counts.entry(*id).or_insert(0) += 1;
                }
                let duplicates: Vec<_> = counts
                    .iter()
                    .filter(|(_, c)| **c > 1)
                    .map(|(e, c)| (*e, *c))
                    .collect();
                if !duplicates.is_empty() {
                    panic!("iter_edges returned duplicates: {:?}", duplicates);
                }

                // Exactly the five edges added (e1..e5). Nodes and
                // the hyperedge `h` must NOT appear.
                let actual_set: HashSet<_> = actual.iter().copied().collect();
                let expected: HashSet<_> = [e1, e2, e3, e4, e5].into_iter().collect();
                assert_eq!(actual_set, expected);
            }
        }

        mod test_iter_nodes {
            use super::*;

            #[test]
            fn iter_nodes() {
                let (g, n1, n2, n3, n4, _e1, _e2, _e3, _e4, _e5, _h) =
                    test_utils::create_sample_graph1();

                let actual: Vec<_> = g.iter_nodes().collect();

                // No duplicates — entities is a HashMap, but assert
                // anyway in case the impl chains in extra sources later.
                let mut counts: HashMap<NodeId, usize> = HashMap::new();
                for id in &actual {
                    *counts.entry(*id).or_insert(0) += 1;
                }
                let duplicates: Vec<_> = counts
                    .iter()
                    .filter(|(_, c)| **c > 1)
                    .map(|(e, c)| (*e, *c))
                    .collect();
                if !duplicates.is_empty() {
                    panic!("iter_nodes returned duplicates: {:?}", duplicates);
                }

                // Exactly the four node ids — edges (e1..e5) and the
                // hyperedge `h` must NOT appear.
                let actual_set: HashSet<_> = actual.iter().copied().collect();
                let expected: HashSet<_> = [n1, n2, n3, n4].into_iter().collect();
                assert_eq!(actual_set, expected);
            }
        }

        mod test_iter_hyperedge {
            use super::*;

            #[test]
            fn test_iter_hyperedge() {
                let (g, _n1, _n2, _n3, _n4, _e1, _e2, _e3, _e4, _e5, h) =
                    test_utils::create_sample_graph1();

                let actual: Vec<_> = g.iter_hyperedge().collect();

                // No duplicates — `hyper_edge` is a HashMap, but
                // assert anyway in case the impl chains in extra
                // sources later.
                let mut counts: HashMap<HyperEdgeId, usize> = HashMap::new();
                for id in &actual {
                    *counts.entry(*id).or_insert(0) += 1;
                }
                let duplicates: Vec<_> = counts
                    .iter()
                    .filter(|(_, c)| **c > 1)
                    .map(|(e, c)| (*e, *c))
                    .collect();
                if !duplicates.is_empty() {
                    panic!("iter_hyperedge returned duplicates: {:?}", duplicates);
                }

                // Exactly the one hyperedge — nodes and edges must
                // NOT appear.
                let actual_set: HashSet<_> = actual.iter().copied().collect();
                let expected: HashSet<_> = [h].into_iter().collect();
                assert_eq!(actual_set, expected);
            }
        }

        mod test_iter_attached {
            use super::*;

            /// No attached objects yet — only nodes live in
            /// `entities`, so `iter_attached` is empty.
            #[test]
            fn empty_when_only_nodes() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("test_field");
                g.add_node(obj.clone());
                g.add_node(obj);
                assert_eq!(g.iter_attached().count(), 0);
            }

            /// Yields exactly the ids on which `attach_obj` placed an
            /// object — both edges and hyperedges qualify.
            #[test]
            fn yields_attach_targets() {
                let (mut g, _n1, _n2, _n3, _n4, e_a, _e_b, _meta_edge, _edge_to_h, h) =
                    test_utils::create_sample_graph2();
                let obj = test_utils::create_simple_obj("attached");

                g.attach_obj(e_a, obj.clone()).unwrap();
                g.attach_obj(h, obj).unwrap();

                let actual: HashSet<_> = g.iter_attached().collect();
                let expected: HashSet<_> = [e_a, h].into_iter().collect();
                assert_eq!(actual, expected);
            }

            /// Complement of `iter_nodes`: a node id, even if it has
            /// an object, must NOT appear in `iter_attached`.
            #[test]
            fn excludes_nodes() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("test_field");
                let n1 = g.add_node(obj);
                let actual: HashSet<_> = g.iter_attached().collect();
                assert!(!actual.contains(&n1));
            }
        }
    }

    mod test_getters {
        use super::*;

        mod test_obj {
            use super::*;

            // Test all kind of object holder: an object can be
            // attached to a regular edge, a meta-edge, and an
            // edge-to-hyperedge alike. Hitting all three structural
            // shapes here exercises every `is_attach_target() == true`
            // branch of `EntityType`.
            //
            // n1 is used to verify that nodes — the "default" holder
            // (object goes in via `add_node`) — keep their object
            // unchanged after these attaches.
            #[test]
            fn test_obj() {
                let (mut graph, n1, _n2, _n3, _n4, e_a, _e_b, meta_edge, edge_to_h, _h) =
                    test_utils::create_sample_graph2();

                let obj = test_utils::create_simple_obj("attached");

                graph.attach_obj(e_a, obj.clone()).unwrap();
                graph.attach_obj(meta_edge, obj.clone()).unwrap();
                graph.attach_obj(edge_to_h, obj.clone()).unwrap();

                assert_eq!(graph.obj(&e_a), Some(&obj));
                assert_eq!(graph.obj(&meta_edge), Some(&obj));
                assert_eq!(graph.obj(&edge_to_h), Some(&obj));

                // n1 is a Node: its object came from `add_node`,
                // independent of any attach. It must still be there.
                assert!(graph.obj(&n1).is_some());
            }

            /// Unknown id resolves to `None`.
            #[test]
            fn obj_unknown() {
                let g = Graph::default();
                assert!(g.obj(&Uuid::new_v4()).is_none());
            }

            /// A bare edge (no `attach_obj` call) has no object.
            #[test]
            fn obj_bare_edge_is_none() {
                let (g, _n1, _n2, _n3, _n4, e_a, _e_b, _meta_edge, _edge_to_h, _h) =
                    test_utils::create_sample_graph2();
                assert!(g.obj(&e_a).is_none());
            }

            /// A bare hyperedge has no object.
            #[test]
            fn obj_bare_hyperedge_is_none() {
                let (g, _n1, _n2, _n3, _n4, _e_a, _e_b, _meta_edge, _edge_to_h, h) =
                    test_utils::create_sample_graph2();
                assert!(g.obj(&h).is_none());
            }
        }

        mod test_edge {
            use super::*;

            /// Regular node-to-node edge round-trips through `edge`.
            #[test]
            fn edge1() {
                let (graph, n1, n2, _n3, _n4, e_a, _e_b, _meta_edge, _edge_to_h, _h) =
                    test_utils::create_sample_graph2();

                let u = graph.edge(&e_a).unwrap();
                assert_eq!(
                    u,
                    Triplet {
                        id: e_a,
                        source: n1.into(),
                        target: n2.into()
                    }
                )
            }

            /// Meta-edge: target is another edge — still in the
            /// `edges` map, so `edge` returns its triplet.
            #[test]
            fn edge2() {
                let (graph, n1, _n2, _n3, _n4, _e_a, e_b, meta_edge, _edge_to_h, _h) =
                    test_utils::create_sample_graph2();

                let u = graph.edge(&meta_edge).unwrap();
                assert_eq!(
                    u,
                    Triplet {
                        id: meta_edge,
                        source: n1.into(),
                        target: e_b.into(),
                    }
                )
            }

            /// Edge whose target is a hyperedge — also lives in the
            /// `edges` map.
            #[test]
            fn edge3() {
                let (graph, n1, _n2, _n3, _n4, _e_a, _e_b, _meta_edge, edge_to_h, h) =
                    test_utils::create_sample_graph2();

                let u = graph.edge(&edge_to_h).unwrap();
                assert_eq!(
                    u,
                    Triplet {
                        id: edge_to_h,
                        source: n1.into(),
                        target: h.into(),
                    }
                )
            }

            /// Self-loop is a valid edge.
            #[test]
            fn edge4() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("test_field");
                let n1 = g.add_node(obj);
                let e1 = g.add_edge(n1, n1).unwrap();

                let u = g.edge(&e1).unwrap();
                assert_eq!(
                    u,
                    Triplet {
                        id: e1,
                        source: n1.into(),
                        target: n1.into(),
                    }
                )
            }

            /// Endpoints can be sub-object paths; `edge` returns
            /// them verbatim.
            #[test]
            fn edge5() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("test_field");
                let n1 = g.add_node(obj.clone());
                let n2 = g.add_node(obj);

                let p1 = Pointee::Path(GlobalObjPath::new(n1, "test_field").unwrap());
                let p2 = Pointee::Path(GlobalObjPath::new(n2, "test_field").unwrap());
                let e1 = g.add_edge(p1.clone(), p2.clone()).unwrap();

                let u = g.edge(&e1).unwrap();
                assert_eq!(
                    u,
                    Triplet {
                        id: e1,
                        source: p1,
                        target: p2,
                    }
                )
            }

            /// Unknown id → `NotFound`.
            #[test]
            fn edge_not_found() {
                let g = Graph::default();
                let id = Uuid::new_v4();
                let err = g.edge(&id).unwrap_err();
                assert!(matches!(
                    err,
                    GetEdgeError::NotFound(EntityNotFoundError { id: x }) if x == id
                ));
            }

            /// A node id is a known entity but not an edge →
            /// `IncorrectType("Node")`.
            #[test]
            fn edge_incorrect_type_node() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("test_field");
                let n1 = g.add_node(obj);

                let err = g.edge(&n1).unwrap_err();
                match err {
                    GetEdgeError::IncorrectType(e) => {
                        assert_eq!(e.entity_id, n1);
                        assert_eq!(e.actual_type, "Node");
                    }
                    other => panic!("expected IncorrectType, got {other:?}"),
                }
            }

            /// A hyperedge id → `IncorrectType("HyperEdge")`.
            #[test]
            fn edge_incorrect_type_hyperedge() {
                let (g, _n1, _n2, _n3, _n4, _e_a, _e_b, _meta_edge, _edge_to_h, h) =
                    test_utils::create_sample_graph2();

                let err = g.edge(&h).unwrap_err();
                match err {
                    GetEdgeError::IncorrectType(e) => {
                        assert_eq!(e.entity_id, h);
                        assert_eq!(e.actual_type, "HyperEdge");
                    }
                    other => panic!("expected IncorrectType, got {other:?}"),
                }
            }

            /// An attached-object id (object placed on top of a
            /// hyperedge) → `IncorrectType("AttachedObject")`. The
            /// hyperedge map lookup wins over the edges map only
            /// because attaching to a hyperedge keeps the id outside
            /// `edges`.
            #[test]
            fn edge_incorrect_type_attached() {
                let (mut g, _n1, _n2, _n3, _n4, _e_a, _e_b, _meta_edge, _edge_to_h, h) =
                    test_utils::create_sample_graph2();
                let obj = test_utils::create_simple_obj("attached");
                g.attach_obj(h, obj).unwrap();

                let err = g.edge(&h).unwrap_err();
                match err {
                    GetEdgeError::IncorrectType(e) => {
                        assert_eq!(e.entity_id, h);
                        assert_eq!(e.actual_type, "AttachedObject");
                    }
                    other => panic!("expected IncorrectType, got {other:?}"),
                }
            }
        }

        mod test_hyperedge {
            use super::*;

            #[test]
            fn hyperedge1() {
                let (graph, _n1, _n2, n3, n4, _e_a, _e_b, _meta_edge, _edge_to_h, h) =
                    test_utils::create_sample_graph2();

                let members = graph.hyperedge_members(&h).unwrap();
                let expected: HashSet<Pointee> = [n3.into(), n4.into()].into_iter().collect();
                assert_eq!(members, &expected);
            }

            /// Unknown id → None.
            #[test]
            fn hyperedge_unknown() {
                let g = Graph::default();
                assert!(g.hyperedge_members(&Uuid::new_v4()).is_none());
            }

            /// An edge id is not a hyperedge → None.
            #[test]
            fn hyperedge_for_edge_id() {
                let (g, _n1, _n2, _n3, _n4, e_a, _e_b, _meta_edge, _edge_to_h, _h) =
                    test_utils::create_sample_graph2();
                assert!(g.hyperedge_members(&e_a).is_none());
            }

            /// `hyperedge_members` returns `None` for an unknown id.
            #[test]
            fn hyperedge_members_unknown() {
                let g = Graph::default();
                assert!(g.hyperedge_members(&Uuid::new_v4()).is_none());
            }
        }

        mod test_probs {
            use super::*;

            mod test_get_type {
                use super::*;

                /// Unknown id resolves to `None`.
                #[test]
                fn unknown_is_none() {
                    let g = Graph::default();
                    assert!(g.get_type(Uuid::new_v4()).is_none());
                }

                /// Pure node — only in `entities`.
                #[test]
                fn node() {
                    let mut g = Graph::default();
                    let obj = test_utils::create_simple_obj("test_field");
                    let n1 = g.add_node(obj);
                    assert!(matches!(g.get_type(n1), Some(EntityType::Node)));
                }

                /// Regular edge — both endpoints are nodes.
                #[test]
                fn edge() {
                    let (g, _n1, _n2, _n3, _n4, e_a, _e_b, _meta_edge, _edge_to_h, _h) =
                        test_utils::create_sample_graph2();
                    assert!(matches!(g.get_type(e_a), Some(EntityType::Edge)));
                }

                /// Edge whose endpoint is another edge → MetaEdge.
                #[test]
                fn meta_edge_with_edge_endpoint() {
                    let (g, _n1, _n2, _n3, _n4, _e_a, _e_b, meta_edge, _edge_to_h, _h) =
                        test_utils::create_sample_graph2();
                    assert!(matches!(g.get_type(meta_edge), Some(EntityType::MetaEdge)));
                }

                /// Edge whose endpoint is a hyperedge → MetaEdge.
                #[test]
                fn meta_edge_with_hyperedge_endpoint() {
                    let (g, _n1, _n2, _n3, _n4, _e_a, _e_b, _meta_edge, edge_to_h, _h) =
                        test_utils::create_sample_graph2();
                    assert!(matches!(g.get_type(edge_to_h), Some(EntityType::MetaEdge)));
                }

                /// Pure hyperedge — only in `hyper_edge`.
                #[test]
                fn hyperedge() {
                    let (g, _n1, _n2, _n3, _n4, _e_a, _e_b, _meta_edge, _edge_to_h, h) =
                        test_utils::create_sample_graph2();
                    assert!(matches!(g.get_type(h), Some(EntityType::HyperEdge)));
                }

                /// Object attached on top of an edge — id collides
                /// in both `entities` and `edges` → AttachedObject.
                #[test]
                fn attached_on_edge() {
                    let (mut g, _n1, _n2, _n3, _n4, e_a, _e_b, _meta_edge, _edge_to_h, _h) =
                        test_utils::create_sample_graph2();
                    let obj = test_utils::create_simple_obj("attached");
                    g.attach_obj(e_a, obj).unwrap();
                    assert!(matches!(g.get_type(e_a), Some(EntityType::AttachedObject)));
                }

                /// Object attached on top of a hyperedge — id
                /// collides in both `entities` and `hyper_edge` →
                /// AttachedObject.
                #[test]
                fn attached_on_hyperedge() {
                    let (mut g, _n1, _n2, _n3, _n4, _e_a, _e_b, _meta_edge, _edge_to_h, h) =
                        test_utils::create_sample_graph2();
                    let obj = test_utils::create_simple_obj("attached");
                    g.attach_obj(h, obj).unwrap();
                    assert!(matches!(g.get_type(h), Some(EntityType::AttachedObject)));
                }

                /// `Pointee::Path` endpoint must NOT promote an edge
                /// to MetaEdge — only edge/hyperedge endpoints do.
                #[test]
                fn path_endpoint_stays_edge() {
                    let mut g = Graph::default();
                    let obj = test_utils::create_simple_obj("test_field");
                    let n1 = g.add_node(obj.clone());
                    let n2 = g.add_node(obj);

                    let p1 = Pointee::Path(GlobalObjPath::new(n1, "test_field").unwrap());
                    let e1 = g.add_edge(p1, n2).unwrap();
                    assert!(matches!(g.get_type(e1), Some(EntityType::Edge)));
                }
            }

            mod test_classify_pointee {
                use super::*;

                #[test]
                fn entity_node() {
                    let mut g = Graph::default();
                    let obj = test_utils::create_simple_obj("test_field");
                    let n1 = g.add_node(obj);
                    assert_eq!(g.classify_pointee(&n1.into()), Some(PointeeKind::Node));
                }

                #[test]
                fn entity_edge() {
                    let (g, _n1, _n2, _n3, _n4, e_a, _e_b, _meta_edge, _edge_to_h, _h) =
                        test_utils::create_sample_graph2();
                    assert_eq!(g.classify_pointee(&e_a.into()), Some(PointeeKind::Edge));
                }

                #[test]
                fn entity_hyperedge() {
                    let (g, _n1, _n2, _n3, _n4, _e_a, _e_b, _meta_edge, _edge_to_h, h) =
                        test_utils::create_sample_graph2();
                    assert_eq!(g.classify_pointee(&h.into()), Some(PointeeKind::HyperEdge));
                }

                #[test]
                fn entity_meta_edge() {
                    let (g, _n1, _n2, _n3, _n4, _e_a, _e_b, meta_edge, _edge_to_h, _h) =
                        test_utils::create_sample_graph2();
                    assert_eq!(
                        g.classify_pointee(&meta_edge.into()),
                        Some(PointeeKind::MetaEdge)
                    );
                }

                #[test]
                fn entity_attached() {
                    let (mut g, _n1, _n2, _n3, _n4, e_a, _e_b, _meta_edge, _edge_to_h, _h) =
                        test_utils::create_sample_graph2();
                    let obj = test_utils::create_simple_obj("attached");
                    g.attach_obj(e_a, obj).unwrap();
                    assert_eq!(
                        g.classify_pointee(&e_a.into()),
                        Some(PointeeKind::AttachedObject)
                    );
                }

                #[test]
                fn entity_unknown() {
                    let g = Graph::default();
                    let unknown: Pointee = Uuid::new_v4().into();
                    assert_eq!(g.classify_pointee(&unknown), None);
                }

                /// Path that resolves to a real field → Subobject.
                #[test]
                fn path_resolves() {
                    let mut g = Graph::default();
                    let obj = test_utils::create_simple_obj("test_field");
                    let n1 = g.add_node(obj);
                    let p = Pointee::Path(GlobalObjPath::new(n1, "test_field").unwrap());
                    assert_eq!(g.classify_pointee(&p), Some(PointeeKind::Subobject));
                }

                /// Path whose entity isn't in the graph → None.
                #[test]
                fn path_unknown_entity() {
                    let g = Graph::default();
                    let p = Pointee::Path(GlobalObjPath::new(Uuid::new_v4(), "x").unwrap());
                    assert_eq!(g.classify_pointee(&p), None);
                }

                /// Path whose entity exists but the field is missing
                /// → None.
                #[test]
                fn path_missing_field() {
                    let mut g = Graph::default();
                    let obj = test_utils::create_simple_obj("test_field");
                    let n1 = g.add_node(obj);
                    let p = Pointee::Path(GlobalObjPath::new(n1, "no_such_field").unwrap());
                    assert_eq!(g.classify_pointee(&p), None);
                }
            }

            mod test_is_exist {
                use super::*;
                #[test]
                fn node_exists() {
                    let mut g = Graph::default();
                    let obj = test_utils::create_simple_obj("test_field");
                    let n1 = g.add_node(obj.clone());
                    assert!(g.is_exist(&n1))
                }

                #[test]
                fn edge_exists() {
                    let mut g = Graph::default();
                    let obj = test_utils::create_simple_obj("test_field");
                    let n1 = g.add_node(obj.clone());
                    let n2 = g.add_node(obj);
                    let e1 = g.add_edge(n1, n2).unwrap();
                    assert!(g.is_exist(&e1))
                }

                #[test]
                fn hyperedge_exists() {
                    let mut g = Graph::default();
                    let obj = test_utils::create_simple_obj("test_field");
                    let n1 = g.add_node(obj.clone());
                    let n2 = g.add_node(obj);
                    let mut m = HashSet::new();
                    m.insert(n1.into());
                    m.insert(n2.into());
                    let h = g.create_hyperedge(m).unwrap();
                    assert!(g.is_exist(&h))
                }

                /// Meta-edge: an edge whose endpoint is another edge.
                #[test]
                fn meta_edge_exists() {
                    let mut g = Graph::default();
                    let obj = test_utils::create_simple_obj("test_field");
                    let n1 = g.add_node(obj.clone());
                    let n2 = g.add_node(obj);
                    let e1 = g.add_edge(n1, n2).unwrap();
                    let meta_edge = g.add_edge(n1, e1).unwrap();
                    assert!(g.is_exist(&meta_edge))
                }
            }

            mod test_is_pointee_exist {
                use super::*;

                #[test]
                fn entity_node() {
                    let mut g = Graph::default();
                    let obj = test_utils::create_simple_obj("test_field");
                    let n1 = g.add_node(obj);
                    assert!(g.is_pointee_exist(&n1.into()));
                }

                #[test]
                fn entity_edge() {
                    let (g, _n1, _n2, _n3, _n4, e_a, _e_b, _meta_edge, _edge_to_h, _h) =
                        test_utils::create_sample_graph2();
                    assert!(g.is_pointee_exist(&e_a.into()));
                }

                #[test]
                fn entity_hyperedge() {
                    let (g, _n1, _n2, _n3, _n4, _e_a, _e_b, _meta_edge, _edge_to_h, h) =
                        test_utils::create_sample_graph2();
                    assert!(g.is_pointee_exist(&h.into()));
                }

                #[test]
                fn entity_meta_edge() {
                    let (g, _n1, _n2, _n3, _n4, _e_a, _e_b, meta_edge, _edge_to_h, _h) =
                        test_utils::create_sample_graph2();
                    assert!(g.is_pointee_exist(&meta_edge.into()));
                }

                #[test]
                fn entity_attached() {
                    let (mut g, _n1, _n2, _n3, _n4, e_a, _e_b, _meta_edge, _edge_to_h, _h) =
                        test_utils::create_sample_graph2();
                    let obj = test_utils::create_simple_obj("attached");
                    g.attach_obj(e_a, obj).unwrap();
                    assert!(g.is_pointee_exist(&e_a.into()));
                }

                #[test]
                fn entity_unknown() {
                    let g = Graph::default();
                    let unknown: Pointee = Uuid::new_v4().into();
                    assert!(!g.is_pointee_exist(&unknown));
                }

                /// Path resolves to a real top-level field.
                #[test]
                fn path_resolves() {
                    let mut g = Graph::default();
                    let obj = test_utils::create_simple_obj("test_field");
                    let n1 = g.add_node(obj);
                    let p = Pointee::Path(GlobalObjPath::new(n1, "test_field").unwrap());
                    assert!(g.is_pointee_exist(&p));
                }

                /// Path descends through a nested `Field::Object`.
                #[test]
                fn path_resolves_nested() {
                    let mut g = Graph::default();
                    let mut inner = Object::new();
                    inner.insert("leaf".into(), Field::Null);
                    let mut obj = Object::new();
                    obj.insert("nested".into(), Field::Object(inner));
                    let n1 = g.add_node(obj);

                    let mut path = GlobalObjPath::new(n1, "nested").unwrap();
                    path.push("leaf").unwrap();
                    let p = Pointee::Path(path);
                    assert!(g.is_pointee_exist(&p));
                }

                /// Entity isn't in the graph.
                #[test]
                fn path_unknown_entity() {
                    let g = Graph::default();
                    let p = Pointee::Path(GlobalObjPath::new(Uuid::new_v4(), "x").unwrap());
                    assert!(!g.is_pointee_exist(&p));
                }

                /// Entity exists but the first segment doesn't match
                /// any top-level field.
                #[test]
                fn path_missing_field() {
                    let mut g = Graph::default();
                    let obj = test_utils::create_simple_obj("test_field");
                    let n1 = g.add_node(obj);
                    let p = Pointee::Path(GlobalObjPath::new(n1, "no_such_field").unwrap());
                    assert!(!g.is_pointee_exist(&p));
                }

                /// Walking a path through a non-Object field
                /// (e.g. `Field::Null`) must fail.
                #[test]
                fn path_through_non_object_fails() {
                    let mut g = Graph::default();
                    // "test_field" is a Field::Null — it is not an
                    // Object, so any further descent must fail.
                    let obj = test_utils::create_simple_obj("test_field");
                    let n1 = g.add_node(obj);

                    let mut path = GlobalObjPath::new(n1, "test_field").unwrap();
                    path.push("anything").unwrap();
                    let p = Pointee::Path(path);
                    assert!(!g.is_pointee_exist(&p));
                }

                /// Edge id with a sub-path doesn't navigate — only
                /// `entities` is consulted, and a bare edge has no
                /// attached object.
                #[test]
                fn path_on_bare_edge_fails() {
                    let (g, _n1, _n2, _n3, _n4, e_a, _e_b, _meta_edge, _edge_to_h, _h) =
                        test_utils::create_sample_graph2();
                    let p = Pointee::Path(GlobalObjPath::new(e_a, "x").unwrap());
                    assert!(!g.is_pointee_exist(&p));
                }
            }
        }
    }

    mod test_constructors {
        use super::*;

        mod test_create_hyperedge {
            use super::*;

            /// Create a hyperedge with two members and verify
            /// both id presence and members round-trip.
            #[test]
            fn members_round_trip() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("test_field");
                let n1 = g.add_node(obj.clone());
                let n2 = g.add_node(obj);

                let mut members = HashSet::new();
                members.insert(n1.into());
                members.insert(n2.into());

                let h = g.create_hyperedge(members.clone()).unwrap();
                assert!(g.is_exist(&h));
                assert_eq!(g.hyperedge_members(&h), Some(&members));
            }

            /// An empty member set is rejected — every hyperedge
            /// must have at least one member.
            #[test]
            fn create_hyperedge_empty_rejected() {
                let mut g = Graph::default();
                let err = g.create_hyperedge(HashSet::new()).unwrap_err();
                assert_eq!(err, CreateHyperEdgeError::EmptyHyperEdge);
            }

            /// Members may include other hyperedges (nesting) and
            /// edge ids — `create_hyperedge` doesn't validate
            /// membership shape.
            #[test]
            fn create_hyperedge_with_edge_and_hyperedge_members() {
                let (mut g, _n1, _n2, _n3, _n4, e_a, _e_b, _meta_edge, _edge_to_h, h) =
                    test_utils::create_sample_graph2();

                let mut members = HashSet::new();
                members.insert(e_a.into());
                members.insert(h.into());

                let h2 = g.create_hyperedge(members.clone()).unwrap();
                assert_eq!(g.hyperedge_members(&h2), Some(&members));
            }

            /// `__create_hyperedge_with_id` rejects a duplicate id.
            #[test]
            fn create_hyperedge_already_exists() {
                let mut g = Graph::default();
                let n1 = g.add_node(test_utils::create_simple_obj("f"));
                let mut members = HashSet::new();
                members.insert(n1.into());
                let h = g.create_hyperedge(members.clone()).unwrap();
                let err = g
                    .silent_create_hyperedge_with_id(&h, members)
                    .unwrap_err();
                assert_eq!(
                    err,
                    CreateHyperEdgeError::HyperEdgeAlreadyExists(HyperEdgeAlreadyExistsError {
                        id: h
                    })
                );
            }

            /// Re-inserting with the same id must NOT clobber the
            /// existing members — the original hyperedge stays
            /// intact.
            #[test]
            fn create_hyperedge_already_exists_preserves_members() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("test_field");
                let n1 = g.add_node(obj.clone());
                let n2 = g.add_node(obj);

                let mut original = HashSet::new();
                original.insert(n1.into());
                let h = g.create_hyperedge(original.clone()).unwrap();

                // Try to overwrite with a different (non-empty) member set.
                let mut other = HashSet::new();
                other.insert(n2.into());
                let _ = g.silent_create_hyperedge_with_id(&h, other);

                assert_eq!(g.hyperedge_members(&h), Some(&original));
            }
        }

        mod test_add_node {
            use super::*;

            /// Adding a node returns an id and the object can be looked up.
            #[test]
            fn add_then_lookup() {
                let mut graph = Graph::default();
                let obj = test_utils::create_simple_obj("test_field");
                let node_id = graph.add_node(obj.clone());
                assert_eq!(graph.obj(&node_id), Some(&obj));
            }

            /// Re-inserting under the same id is rejected and the
            /// original object stays untouched.
            #[test]
            fn rejects_duplicate_id() {
                let mut graph = Graph::default();
                let obj = test_utils::create_simple_obj("test_field");
                let n1 = graph.add_node(obj.clone());
                let obj2 = test_utils::create_simple_obj("test_field2");
                let result2 = graph.silent_add_node_with_id(n1, obj2.clone());
                assert_eq!(
                    result2.clone().unwrap_err(),
                    NodeAlreadyExistsError {
                        id: result2.unwrap_err().id
                    }
                );
                // Check thats change doesnt apply
                assert_eq!(graph.obj(&n1), Some(&obj))
            }
        }

        mod test_add_edge {
            use super::*;

            /// Adding a basic edge stores its (source, target) pair.
            #[test]
            fn add_basic_edge() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("test_field");
                let n1 = g.add_node(obj.clone());
                let n2 = g.add_node(obj);

                let e1 = g.add_edge(n1, n2).unwrap();
                assert_eq!(
                    g.edge(&e1).unwrap(),
                    Triplet {
                        id: e1,
                        source: Pointee::EntityId(n1),
                        target: Pointee::EntityId(n2),
                    }
                )
            }

            /// Self-loop: an edge with both endpoints equal is allowed.
            #[test]
            fn allows_self_loop() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("test_field");
                let n1 = g.add_node(obj.clone());

                let e1 = g.add_edge(n1, n1).unwrap();
                assert_eq!(
                    g.edge(&e1).unwrap(),
                    Triplet {
                        id: e1,
                        source: Pointee::EntityId(n1),
                        target: Pointee::EntityId(n1),
                    }
                )
            }

            /// Both endpoints unresolved → MissingEndpoints with both ids.
            #[test]
            fn rejects_both_endpoints_missing() {
                let mut g = Graph::default();
                let n1 = Uuid::new_v4();
                let n2 = Uuid::new_v4();

                let err = g.add_edge(n1, n2).unwrap_err();
                assert_eq!(
                    err,
                    AddEdgeError::MissingEndpoints(MissingEndpointsError {
                        missing_endpoints: vec![Pointee::EntityId(n1), Pointee::EntityId(n2)],
                    })
                )
            }

            /// Re-inserting an edge under the same id is rejected.
            #[test]
            fn rejects_duplicate_edge_id() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("test_field");
                let n1 = g.add_node(obj.clone());
                let n2 = g.add_node(obj);

                let e1 = g.add_edge(n1, n2).unwrap();
                let err = g
                    .silent_add_edge_with_id(e1, n1.into(), n2.into())
                    .unwrap_err();
                assert_eq!(
                    err,
                    AddEdgeError::EdgeAlreadyExists(EdgeAlreadyExistsError { id: e1 })
                )
            }
        }
    }

    mod test_destructors {
        use super::*;

        mod test_remove_node {
            use super::*;

            /// `remove_node` rejects an unknown id.
            #[test]
            fn unknown_id() {
                let mut g = Graph::default();
                let err = g.remove_node(&Uuid::new_v4()).unwrap_err();
                assert!(matches!(err, NodeNotFoundError { .. }));
            }

            /// Removing a node returns its attached object as Field::Object.
            #[test]
            fn returns_object() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("f");
                let n1 = g.add_node(obj.clone());
                let returned = g.remove_node(&n1).unwrap();
                assert_eq!(returned, Field::Object(obj));
                assert!(!g.is_exist(&n1));
                test_utils::check_index_invariant(&g);
            }

            /// Removing `n1` cascades to `e: n1 → n2`.
            /// `n2` survives but its target bucket no longer contains `e`.
            #[test]
            fn cascades_direct_edge_reference() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("f");
                let n1 = g.add_node(obj.clone());
                let n2 = g.add_node(obj);
                let e = g.add_edge(n1, n2).unwrap();

                g.remove_node(&n1).unwrap();

                assert!(!g.is_exist(&n1));
                assert!(!g.edges.contains_key(&e));
                assert!(g.is_exist(&n2));
                test_utils::check_index_invariant(&g);
            }

            /// Removing `n2` cascades to `e: n1 → n2/field` (Path-pointee).
            /// Verifies the `entity_to_path_pointees` lookup path.
            #[test]
            fn cascades_path_reference() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("test_field");
                let n1 = g.add_node(obj.clone());
                let n2 = g.add_node(obj);

                let path = Pointee::Path(GlobalObjPath::new(n2, "test_field").unwrap());
                let e = g.add_edge(n1, path.clone()).unwrap();

                g.remove_node(&n2).unwrap();

                assert!(!g.is_exist(&n2));
                assert!(!g.edges.contains_key(&e));
                assert!(g.is_exist(&n1));
                assert!(!g.entity_to_path_pointees.contains_key(&n2));
                test_utils::check_index_invariant(&g);
            }

            /// Self-loop `e: n1 → n1`: index is fully drained on removal.
            #[test]
            fn cascades_self_loop() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("f");
                let n1 = g.add_node(obj);
                let e = g.add_edge(n1, n1).unwrap();

                g.remove_node(&n1).unwrap();

                assert!(!g.edges.contains_key(&e));
                assert!(g.pointee_uses.is_empty());
                assert!(g.entity_to_path_pointees.is_empty());
                test_utils::check_index_invariant(&g);
            }

            /// Chain: `e1: n1 → n2`, `e2: n3 → e1`. Removing `n1` must
            /// cascade to `e1` and then to `e2` (since `e1` is `e2`'s target).
            #[test]
            fn cascades_through_meta_edge() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("f");
                let n1 = g.add_node(obj.clone());
                let n2 = g.add_node(obj.clone());
                let n3 = g.add_node(obj);

                let e1 = g.add_edge(n1, n2).unwrap();
                let e2 = g.add_edge(n3, e1).unwrap();

                g.remove_node(&n1).unwrap();

                assert!(!g.edges.contains_key(&e1));
                assert!(!g.edges.contains_key(&e2));
                assert!(g.is_exist(&n2));
                assert!(g.is_exist(&n3));
                test_utils::check_index_invariant(&g);
            }

            /// Hyperedge with two members loses one — survives with the other.
            #[test]
            fn hyperedge_loses_member_but_survives() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("f");
                let n1 = g.add_node(obj.clone());
                let n2 = g.add_node(obj);

                let mut m = HashSet::new();
                m.insert(n1.into());
                m.insert(n2.into());
                let h = g.create_hyperedge(m).unwrap();

                g.remove_node(&n1).unwrap();

                let mut expected = HashSet::new();
                expected.insert(n2.into());
                assert_eq!(g.hyperedge_members(&h), Some(&expected));
                test_utils::check_index_invariant(&g);
            }

            /// Hyperedge with the only member `n1` becomes empty when `n1` dies
            /// and is itself cascade-removed.
            #[test]
            fn hyperedge_empties_and_dies() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("f");
                let n1 = g.add_node(obj);

                let mut m = HashSet::new();
                m.insert(n1.into());
                let h = g.create_hyperedge(m).unwrap();

                g.remove_node(&n1).unwrap();

                assert!(!g.hyper_edge.contains_key(&h));
                test_utils::check_index_invariant(&g);
            }

            /// Cascade reaches edges that pointed at a hyperedge that
            /// itself died from emptying.
            #[test]
            fn cascade_through_dead_hyperedge() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("f");
                let n1 = g.add_node(obj.clone());
                let n2 = g.add_node(obj);

                let mut m = HashSet::new();
                m.insert(n1.into());
                let h = g.create_hyperedge(m).unwrap();
                // edge from n2 to the soon-to-die hyperedge
                let e = g.add_edge(n2, h).unwrap();

                g.remove_node(&n1).unwrap();

                assert!(!g.hyper_edge.contains_key(&h));
                assert!(!g.edges.contains_key(&e));
                assert!(g.is_exist(&n2));
                test_utils::check_index_invariant(&g);
            }

            /// `remove_node` rejects ids that live in `entities` because of
            /// `attach_obj` on an edge — those are NOT nodes.
            #[test]
            fn rejects_attached_object_id() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("f");
                let n1 = g.add_node(obj.clone());
                let n2 = g.add_node(obj.clone());
                let e = g.add_edge(n1, n2).unwrap();
                g.attach_obj(e, obj).unwrap();

                let err = g.remove_node(&e).unwrap_err();
                assert!(matches!(err, NodeNotFoundError { .. }));
                // Edge itself untouched.
                assert!(g.edges.contains_key(&e));
                test_utils::check_index_invariant(&g);
            }

            /// Records exactly one `Patch::RemoveNode` even on a cascade.
            #[test]
            fn records_single_patch_on_cascade() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("f");
                let n1 = g.add_node(obj.clone());
                let n2 = g.add_node(obj);
                let _e = g.add_edge(n1, n2).unwrap();

                g.remove_node(&n1).unwrap();

                let last = g.events.last().unwrap();
                assert_eq!(*last, Patch::RemoveNode { id: n1 });
                let remove_count = g
                    .events
                    .iter()
                    .filter(|p| matches!(p, Patch::RemoveNode { .. }))
                    .count();
                assert_eq!(remove_count, 1);
            }
        }

        mod test_remove_hyperedge {
            use super::*;

            /// Removing an unknown id returns an error.
            #[test]
            fn unknown_id() {
                let mut g = Graph::default();
                let err = g.remove_hyperedge(&Uuid::new_v4()).unwrap_err();
                assert!(matches!(err, HyperEdgeNotFoundError { .. }));
            }

            /// Plain remove: hyperedge gone, members survive without
            /// `hid` in their `hyperedges` set.
            #[test]
            fn removes_and_unregisters_members() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("f");
                let n1 = g.add_node(obj.clone());
                let n2 = g.add_node(obj);

                let mut m = HashSet::new();
                m.insert(n1.into());
                m.insert(n2.into());
                let h = g.create_hyperedge(m.clone()).unwrap();

                let returned = g.remove_hyperedge(&h).unwrap();

                assert_eq!(returned, m);
                assert!(!g.hyper_edge.contains_key(&h));
                assert!(g.is_exist(&n1));
                assert!(g.is_exist(&n2));
                test_utils::check_index_invariant(&g);
            }

            /// Edges that pointed at the hyperedge cascade away.
            #[test]
            fn cascades_to_referencing_edges() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("f");
                let n1 = g.add_node(obj.clone());
                let n2 = g.add_node(obj);

                let mut m = HashSet::new();
                m.insert(n1.into());
                let h = g.create_hyperedge(m).unwrap();
                let e = g.add_edge(n2, h).unwrap();

                g.remove_hyperedge(&h).unwrap();

                assert!(!g.hyper_edge.contains_key(&h));
                assert!(!g.edges.contains_key(&e));
                assert!(g.is_exist(&n2));
                test_utils::check_index_invariant(&g);
            }

            /// Removing a hyperedge that's a member of another hyperedge:
            /// the parent loses this member; if parent becomes empty, it
            /// dies too.
            #[test]
            fn cascades_to_parent_hyperedge_when_emptied() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("f");
                let n1 = g.add_node(obj);

                let mut inner_m = HashSet::new();
                inner_m.insert(n1.into());
                let inner = g.create_hyperedge(inner_m).unwrap();

                let mut outer_m = HashSet::new();
                outer_m.insert(inner.into());
                let outer = g.create_hyperedge(outer_m).unwrap();

                g.remove_hyperedge(&inner).unwrap();

                assert!(!g.hyper_edge.contains_key(&inner));
                assert!(!g.hyper_edge.contains_key(&outer));
                test_utils::check_index_invariant(&g);
            }

            /// Parent hyperedge with multiple members loses one — survives.
            #[test]
            fn parent_hyperedge_loses_only_this_member() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("f");
                let n1 = g.add_node(obj.clone());
                let n2 = g.add_node(obj);

                let mut inner_m = HashSet::new();
                inner_m.insert(n1.into());
                let inner = g.create_hyperedge(inner_m).unwrap();

                let mut outer_m = HashSet::new();
                outer_m.insert(inner.into());
                outer_m.insert(n2.into());
                let outer = g.create_hyperedge(outer_m).unwrap();

                g.remove_hyperedge(&inner).unwrap();

                let mut expected = HashSet::new();
                expected.insert(n2.into());
                assert_eq!(g.hyperedge_members(&outer), Some(&expected));
                test_utils::check_index_invariant(&g);
            }

            /// Attached object on the hyperedge is dropped along with it.
            #[test]
            fn drops_attached_object() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("f");
                let n1 = g.add_node(obj.clone());

                let mut m = HashSet::new();
                m.insert(n1.into());
                let h = g.create_hyperedge(m).unwrap();
                g.attach_obj(h, obj).unwrap();
                assert!(g.entities.contains_key(&h));

                g.remove_hyperedge(&h).unwrap();

                assert!(!g.entities.contains_key(&h));
                test_utils::check_index_invariant(&g);
            }

            /// Records exactly one `Patch::RemoveHyperEdge` even on a cascade.
            #[test]
            fn records_single_patch_on_cascade() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("f");
                let n1 = g.add_node(obj.clone());
                let n2 = g.add_node(obj);

                let mut m = HashSet::new();
                m.insert(n1.into());
                let h = g.create_hyperedge(m).unwrap();
                let _e = g.add_edge(n2, h).unwrap();

                g.remove_hyperedge(&h).unwrap();

                let last = g.events.last().unwrap();
                assert_eq!(*last, Patch::RemoveHyperEdge { id: h });
                let count = g
                    .events
                    .iter()
                    .filter(|p| matches!(p, Patch::RemoveHyperEdge { .. }))
                    .count();
                assert_eq!(count, 1);
            }
        }

        mod test_remove_attached {
            use super::*;

            /// Unknown id — neither node nor attach target.
            #[test]
            fn unknown_id() {
                let mut g = Graph::default();
                let err = g.remove_attached(Uuid::new_v4()).unwrap_err();
                assert!(matches!(err, NoAttachedObjectError { .. }));
            }

            /// A bare node has no attached object — removal must fail
            /// rather than silently delete the node.
            #[test]
            fn rejects_node_id() {
                let mut g = Graph::default();
                let n1 = g.add_node(test_utils::create_simple_obj("f"));

                let err = g.remove_attached(n1).unwrap_err();
                assert!(matches!(err, NoAttachedObjectError { .. }));
                assert!(g.is_exist(&n1));
                assert!(g.entities.contains_key(&n1));
            }

            /// A bare edge (no attach_obj called) — nothing to remove.
            #[test]
            fn rejects_bare_edge() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("f");
                let n1 = g.add_node(obj.clone());
                let n2 = g.add_node(obj);
                let e = g.add_edge(n1, n2).unwrap();

                let err = g.remove_attached(e).unwrap_err();
                assert!(matches!(err, NoAttachedObjectError { .. }));
                assert!(g.edges.contains_key(&e));
            }

            /// Attached object on an edge: edge stays alive, attached
            /// object gone, RemoveEdgeData patch recorded.
            #[test]
            fn removes_edge_attached() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("f");
                let n1 = g.add_node(obj.clone());
                let n2 = g.add_node(obj.clone());
                let e = g.add_edge(n1, n2).unwrap();
                g.attach_obj(e, obj).unwrap();

                g.remove_attached(e).unwrap();

                assert!(g.edges.contains_key(&e));
                assert!(!g.entities.contains_key(&e));
                assert_eq!(*g.events.last().unwrap(), Patch::RemoveEdgeData { id: e });
                test_utils::check_index_invariant(&g);
            }

            /// Attached object on a hyperedge: hyperedge stays alive,
            /// attached object gone, RemoveHyperEdgeData patch recorded.
            #[test]
            fn removes_hyperedge_attached() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("f");
                let n1 = g.add_node(obj.clone());

                let mut m = HashSet::new();
                m.insert(n1.into());
                let h = g.create_hyperedge(m).unwrap();
                g.attach_obj(h, obj).unwrap();

                g.remove_attached(h).unwrap();

                assert!(g.hyper_edge.contains_key(&h));
                assert!(!g.entities.contains_key(&h));
                assert_eq!(
                    *g.events.last().unwrap(),
                    Patch::RemoveHyperEdgeData { id: h }
                );
                test_utils::check_index_invariant(&g);
            }

            /// EntityId references survive — only Path references die.
            #[test]
            fn entity_id_references_survive() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("f");
                let n1 = g.add_node(obj.clone());
                let n2 = g.add_node(obj.clone());
                let e = g.add_edge(n1, n2).unwrap();
                g.attach_obj(e, obj).unwrap();

                // Edge that points at `e` as a whole entity.
                let n3 = g.add_node(test_utils::create_simple_obj("g"));
                let meta = g.add_edge(n3, e).unwrap();

                g.remove_attached(e).unwrap();

                assert!(g.edges.contains_key(&e));
                assert!(g.edges.contains_key(&meta));
                test_utils::check_index_invariant(&g);
            }

            /// Path references through the attach target die.
            #[test]
            fn cascades_path_references() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("f");
                let n1 = g.add_node(obj.clone());
                let n2 = g.add_node(obj.clone());
                let e = g.add_edge(n1, n2).unwrap();
                let attached = test_utils::create_simple_obj("data");
                g.attach_obj(e, attached).unwrap();

                // Edge whose endpoint is a path through `e`'s attached object.
                let n3 = g.add_node(test_utils::create_simple_obj("g"));
                let path = Pointee::Path(GlobalObjPath::new(e, "data").unwrap());
                let dangling = g.add_edge(n3, path).unwrap();

                g.remove_attached(e).unwrap();

                assert!(g.edges.contains_key(&e));
                assert!(!g.edges.contains_key(&dangling));
                assert!(!g.entity_to_path_pointees.contains_key(&e));
                test_utils::check_index_invariant(&g);
            }

            /// Records exactly one data-removal patch even when the
            /// path-cascade kills downstream structures.
            #[test]
            fn records_single_patch_on_cascade() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("f");
                let n1 = g.add_node(obj.clone());
                let n2 = g.add_node(obj.clone());
                let e = g.add_edge(n1, n2).unwrap();
                g.attach_obj(e, test_utils::create_simple_obj("data"))
                    .unwrap();

                let n3 = g.add_node(obj);
                let path = Pointee::Path(GlobalObjPath::new(e, "data").unwrap());
                let _dangling = g.add_edge(n3, path).unwrap();

                g.remove_attached(e).unwrap();

                let count = g
                    .events
                    .iter()
                    .filter(|p| {
                        matches!(
                            p,
                            Patch::RemoveEdgeData { .. } | Patch::RemoveHyperEdgeData { .. }
                        )
                    })
                    .count();
                assert_eq!(count, 1);
            }
        }
    }

    mod test_modifiers {
        use super::*;

        mod test_attach_obj {
            use super::*;

            /// Attaching to an unknown id is rejected — no patch recorded.
            #[test]
            fn unknown_target() {
                let mut g = Graph::default();
                let err = g
                    .attach_obj(Uuid::new_v4(), test_utils::create_simple_obj("f"))
                    .unwrap_err();
                assert!(matches!(err, AttachObjectError::AttachTargetNotFound(_)));
                assert!(g.events.is_empty());
            }

            /// A node is not an attach target.
            #[test]
            fn rejects_node() {
                let mut g = Graph::default();
                let n1 = g.add_node(test_utils::create_simple_obj("f"));
                let err = g
                    .attach_obj(n1, test_utils::create_simple_obj("g"))
                    .unwrap_err();
                assert!(matches!(err, AttachObjectError::IncorrectType(_)));
                let extra_events = g
                    .events
                    .iter()
                    .filter(|p| {
                        matches!(p, Patch::UpsertEdgeData { .. } | Patch::UpsertHyperEdgeData { .. })
                    })
                    .count();
                assert_eq!(extra_events, 0);
            }

            /// Re-attaching is rejected (target is already AttachedObject).
            #[test]
            fn rejects_double_attach() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("f");
                let n1 = g.add_node(obj.clone());
                let n2 = g.add_node(obj.clone());
                let e = g.add_edge(n1, n2).unwrap();
                g.attach_obj(e, obj.clone()).unwrap();

                let err = g.attach_obj(e, obj).unwrap_err();
                assert!(matches!(err, AttachObjectError::IncorrectType(_)));
                let count = g
                    .events
                    .iter()
                    .filter(|p| matches!(p, Patch::UpsertEdgeData { .. }))
                    .count();
                assert_eq!(count, 1, "second attach must NOT have recorded a patch");
            }

            /// Attach onto a plain edge → records UpsertEdgeData.
            #[test]
            fn records_upsert_edge_data() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("f");
                let n1 = g.add_node(obj.clone());
                let n2 = g.add_node(obj.clone());
                let e = g.add_edge(n1, n2).unwrap();
                let attached = test_utils::create_simple_obj("data");

                g.attach_obj(e, attached.clone()).unwrap();

                assert_eq!(
                    *g.events.last().unwrap(),
                    Patch::UpsertEdgeData {
                        id: e,
                        obj: attached,
                    }
                );
            }

            /// Attach onto a hyperedge → records UpsertHyperEdgeData.
            #[test]
            fn records_upsert_hyperedge_data() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("f");
                let n1 = g.add_node(obj.clone());
                let mut m = HashSet::new();
                m.insert(n1.into());
                let h = g.create_hyperedge(m).unwrap();
                let attached = test_utils::create_simple_obj("data");

                g.attach_obj(h, attached.clone()).unwrap();

                assert_eq!(
                    *g.events.last().unwrap(),
                    Patch::UpsertHyperEdgeData {
                        id: h,
                        obj: attached,
                    }
                );
            }

            /// Attach onto a meta-edge (edge whose endpoint is another
            /// edge) — still recorded as UpsertEdgeData since meta-edges
            /// live in `self.edges`.
            #[test]
            fn meta_edge_records_upsert_edge_data() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("f");
                let n1 = g.add_node(obj.clone());
                let n2 = g.add_node(obj.clone());
                let e1 = g.add_edge(n1, n2).unwrap();
                let n3 = g.add_node(obj.clone());
                let meta = g.add_edge(n3, e1).unwrap();
                let attached = test_utils::create_simple_obj("data");

                g.attach_obj(meta, attached.clone()).unwrap();

                assert_eq!(
                    *g.events.last().unwrap(),
                    Patch::UpsertEdgeData {
                        id: meta,
                        obj: attached,
                    }
                );
            }
        }

        mod test_add_hyperedge_members {
            use super::*;

            /// Unknown hyperedge id is rejected.
            #[test]
            fn unknown_hyperedge() {
                let mut g = Graph::default();
                let n1 = g.add_node(test_utils::create_simple_obj("f"));
                let mut m = HashSet::new();
                m.insert(n1.into());
                let err = g.add_hyperedge_members(Uuid::new_v4(), m).unwrap_err();
                assert!(matches!(err, AddHyperedgeMembersError::HyperEdgeNotFound(_)));
            }

            /// Members that don't exist as pointees are rejected.
            #[test]
            fn missing_pointee() {
                let mut g = Graph::default();
                let n1 = g.add_node(test_utils::create_simple_obj("f"));
                let mut original = HashSet::new();
                original.insert(n1.into());
                let h = g.create_hyperedge(original).unwrap();

                let mut m = HashSet::new();
                m.insert(Pointee::EntityId(Uuid::new_v4()));
                let err = g.add_hyperedge_members(h, m).unwrap_err();
                assert!(matches!(err, AddHyperedgeMembersError::PointeesNotFound(_)));
            }

            /// Adding a member that's already there is rejected,
            /// and nothing is partially applied.
            #[test]
            fn duplicate_member_rejected_atomically() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("f");
                let n1 = g.add_node(obj.clone());
                let n2 = g.add_node(obj);

                let mut original = HashSet::new();
                original.insert(n1.into());
                let h = g.create_hyperedge(original.clone()).unwrap();

                let mut m = HashSet::new();
                m.insert(n1.into()); // duplicate
                m.insert(n2.into()); // would-be new
                let err = g.add_hyperedge_members(h, m).unwrap_err();
                assert!(matches!(err, AddHyperedgeMembersError::MembersAlreadyExist(_)));

                // Atomicity: n2 was NOT added.
                assert_eq!(g.hyperedge_members(&h), Some(&original));
                test_utils::check_index_invariant(&g);
            }

            /// Successful add: members extended, reverse index updated,
            /// patch recorded.
            #[test]
            fn adds_members_and_records_patch() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("f");
                let n1 = g.add_node(obj.clone());
                let n2 = g.add_node(obj.clone());
                let n3 = g.add_node(obj);

                let mut original = HashSet::new();
                original.insert(n1.into());
                let h = g.create_hyperedge(original).unwrap();

                let mut to_add = HashSet::new();
                to_add.insert(n2.into());
                to_add.insert(n3.into());
                g.add_hyperedge_members(h, to_add.clone()).unwrap();

                let mut expected = HashSet::new();
                expected.insert(n1.into());
                expected.insert(n2.into());
                expected.insert(n3.into());
                assert_eq!(g.hyperedge_members(&h), Some(&expected));

                assert_eq!(
                    *g.events.last().unwrap(),
                    Patch::AddElementsToHyperEdge {
                        id: h,
                        members: to_add,
                    }
                );
                test_utils::check_index_invariant(&g);
            }

            /// Adding a Path-pointee tracks it in `entity_to_path_pointees`.
            #[test]
            fn tracks_path_member() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("test_field");
                let n1 = g.add_node(obj.clone());
                let n2 = g.add_node(obj);

                let mut original = HashSet::new();
                original.insert(n1.into());
                let h = g.create_hyperedge(original).unwrap();

                let path = Pointee::Path(GlobalObjPath::new(n2, "test_field").unwrap());
                let mut to_add = HashSet::new();
                to_add.insert(path.clone());
                g.add_hyperedge_members(h, to_add).unwrap();

                assert!(g
                    .entity_to_path_pointees
                    .get(&n2)
                    .is_some_and(|s| s.contains(&path)));
                test_utils::check_index_invariant(&g);
            }

            /// Empty input is a no-op success.
            #[test]
            fn empty_input_is_noop() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("f");
                let n1 = g.add_node(obj);
                let mut original = HashSet::new();
                original.insert(n1.into());
                let h = g.create_hyperedge(original.clone()).unwrap();

                g.add_hyperedge_members(h, HashSet::new()).unwrap();

                assert_eq!(g.hyperedge_members(&h), Some(&original));
                test_utils::check_index_invariant(&g);
            }
        }

        mod test_remove_hyperedge_members {
            use super::*;

            /// Unknown hyperedge id is rejected.
            #[test]
            fn unknown_hyperedge() {
                let mut g = Graph::default();
                let mut m = HashSet::new();
                m.insert(Pointee::EntityId(Uuid::new_v4()));
                let err = g.remove_hyperedge_members(Uuid::new_v4(), m).unwrap_err();
                assert!(matches!(err, RemoveHyperedgeMembersError::HyperEdgeNotFound(_)));
            }

            /// Removing a pointee that's not a current member is rejected,
            /// and nothing is partially applied.
            #[test]
            fn member_not_in_hyperedge_atomic() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("f");
                let n1 = g.add_node(obj.clone());
                let n2 = g.add_node(obj.clone());
                let n3 = g.add_node(obj);

                let mut original = HashSet::new();
                original.insert(n1.into());
                original.insert(n2.into());
                let h = g.create_hyperedge(original.clone()).unwrap();

                let mut m = HashSet::new();
                m.insert(n1.into()); // valid
                m.insert(n3.into()); // not a member
                let err = g.remove_hyperedge_members(h, m).unwrap_err();
                assert!(matches!(
                    err,
                    RemoveHyperedgeMembersError::MembersNotInHyperedge(_)
                ));

                // Atomicity: n1 was NOT removed.
                assert_eq!(g.hyperedge_members(&h), Some(&original));
                test_utils::check_index_invariant(&g);
            }

            /// Successful partial removal: hyperedge survives with the rest.
            #[test]
            fn removes_subset_and_records_patch() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("f");
                let n1 = g.add_node(obj.clone());
                let n2 = g.add_node(obj);

                let mut original = HashSet::new();
                original.insert(n1.into());
                original.insert(n2.into());
                let h = g.create_hyperedge(original).unwrap();

                let mut to_remove = HashSet::new();
                to_remove.insert(n1.into());
                g.remove_hyperedge_members(h, to_remove.clone()).unwrap();

                let mut expected = HashSet::new();
                expected.insert(n2.into());
                assert_eq!(g.hyperedge_members(&h), Some(&expected));

                assert_eq!(
                    *g.events.last().unwrap(),
                    Patch::RemoveElementsFromHyperEdge {
                        id: h,
                        members: to_remove,
                    }
                );
                test_utils::check_index_invariant(&g);
            }

            /// Reverse index is cleaned: removed member's bucket no longer
            /// references this hyperedge.
            #[test]
            fn removed_member_loses_hyperedge_link() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("f");
                let n1 = g.add_node(obj.clone());
                let n2 = g.add_node(obj);

                let mut original = HashSet::new();
                original.insert(n1.into());
                original.insert(n2.into());
                let h = g.create_hyperedge(original).unwrap();

                let mut to_remove = HashSet::new();
                to_remove.insert(n1.into());
                g.remove_hyperedge_members(h, to_remove).unwrap();

                // n1 had only this hyperedge link → bucket fully gone.
                assert!(!g.pointee_uses.contains_key(&Pointee::EntityId(n1)));
                test_utils::check_index_invariant(&g);
            }

            /// Removing all members empties the hyperedge — it dies and
            /// any references to it cascade.
            #[test]
            fn empties_and_kills_hyperedge() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("f");
                let n1 = g.add_node(obj.clone());
                let n2 = g.add_node(obj);

                let mut original = HashSet::new();
                original.insert(n1.into());
                let h = g.create_hyperedge(original.clone()).unwrap();
                let e = g.add_edge(n2, h).unwrap();

                g.remove_hyperedge_members(h, original).unwrap();

                assert!(!g.hyper_edge.contains_key(&h));
                assert!(!g.edges.contains_key(&e));
                test_utils::check_index_invariant(&g);
            }

            /// Empty input is a no-op success.
            #[test]
            fn empty_input_is_noop() {
                let mut g = Graph::default();
                let n1 = g.add_node(test_utils::create_simple_obj("f"));
                let mut original = HashSet::new();
                original.insert(n1.into());
                let h = g.create_hyperedge(original.clone()).unwrap();

                g.remove_hyperedge_members(h, HashSet::new()).unwrap();

                assert_eq!(g.hyperedge_members(&h), Some(&original));
                test_utils::check_index_invariant(&g);
            }

            /// Path-pointee removal also untracks from entity_to_path_pointees
            /// (when its bucket fully empties).
            #[test]
            fn removes_path_member_and_untracks() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("test_field");
                let n1 = g.add_node(obj.clone());
                let n2 = g.add_node(obj);

                let path = Pointee::Path(GlobalObjPath::new(n2, "test_field").unwrap());
                let mut original = HashSet::new();
                original.insert(n1.into());
                original.insert(path.clone());
                let h = g.create_hyperedge(original).unwrap();

                let mut to_remove = HashSet::new();
                to_remove.insert(path.clone());
                g.remove_hyperedge_members(h, to_remove).unwrap();

                assert!(!g.pointee_uses.contains_key(&path));
                assert!(!g.entity_to_path_pointees.contains_key(&n2));
                test_utils::check_index_invariant(&g);
            }
        }

        mod test_replace_node {
            use super::*;

            fn obj_with(fields: &[(&str, Field)]) -> Object {
                let mut o = Object::new();
                for (k, v) in fields {
                    o.insert((*k).into(), v.clone());
                }
                o
            }

            /// Unknown id is rejected.
            #[test]
            fn unknown_id() {
                let mut g = Graph::default();
                let err = g
                    .replace_node(&Uuid::new_v4(), test_utils::create_simple_obj("f"))
                    .unwrap_err();
                assert!(matches!(err, NodeNotFoundError { .. }));
            }

            /// Edges (with attached object) are not nodes — rejected.
            #[test]
            fn rejects_attached_target() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("f");
                let n1 = g.add_node(obj.clone());
                let n2 = g.add_node(obj.clone());
                let e = g.add_edge(n1, n2).unwrap();
                g.attach_obj(e, obj.clone()).unwrap();

                let err = g.replace_node(&e, obj).unwrap_err();
                assert!(matches!(err, NodeNotFoundError { .. }));
            }

            /// Returns the previous object (wrapped in `Field::Object`).
            #[test]
            fn returns_old_object() {
                let mut g = Graph::default();
                let old = obj_with(&[("a", Field::Number(1))]);
                let n1 = g.add_node(old.clone());
                let new = obj_with(&[("a", Field::Number(2))]);

                let returned = g.replace_node(&n1, new).unwrap();
                assert_eq!(returned, Field::Object(old));
            }

            /// Few changed fields (delta_size <= new_size) → ChangeNode.
            #[test]
            fn small_delta_emits_change_node() {
                let mut g = Graph::default();
                let n1 = g.add_node(obj_with(&[
                    ("a", Field::Number(1)),
                    ("b", Field::Number(2)),
                    ("c", Field::Number(3)),
                ]));

                let new = obj_with(&[
                    ("a", Field::Number(1)),
                    ("b", Field::Number(2)),
                    ("c", Field::Number(99)), // only c changed
                ]);
                g.replace_node(&n1, new).unwrap();

                match g.events.last().unwrap() {
                    Patch::ChangeNode { id, delta } => {
                        assert_eq!(*id, n1);
                        assert_eq!(delta.len(), 1);
                        assert!(matches!(
                            &delta[0],
                            ObjectPatch::UpsertField { name, .. } if name == "c"
                        ));
                    }
                    other => panic!("expected ChangeNode, got {:?}", other),
                }
            }

            /// Many changes (delta_size > new_size) → UpsertNode.
            /// Old: {a,b,c}, New: {x,y,z} → delta would be 6 ops, new has 3 fields.
            #[test]
            fn large_delta_emits_upsert_node() {
                let mut g = Graph::default();
                let n1 = g.add_node(obj_with(&[
                    ("a", Field::Number(1)),
                    ("b", Field::Number(2)),
                    ("c", Field::Number(3)),
                ]));

                let new = obj_with(&[
                    ("x", Field::Number(10)),
                    ("y", Field::Number(20)),
                    ("z", Field::Number(30)),
                ]);
                g.replace_node(&n1, new.clone()).unwrap();

                assert_eq!(
                    *g.events.last().unwrap(),
                    Patch::UpsertNode { id: n1, obj: new }
                );
            }

            /// Boundary `delta_size == new_size` — still ChangeNode (`<=`).
            #[test]
            fn equal_size_emits_change_node() {
                let mut g = Graph::default();
                let n1 = g.add_node(obj_with(&[
                    ("a", Field::Number(1)),
                    ("b", Field::Number(2)),
                ]));

                // Both fields changed → 2 UpsertField ops, new has 2 fields.
                let new = obj_with(&[
                    ("a", Field::Number(10)),
                    ("b", Field::Number(20)),
                ]);
                g.replace_node(&n1, new).unwrap();

                assert!(matches!(
                    g.events.last().unwrap(),
                    Patch::ChangeNode { .. }
                ));
            }

            /// Identical replacement: delta is empty → ChangeNode with empty delta.
            #[test]
            fn identical_obj_emits_empty_change_node() {
                let mut g = Graph::default();
                let obj = obj_with(&[("a", Field::Number(1))]);
                let n1 = g.add_node(obj.clone());

                g.replace_node(&n1, obj).unwrap();

                match g.events.last().unwrap() {
                    Patch::ChangeNode { id, delta } => {
                        assert_eq!(*id, n1);
                        assert!(delta.is_empty());
                    }
                    other => panic!("expected ChangeNode, got {:?}", other),
                }
            }

            /// Path-pointee that still resolves under the new object survives.
            #[test]
            fn path_pointee_survives_when_field_kept() {
                let mut g = Graph::default();
                let n1 = g.add_node(obj_with(&[("data", Field::Number(1))]));
                let n2 = g.add_node(obj_with(&[("x", Field::Null)]));
                let path = Pointee::Path(GlobalObjPath::new(n1, "data").unwrap());
                let e = g.add_edge(n2, path.clone()).unwrap();

                // Replace but keep `data` field.
                g.replace_node(&n1, obj_with(&[("data", Field::Number(99))]))
                    .unwrap();

                assert!(g.edges.contains_key(&e));
                assert!(g.pointee_uses.contains_key(&path));
                test_utils::check_index_invariant(&g);
            }

            /// Path-pointee that no longer resolves cascades away.
            #[test]
            fn path_pointee_cascades_when_field_dropped() {
                let mut g = Graph::default();
                let n1 = g.add_node(obj_with(&[("data", Field::Number(1))]));
                let n2 = g.add_node(obj_with(&[("x", Field::Null)]));
                let path = Pointee::Path(GlobalObjPath::new(n1, "data").unwrap());
                let e = g.add_edge(n2, path.clone()).unwrap();

                // Replace with an object that lacks `data`.
                g.replace_node(&n1, obj_with(&[("other", Field::Null)]))
                    .unwrap();

                assert!(!g.edges.contains_key(&e));
                assert!(!g.pointee_uses.contains_key(&path));
                assert!(!g.entity_to_path_pointees.contains_key(&n1));
                // Node itself still alive.
                assert!(g.is_exist(&n1));
                test_utils::check_index_invariant(&g);
            }

            /// EntityId references survive — only path references can die.
            #[test]
            fn entity_id_references_survive() {
                let mut g = Graph::default();
                let n1 = g.add_node(obj_with(&[("a", Field::Null)]));
                let n2 = g.add_node(obj_with(&[("b", Field::Null)]));
                let e = g.add_edge(n2, n1).unwrap();

                g.replace_node(&n1, obj_with(&[("c", Field::Null)]))
                    .unwrap();

                assert!(g.edges.contains_key(&e));
                test_utils::check_index_invariant(&g);
            }
        }

        mod test_replace_attached_obj {
            use super::*;

            fn obj_with(fields: &[(&str, Field)]) -> Object {
                let mut o = Object::new();
                for (k, v) in fields {
                    o.insert((*k).into(), v.clone());
                }
                o
            }

            /// Unknown id is rejected.
            #[test]
            fn unknown_id() {
                let mut g = Graph::default();
                let err = g
                    .replace_attached_obj(&Uuid::new_v4(), obj_with(&[]))
                    .unwrap_err();
                assert!(matches!(err, NoAttachedObjectError { .. }));
            }

            /// A node is not an attach target.
            #[test]
            fn rejects_node() {
                let mut g = Graph::default();
                let n1 = g.add_node(obj_with(&[("a", Field::Null)]));
                let err = g.replace_attached_obj(&n1, obj_with(&[])).unwrap_err();
                assert!(matches!(err, NoAttachedObjectError { .. }));
            }

            /// A bare edge (no attach_obj called) — rejected.
            #[test]
            fn rejects_bare_edge() {
                let mut g = Graph::default();
                let n1 = g.add_node(obj_with(&[("a", Field::Null)]));
                let n2 = g.add_node(obj_with(&[("a", Field::Null)]));
                let e = g.add_edge(n1, n2).unwrap();

                let err = g.replace_attached_obj(&e, obj_with(&[])).unwrap_err();
                assert!(matches!(err, NoAttachedObjectError { .. }));
            }

            /// A bare hyperedge — rejected.
            #[test]
            fn rejects_bare_hyperedge() {
                let mut g = Graph::default();
                let n1 = g.add_node(obj_with(&[("a", Field::Null)]));
                let mut m = HashSet::new();
                m.insert(n1.into());
                let h = g.create_hyperedge(m).unwrap();

                let err = g.replace_attached_obj(&h, obj_with(&[])).unwrap_err();
                assert!(matches!(err, NoAttachedObjectError { .. }));
            }

            /// Returns the previous attached object.
            #[test]
            fn returns_old_object() {
                let mut g = Graph::default();
                let obj = obj_with(&[("a", Field::Null)]);
                let n1 = g.add_node(obj.clone());
                let n2 = g.add_node(obj);
                let e = g.add_edge(n1, n2).unwrap();
                let old = obj_with(&[("data", Field::Number(1))]);
                g.attach_obj(e, old.clone()).unwrap();

                let returned = g
                    .replace_attached_obj(&e, obj_with(&[("data", Field::Number(2))]))
                    .unwrap();
                assert_eq!(returned, Field::Object(old));
            }

            /// Edge with small delta → ChangeEdgeData.
            #[test]
            fn edge_small_delta_emits_change() {
                let mut g = Graph::default();
                let leaf = obj_with(&[("a", Field::Null)]);
                let n1 = g.add_node(leaf.clone());
                let n2 = g.add_node(leaf);
                let e = g.add_edge(n1, n2).unwrap();
                g.attach_obj(
                    e,
                    obj_with(&[
                        ("a", Field::Number(1)),
                        ("b", Field::Number(2)),
                        ("c", Field::Number(3)),
                    ]),
                )
                .unwrap();

                g.replace_attached_obj(
                    &e,
                    obj_with(&[
                        ("a", Field::Number(1)),
                        ("b", Field::Number(2)),
                        ("c", Field::Number(99)),
                    ]),
                )
                .unwrap();

                match g.events.last().unwrap() {
                    Patch::ChangeEdgeData { id, delta } => {
                        assert_eq!(*id, e);
                        assert_eq!(delta.len(), 1);
                    }
                    other => panic!("expected ChangeEdgeData, got {:?}", other),
                }
            }

            /// Edge with large delta → UpsertEdgeData.
            #[test]
            fn edge_large_delta_emits_upsert() {
                let mut g = Graph::default();
                let leaf = obj_with(&[("a", Field::Null)]);
                let n1 = g.add_node(leaf.clone());
                let n2 = g.add_node(leaf);
                let e = g.add_edge(n1, n2).unwrap();
                g.attach_obj(
                    e,
                    obj_with(&[
                        ("a", Field::Number(1)),
                        ("b", Field::Number(2)),
                        ("c", Field::Number(3)),
                    ]),
                )
                .unwrap();

                let new = obj_with(&[
                    ("x", Field::Number(10)),
                    ("y", Field::Number(20)),
                    ("z", Field::Number(30)),
                ]);
                g.replace_attached_obj(&e, new.clone()).unwrap();

                assert_eq!(
                    *g.events.last().unwrap(),
                    Patch::UpsertEdgeData { id: e, obj: new }
                );
            }

            /// Hyperedge with small delta → ChangeHyperEdgeData.
            #[test]
            fn hyperedge_small_delta_emits_change() {
                let mut g = Graph::default();
                let n1 = g.add_node(obj_with(&[("x", Field::Null)]));
                let mut m = HashSet::new();
                m.insert(n1.into());
                let h = g.create_hyperedge(m).unwrap();
                g.attach_obj(
                    h,
                    obj_with(&[("a", Field::Number(1)), ("b", Field::Number(2))]),
                )
                .unwrap();

                g.replace_attached_obj(
                    &h,
                    obj_with(&[("a", Field::Number(1)), ("b", Field::Number(99))]),
                )
                .unwrap();

                match g.events.last().unwrap() {
                    Patch::ChangeHyperEdgeData { id, delta } => {
                        assert_eq!(*id, h);
                        assert_eq!(delta.len(), 1);
                    }
                    other => panic!("expected ChangeHyperEdgeData, got {:?}", other),
                }
            }

            /// Hyperedge with large delta → UpsertHyperEdgeData.
            #[test]
            fn hyperedge_large_delta_emits_upsert() {
                let mut g = Graph::default();
                let n1 = g.add_node(obj_with(&[("x", Field::Null)]));
                let mut m = HashSet::new();
                m.insert(n1.into());
                let h = g.create_hyperedge(m).unwrap();
                g.attach_obj(
                    h,
                    obj_with(&[
                        ("a", Field::Number(1)),
                        ("b", Field::Number(2)),
                        ("c", Field::Number(3)),
                    ]),
                )
                .unwrap();

                let new = obj_with(&[
                    ("x", Field::Number(10)),
                    ("y", Field::Number(20)),
                    ("z", Field::Number(30)),
                ]);
                g.replace_attached_obj(&h, new.clone()).unwrap();

                assert_eq!(
                    *g.events.last().unwrap(),
                    Patch::UpsertHyperEdgeData { id: h, obj: new }
                );
            }

            /// Path-pointee through the attach target gets cascaded if
            /// the field it pointed at is dropped by the replacement.
            #[test]
            fn path_pointee_cascades_when_field_dropped() {
                let mut g = Graph::default();
                let leaf = obj_with(&[("a", Field::Null)]);
                let n1 = g.add_node(leaf.clone());
                let n2 = g.add_node(leaf);
                let e = g.add_edge(n1, n2).unwrap();
                g.attach_obj(e, obj_with(&[("data", Field::Number(1))]))
                    .unwrap();

                let n3 = g.add_node(obj_with(&[("a", Field::Null)]));
                let path = Pointee::Path(GlobalObjPath::new(e, "data").unwrap());
                let dangling = g.add_edge(n3, path.clone()).unwrap();

                g.replace_attached_obj(&e, obj_with(&[("other", Field::Null)]))
                    .unwrap();

                assert!(g.edges.contains_key(&e));
                assert!(!g.edges.contains_key(&dangling));
                assert!(!g.entity_to_path_pointees.contains_key(&e));
                test_utils::check_index_invariant(&g);
            }

            /// Path-pointee survives when the referenced field is kept.
            #[test]
            fn path_pointee_survives_when_field_kept() {
                let mut g = Graph::default();
                let leaf = obj_with(&[("a", Field::Null)]);
                let n1 = g.add_node(leaf.clone());
                let n2 = g.add_node(leaf);
                let e = g.add_edge(n1, n2).unwrap();
                g.attach_obj(e, obj_with(&[("data", Field::Number(1))]))
                    .unwrap();

                let n3 = g.add_node(obj_with(&[("a", Field::Null)]));
                let path = Pointee::Path(GlobalObjPath::new(e, "data").unwrap());
                let kept = g.add_edge(n3, path.clone()).unwrap();

                g.replace_attached_obj(&e, obj_with(&[("data", Field::Number(99))]))
                    .unwrap();

                assert!(g.edges.contains_key(&kept));
                assert!(g.pointee_uses.contains_key(&path));
                test_utils::check_index_invariant(&g);
            }
        }

        mod test_retarget_edge {
            use super::*;

            /// Unknown edge id is rejected.
            #[test]
            fn unknown_edge() {
                let mut g = Graph::default();
                let n1 = g.add_node(test_utils::create_simple_obj("f"));
                let err = g
                    .retarget_edge(&Uuid::new_v4(), RetargetEdge::Source(n1.into()))
                    .unwrap_err();
                assert!(matches!(err, RetargetError::EdgeNotFound(_)));
            }

            /// New endpoint must resolve in the graph.
            #[test]
            fn invalid_target_pointee() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("f");
                let n1 = g.add_node(obj.clone());
                let n2 = g.add_node(obj);
                let e = g.add_edge(n1, n2).unwrap();

                let err = g
                    .retarget_edge(&e, RetargetEdge::Target(Pointee::EntityId(Uuid::new_v4())))
                    .unwrap_err();
                assert!(matches!(err, RetargetError::InvalidTarget(_)));
                // Edge is unchanged.
                assert_eq!(
                    g.edges.get(&e),
                    Some(&(Pointee::EntityId(n1), Pointee::EntityId(n2)))
                );
                test_utils::check_index_invariant(&g);
            }

            /// Retarget the source: edge updated, indexes swapped.
            #[test]
            fn retargets_source() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("f");
                let n1 = g.add_node(obj.clone());
                let n2 = g.add_node(obj.clone());
                let n3 = g.add_node(obj);
                let e = g.add_edge(n1, n2).unwrap();

                g.retarget_edge(&e, RetargetEdge::Source(n3.into())).unwrap();

                assert_eq!(
                    g.edges.get(&e),
                    Some(&(Pointee::EntityId(n3), Pointee::EntityId(n2)))
                );
                // Old source bucket is now empty (n1 had only this edge).
                assert!(!g.pointee_uses.contains_key(&Pointee::EntityId(n1)));
                // New source bucket has the edge.
                assert!(g
                    .pointee_uses
                    .get(&Pointee::EntityId(n3))
                    .is_some_and(|b| b.edges_as_source.contains(&e)));
                test_utils::check_index_invariant(&g);
            }

            /// Retarget the target: edge updated, indexes swapped.
            #[test]
            fn retargets_target() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("f");
                let n1 = g.add_node(obj.clone());
                let n2 = g.add_node(obj.clone());
                let n3 = g.add_node(obj);
                let e = g.add_edge(n1, n2).unwrap();

                g.retarget_edge(&e, RetargetEdge::Target(n3.into())).unwrap();

                assert_eq!(
                    g.edges.get(&e),
                    Some(&(Pointee::EntityId(n1), Pointee::EntityId(n3)))
                );
                assert!(!g.pointee_uses.contains_key(&Pointee::EntityId(n2)));
                assert!(g
                    .pointee_uses
                    .get(&Pointee::EntityId(n3))
                    .is_some_and(|b| b.edges_as_target.contains(&e)));
                test_utils::check_index_invariant(&g);
            }

            /// No-op when the new endpoint equals the old one.
            #[test]
            fn no_op_same_endpoint() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("f");
                let n1 = g.add_node(obj.clone());
                let n2 = g.add_node(obj);
                let e = g.add_edge(n1, n2).unwrap();

                g.retarget_edge(&e, RetargetEdge::Source(n1.into())).unwrap();

                assert_eq!(
                    g.edges.get(&e),
                    Some(&(Pointee::EntityId(n1), Pointee::EntityId(n2)))
                );
                test_utils::check_index_invariant(&g);
            }

            /// Retargeting to a Path-pointee tracks it in
            /// `entity_to_path_pointees`; removing the last reference
            /// to the old path-pointee untracks it.
            #[test]
            fn path_endpoints_track_and_untrack() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("test_field");
                let n1 = g.add_node(obj.clone());
                let n2 = g.add_node(obj.clone());
                let n3 = g.add_node(obj);

                let old_path = Pointee::Path(GlobalObjPath::new(n2, "test_field").unwrap());
                let e = g.add_edge(n1, old_path.clone()).unwrap();
                assert!(g.entity_to_path_pointees.contains_key(&n2));

                let new_path = Pointee::Path(GlobalObjPath::new(n3, "test_field").unwrap());
                g.retarget_edge(&e, RetargetEdge::Target(new_path.clone()))
                    .unwrap();

                // Old path's entity untracked (was its only reference).
                assert!(!g.entity_to_path_pointees.contains_key(&n2));
                assert!(!g.pointee_uses.contains_key(&old_path));

                // New path tracked.
                assert!(g
                    .entity_to_path_pointees
                    .get(&n3)
                    .is_some_and(|s| s.contains(&new_path)));
                assert!(g.pointee_uses.contains_key(&new_path));
                test_utils::check_index_invariant(&g);
            }

            /// Self-loop: retargeting source while target equals source —
            /// the bucket isn't lost mid-op since the same pointee is still
            /// the target.
            #[test]
            fn self_loop_retarget_source_preserves_target_bucket() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("f");
                let n1 = g.add_node(obj.clone());
                let n2 = g.add_node(obj);
                let e = g.add_edge(n1, n1).unwrap();

                g.retarget_edge(&e, RetargetEdge::Source(n2.into())).unwrap();

                assert_eq!(
                    g.edges.get(&e),
                    Some(&(Pointee::EntityId(n2), Pointee::EntityId(n1)))
                );
                // n1 still tracked as target.
                assert!(g
                    .pointee_uses
                    .get(&Pointee::EntityId(n1))
                    .is_some_and(|b| b.edges_as_target.contains(&e)));
                test_utils::check_index_invariant(&g);
            }

            /// Records the patch.
            #[test]
            fn records_patch() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("f");
                let n1 = g.add_node(obj.clone());
                let n2 = g.add_node(obj.clone());
                let n3 = g.add_node(obj);
                let e = g.add_edge(n1, n2).unwrap();

                g.retarget_edge(&e, RetargetEdge::Target(n3.into())).unwrap();

                assert_eq!(
                    *g.events.last().unwrap(),
                    Patch::RetargetEdge {
                        id: e,
                        new_target: RetargetEdge::Target(Pointee::EntityId(n3)),
                    }
                );
            }
        }
    }

    mod test_obj_apply_patch {
        use super::*;

        fn obj_with(fields: &[(&str, Field)]) -> Object {
            let mut o = Object::new();
            for (k, v) in fields {
                o.insert((*k).into(), v.clone());
            }
            o
        }

        #[test]
        fn unknown_id() {
            let mut g = Graph::default();
            let err = g
                .obj_apply_patch(Uuid::new_v4(), vec![])
                .unwrap_err();
            assert!(matches!(err, DeltaError::NotFound(_)));
        }

        #[test]
        fn empty_patch_is_noop() {
            let mut g = Graph::default();
            let n1 = g.add_node(obj_with(&[("a", Field::Number(1))]));
            g.obj_apply_patch(n1, vec![]).unwrap();
            assert_eq!(g.obj(&n1), Some(&obj_with(&[("a", Field::Number(1))])));
            test_utils::check_index_invariant(&g);
        }

        #[test]
        fn add_field_on_fresh_key() {
            let mut g = Graph::default();
            let n1 = g.add_node(obj_with(&[]));
            g.obj_apply_patch(
                n1,
                vec![ObjectPatch::AddField {
                    name: "x".into(),
                    field: Field::Number(7),
                }],
            )
            .unwrap();
            assert_eq!(g.obj(&n1).unwrap().get("x"), Some(&Field::Number(7)));
        }

        #[test]
        fn add_field_on_existing_key_errors() {
            let mut g = Graph::default();
            let n1 = g.add_node(obj_with(&[("x", Field::Number(1))]));
            let err = g
                .obj_apply_patch(
                    n1,
                    vec![ObjectPatch::AddField {
                        name: "x".into(),
                        field: Field::Number(7),
                    }],
                )
                .unwrap_err();
            assert!(matches!(
                err,
                DeltaError::Delta(ObjectPatchError::FieldAlreadyExists { .. })
            ));
        }

        #[test]
        fn remove_field_existing() {
            let mut g = Graph::default();
            let n1 = g.add_node(obj_with(&[("x", Field::Number(1)), ("y", Field::Null)]));
            g.obj_apply_patch(
                n1,
                vec![ObjectPatch::RemoveField { name: "x".into() }],
            )
            .unwrap();
            assert!(!g.obj(&n1).unwrap().contains_key("x"));
            assert!(g.obj(&n1).unwrap().contains_key("y"));
        }

        #[test]
        fn remove_field_missing_errors() {
            let mut g = Graph::default();
            let n1 = g.add_node(obj_with(&[]));
            let err = g
                .obj_apply_patch(n1, vec![ObjectPatch::RemoveField { name: "x".into() }])
                .unwrap_err();
            assert!(matches!(
                err,
                DeltaError::Delta(ObjectPatchError::FieldNotFound { .. })
            ));
        }

        #[test]
        fn upsert_field_inserts_and_replaces() {
            let mut g = Graph::default();
            let n1 = g.add_node(obj_with(&[("x", Field::Number(1))]));
            g.obj_apply_patch(
                n1,
                vec![
                    ObjectPatch::UpsertField {
                        name: "x".into(),
                        field: Field::Number(99),
                    },
                    ObjectPatch::UpsertField {
                        name: "y".into(),
                        field: Field::Null,
                    },
                ],
            )
            .unwrap();
            assert_eq!(g.obj(&n1).unwrap().get("x"), Some(&Field::Number(99)));
            assert_eq!(g.obj(&n1).unwrap().get("y"), Some(&Field::Null));
        }

        #[test]
        fn array_patch_adds_and_removes() {
            let mut g = Graph::default();
            let n1 = g.add_node(obj_with(&[(
                "arr",
                Field::Array(vec![
                    Field::Number(1),
                    Field::Number(2),
                    Field::Number(3),
                ]),
            )]));
            g.obj_apply_patch(
                n1,
                vec![ObjectPatch::ArrayPatch {
                    name: "arr".into(),
                    removed_indices: vec![0],
                    added_fields: vec![(2, Field::Number(99))],
                }],
            )
            .unwrap();
            // After remove(0): [2, 3]; after insert at 2: [2, 3, 99].
            assert_eq!(
                g.obj(&n1).unwrap().get("arr"),
                Some(&Field::Array(vec![
                    Field::Number(2),
                    Field::Number(3),
                    Field::Number(99),
                ]))
            );
        }

        #[test]
        fn array_patch_on_non_array_errors() {
            let mut g = Graph::default();
            let n1 = g.add_node(obj_with(&[("x", Field::Number(1))]));
            let err = g
                .obj_apply_patch(
                    n1,
                    vec![ObjectPatch::ArrayPatch {
                        name: "x".into(),
                        removed_indices: vec![],
                        added_fields: vec![],
                    }],
                )
                .unwrap_err();
            assert!(matches!(
                err,
                DeltaError::Delta(ObjectPatchError::NotAnArray { .. })
            ));
        }

        #[test]
        fn array_patch_index_out_of_bounds() {
            let mut g = Graph::default();
            let n1 = g.add_node(obj_with(&[("arr", Field::Array(vec![Field::Number(1)]))]));
            let err = g
                .obj_apply_patch(
                    n1,
                    vec![ObjectPatch::ArrayPatch {
                        name: "arr".into(),
                        removed_indices: vec![5],
                        added_fields: vec![],
                    }],
                )
                .unwrap_err();
            assert!(matches!(
                err,
                DeltaError::Delta(ObjectPatchError::IndexOutOfBounds { index: 5 })
            ));
        }

        #[test]
        fn sub_object_patch_navigates_and_applies() {
            let mut g = Graph::default();
            let n1 = g.add_node(obj_with(&[(
                "inner",
                Field::Object(obj_with(&[("a", Field::Number(1))])),
            )]));
            g.obj_apply_patch(
                n1,
                vec![ObjectPatch::SubObjectPatch {
                    path: LocalObjPath::new("inner").unwrap(),
                    delta: vec![ObjectPatch::UpsertField {
                        name: "a".into(),
                        field: Field::Number(99),
                    }],
                }],
            )
            .unwrap();
            match g.obj(&n1).unwrap().get("inner") {
                Some(Field::Object(inner)) => {
                    assert_eq!(inner.get("a"), Some(&Field::Number(99)));
                }
                _ => panic!("inner should be an Object"),
            }
        }

        #[test]
        fn sub_object_patch_through_non_object_errors() {
            let mut g = Graph::default();
            let n1 = g.add_node(obj_with(&[("inner", Field::Number(1))]));
            let err = g
                .obj_apply_patch(
                    n1,
                    vec![ObjectPatch::SubObjectPatch {
                        path: LocalObjPath::new("inner").unwrap(),
                        delta: vec![],
                    }],
                )
                .unwrap_err();
            assert!(matches!(
                err,
                DeltaError::Delta(ObjectPatchError::NotAnObject { .. })
            ));
        }

        #[test]
        fn cascades_path_pointees_after_field_removal() {
            let mut g = Graph::default();
            let n1 = g.add_node(obj_with(&[("data", Field::Number(1))]));
            let n2 = g.add_node(obj_with(&[("a", Field::Null)]));
            let path = Pointee::Path(GlobalObjPath::new(n1, "data").unwrap());
            let e = g.add_edge(n2, path.clone()).unwrap();

            g.obj_apply_patch(n1, vec![ObjectPatch::RemoveField { name: "data".into() }])
                .unwrap();

            // The Pointee::Path no longer resolves → cascade kills the edge.
            assert!(!g.edges.contains_key(&e));
            assert!(!g.pointee_uses.contains_key(&path));
            assert!(g.is_exist(&n1));
            test_utils::check_index_invariant(&g);
        }
    }

    mod test_apply_patch {
        use super::*;

        /// Replay the recorded events on a fresh graph and assert
        /// structural equality with the original.
        fn assert_replay_matches(original: &Graph) {
            let mut replayed = Graph::default();
            replayed
                .apply_patch(original.events.clone())
                .expect("replay must succeed");
            test_utils::check_index_invariant(&replayed);
            assert_eq!(original.entities, replayed.entities, "entities mismatch");
            assert_eq!(original.edges, replayed.edges, "edges mismatch");
            assert_eq!(
                original.hyper_edge, replayed.hyper_edge,
                "hyper_edge mismatch"
            );
        }

        #[test]
        fn nodes_and_edges() {
            let mut g = Graph::default();
            let obj = test_utils::create_simple_obj("f");
            let n1 = g.add_node(obj.clone());
            let n2 = g.add_node(obj.clone());
            let n3 = g.add_node(obj);
            g.add_edge(n1, n2).unwrap();
            g.add_edge(n3, n1).unwrap();

            assert_replay_matches(&g);
        }

        #[test]
        fn hyperedge_lifecycle() {
            let mut g = Graph::default();
            let obj = test_utils::create_simple_obj("f");
            let n1 = g.add_node(obj.clone());
            let n2 = g.add_node(obj.clone());
            let n3 = g.add_node(obj);

            let mut m = HashSet::new();
            m.insert(n1.into());
            m.insert(n2.into());
            let h = g.create_hyperedge(m).unwrap();

            let mut to_add = HashSet::new();
            to_add.insert(n3.into());
            g.add_hyperedge_members(h, to_add).unwrap();

            let mut to_remove = HashSet::new();
            to_remove.insert(n1.into());
            g.remove_hyperedge_members(h, to_remove).unwrap();

            assert_replay_matches(&g);
        }

        #[test]
        fn attach_then_replace_attached() {
            let mut g = Graph::default();
            let obj = test_utils::create_simple_obj("f");
            let n1 = g.add_node(obj.clone());
            let n2 = g.add_node(obj.clone());
            let e = g.add_edge(n1, n2).unwrap();
            g.attach_obj(e, obj.clone()).unwrap();
            // Replace with same key but different value → small delta path.
            let mut new = Object::new();
            new.insert("f".into(), Field::Number(42));
            g.replace_attached_obj(&e, new).unwrap();

            assert_replay_matches(&g);
        }

        #[test]
        fn replace_node_change_path() {
            let mut g = Graph::default();
            let mut o1 = Object::new();
            o1.insert("a".into(), Field::Number(1));
            o1.insert("b".into(), Field::Number(2));
            let n1 = g.add_node(o1);

            let mut o2 = Object::new();
            o2.insert("a".into(), Field::Number(1));
            o2.insert("b".into(), Field::Number(99)); // small delta
            g.replace_node(&n1, o2).unwrap();

            assert_replay_matches(&g);
        }

        #[test]
        fn replace_node_upsert_path() {
            let mut g = Graph::default();
            let mut o1 = Object::new();
            o1.insert("a".into(), Field::Number(1));
            let n1 = g.add_node(o1);

            // 2 ops (Remove a, Add x) > 1 field → Upsert path.
            let mut o2 = Object::new();
            o2.insert("x".into(), Field::Number(99));
            g.replace_node(&n1, o2).unwrap();

            assert_replay_matches(&g);
        }

        #[test]
        fn retarget_edge() {
            let mut g = Graph::default();
            let obj = test_utils::create_simple_obj("f");
            let n1 = g.add_node(obj.clone());
            let n2 = g.add_node(obj.clone());
            let n3 = g.add_node(obj);
            let e = g.add_edge(n1, n2).unwrap();
            g.retarget_edge(&e, RetargetEdge::Target(n3.into())).unwrap();

            assert_replay_matches(&g);
        }

        #[test]
        fn remove_node_cascade_replays() {
            let mut g = Graph::default();
            let obj = test_utils::create_simple_obj("f");
            let n1 = g.add_node(obj.clone());
            let n2 = g.add_node(obj.clone());
            let n3 = g.add_node(obj);
            g.add_edge(n1, n2).unwrap();
            g.add_edge(n3, n1).unwrap();
            g.remove_node(&n1).unwrap();

            assert_replay_matches(&g);
        }

        /// A patch whose precondition is violated propagates as an error.
        #[test]
        fn missing_precondition_errors() {
            let mut g = Graph::default();
            let err = g
                .apply_patch(vec![Patch::RemoveNode { id: Uuid::new_v4() }])
                .unwrap_err();
            assert!(matches!(err, ApplyPatchError::NodeNotFound(_)));
        }
    }

    mod lift_events {
        use super::*;

        #[test]
        fn test_create_hyperedge() {
            let mut g = Graph::default();
            let obj = test_utils::create_simple_obj("field");
            let n1 = g.add_node(obj.clone());
            let n2 = g.add_node(obj);

            let mut m = HashSet::new();
            m.insert(n1.into());
            m.insert(n2.into());

            let h = g.create_hyperedge(m.clone()).unwrap();

            assert_eq!(
                *g.events.last().unwrap(),
                Patch::CreateHyperEdge { id: h, members: m }
            )
        }

        #[test]
        fn test_add_node() {
            let mut g = Graph::default();
            let obj = test_utils::create_simple_obj("field");
            let n1 = g.add_node(obj.clone());

            assert_eq!(*g.events.last().unwrap(), Patch::AddNode { id: n1, obj })
        }

        #[test]
        fn test_add_edge() {
            let mut g = Graph::default();
            let obj = test_utils::create_simple_obj("field");
            let n1 = g.add_node(obj.clone());
            let n2 = g.add_node(obj);

            let e = g.add_edge(n1, n2).unwrap();

            assert_eq!(
                *g.events.last().unwrap(),
                Patch::AddEdge {
                    id: e,
                    source: n1.into(),
                    target: n2.into()
                }
            )
        }

        #[test]
        fn test_remove_node() {
            let mut g = Graph::default();
            let obj = test_utils::create_simple_obj("field");
            let n1 = g.add_node(obj.clone());
            let n2 = g.add_node(obj.clone());

            let e = g.add_edge(n1, n2).unwrap();

            g.remove_node(&n1).unwrap();

            // To ensure that remove doesn't produce remove event's
            assert_eq!(
                g.events[0],
                Patch::AddNode {
                    id: n1,
                    obj: obj.clone()
                }
            );
            assert_eq!(g.events[1], Patch::AddNode { id: n2, obj: obj });
            assert_eq!(
                g.events[2],
                Patch::AddEdge {
                    id: e,
                    source: n1.into(),
                    target: n2.into()
                }
            );
            assert_eq!(g.events[3], Patch::RemoveNode { id: n1 })
        }
    }

    #[test]
    fn test_remove_edge() {
        let mut g = Graph::default();
        let obj = test_utils::create_simple_obj("field");
        let n1 = g.add_node(obj.clone());
        let n2 = g.add_node(obj.clone());

        let e = g.add_edge(n1, n2).unwrap();

        g.remove_edge(&e).unwrap();

        assert_eq!(*g.events.last().unwrap(), Patch::RemoveEdge { id: e });
    }
}
