//! Dependency keys: what an invocation read, and which keys a change
//! of the graph touches. An invocation must re-run when a change
//! touches a key it read, so for every read in [`crate::ViewGraph`]
//! the patches that can change its result have to touch its key.

use std::collections::HashSet;

use common::*;

use crate::object_patch::apply_object_patches;

/// One thing an invocation read.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum DepKey {
    /// Existence and kind of an entity.
    Entity(EntityId),
    /// The object of a node, or the attached object of an edge or
    /// hyperedge.
    Obj(EntityId),
    /// One top-level field of that object.
    Field(EntityId, String),
    /// Endpoints of an edge.
    Edge(EdgeId),
    /// Members of a hyperedge.
    Members(HyperedgeId),
    /// Edges whose source is the pointee.
    OutEdges(Pointee),
    /// Edges whose target is the pointee.
    InEdges(Pointee),
    /// Hyperedges that have the pointee as a member.
    HyperedgesOf(Pointee),
    /// Nodes whose top-level field equals the value.
    NodesWhere(String, Field),
    AllNodes,
    AllEdges,
    AllHyperedges,
    AllAttached,
    AllEntities,
}

impl DepKey {
    /// Keys the existence of `p` depends on: the entity and, for a
    /// path, the top-level field the path goes through.
    pub(crate) fn pointee(p: &Pointee) -> impl Iterator<Item = DepKey> {
        let field = match p {
            Pointee::EntityId(_) => None,
            Pointee::Path(path) => path
                .local()
                .iter()
                .next()
                .map(|first| DepKey::Field(path.entity(), first.to_string())),
        };
        std::iter::once(DepKey::Entity(p.entity())).chain(field)
    }
}

/// Add every key `patch` touches to `out`. A patch and its inverse
/// touch the same keys.
pub(crate) fn touched_keys(patch: &FullPatch, out: &mut HashSet<DepKey>) {
    match patch {
        FullPatch::AddNode { id, obj } | FullPatch::RemoveNode { id, obj } => {
            out.extend([
                DepKey::Entity(*id),
                DepKey::Obj(*id),
                DepKey::AllNodes,
                DepKey::AllEntities,
            ]);
            for (name, value) in obj {
                out.insert(DepKey::Field(*id, name.clone()));
                out.insert(DepKey::NodesWhere(name.clone(), value.clone()));
            }
        }
        FullPatch::ChangeNode { id, delta, old } => {
            object_changed(*id, old, &patched(old, delta), true, out)
        }
        FullPatch::UpsertNode { id, obj, old } => object_changed(*id, old, obj, true, out),

        FullPatch::AddEdge { id, source, target } => edge_keys(*id, source, target, out),
        FullPatch::RemoveEdge {
            id,
            source,
            target,
            data,
        } => {
            edge_keys(*id, source, target, out);
            if let Some(obj) = data {
                attached_keys(*id, obj, out);
            }
        }
        FullPatch::RetargetEdge {
            id,
            new_target,
            old_target,
        } => {
            out.insert(DepKey::Edge(*id));
            for side in [new_target, old_target] {
                out.insert(match side {
                    RetargetEdge::Source(p) => DepKey::OutEdges(p.clone()),
                    RetargetEdge::Target(p) => DepKey::InEdges(p.clone()),
                });
            }
        }

        FullPatch::UpsertEdgeData { id, obj, old }
        | FullPatch::UpsertHyperedgeData { id, obj, old } => match old {
            Some(old) => object_changed(*id, old, obj, false, out),
            None => attached_keys(*id, obj, out),
        },
        FullPatch::ChangeEdgeData { id, delta, old }
        | FullPatch::ChangeHyperedgeData { id, delta, old } => {
            object_changed(*id, old, &patched(old, delta), false, out)
        }
        FullPatch::RemoveEdgeData { id, obj } | FullPatch::RemoveHyperedgeData { id, obj } => {
            attached_keys(*id, obj, out)
        }

        FullPatch::CreateHyperedge { id, members } => hyperedge_keys(*id, members, out),
        FullPatch::RemoveHyperedge { id, members, data } => {
            hyperedge_keys(*id, members, out);
            if let Some(obj) = data {
                attached_keys(*id, obj, out);
            }
        }
        FullPatch::AddHyperedgeMembers { id, members }
        | FullPatch::RemoveHyperedgeMembers { id, members } => {
            out.insert(DepKey::Members(*id));
            out.extend(members.iter().cloned().map(DepKey::HyperedgesOf));
        }
    }
}

fn edge_keys(id: EdgeId, source: &Pointee, target: &Pointee, out: &mut HashSet<DepKey>) {
    out.extend([
        DepKey::Entity(id),
        DepKey::Edge(id),
        DepKey::OutEdges(source.clone()),
        DepKey::InEdges(target.clone()),
        DepKey::AllEdges,
        DepKey::AllEntities,
    ]);
}

fn hyperedge_keys(id: HyperedgeId, members: &HashSet<Pointee>, out: &mut HashSet<DepKey>) {
    out.extend([
        DepKey::Entity(id),
        DepKey::Members(id),
        DepKey::AllHyperedges,
        DepKey::AllEntities,
    ]);
    out.extend(members.iter().cloned().map(DepKey::HyperedgesOf));
}

/// An attached object appeared or disappeared. That changes the
/// kind of the entity too (see `classify_pointee`).
fn attached_keys(id: AttachTargetId, obj: &Object, out: &mut HashSet<DepKey>) {
    out.extend([DepKey::Entity(id), DepKey::Obj(id), DepKey::AllAttached]);
    out.extend(obj.keys().map(|name| DepKey::Field(id, name.clone())));
}

/// The object at `id` went from `old` to `new`. `node` says whether
/// `nodes_where` can see it.
fn object_changed(id: EntityId, old: &Object, new: &Object, node: bool, out: &mut HashSet<DepKey>) {
    out.insert(DepKey::Obj(id));
    for name in old.keys().chain(new.keys()) {
        let (before, after) = (old.get(name), new.get(name));
        if before == after {
            continue;
        }
        out.insert(DepKey::Field(id, name.clone()));
        if node {
            for value in before.into_iter().chain(after) {
                out.insert(DepKey::NodesWhere(name.clone(), value.clone()));
            }
        }
    }
}

/// `old` with `delta` applied.
fn patched(old: &Object, delta: &[ObjectPatch]) -> Object {
    let mut new = old.clone();
    apply_object_patches(&mut new, delta.to_vec())
        .expect("a recorded delta applies to the object it was made from");
    new
}
