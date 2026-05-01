use std::collections::HashMap;
use uuid::Uuid;
use std::path::PathBuf;

type NodeID = Uuid;
type EdgeID = Uuid;
type ListernerID = Uuid;

#[derive(Debug)]
struct UnexistentPathError {
    /// If we pass an nonexistent path, for example 
    /// "/subgraph1/subgraph2/subgraph3", 
    /// where subgpraph1 exist but subgraph2 doesn't, 
    /// then we will return error with valid_path_part = "/subgraph1" 
    valid_path_part: PathBuf,
}
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
struct IncorrectTypeError {
    node_id: NodeID,
    expected_type: String,
    actual_type: String,
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
    IncorrectType(IncorrectTypeError),
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
    /// Triplet is a link beetween two target's
    /// example: node1 --(edge)-- node2
    /// But the target can be not only a node representing a 
    /// field, but also another edge
    /// example:
    ///
    ///  node1             node3
    ///   |                  |
    /// (edge) --(edge)-- (edge2)
    ///   |                  |
    ///  node2             node4
    edges: Vec<Triplet>,
    /// Storage for data of nodes and edges at same level.
    datas: HashMap<Uuid, Field>,
    
    /// Edges beetween subgraphs and parent graph 
    /// or beetween subgraphs, regardless level for example
    /// parent have two subgraphs: subgraph1 and subgraph2, 
    /// and edge beetween them, then it will be in beetween_edges
    beetween_edges: Vec<Triplet>,
    /// Storage for data of edges beetween subgraphs and 
    /// parent graph or beetween subgraphs
    beetween_edges_data: HashMap<Uuid, Field>,
    subgraphs: HashMap<String, Graph>,

    /// Listeners thats will trigger when graph or holded
    /// subgraph will be changed.
    listeners: HashMap<ListernerID, Box<dyn Fn(Patch)>>,
}

impl Graph {
    /* ------------ START GETTERS ------------------- */

    /// Get all neighbours (node or edges) of node with id, 
    /// including nodes from subgraphs and edges beetween subgraphs 
    /// and parent graph,
    pub fn get_neighbours(&self, id: &Uuid) -> Vec<Uuid> {
        let mut neighbours = Vec::new();
        for triplet in self.edges.iter() {
            if &triplet.source == id {
                neighbours.push(triplet.target);
            } else if &triplet.target == id {
                neighbours.push(triplet.source);
            }
        }

        for triplet in self.beetween_edges.iter() {
            if &triplet.source == id {
                neighbours.push(triplet.target);
            } else if &triplet.target == id {
                neighbours.push(triplet.source);
            }
        }

        for graph in self.subgraphs.values() {
            let subgraph_neighbours = graph.get_neighbours(id);
            neighbours.extend(subgraph_neighbours);
        }

        neighbours
    }

    pub fn get_outcoming_edges(&self, id: &Uuid) -> Vec<Uuid> {
        unimplemented!()
    }

    /// Get subgraph by relative path, returns None if subgraph doesn't exist
    pub fn get_subgraph(&mut self, path: &PathBuf) -> Option<&mut Graph> {
        let mut current_graph = self;
        for chank in path.iter() {
            current_graph = current_graph.subgraphs.get_mut(chank.to_str()?)?;
        }
        Some(current_graph)
    }

    /// Get node by id from the whole graph (including subgraphs), 
    /// returns None if node doesn't exist
    pub fn get_node(&self, id: &Uuid) -> Option<&Field> {
        if let Some(field) = self.datas.get(id) {
            return Some(field);
        }

        if let Some(field) = self.beetween_edges_data.get(id) {
            return Some(field);
        }

        for graph in self.subgraphs.values() {
            if let Some(field) = graph.get_node(id) {
                return Some(field);
            }
        }
        None
    }

    /// Get edge by id from the whole graph (including subgraphs),
    /// returns None if edge doesn't exist
    pub fn get_edge(&self, id: &Uuid) -> Option<&Triplet> {
        if let Some(triplet) = self.edges.iter().find(|triplet| &triplet.edge == id) {
            return Some(triplet);
        }
        
        if let Some(triplet) = self.beetween_edges.iter().find(|triplet| &triplet.edge == id) {
            return Some(triplet);
        }

        for graph in self.subgraphs.values() {
            if let Some(triplet) = graph.get_edge(id) {
                return Some(triplet);
            }
        }
        None
    }
    /* ------------ END GETTERS -------------------- */

    /* ------------ START PROBS -------------------- */

    /// Check if id is edge from the whole graph (including subgraph 
    /// and endge beetween subgraphs or parent and subgraphs), 
    /// returns true if edge exists, false otherwise
    pub fn is_edge(&self, id: &Uuid) -> bool {
        self.edges.iter().any(|triplet| &triplet.edge == id)
    }

    /// Check if id is node (including subgraph), returns true
    /// if node exists, false otherwise
    pub fn is_node(&self, id: &Uuid) -> bool {
        if self.datas.contains_key(id) {
            return true;
        }
        self.subgraphs.values().any(|g| g.is_node(id))
    }

    /// Check if id is node or edge from the 
    /// whole graph (including subgraph), 
    /// returns true if node or edge exists, false otherwise
    pub fn is_exist(&self, id: &Uuid) -> bool {
        self.is_node(id) || self.is_edge(id)
    }

    /// Check if exist path between source and target
    pub fn is_existing_path(&self, source: &Uuid, target: &Uuid) -> Result<bool, IncorrectTypeError> { 
       unimplemented!()
    }
    /* ------------ END PROBS -------------------- */

    /* ------------ START CONSTRUCTORS ----------- */

    /// Walks the parent path (all components except the last); 
    /// the last component is the name under which `graph` will 
    /// be inserted in that parent.
    fn add_subgraph(&mut self, name: &PathBuf, graph: Graph) -> Result<(), UnexistentPathError> {
        let mut components = name.iter();
        let subgraph_name = components.next_back().unwrap().to_str().unwrap().to_string();

        let mut current_graph = self;
        let mut current_path = PathBuf::new();
        for chank in components {
            let key = chank.to_str().unwrap();
            if let Some(subgraph) = current_graph.subgraphs.get_mut(key) {
                current_graph = subgraph;
                current_path.push(chank);
            } else {
                return Err(UnexistentPathError { valid_path_part: current_path });
            }
        }
        current_graph.subgraphs.insert(subgraph_name, graph);
        Ok(())
    }

    fn __add_node_with_id(&mut self, id: Uuid, field: Field) -> Result<(), NodeAlreadyExistsError> {
        if self.get_node(&id).is_some() {
            return Err(NodeAlreadyExistsError(id));
        
        }
        
        self.datas.insert(id, field);
        Ok(())
    }

    pub fn add_node(&mut self, field: Field) -> Result<Uuid, NodeAlreadyExistsError> {
        let id = Uuid::new_v4();
        self.__add_node_with_id(id, field).map(|_| id)
    }

    fn __add_edge_with_id(&mut self, id: Uuid, source: Uuid, target: Uuid) -> Result<(), AddEdgeError> {
        if self.get_edge(&id).is_some() {
            return Err(AddEdgeError::EdgeAlreadyExists(id));
        }

        let source_exists = self.is_exist(&source);
        let target_exists = self.is_exist(&target);
        if !source_exists || !target_exists {
            let mut missing_targets = Vec::new();
            if !source_exists {
                missing_targets.push(source);
            }
            if !target_exists {
                missing_targets.push(target);
            }
            return Err(AddEdgeError::MissingTargets(MissingTargetsError { missing_targets }));
        }

        let triplet = Triplet { source, target, edge: id };
        let source_local = self.datas.contains_key(&source)
            || self.edges.iter().any(|t| t.edge == source);
        let target_local = self.datas.contains_key(&target)
            || self.edges.iter().any(|t| t.edge == target);
        if source_local && target_local {
            self.edges.push(triplet);
        } else {
            self.beetween_edges.push(triplet);
        }
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
        self.edges
            .iter()
            .position(|triplet| &triplet.edge == id)
            .map(|index| self.edges.remove(index))
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
        fn test_get_neighbours_nodes_at_same_lvl() {
            // Built graph:
            //  node1 --(edge)--> node2
            //   |
            // (edge2)
            //   |
            //   ˅
            //  node3    
            let mut graph = Graph::default();
            let field1 = Field::String("node1".to_string());
            let field2 = Field::String("node2".to_string());
            let field3 = Field::String("node3".to_string());
            let node_id1 = graph.add_node(field1).unwrap();
            let node_id2 = graph.add_node(field2).unwrap();
            let node_id3 = graph.add_node(field3).unwrap();
            graph.add_edge(node_id1, node_id2).unwrap();
            graph.add_edge(node_id1, node_id3).unwrap();

            let neighbours = graph.get_neighbours(&node_id1);
            assert_eq!(neighbours, vec![node_id2, node_id3]);
        }

        #[test]
        fn test_get_neighbours_edges_at_same_lvl() {
           let mut graph = Graph::default();

            let field1 = Field::String("node1".to_string());
            let field2 = Field::String("node2".to_string());
            let node_id1 = graph.add_node(field1).unwrap();
            let node_id2 = graph.add_node(field2).unwrap();
            let edge1 = graph.add_edge(node_id1, node_id2).unwrap();

            let field3 = Field::String("node3".to_string());
            let field4 = Field::String("node4".to_string());
            let node_id3 = graph.add_node(field3).unwrap();
            let node_id4 = graph.add_node(field4).unwrap();
            let edge2 = graph.add_edge(node_id3, node_id4).unwrap();

            let edge3 = graph.add_edge(edge1, edge2).unwrap();
            let neighbours = graph.get_neighbours(&edge3);
            assert_eq!(neighbours, vec![edge1, edge2]);
        }

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
        
        #[test]
        fn test_create_subgraph_and_get_it() {
            let mut graph = Graph::default();
            let graph2 = Graph::default();
            let graph3 = Graph::default();
            let mut path = PathBuf::from("subgraph1");
            graph.add_subgraph(&path, graph2).unwrap();
            path.push("subgraph2");
            graph.add_subgraph(&path, graph3).unwrap();
            assert!(graph.get_subgraph(&path).is_some());
        }

        #[test]
        fn test_get_node_from_subgraph() {
            let mut graph = Graph::default();
            let graph2 = Graph::default();
            let path = PathBuf::from("subgraph");
            graph.add_subgraph(&path, graph2).unwrap();
            let field = Field::String("test".to_string());
            let node_id = graph.get_subgraph(&path).unwrap().add_node(field.clone()).unwrap();
            assert_eq!(graph.get_subgraph(&path).unwrap().get_node(&node_id), Some(&field));
        }

        #[test]
        fn test_get_edge_beetween_subgraph_and_parent() {
            let mut graph = Graph::default();
            let graph2 = Graph::default();
            let path = PathBuf::from("subgraph");
            graph.add_subgraph(&path, graph2).unwrap();

            let field1 = Field::String("node1".to_string());
            let field2 = Field::String("node2".to_string());
            let node_id1 = graph.add_node(field1).unwrap();
            let node_id2 = graph.get_subgraph(&path).unwrap().add_node(field2).unwrap();

            let edge_id = graph.add_edge(node_id1, node_id2).unwrap();
            assert!(graph.get_edge(&edge_id).is_some());
        }

        #[test]
        fn test_get_edge_beetween_subgraphs_at_same_lvl() {
            unimplemented!()
        }

        #[test]
        fn test_get_node_of_edge_beetween_subgraphs_at_different_lvl() {
            unimplemented!()
        }

        #[test]
        fn test_get_node_of_edge_beetween_subgraph_and_parent() {
           unimplemented!()
        }

        #[test]
        fn test_get_node_of_edge_beetween_subgraphs_at_same_lvl() {
            unimplemented!()
        }

        #[test]
        fn test_get_edge_beetween_subgraphs_at_different_lvl() {
            unimplemented!()
        }
    }

    mod test_probs {
        use super::*;

        #[test]
        fn test_is_node_and_is_edge() {
            let mut graph = Graph::default();
            let field = Field::String("test".to_string());
            let node_id = graph.add_node(field).unwrap();
            assert!(graph.is_node(&node_id));
            assert!(!graph.is_edge(&node_id));

            let edge_id = graph.add_edge(node_id, node_id).unwrap();
            assert!(graph.is_edge(&edge_id));
        }

        #[test]
        fn test_is_existing_path() {
            let mut graph = Graph::default();
            let field1 = Field::String("node1".to_string());
            let field2 = Field::String("node2".to_string());
            let node_id1 = graph.add_node(field1).unwrap();
            let node_id2 = graph.add_node(field2).unwrap();
            graph.add_edge(node_id1, node_id2).unwrap();
            
            let field3 = Field::String("node3".to_string());
            let field4 = Field::String("node4".to_string());
            let node_id3 = graph.add_node(field3).unwrap();
            let node_id4 = graph.add_node(field4).unwrap();
            graph.add_edge(node_id3, node_id4).unwrap();

            graph.add_edge(node_id1, node_id4).unwrap();

            assert!(graph.is_existing_path(&node_id3, &node_id2).unwrap());
        }
    }
}

fn main() {
    println!("Hello, world!");
}
