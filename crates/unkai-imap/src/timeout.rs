//! Deadlines for the IMAP socket.
//!
//! # Why this exists
//!
//! `async-imap` has no notion of a timeout: every command is "write
//! the request, then read until the tagged response arrives". If the
//! peer stops answering but never closes the connection, that read
//! stays `Pending` forever and the future never resolves. Nothing
//! upstream can recover from that — the `fetch_envelopes` command
//! never returns, so the frontend's refresh promise never settles and
//! the pull-to-refresh spinner spins until the app is killed.
//!
//! On a desktop that is a rare annoyance. On a phone it is the normal
//! case: iOS suspends the process on backgrounding and tears down the
//! radio, so a socket opened over Wi-Fi is frequently a half-open
//! zombie by the time the app comes back — the TCP connection looks
//! alive locally, and every write into it succeeds, but no byte will
//! ever come back.
//!
//! # The approach
//!
//! Rather than wrap ~30 public client methods in
//! `tokio::time::timeout` (easy to add, easy to forget on the next
//! method), the deadline lives one layer below, on the byte stream
//! itself. [`IdleTimeoutStream`] wraps the TLS stream and fails any
//! I/O that makes **no progress at all** for [`Self::timeout`]. Every
//! command inherits it, including ones written later.
//!
//! "No progress" is deliberately the trigger, not "no completion": a
//! large FETCH that streams a 30 MB attachment over two minutes keeps
//! resetting the timer because bytes keep arriving. Only genuine
//! silence trips it. That is what makes a single, fairly short
//! timeout safe to apply to slow operations.

use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::time::{Instant, Sleep, sleep_until};

/// How long the socket may be completely silent before we give up.
///
/// Sized for a phone on a bad mobile connection: long enough that a
/// slow server or a brief tunnel doesn't abort a legitimate sync,
/// short enough that the user gets an error and a working refresh
/// button instead of an indefinite spinner.
pub const IDLE_TIMEOUT: Duration = Duration::from_secs(45);

/// How long to wait for the TCP connect and the TLS handshake.
///
/// Shorter than [`IDLE_TIMEOUT`]: reaching a server at all is either
/// quick or not going to happen, and this is the path the user waits
/// on with nothing yet on screen.
pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(20);

/// A stream that fails once no read or write has made progress for
/// `timeout`.
///
/// The deadline is shared between the read and write halves and is
/// reset by progress on *either*, so the semantic is exactly "this
/// connection has gone silent" rather than "this particular direction
/// is idle" — an IMAP session that is busy uploading an APPEND is not
/// stalled just because nothing is coming back yet.
#[derive(Debug)]
pub struct IdleTimeoutStream<S> {
    inner: S,
    timeout: Duration,
    /// Fires when the connection has been silent for `timeout`.
    /// Re-armed on every byte moved in either direction.
    deadline: Pin<Box<Sleep>>,
}

impl<S> IdleTimeoutStream<S> {
    pub fn new(inner: S, timeout: Duration) -> Self {
        Self {
            inner,
            timeout,
            deadline: Box::pin(sleep_until(Instant::now() + timeout)),
        }
    }

    /// Push the deadline out; called whenever the socket moved bytes.
    fn bump(&mut self) {
        let next = Instant::now() + self.timeout;
        self.deadline.as_mut().reset(next);
    }

    /// Has the connection been silent for too long?
    ///
    /// Polling the sleep is also what registers the waker, so this
    /// must be called on the `Pending` path of every I/O method —
    /// otherwise nothing would ever wake the task to notice the
    /// timeout, and a stalled connection would hang exactly as it did
    /// before.
    fn expired(&mut self, cx: &mut Context<'_>) -> bool {
        self.deadline.as_mut().poll(cx).is_ready()
    }
}

/// The error a stalled connection surfaces as.
///
/// `TimedOut` rather than a bespoke type so it travels the whole way
/// up through `async-imap`'s `io::Error` conversions and lands in
/// `UnkaiError::Network` with a message the user can act on.
fn stalled() -> io::Error {
    io::Error::new(
        io::ErrorKind::TimedOut,
        "the mail server stopped responding",
    )
}

// `poll` on a `Pin<Box<Sleep>>` needs the Future trait in scope.
use std::future::Future;

impl<S: AsyncRead + Unpin> AsyncRead for IdleTimeoutStream<S> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        match Pin::new(&mut this.inner).poll_read(cx, buf) {
            Poll::Ready(result) => {
                this.bump();
                Poll::Ready(result)
            }
            Poll::Pending => {
                if this.expired(cx) {
                    Poll::Ready(Err(stalled()))
                } else {
                    Poll::Pending
                }
            }
        }
    }
}

impl<S: AsyncWrite + Unpin> AsyncWrite for IdleTimeoutStream<S> {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        let this = self.get_mut();
        match Pin::new(&mut this.inner).poll_write(cx, buf) {
            Poll::Ready(result) => {
                this.bump();
                Poll::Ready(result)
            }
            Poll::Pending => {
                if this.expired(cx) {
                    Poll::Ready(Err(stalled()))
                } else {
                    Poll::Pending
                }
            }
        }
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        match Pin::new(&mut this.inner).poll_flush(cx) {
            Poll::Ready(result) => {
                this.bump();
                Poll::Ready(result)
            }
            Poll::Pending => {
                if this.expired(cx) {
                    Poll::Ready(Err(stalled()))
                } else {
                    Poll::Pending
                }
            }
        }
    }

    /// Shutdown deliberately does **not** enforce the deadline.
    ///
    /// This runs during `logout()`, after the useful work is done. A
    /// server that accepts the close but never acknowledges it should
    /// not turn a successful sync into a failed one — the caller
    /// already ignores the result (`let _ = client.logout()`), and the
    /// socket is dropped either way.
    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        Pin::new(&mut this.inner).poll_shutdown(cx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    /// A stream that accepts writes but never produces a byte —
    /// the half-open socket this module exists for.
    struct Silent;

    impl AsyncRead for Silent {
        fn poll_read(
            self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
            _buf: &mut ReadBuf<'_>,
        ) -> Poll<io::Result<()>> {
            Poll::Pending
        }
    }

    impl AsyncWrite for Silent {
        fn poll_write(
            self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
            buf: &[u8],
        ) -> Poll<io::Result<usize>> {
            Poll::Ready(Ok(buf.len()))
        }
        fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
            Poll::Ready(Ok(()))
        }
        fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
            Poll::Ready(Ok(()))
        }
    }

    #[tokio::test(start_paused = true)]
    async fn read_from_a_silent_peer_times_out() {
        let mut stream = IdleTimeoutStream::new(Silent, Duration::from_secs(5));
        let mut buf = [0u8; 16];
        let err = stream.read(&mut buf).await.unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::TimedOut);
    }

    /// The point of resetting on progress: a write keeps the
    /// connection alive, so the deadline must not fire against a
    /// session that is busy sending.
    #[tokio::test(start_paused = true)]
    async fn writes_push_the_deadline_out() {
        let mut stream = IdleTimeoutStream::new(Silent, Duration::from_secs(5));
        for _ in 0..10 {
            tokio::time::sleep(Duration::from_secs(3)).await;
            stream.write_all(b"ping").await.unwrap();
        }
        // 30s elapsed against a 5s timeout, and still healthy.
        stream.flush().await.unwrap();
    }
}
