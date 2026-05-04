use super::*;

/* 
struct TargetsIncorrectTypes((IncorrectTypeError, IncorrectTypeError));
struct TargetsNotFound(());

enum SwapError {
    TargetsNotFound(TargetsNotFound),
    TargetsIncorrectTypes(TargetsIncorrectTypes),
}

/// Swap two object on graph
///
/// `lhs` and `rhs` - is whole object or part of object
pub fn swap(g: &mut Graph, lhs: EntityId, rhs: EntityId) -> Result<(), SwapError> {
    let t_lhs = g.get_type(lhs);
    let t_rhs = g.get_type( rhs);

    todo!()
}

/// Returns `path` of beetween nodes
pub fn path(g: &mut Graph) -> PathBuf {
    
}

/// Get subgraph by path, returns None if subgraph doesn't exist
pub fn subgraph(&mut self, path: &PathBuf) -> Option<&mut Graph> {
    let mut current_graph = self;
    for chank in path.iter() {
        current_graph = current_graph.subgraphs.get_mut(chank.to_str()?)?;
    }
    Some(current_graph)
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
pub fn sheave(&self, lhs: &Graph, rhs: &Graph) -> &mut Graph {
    unimplemented!()
}

*/