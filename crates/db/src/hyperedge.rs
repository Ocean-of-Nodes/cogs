use super::*;

/* 
#[derive(Debug)]
struct HyperEdgeTargesNotFound((HyperEdgeNotFound, HyperEdgeNotFound));

enum OverlayStatus {
    Separated,
    Inside,
    Intersect,
}

pub fn overlay_status(g: &mut Graph, lhs: HyperEdgeId, rhs: HyperEdgeId) -> Result<OverlayStatus, HyperEdgeTargesNotFound> {
    match (g.hyperedge_members(&lhs), g.hyperedge_members(&rhs)) {
        (None, None) => Err(HyperEdgeTargesNotFound((HyperEdgeNotFound(lhs), HyperEdgeNotFound(rhs)))),
        (None, Some(_)) => Err(HyperEdgeTargesNotFound((HyperEdgeNotFound(lhs), HyperEdgeNotFound(lhs)))),
        (Some(_), None) => Err(HyperEdgeTargesNotFound((HyperEdgeNotFound(rhs), HyperEdgeNotFound(rhs)))),
        (Some(lhs_members), Some(rhs_members)) => {
            if lhs_members.is_subset(rhs_members) {
                Ok(OverlayStatus::Inside)
            } else if lhs_members.is_disjoint(rhs_members) {
                Ok(OverlayStatus::Separated)
            } else {
                Ok(OverlayStatus::Intersect)
            }
        }
    }
}

/// Disassemble hyperedge into edges
///
/// The attached object will be transformed to source node.
/// If object object unexist source node is empty object.
fn disassemble_into_edges(g: &mut Graph, h: HyperEdgeId) -> Option<HyperEdgeNotFound> {
    if let Some(members) = g.hyperedge_members(&h) {
        let members = members.clone();
        let obj = g.obj(&h).cloned();

        let source_id = match obj {
            Some(obj) => g.add_node(obj),
            None => {
                let empty = Object::new();
                g.add_node(empty)
            }
        };

        for member in members {
            g.add_edge(source_id, member.clone()).unwrap();
        }

        // Remove after creating edges to prevent removal of dangling
        g.remove_hyperedge(&h).unwrap();

        return None;
    }

    Some(HyperEdgeNotFound(h))
}

/// Merge multiple edges into hyperedge
///
/// Accept `reducer` that accept acc(hyperedge attached) the first parameter and
/// the second each node in order
fn merge_edges(g: &mut Graph, edges: HashSet<EdgeID>, mut reducer: impl FnMut(&mut Object, &Object)) {
    let members: HashSet<Pointee> = edges
        .clone()
        .into_iter()
        .map(|e| Pointee::EntityId(e))
        .collect();
    let h = g.create_hyperedge(members);

    let mut attached: Option<Object> = None;
    for edge in edges {
        if let Some(obj) = g.obj(&edge) {
            let mut a = attached.unwrap_or_default();
            reducer(&mut a, obj);
            if !a.is_empty() {
                g.attach_obj(h, a.clone()).unwrap();
            }
            attached = Some(a);
        }

        // Remove after creating hyperedge to prevent removal of dangling
        g.remove_edge(&edge).unwrap();
    }
}

/// Merge hyperedges into one hyperedge
fn merge_hyperedges(
    g: &mut Graph,
    hs: HashSet<HyperEdgeId>,
    mut reducer: impl FnMut(&mut Object, &Object),
) {
    todo!()
}

/// Split hyperede into N number of hyperedges
///
/// Accept spliter thats separate members
fn split_hyperedges(
    g: &mut Graph,
    hs: HashSet<HyperEdgeId>,
    mut spliter: impl FnMut(HashSet<EntityId>) -> Vec<(Object, HashSet<EntityId>)>,
) {
    todo!()
}
*/