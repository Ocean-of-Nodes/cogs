use std::future::Future;
use std::io;
use std::pin::Pin;

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::{TcpStream, ToSocketAddrs};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

#[cfg(unix)]
use std::path::Path;
#[cfg(unix)]
use tokio::net::UnixStream;

use protocol::{InFrame, OutFrame, Transport, TransportError};

// ============================================================
//  In-memory transport (for tests)
// ============================================================

/// Construct an in-memory client/server pair. The first value is the
/// client end (impls [`Transport`]); the second is a [`ServerHandle`]
/// that lets a test act as the server — it reads `OutFrame`s the
/// client sends and pushes `InFrame`s back.
pub fn in_memory_pair() -> (InMemoryTransport, ServerHandle) {
    let (out_tx, out_rx) = mpsc::channel::<OutFrame>(64);
    let (in_tx, in_rx) = mpsc::channel::<InFrame>(64);
    (
        InMemoryTransport {
            out_tx: Some(out_tx),
            in_rx,
        },
        ServerHandle { out_rx, in_tx },
    )
}

/// Channel-backed [`Transport`]. No serialization, no I/O.
pub struct InMemoryTransport {
    out_tx: Option<mpsc::Sender<OutFrame>>,
    in_rx: mpsc::Receiver<InFrame>,
}

impl Transport for InMemoryTransport {
    fn send(
        &mut self,
        msg: OutFrame,
    ) -> Pin<Box<dyn Future<Output = Result<(), TransportError>> + Send + '_>> {
        let tx = self.out_tx.clone();
        Box::pin(async move {
            match tx {
                Some(tx) => tx.send(msg).await.map_err(|_| TransportError::Closed),
                None => Err(TransportError::Closed),
            }
        })
    }

    fn recv(
        &mut self,
    ) -> Pin<Box<dyn Future<Output = Result<Option<InFrame>, TransportError>> + Send + '_>> {
        Box::pin(async move { Ok(self.in_rx.recv().await) })
    }

    fn close(
        &mut self,
    ) -> Pin<Box<dyn Future<Output = Result<(), TransportError>> + Send + '_>> {
        self.out_tx = None;
        Box::pin(async move { Ok(()) })
    }
}

/// Server end of an [`in_memory_pair`]. Not a [`Transport`].
pub struct ServerHandle {
    out_rx: mpsc::Receiver<OutFrame>,
    in_tx: mpsc::Sender<InFrame>,
}

impl ServerHandle {
    /// Receive the next frame the client sent.
    pub async fn recv(&mut self) -> Option<OutFrame> {
        self.out_rx.recv().await
    }

    /// Push a frame to the client.
    pub async fn send(&self, frame: InFrame) -> Result<(), TransportError> {
        self.in_tx
            .send(frame)
            .await
            .map_err(|_| TransportError::Closed)
    }
}

// ============================================================
//  Socket transport (TCP + Unix domain)
// ============================================================

/// Socket-backed [`Transport`] with bincode framing (4-byte big-endian
/// length prefix + payload).
///
/// Internally spawns reader and writer tasks that own the two halves
/// of the underlying socket. Public `send` / `recv` are thin wrappers
/// over `mpsc` channels into those tasks, which makes them
/// cancellation-safe regardless of the underlying I/O.
pub struct SocketTransport {
    out_tx: Option<mpsc::Sender<OutFrame>>,
    in_rx: mpsc::Receiver<Result<InFrame, TransportError>>,
    _io: JoinHandle<()>,
}

impl SocketTransport {
    // -------- TCP --------

    /// Open a TCP connection and wrap it.
    pub async fn connect_tcp(addr: impl ToSocketAddrs) -> io::Result<Self> {
        let stream = TcpStream::connect(addr).await?;
        Ok(Self::from_tcp(stream))
    }

    /// Wrap an already-connected `TcpStream` (e.g. one obtained from
    /// `TcpListener::accept`).
    pub fn from_tcp(stream: TcpStream) -> Self {
        let (r, w) = stream.into_split();
        Self::from_halves(r, w)
    }

    // -------- Unix domain --------

    /// Open a Unix-domain connection at `path` and wrap it.
    #[cfg(unix)]
    pub async fn connect_unix(path: impl AsRef<Path>) -> io::Result<Self> {
        let stream = UnixStream::connect(path).await?;
        Ok(Self::from_unix(stream))
    }

    /// Wrap an already-connected `UnixStream`.
    #[cfg(unix)]
    pub fn from_unix(stream: UnixStream) -> Self {
        let (r, w) = stream.into_split();
        Self::from_halves(r, w)
    }

    /// Two `SocketTransport`s connected in-kernel via `socketpair(2)`.
    /// Useful for tests and for in-process IPC without a filesystem
    /// path.
    #[cfg(unix)]
    pub fn unix_pair() -> io::Result<(Self, Self)> {
        let (a, b) = UnixStream::pair()?;
        Ok((Self::from_unix(a), Self::from_unix(b)))
    }

    // -------- Generic over halves --------

    fn from_halves<R, W>(read: R, write: W) -> Self
    where
        R: AsyncRead + Unpin + Send + 'static,
        W: AsyncWrite + Unpin + Send + 'static,
    {
        let (out_tx, out_rx) = mpsc::channel::<OutFrame>(64);
        let (in_tx, in_rx) = mpsc::channel::<Result<InFrame, TransportError>>(64);

        let io = tokio::spawn(async move {
            let reader = tokio::spawn(read_loop(read, in_tx));
            let writer = tokio::spawn(write_loop(write, out_rx));
            // When either side dies, drop the other so we don't leak the half.
            tokio::select! {
                _ = reader => {},
                _ = writer => {},
            }
        });

        Self {
            out_tx: Some(out_tx),
            in_rx,
            _io: io,
        }
    }
}

impl Transport for SocketTransport {
    fn send(
        &mut self,
        msg: OutFrame,
    ) -> Pin<Box<dyn Future<Output = Result<(), TransportError>> + Send + '_>> {
        let tx = self.out_tx.clone();
        Box::pin(async move {
            match tx {
                Some(tx) => tx.send(msg).await.map_err(|_| TransportError::Closed),
                None => Err(TransportError::Closed),
            }
        })
    }

    fn recv(
        &mut self,
    ) -> Pin<Box<dyn Future<Output = Result<Option<InFrame>, TransportError>> + Send + '_>> {
        Box::pin(async move {
            match self.in_rx.recv().await {
                Some(Ok(frame)) => Ok(Some(frame)),
                Some(Err(e)) => Err(e),
                None => Ok(None),
            }
        })
    }

    fn close(
        &mut self,
    ) -> Pin<Box<dyn Future<Output = Result<(), TransportError>> + Send + '_>> {
        // Drop the outgoing sender → writer drains and exits → its
        // exit aborts the reader through the driver's select!.
        self.out_tx = None;
        Box::pin(async move { Ok(()) })
    }
}

// ---------- framing ----------

async fn read_loop<R>(mut read: R, in_tx: mpsc::Sender<Result<InFrame, TransportError>>)
where
    R: AsyncRead + Unpin,
{
    loop {
        let mut len_buf = [0u8; 4];
        match read.read_exact(&mut len_buf).await {
            Ok(_) => {}
            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return,
            Err(e) => {
                let _ = in_tx.send(Err(TransportError::Io(e))).await;
                return;
            }
        }
        let len = u32::from_be_bytes(len_buf) as usize;
        let mut buf = vec![0u8; len];
        if let Err(e) = read.read_exact(&mut buf).await {
            let _ = in_tx.send(Err(TransportError::Io(e))).await;
            return;
        }
        match bincode::deserialize::<InFrame>(&buf) {
            Ok(frame) => {
                if in_tx.send(Ok(frame)).await.is_err() {
                    return;
                }
            }
            Err(e) => {
                let _ = in_tx
                    .send(Err(TransportError::Codec(e.to_string())))
                    .await;
                return;
            }
        }
    }
}

async fn write_loop<W>(mut write: W, mut out_rx: mpsc::Receiver<OutFrame>)
where
    W: AsyncWrite + Unpin,
{
    while let Some(frame) = out_rx.recv().await {
        let bytes = match bincode::serialize(&frame) {
            Ok(b) => b,
            Err(_) => return,
        };
        let len = (bytes.len() as u32).to_be_bytes();
        if write.write_all(&len).await.is_err() {
            return;
        }
        if write.write_all(&bytes).await.is_err() {
            return;
        }
    }
    let _ = write.shutdown().await;
}

// ============================================================
//  tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;
    use protocol::HyperedgeId;
    use tokio::net::TcpListener;

    fn arg_a() -> HyperedgeId {
        HyperedgeId::from_u128(0xa)
    }
    fn arg_b() -> HyperedgeId {
        HyperedgeId::from_u128(0xb)
    }

    fn sample_call() -> OutFrame {
        OutFrame::Call {
            id: 1,
            name: "echo".into(),
            args: vec![arg_a(), arg_b()],
        }
    }

    fn sample_reply() -> InFrame {
        InFrame::CallReply {
            id: 1,
            ret: Ok(vec![]),
        }
    }

    #[tokio::test]
    async fn in_memory_roundtrip() {
        let (mut client, mut server) = in_memory_pair();

        client.send(sample_call()).await.unwrap();
        match server.recv().await.unwrap() {
            OutFrame::Call { id, name, args } => {
                assert_eq!(id, 1);
                assert_eq!(name, "echo");
                assert_eq!(args, vec![arg_a(), arg_b()]);
            }
            _ => panic!("unexpected frame"),
        }

        server.send(sample_reply()).await.unwrap();
        match client.recv().await.unwrap().expect("recv yielded None") {
            InFrame::CallReply { id, ret } => {
                assert_eq!(id, 1);
                let patches = ret.expect("call should succeed");
                assert!(patches.is_empty());
            }
            other => panic!("unexpected frame: {other:?}"),
        }
    }

    #[tokio::test]
    async fn in_memory_close_makes_send_fail() {
        let (mut client, _server) = in_memory_pair();
        client.close().await.unwrap();

        let err = client.send(sample_call()).await.unwrap_err();
        assert!(matches!(err, TransportError::Closed));
    }

    /// Write an `InFrame` directly on the wire from the server side
    /// (so the client's transport can decode it via `recv`).
    async fn write_in_frame<W: AsyncWrite + Unpin>(w: &mut W, frame: &InFrame) {
        let bytes = bincode::serialize(frame).unwrap();
        let len = (bytes.len() as u32).to_be_bytes();
        w.write_all(&len).await.unwrap();
        w.write_all(&bytes).await.unwrap();
    }

    /// Read an `OutFrame` directly from the wire (so the server side
    /// can see what the client's transport sent via `send`).
    async fn read_out_frame<R: AsyncRead + Unpin>(r: &mut R) -> OutFrame {
        let mut len_buf = [0u8; 4];
        r.read_exact(&mut len_buf).await.unwrap();
        let len = u32::from_be_bytes(len_buf) as usize;
        let mut buf = vec![0u8; len];
        r.read_exact(&mut buf).await.unwrap();
        bincode::deserialize::<OutFrame>(&buf).unwrap()
    }

    #[tokio::test]
    async fn tcp_roundtrip() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let frame = read_out_frame(&mut stream).await;
            assert!(matches!(frame, OutFrame::Call { id: 1, .. }));
            write_in_frame(&mut stream, &sample_reply()).await;
            // Half-close so the client sees EOF on its next recv.
            stream.shutdown().await.unwrap();
        });

        let mut client = SocketTransport::connect_tcp(addr).await.unwrap();
        client.send(sample_call()).await.unwrap();

        match client.recv().await.unwrap().expect("recv yielded None") {
            InFrame::CallReply { id, ret } => {
                assert_eq!(id, 1);
                let patches = ret.expect("call should succeed");
                assert!(patches.is_empty());
            }
            other => panic!("unexpected frame: {other:?}"),
        }

        server.await.unwrap();
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn unix_socket_roundtrip() {
        let (mut server_wire, client_wire) = UnixStream::pair().unwrap();

        let server = tokio::spawn(async move {
            let frame = read_out_frame(&mut server_wire).await;
            assert!(matches!(frame, OutFrame::Call { id: 1, .. }));
            write_in_frame(&mut server_wire, &sample_reply()).await;
            server_wire.shutdown().await.unwrap();
        });

        let mut client = SocketTransport::from_unix(client_wire);
        client.send(sample_call()).await.unwrap();

        match client.recv().await.unwrap().expect("recv yielded None") {
            InFrame::CallReply { id, ret } => {
                assert_eq!(id, 1);
                let patches = ret.expect("call should succeed");
                assert!(patches.is_empty());
            }
            other => panic!("unexpected frame: {other:?}"),
        }

        server.await.unwrap();
    }
}
