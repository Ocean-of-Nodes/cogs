//! Pure functions over [`Object`]: apply a `Vec<ObjectPatch>` to an
//! object in place, and diff two objects to produce a patch sequence.

use common::*;

use crate::errors::ObjectPatchError;

/// Apply a sequence of `ObjectPatch` to an `Object` in place. Used by
/// `apply_object_delta` to replay `Change*` patches.
pub(crate) fn apply_object_patches(
    obj: &mut Object,
    patches: Vec<ObjectPatch>,
) -> Result<(), ObjectPatchError> {
    for p in patches {
        match p {
            ObjectPatch::AddField { name, field } => {
                if obj.contains_key(&name) {
                    return Err(ObjectPatchError::FieldAlreadyExists { field_name: name });
                }
                obj.insert(name, field);
            }
            ObjectPatch::RemoveField { name } => {
                if obj.remove(&name).is_none() {
                    return Err(ObjectPatchError::FieldNotFound { field_name: name });
                }
            }
            ObjectPatch::UpsertField { name, field } => {
                obj.insert(name, field);
            }
            ObjectPatch::ArrayPatch {
                name,
                removed_indices,
                added_fields,
            } => {
                let arr = match obj.get_mut(&name) {
                    Some(Field::Array(a)) => a,
                    Some(_) => return Err(ObjectPatchError::NotAnArray { field_name: name }),
                    None => return Err(ObjectPatchError::FieldNotFound { field_name: name }),
                };
                for &idx in &removed_indices {
                    if idx >= arr.len() {
                        return Err(ObjectPatchError::IndexOutOfBounds { index: idx });
                    }
                }
                let mut to_remove = removed_indices;
                to_remove.sort_unstable_by(|a, b| b.cmp(a));
                for idx in to_remove {
                    arr.remove(idx);
                }
                let mut to_add = added_fields;
                to_add.sort_by_key(|(idx, _)| *idx);
                for (idx, field) in to_add {
                    if idx > arr.len() {
                        return Err(ObjectPatchError::IndexOutOfBounds { index: idx });
                    }
                    arr.insert(idx, field);
                }
            }
            ObjectPatch::SubObjectPatch { path, delta } => {
                let mut cursor: &mut Object = obj;
                for seg in &path {
                    match cursor.get_mut(seg) {
                        Some(Field::Object(inner)) => cursor = inner,
                        Some(_) => {
                            return Err(ObjectPatchError::NotAnObject {
                                field_name: seg.to_string(),
                            })
                        }
                        None => {
                            return Err(ObjectPatchError::FieldNotFound {
                                field_name: seg.to_string(),
                            })
                        }
                    }
                }
                apply_object_patches(cursor, delta)?;
            }
        }
    }
    Ok(())
}

/// Shallow diff: produce a patch sequence that turns `old` into `new`.
/// Used by `replace_node` to decide whether emitting a delta is more
/// compact than emitting the full object via Upsert.
pub(crate) fn diff_object(old: &Object, new: &Object) -> Vec<ObjectPatch> {
    let mut patches = Vec::new();
    for k in old.keys() {
        if !new.contains_key(k) {
            patches.push(ObjectPatch::RemoveField { name: k.clone() });
        }
    }
    for (k, v) in new {
        match old.get(k) {
            Some(old_v) if old_v == v => {}
            Some(_) => patches.push(ObjectPatch::UpsertField {
                name: k.clone(),
                field: v.clone(),
            }),
            None => patches.push(ObjectPatch::AddField {
                name: k.clone(),
                field: v.clone(),
            }),
        }
    }
    patches
}
