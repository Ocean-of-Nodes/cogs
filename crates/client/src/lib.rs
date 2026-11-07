mod errors;

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;

use db::*;
use errors::*;
use protocol::*;

macro_rules! graph_forward_read_interface {
    () => {
        // ------------ ROOTS ------------------- //
        pub fn iter_entities(&self) -> impl Iterator<Item = EntityId> {
            self.graph.iter_entities()
        }

        pub fn iter_edges(&self) -> impl Iterator<Item = EdgeId> {
            self.graph.iter_edges()
        }

        pub fn iter_nodes(&self) -> impl Iterator<Item = NodeId> + '_ {
            self.graph.iter_nodes()
        }

        pub fn iter_hyperedges(&self) -> impl Iterator<Item = HyperedgeId> {
            self.graph.iter_hyperedges()
        }

        pub fn iter_attached(&self) -> impl Iterator<Item = AttachTargetId> + '_ {
            self.graph.iter_attached()
        }

        // ------------ GETTERS ------------------- //

        pub fn obj(&self, id: &EntityId) -> Option<&Object> {
            self.graph.obj(id)
        }

        pub fn edge(&self, id: &EdgeId) -> Result<EdgeView, GetEdgeError> {
            self.graph.edge(id)
        }

        pub fn hyperedge_members(&self, id: &HyperedgeId) -> Option<&HashSet<Pointee>> {
            self.graph.hyperedge_members(id)
        }

        // ------------ NEIGHBOURHOOD ------------------- //

        pub fn out_edges(&self, p: &Pointee) -> impl Iterator<Item = EdgeId> + '_ {
            self.graph.out_edges(p)
        }

        pub fn in_edges(&self, p: &Pointee) -> impl Iterator<Item = EdgeId> + '_ {
            self.graph.in_edges(p)
        }

        pub fn hyperedges_of(&self, p: &Pointee) -> impl Iterator<Item = HyperedgeId> + '_ {
            self.graph.hyperedges_of(p)
        }

        pub fn nodes_where<'a>(
            &'a self,
            field: &'a str,
            value: &'a Field,
        ) -> impl Iterator<Item = NodeId> + 'a {
            self.graph.nodes_where(field, value)
        }

        // ------------ PREDICATES / CLASSIFICATION ------------------- //
        pub fn classify_pointee(&self, p: &Pointee) -> Option<PointeeKind> {
            self.graph.classify_pointee(p)
        }

        pub fn is_exist(&self, id: &EntityId) -> bool {
            self.graph.is_exist(id)
        }

        pub fn is_pointee_exist(&self, p: &Pointee) -> bool {
            self.graph.is_pointee_exist(p)
        }
    };
}

pub struct ImmutableGraph {
    graph: Graph,
}

impl ImmutableGraph {
    pub fn new(snapshot: Delta) -> Self {
        let mut graph = Graph::default();
        if let Err(err) = graph.apply_delta(snapshot) {
            panic!("Failed to apply snapshot: {err:?}");
        }
        ImmutableGraph { graph }
    }

    graph_forward_read_interface!();
}

pub struct ColdGraph {
    id: TrackerId,
    graph: Graph,
}

impl ColdGraph {
    pub fn new(id: TrackerId, snapshot: Delta) -> Self {
        let mut graph = Graph::default();
        if let Err(err) = graph.apply_delta(snapshot) {
            panic!("Failed to apply snapshot: {err:?}");
        }
        ColdGraph { id, graph }
    }

    graph_forward_read_interface!();
 

    fn apply_delta(&mut self, delta: Delta) -> Result<(), ApplyPatchError> {
        self.graph.apply_delta(delta)
    }
}

pub struct MaterializedGraph {
    id: TrackerId,
    graph: Graph,
}

impl MaterializedGraph {
    // pub async fn new(
    //     id: TrackerId,
    //     snapshot: Delta,
    //     mut puller: Receiver<Delta>,
    // ) -> Self {
    //     let mut graph = Graph::default();

    //     graph.apply_delta(snapshot)
    //         .expect("Failed to apply snapshot");

    //     let graph = Arc::new(RwLock::new(graph));

    //     let graph_clone = graph.clone();

    //     std::thread::spawn(move || {
    //         let runtime = tokio::runtime::Builder::new_current_thread()
    //             .enable_all()
    //             .build()
    //             .expect("Failed to create runtime");

    //         runtime.block_on(async move {
    //             while let Some(delta) = puller.recv().await {
    //                 let mut graph = graph_clone.write().await;

    //                 graph.apply_delta(delta)
    //                     .expect("Failed to apply delta");
    //             }
    //         });
    //     });

    //     MaterializedGraph { id, graph }
    // }

    graph_forward_read_interface!();
}

pub struct Client {
    /// Outgoing-frame queue — drained by the demux task.
    tx: mpsc::Sender<OutFrame>,

    /// Pending response registry: request id → where to put the result.
    pending_calls: Arc<Mutex<HashMap<RequestId, oneshot::Sender<InFrame>>>>,

    /// Active subscriptions: subscription id → where to forward the frame.
    subscriptions: Arc<Mutex<HashMap<SubId, mpsc::Sender<InFrame>>>>,

    /// Counter shared between request and subscription ids.
    next_id: AtomicU64,

    /// Demux task — owns the transport and routes frames in both directions.
    /// Aborted on drop.
    _demux: JoinHandle<()>,
}

impl Client {
    pub fn connect(mut transport: Box<dyn Transport>) -> Self {
        let (out_tx, mut out_rx) = mpsc::channel::<OutFrame>(64);
        let pending: Arc<Mutex<HashMap<RequestId, oneshot::Sender<InFrame>>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let subs: Arc<Mutex<HashMap<SubId, mpsc::Sender<InFrame>>>> =
            Arc::new(Mutex::new(HashMap::new()));

        let demux = {
            let pending = pending.clone();
            let _subs = subs.clone();
            tokio::spawn(async move {
                loop {
                    tokio::select! {
                        maybe_out = out_rx.recv() => match maybe_out {
                            Some(frame) => {
                                let _ = transport.send(frame).await;
                            }
                            // All callers dropped — nothing more will be sent.
                            None => break,
                        },
                        res = transport.recv() => match res {
                            Ok(Some(InFrame::CallReply { id, ret })) => {
                                let waiter = pending.lock().unwrap().remove(&id);
                                if let Some(s) = waiter {
                                    let _ = s.send(InFrame::CallReply { id, ret });
                                }
                            }
                            Ok(Some(_)) => {
                                // TODO: route Subscribe* / Cold* frames to
                                // the subscriptions registry once
                                // materialized()/cold_view() are implemented.
                            }
                            Ok(None) | Err(_) => break,
                        },
                    }
                }
                let _ = transport.close().await;
            })
        };

        Client {
            tx: out_tx,
            pending_calls: pending,
            subscriptions: subs,
            next_id: AtomicU64::new(0),
            _demux: demux,
        }
    }

    pub async fn call(
        &self,
        name: String,
        args: Vec<Object>,
    ) -> Result<ImmutableGraph, ClientCallError> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let (reply_tx, reply_rx) = oneshot::channel();

        // Register first, send second — the reply can't race the registration.
        self.pending_calls.lock().unwrap().insert(id, reply_tx);
        self.tx.send(OutFrame::Call { id, name, args }).await?;

        let snapshot = match reply_rx.await? {
            InFrame::CallReply { ret, .. } => ret?,
            _ => return Err(ClientCallError::UnexpectedFrame),
        };

        Ok(ImmutableGraph::new(snapshot))
    }

    pub async fn cold_view(&self, name: String) -> Result<ColdGraph, ClientCallError> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let (reply_tx, reply_rx) = oneshot::channel();

        // Register first, send second — the reply can't race the registration.
        self.pending_calls.lock().unwrap().insert(id, reply_tx);
        self.tx.send(OutFrame::Cold { id, name }).await?;

        let response = match reply_rx.await? {
            InFrame::ColdInitialReply { tracker, delta, .. } => (tracker, delta),
            _ => return Err(ClientCallError::UnexpectedFrame),
        };
        Ok(ColdGraph::new(response.0, response.1))
    }

    pub async fn pull_off_patch(&self, graph: &mut ColdGraph) -> Result<(), ClientCallError> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let (reply_tx, reply_rx) = oneshot::channel();

        // Register first, send second — the reply can't race the registration.
        self.pending_calls.lock().unwrap().insert(id, reply_tx);
        self.tx
            .send(OutFrame::ColdRequestDelta {
                id,
                tracker: graph.id,
            })
            .await?;

        let response = match reply_rx.await? {
            InFrame::ColdDelta { delta, .. } => delta,
            _ => return Err(ClientCallError::UnexpectedFrame),
        };
        // FIXME: the error is not handled
        graph.apply_delta(response).unwrap();
        Ok(())
    }

    pub async fn unsubscribe_cold_view(
        &self,
        cold_graph: &ColdGraph,
    ) -> Result<(), ClientCallError> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let (reply_tx, reply_rx) = oneshot::channel();

        // Register first, send second — the reply can't race the registration.
        self.pending_calls.lock().unwrap().insert(id, reply_tx);
        self.tx
            .send(OutFrame::ColdDropTracker {
                tracker: cold_graph.id,
            })
            .await?;

        match reply_rx.await? {
            InFrame::ColdUnsubscribe { .. } => {},
            _ => return Err(ClientCallError::UnexpectedFrame),
        };

        Ok(())
    }

    pub async fn materialized(&self, name: String) -> Result<MaterializedGraph, ClientCallError> {
        unimplemented!()
    }

    pub async fn unsubscribe_materialized(
        &self,
        graph: &mut MaterializedGraph,
    ) -> Result<(), ClientCallError> {
        unimplemented!()
    }

    /// Run a query on the server and return the resulting graph.
    pub async fn query(&self, _query: &str) -> Result<ImmutableGraph, ClientCallError> {
        unimplemented!("the query dialect is not currently implemented")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use transports::in_memory_pair;

    #[tokio::test]
    #[ignore = "ImmutableGraph::new is todo!() — rewrite once it's implemented"]
    async fn test_call() {
        let (transport, mut server) = in_memory_pair();
        let client = Client::connect(Box::new(transport));

        // Fake server: reply to every Call with an empty patch list.
        let server_task = tokio::spawn(async move {
            while let Some(frame) = server.recv().await {
                if let OutFrame::Call { id, .. } = frame {
                    let _ = server
                        .send(InFrame::CallReply {
                            id,
                            ret: Ok(vec![]),
                        })
                        .await;
                }
            }
        });

        let result = client.call("ping".into(), vec![]).await;
        assert!(result.is_ok());

        drop(client);
        server_task.await.unwrap();
    }
}
