mod bench {
        use super::*;

        /// TODO: after implementation of JIT, test that
        ///
        /// G.entities().filter(|id| G.is_edge(id)).collect()
        /// and
        /// G.edges().collect()
        /// should have same behavior after JIT
        /// and same speed
        fn eq_behavior1() {
            let mut graph = Graph::default();
            unimplemented!();
            graph
                .global_entities()
                .filter(|id| graph.is_edge(id))
                .for_each(|_| {});
            graph.global_edges().for_each(|_| {});
        }

        /// TODO: after implementation of JIT, test that
        ///
        /// G.entities().filter(|id| G.is_node(id)).collect()
        /// and
        /// G.nodes().collect()
        /// should have same behavior after JIT
        /// and same speed
        fn eq_behavior2() {
            let mut graph = Graph::default();
            unimplemented!();
            graph
                .global_entities()
                .filter(|id| graph.is_node(id))
                .for_each(|_| {});
            graph.global_nodes().for_each(|_| {});
        }
    }
