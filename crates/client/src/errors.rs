use protocol::CallError;
use tokio::sync::{mpsc, oneshot};

/// Failure modes of [`Client::call`].
#[derive(Debug)]
pub enum ClientCallError {
    /// The server processed the request and returned a domain error.
    Call(CallError),
    /// The transport closed before we received a reply (peer
    /// disconnect, demux task gone, channel closed).
    Closed,
    /// The server sent a frame that wasn't a `CallReply` for our id.
    /// This can only happen if the demux registry is corrupted or the
    /// server is misbehaving.
    UnexpectedFrame,
}

impl std::fmt::Display for ClientCallError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Call(e) => write!(f, "call failed: {e:?}"),
            Self::Closed => f.write_str("client transport closed"),
            Self::UnexpectedFrame => f.write_str("server sent an unexpected frame"),
        }
    }
}

impl std::error::Error for ClientCallError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        // CallError doesn't currently impl Error itself; once it
        // does, return Some(e) here.
        None
    }
}

impl From<CallError> for ClientCallError {
    fn from(e: CallError) -> Self {
        Self::Call(e)
    }
}

impl<T> From<mpsc::error::SendError<T>> for ClientCallError {
    fn from(_: mpsc::error::SendError<T>) -> Self {
        Self::Closed
    }
}

impl From<oneshot::error::RecvError> for ClientCallError {
    fn from(_: oneshot::error::RecvError) -> Self {
        Self::Closed
    }
}