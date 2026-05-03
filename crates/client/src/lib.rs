use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;

use protocol::{InFrame, OutFrame, RequestId, SubId, Transport};

type BoxError = Box<dyn std::error::Error + Send + Sync>;

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
                            Ok(Some(InFrame::Reply { id, ret })) => {
                                let waiter = pending.lock().unwrap().remove(&id);
                                if let Some(s) = waiter {
                                    let _ = s.send(InFrame::Reply { id, ret });
                                }
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

    pub async fn call(&self, name: String, args: Vec<u8>) -> Result<Vec<u8>, BoxError> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let (reply_tx, reply_rx) = oneshot::channel();

        // Register first, send second — the reply can't race the registration.
        self.pending_calls.lock().unwrap().insert(id, reply_tx);
        self.tx.send(OutFrame::Call { id, name, args }).await?;

        match reply_rx.await? {
            InFrame::Reply { ret, .. } => Ok(ret),
        }
    }

    pub fn cold_view(&self, _name: String, _args: Vec<u8>) -> Result<Vec<u8>, BoxError> {
        unimplemented!()
    }

    pub async fn materialized(&self, _name: String, _args: Vec<u8>) -> Result<Vec<u8>, BoxError> {
        unimplemented!()
    }
}

mod tests {

    #[test]
    fn test_call() {
        unimplemented!()
    }

    #[test]
    fn test_cold() {
        unimplemented!()
    }

    #[test]
    fn test_materialized() {
        unimplemented!()
    }
}