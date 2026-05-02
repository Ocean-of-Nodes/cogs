use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;

type RequestId = u64;

/// Wire frame type of outgoing msg.
enum OutFrame {
    Call { id: RequestId, name: String, args: Vec<u8> },
    Subscribe { reducer: String },
}

/// Wire frame type of incoming msg.
enum InFrame {
    Reply { id: RequestId, ret: Vec<u8> }
}

/// Failure mode of the underlying transport — I/O errors,
/// codec failures, peer disconnects.
enum TransportError {

}

/// Bidirectional message stream — a duplex channel over which two
/// peers exchange framed messages.
///
/// Each direction is independent: `send` pushes a message towards
/// the peer, `recv` yields the next message the peer sent us. The
/// two halves are decoupled — a peer may close its outgoing half
/// (via `close`) while the incoming half remains open, and `send`
/// / `recv` may be driven concurrently from different tasks.
///
/// Within one direction, messages are delivered in FIFO order.
/// Across directions there is no ordering relation.
///
/// ```text
///    +---- self ----+                    +---- peer ----+
///    |              |  ----- send ---->  |              |
///    |              |  <---- recv -----  |              |
///    +--------------+                    +--------------+
/// ```
///
/// `Transport` is intentionally agnostic about the protocol it
/// carries: distinguishing requests, responses, and server pushes
/// is the job of a higher layer that picks the `Message` type.
trait Transport {
    /// Send `msg` to the peer.
    ///
    /// Resolves once the message has been handed off to the
    /// underlying transport (not necessarily delivered to the
    /// peer's application).
    fn send(
        &mut self,
        msg: OutFrame,
    ) -> Pin<Box<dyn Future<Output = Result<(), TransportError>> + Send + '_>>;

    /// Wait for the next message from the peer.
    ///
    /// Resolves to `Ok(None)` once the peer has closed its
    /// outgoing half — no further messages will arrive on this
    /// transport.
    fn recv(
        &mut self,
    ) -> Pin<Box<dyn Future<Output = Result<Option<InFrame>, TransportError>> + Send + '_>>;

    /// Close the local outgoing half.
    ///
    /// Pending sends are flushed first; subsequent calls to `send`
    /// must return an error. The remote side's `recv` will
    /// eventually yield `Ok(None)`.
    fn close(
        &mut self,
    ) -> Pin<Box<dyn Future<Output = Result<(), TransportError>> + Send + '_>>;
}

struct Client {
    /// Transport write-half
    tx: Arc<Mutex<TransportWriter>>,

    /// Pending response registry: request id → where to put the result.
    pending_calls: Arc<Mutex<HashMap<RequestId, oneshot::Sender<InFrame>>>>,

    /// Register of active subscriptions: 
    /// subscription ID → where to send the delta.
    subscriptions: Arc<Mutex<HashMap<SubId, mpsc::Sender<InFrame>>>>,

    /// Message ID counter
    next_id: AtomicU64,

    /// Demultiplexer background task
    _demux: JoinHandle<()>,
}

impl Client {
    pub fn connect(transport: Box<dyn Transport>) -> Self {
        let (out_tx, mut out_rx) = mpsc::channel::<OutFrame>();
        let pending = Arc::new(Mutex::new(HashMap::new()));
        let subs    = Arc::new(Mutex::new(HashMap::new()));

        let demux = tokio::spawn({
            let pending = pending.clone();
            let subs    = subs.clone();

            async move {
                loop {
                    tokio::select! {
                        // Outgoing: given to transport
                        Some(out) = out_rx.recv() => {
                            let _ = transport.send(out).await;
                        }
                        // Incoming: routing
                        res = transport.recv() => match res {
                            Ok(Some(InFrame::Reply { id, .. })) => {
                                if let Some(s) = pending.lock().unwrap().remove(&id) {
                                    let _ = s.send(/* … */);
                                }
                            }
                            Ok(Some(InFrame::Patch { sub, .. })) => {
                                if let Some(tx) = subs.lock().unwrap().get(&sub) {
                                    let _ = tx.send(/* … */).await;
                                }
                            }
                            Ok(None) | Err(_) => break,
                            _ => {}
                        }
                    }
                }
            }
        });

        Client { 
            tx: Arc::new(Mutex::new(tx)), 
            pending_calls: pending,
            subscriptions: subs, 
            next_id: 0.into(), 
            _demux: demux,
        }
    }

    pub async fn call(&self, reducer: String) {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
        self.pending_calls.lock().unwrap().insert(id, reply_tx);

        self.tx.lock().await
            .send(OutFrame::Call { id, name })
            .await?;

        Ok(reply_rx.await?)
    }

    pub fn cold_view(reducer: String) {
        unimplemented!()
    }

    pub async fn materialized(&mut self, reducer: String) -> Result<, _> {
        // Server subscription
        self.transport.send(OutFrame::Subscribe { reducer }).await?;

        let initial = match self.transport.recv().await? {
            Some(InFrame::Snapshot(g)) => g,
            _ => return Err(/* protocol error */)
        };

        let graph = Arc::new(Mutex::new(initial));


    }
}