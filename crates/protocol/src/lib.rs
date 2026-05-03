use std::future::Future;
use std::pin::Pin;

pub type RequestId = u64;
pub type SubId = u64;

/// Wire frame type of outgoing msg.
#[derive(Debug)]
pub enum OutFrame {
    Call { id: RequestId, name: String, args: Vec<u8> },
    Subscribe { sub: SubId, name: String },
}

/// Wire frame type of incoming msg.
#[derive(Debug)]
pub enum InFrame {
    Reply { id: RequestId, ret: Vec<u8> },
}

/// Failure mode of the underlying transport — I/O errors,
/// codec failures, peer disconnects.
#[derive(Debug)]
pub enum TransportError {}

/// Bidirectional message stream — a duplex channel over which two
/// peers exchange framed messages.
///
/// Each direction is independent: `send` pushes a message towards
/// the peer, `recv` yields the next message the peer sent us. The
/// two halves are decoupled — a peer may close its outgoing half
/// (via `close`) while the incoming half remains open.
///
/// `Transport` is intentionally agnostic about the protocol it
/// carries: distinguishing requests, responses, and server pushes
/// is the job of a higher layer that picks the frame types.
///
/// # Cancellation safety
///
/// `recv` is typically driven inside a `tokio::select!`, which means
/// it can be dropped mid-poll. Implementations MUST be
/// cancellation-safe: dropping the `recv` future must not lose
/// buffered frames.
pub trait Transport: Send {
    fn send(
        &mut self,
        msg: OutFrame,
    ) -> Pin<Box<dyn Future<Output = Result<(), TransportError>> + Send + '_>>;

    fn recv(
        &mut self,
    ) -> Pin<Box<dyn Future<Output = Result<Option<InFrame>, TransportError>> + Send + '_>>;

    fn close(
        &mut self,
    ) -> Pin<Box<dyn Future<Output = Result<(), TransportError>> + Send + '_>>;
}
