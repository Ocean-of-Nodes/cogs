use super::*;

fn banch_create(g: &mut Graph, objects: impl Iterator<Item = Object>) {
    for obj in objects {
        g.add_node(obj);
    }
}

fn banch_remove() {
    todo!()
}

enum RemoveFilter {
    /// Remove all `free` nodes
    All,
    /// Keep nodes with internal link fields
    KeepWithInternalLinks,
}

fn remove_free(g: &mut Graph, target: Option<HyperedgeId>, which: RemoveFilter) -> HashSet<(NodeId, Object)> {
    let mut ret = HashSet::new();
    
    match target {
        Some(hyper_id) => {
            let members = g.hyperedge_members(hyper_id);
            for member in members {
                if g.neighbours(member).is_some() {
                    continue;
                }
                
                if let KeepWithInternalLinks = which && /* member have link field */ {
                   continue;    
                }
                h.insert((member, g.remove_node().unwrap()));
            }
        },
        None => ret,
    }
}