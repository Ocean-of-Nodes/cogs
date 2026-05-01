use std::collections::HashMap;
use uuid::Uuid;
use std::path::PathBuf;

type NodeID = Uuid;
type EdgeID = Uuid;
type ListernerID = Uuid;

#[derive(Debug)]
struct NodeAlreadyExistsError(NodeID);
#[derive(Debug)]
struct NodeNotFoundError(NodeID);
#[derive(Debug)]
struct EdgeNotFoundError(EdgeID);
#[derive(Debug)]
struct MissingTargetsError {
    missing_targets: Vec<Uuid>,
}

#[derive(Debug)]
enum AddEdgeError {
    MissingTargets(MissingTargetsError),
    EdgeAlreadyExists(EdgeID),
}

#[derive(Debug)]
enum ApplyDeltaError {
    EdgeNotFoundError(EdgeNotFoundError),
    NodeAlreadyExists(NodeAlreadyExistsError),
    NodeNotFound(NodeNotFoundError),
    MissingTargetsError(MissingTargetsError),
    AddEdgeError(AddEdgeError),
    RetargetError(RetargetError),
    FieldDoesntObject {
        node_id: NodeID,
        actual_type: String,
    },
}

#[derive(Debug)]
enum RetargetError {
    EdgeNotFound(EdgeID),
    InvalidTarget(RetrargetEdge),
}

#[derive(Debug, Clone)]
enum ObjectDelta {
    AddField {
        name: String,
        field: Field,
    },
    RemoveField {
        name: String,
    },
    ReplaceField {
        name: String,
        field: Field,
    },
    ArrayDelta {
        name: String,
        removed_indices: Vec<usize>,
        added_fields: Vec<(usize, Field)>,
    },
    SubObjectDelta {
        /// Path is a slash-separated string representing the path to the nested object
        path: PathBuf,
        delta: Vec<ObjectDelta>,
    },
}

#[derive(Debug, Clone)]
enum RetrargetEdge {
    Source(Uuid),
    Target(Uuid),
}

#[derive(Debug, Clone)]
enum Patch {
    AddNode {
        id: Uuid,
        field: Field,
    },
    RemoveNode {
        id: Uuid,
    },
    /// When field fundamental type replaced
    ReplaceNode {
        id: Uuid,
        field: Field,
    },
    /// Diff for object node, contains list of changes in fields
    ChangeNode {
        id: Uuid,
        delta: Vec<ObjectDelta>,
    },

    AddEdge {
        id: Uuid,
        source: Uuid,
        target: Uuid,
    },
    RemoveEdge {
        id: Uuid,
    },
    RetrargetEdge {
        id: Uuid,
        new_target: RetrargetEdge,
    },
}

#[derive(PartialEq, Eq, Debug, Clone)]
pub enum Field {
    // Composite types
    Array(Vec<Field>),
    Object(HashMap<String, Field>),
    // Fundamental types
    String(String),
    Bool(bool),
    Number(i128),
    Null,
}

#[derive(Debug, Default)]
struct Triplet {
    source: Uuid,
    target: Uuid,
    edge: Uuid,
}

#[derive(Default)]
struct Graph {
    // Triplet is a link beetween two target's
    // example: node1 --(edge)-- node2
    // But the target can be not only a node representing a field, but also another edge
    // example:
    //
    //  node1             node3
    //   |                  |
    // (edge) --(edge)-- (edge2)
    //   |                  |
    //  node2             node4
    triplet: Vec<Triplet>,
    datas: HashMap<Uuid, Field>,
    listeners: HashMap<ListernerID, Box<dyn Fn(Patch)>>,
    subgraphs: HashMap<PathBuf, Graph>,
}

impl Graph {
    /* ------------ START GETTERS ------------------- */
    pub fn get_node(&self, id: &Uuid) -> Option<&Field> {
        self.datas.get(id)
    }

    pub fn get_edge(&self, id: &Uuid) -> Option<&Triplet> {
        self.triplet.iter().find(|triplet| &triplet.edge == id)
    }
    /* ------------ END GETTERS -------------------- */

    /* ------------ START PROBS -------------------- */
    pub fn is_edge(&self, id: &Uuid) -> bool {
        self.triplet.iter().any(|triplet| &triplet.edge == id)
    }

    pub fn is_node(&self, id: &Uuid) -> bool {
        self.datas.contains_key(id)
    }

    pub fn is_existing_path(&self, source: &Uuid, target: &Uuid) -> bool {
        unimplemented!()
    }
    /* ------------ END PROBS -------------------- */

    /* ------------ START CONSTRUCTORS ----------- */
    fn __add_node_with_id(&mut self, id: Uuid, field: Field) -> Result<(), NodeAlreadyExistsError> {
        self.datas.insert(id, field).map_or_else(
            || Ok(()),
            |_| Err(NodeAlreadyExistsError(id)),
        )
    }

    pub fn add_node(&mut self, field: Field) -> Result<Uuid, NodeAlreadyExistsError> {
        let id = Uuid::new_v4();
        self.__add_node_with_id(id, field).map(|_| id)
    }

    fn __add_edge_with_id(&mut self, id: Uuid, source: Uuid, target: Uuid) -> Result<(), AddEdgeError> {
        if self.get_edge(&id).is_some() {
            return Err(AddEdgeError::EdgeAlreadyExists(id));
        }

        if !self.is_node(&source) || !self.is_node(&target) {
            let mut missing_targets = Vec::new();
            if !self.is_node(&source) {
                missing_targets.push(source);
            }
            if !self.is_node(&target) {
                missing_targets.push(target);
            }
            return Err(AddEdgeError::MissingTargets(MissingTargetsError { missing_targets }));
        }

        self.triplet.push(Triplet { source, target, edge: id });
        Ok(())
    }

    pub fn add_edge(&mut self, source: Uuid, target: Uuid) -> Result<EdgeID, AddEdgeError> {
        let edge_id = Uuid::new_v4();
        self.__add_edge_with_id(edge_id, source, target).map(|_| edge_id)?;
        Ok(edge_id)
    }
    /* ------------ END CONSTRUCTORS ------------- */

    /* ------------ START DESTRUCTORS ----------- */
    pub fn remove_node(&mut self, id: &Uuid) -> Result<Field, NodeNotFoundError> {
        self.datas.remove(id).ok_or_else(|| NodeNotFoundError(*id))
    }

    pub fn remove_edge(&mut self, id: &Uuid) -> Result<Triplet, EdgeNotFoundError> {
        self.triplet
            .iter()
            .position(|triplet| &triplet.edge == id)
            .map(|index| self.triplet.remove(index))
            .ok_or_else(|| EdgeNotFoundError(*id))
    }

    /* ------------ END DESTRUCTORS ------------- */

    /* ------------ START MODIFIERS ----------- */
    pub fn replace_node(&mut self, id: &Uuid, field: Field) -> Result<Field, NodeNotFoundError> {
        unimplemented!()
    }

    pub fn replace_edge_data(&mut self, id: &Uuid, field: Field) -> Result<(), EdgeNotFoundError> {
        unimplemented!()
    }

    pub fn retraget_edge(&mut self, id: &Uuid, new_target: RetrargetEdge) -> Result<(), RetargetError> {
        unimplemented!()
    }

    pub fn apply_delta(&mut self, id: &Uuid, delta: Patch) -> Result<(), ApplyDeltaError> {
        match delta {
            Patch::AddNode { id, field } => {
                self.__add_node_with_id(id, field).map_err(|e| ApplyDeltaError::NodeAlreadyExists(e))
            }
            Patch::RemoveNode { id } => {
                self.remove_node(&id)
                    .map(|_| ())
                    .map_err(|_| ApplyDeltaError::NodeNotFound(NodeNotFoundError(id)))
            }
            Patch::ReplaceNode { id, field } => {
                self.replace_node(&id, field)
                    .map(|_| ())
                    .map_err(|_| ApplyDeltaError::NodeNotFound(NodeNotFoundError(id)))
            }
            Patch::ChangeNode { id, delta} => {
                // self.get_node(&id)
                //     .ok_or_else(|| ApplyDeltaError::NodeNotFound(NodeNotFoundError(id)))
                //     .and_then(|field| {
                //         if let Field::Object(ref mut obj) = field {
                //             // for change in delta {
                //             //    unimplemented!()
                //             // }
                //             unimplemented!()
                //         } else {
                //             Err(ApplyDeltaError::FieldDoesntObject {
                //                 node_id: id,
                //                 actual_type: format!("{:?}", field),
                //             })
                //         }
                //     })
                unimplemented!()
            },
            Patch::AddEdge { id, source, target } => {
                self.__add_edge_with_id(id, source, target)
                    .map_err(|e| ApplyDeltaError::AddEdgeError(e))
            }
            Patch::RemoveEdge { id } => {
                 self.remove_edge(&id)
                    .map(|_| ())
                    .map_err(|_| ApplyDeltaError::EdgeNotFoundError(EdgeNotFoundError(id)))
            }
            Patch::RetrargetEdge { id, new_target } => {
                 self.retraget_edge(&id, new_target)
                    .map_err(|e| ApplyDeltaError::RetargetError(e))
            }
        }
    }
    /* ------------ END MODIFIERS ------------- */

    /* ------------ START LISTENERS ----------- */
    fn notify_listeners(&self, patch: Patch) {
        for listener in self.listeners.values() {
            listener(patch.clone());
        }
    }

    pub fn subscribe_on_change(&mut self, listener: Box<dyn Fn(Patch)>) -> ListernerID {
        let id = Uuid::new_v4();
        self.listeners.insert(id, listener);
        id
    }

    pub fn unsubscribe_on_change(&mut self, id: ListernerID) {
        self.listeners.remove(&id);
    }
    /* ------------ END LISTENERS ------------- */
}

mod tests {
    use super::*;

    mod test_constructors_and_getters {
        use super::*;

        #[test]
        fn test_add_node() {
            let mut graph = Graph::default();
            let field = Field::String("test".to_string());
            let result = graph.add_node(field.clone());
            assert!(result.is_ok());
            let node_id = result.unwrap();
            assert_eq!(graph.get_node(&node_id), Some(&field));
        }

        #[test]
        fn test_add_edge() {
            let mut graph = Graph::default();
            let field1 = Field::String("node1".to_string());
            let field2 = Field::String("node2".to_string());
            let node_id1 = graph.add_node(field1).unwrap();
            let node_id2 = graph.add_node(field2).unwrap();

            let edge_result = graph.add_edge(node_id1, node_id2);
            assert!(edge_result.is_ok());
            let edge_id = edge_result.unwrap();
            assert!(graph.get_edge(&edge_id).is_some());
        }
    }
}

fn main() {
    println!("Hello, world!");
}
