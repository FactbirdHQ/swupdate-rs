use std::io;

/// Result type for SWUpdate IPC operations.
pub type Result<T> = std::result::Result<T, Error>;

/// Errors from SWUpdate IPC communication.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Socket connection or I/O failure.
    #[error("connection error: {0}")]
    Connection(#[from] io::Error),

    /// Protocol violation (unexpected message type, bad magic, etc.).
    #[error("protocol error: {0}")]
    Protocol(String),

    /// Server rejected the request (NACK).
    #[error("request rejected by swupdate")]
    Rejected,

    /// API version mismatch between client and server.
    #[error("version mismatch: expected {expected:#x}, got {actual:#x}")]
    VersionMismatch { expected: u32, actual: u32 },

    /// Timeout waiting for response.
    #[error("timeout after {0:?}")]
    Timeout(std::time::Duration),

    /// Invalid socket path.
    #[error("invalid socket path: {0}")]
    InvalidPath(String),
}
