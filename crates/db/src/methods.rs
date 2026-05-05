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

/// Get subgraph by path, returns None if subgraph doesn't exist
pub fn subgraph(&mut self, path: &PathBuf) -> Option<&mut Graph> {
    let mut current_graph = self;
    for chank in path.iter() {
        current_graph = current_graph.subgraphs.get_mut(chank.to_str()?)?;
    }
    Some(current_graph)
}

https://www.quantamagazine.org/researchers-achieve-absurdly-fast-algorithm-for-network-flow-20220608/

*/