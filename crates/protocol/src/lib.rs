use std::future::Future;
use std::pin::Pin;

use serde::{Deserialize, Serialize};

pub use common::*;

pub type RequestId = u64;
pub type SubId = u64;

/// Wire frame type of outgoing msg.
#[derive(Debug, Serialize, Deserialize)]
pub enum OutFrame {
    Call {
        id: RequestId,
        name: String,
        args: Vec<HyperEdgeId>,
    },
    
    Subscribe {
        sub: SubId,
        name: HyperEdgeId,
    },
    Unsubscribe {
        sub: SubId,
    },
    
    Cold {
        id: RequestId,
        name: HyperEdgeId,
    },
    ColdRequestDelta {
        id: RequestId,
        tracker: TrackerId
    },
    ColdDropTracker {
        tracker: TrackerId
    },
}

#[derive(Debug, Serialize, Deserialize)]
pub enum CallError {
    /// Content correct number of args
    IncorrectNumberOfArgs(u8),
    /// Content not founded hyper edges
    ArgumentNotFound(Vec<HyperEdgeId>)
}

/// Wire frame type of incoming msg.
#[derive(Debug, Serialize, Deserialize)]
pub enum InFrame {
    CallReply { id: RequestId, ret: Result<Vec<Patch>, CallError> },
    /// Sended when hyper edge not found
    SubscribeError { id: SubId },
    SubscribeDelta { id: SubId, delta: Vec<Patch> },
    
    ColdInitialReply { id: RequestId, tracker: TrackerId, delta: Vec<Patch> },
    /// Sended when hyper edge not found
    ColdError { id: SubId },
    ColdDelta { id: RequestId, delta: Vec<Patch> }
}

/// Failure mode of the underlying transport — I/O errors,
/// codec failures, peer disconnects.
#[derive(Debug)]
pub enum TransportError {
    /// Transport (or one of its halves) has been closed.
    Closed,
    /// Underlying I/O error.
    Io(std::io::Error),
    /// Frame failed to encode or decode.
    Codec(String),
}

impl std::fmt::Display for TransportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Closed => write!(f, "transport closed"),
            Self::Io(e) => write!(f, "i/o error: {e}"),
            Self::Codec(s) => write!(f, "codec error: {s}"),
        }
    }
}

impl std::error::Error for TransportError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<std::io::Error> for TransportError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

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

    fn close(&mut self) -> Pin<Box<dyn Future<Output = Result<(), TransportError>> + Send + '_>>;
}
