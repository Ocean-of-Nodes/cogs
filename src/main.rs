use std::collections::HashMap;
use uuid::Uuid;

type NodeID = Uuid;
type EdgeID = Uuid;
type ListernerID = Uuid;

struct AddEdgeError {
    missing_targets: Vec<Uuid>,
}

#[derive(Debug, Clone)]
enum Patch {
    AddNode { id: Uuid, field: Field },
    AddEdge { id: Uuid, source: Uuid, target: Uuid },
    RemoveNode { id: Uuid },
    RemoveEdge { id: Uuid },
}

#[derive(Debug, Clone)]
enum Field {
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
    pub fn add_node(&mut self, field: Field) -> NodeID {
        let id = Uuid::new_v4();
        self.datas.insert(id, field);
        id
    }

    pub fn add_edge(&mut self, source: Uuid, target: Uuid) -> Result<EdgeID, AddEdgeError> {
        let edge_id = Uuid::new_v4();

        let mut missing_targets = Vec::new();
        if !self.is_edge(&source) && !self.datas.contains_key(&source) {
            missing_targets.push(source);
        }
        if !self.is_edge(&target) && !self.datas.contains_key(&target) {
            missing_targets.push(target);
        }
        if !missing_targets.is_empty() {
            return Err(AddEdgeError { missing_targets });
        }

        self.triplet.push(Triplet {
            source,
            target,
            edge: edge_id,
        });
        Ok(edge_id)
    }
    /* ------------ END CONSTRUCTORS ------------- */

    /* ------------ START DESTRUCTORS ----------- */
    pub fn remove_node(&mut self, id: &Uuid) -> Option<Field> {
        self.datas.remove(id)
    }

    pub fn remove_edge(&mut self, id: &Uuid) -> Option<Triplet> {
        self.triplet
            .iter()
            .position(|triplet| &triplet.edge == id)
            .map(|index| self.triplet.remove(index))
    }

    /* ------------ END DESTRUCTORS ------------- */

    /* ------------ START MODIFIERS ----------- */
    pub fn update_node(&mut self, id: &Uuid, field: Field) {
        unimplemented!()
    }

    pub fn update_edge_data(&mut self, id: &Uuid, field: Field) {
        unimplemented!()
    }

    pub fn update_edge_source(&mut self, id: &Uuid, source: Uuid) {
        unimplemented!()
    }

    pub fn update_edge_target(&mut self, id: &Uuid, target: Uuid) {
        unimplemented!()
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

fn main() {
    println!("Hello, world!");
}
