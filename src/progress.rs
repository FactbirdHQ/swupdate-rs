use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;
use tracing::{debug, warn};
use zerocopy::{FromZeros, IntoBytes};

use crate::error::{Error, Result};
use crate::socket::SocketConfig;
use crate::types::ProgressEvent;
use crate::wire::{
    RawProgressAck, RawProgressMsg, PROGRESS_API_VERSION, PROGRESS_CONNECT_ACK_MAGIC,
};

/// Handle for receiving progress events from SWUpdate.
///
/// Created via [`Installation::progress()`](crate::Installation::progress).
/// Each call opens a new independent connection to the progress socket —
/// like subscribing to a channel. Multiple handles can coexist.
pub struct ProgressClient {
    stream: UnixStream,
    config: SocketConfig,
}

impl ProgressClient {
    /// Connect to the progress socket and perform the handshake.
    ///
    /// Typically created via [`Installation::progress()`](crate::Installation::progress),
    /// but can also be constructed directly for standalone monitoring.
    pub async fn connect(config: &SocketConfig) -> Result<Self> {
        let stream = config.connect_progress().await?;
        debug!(path = %config.progress_socket_path().display(), "Connected to progress socket");

        let mut client = Self {
            stream,
            config: config.clone(),
        };
        client.handshake().await?;
        Ok(client)
    }

    /// Receive the next progress event.
    ///
    /// Blocks until a message arrives.
    pub async fn receive(&mut self) -> Result<ProgressEvent> {
        let raw = self.read_raw().await?;
        Ok(ProgressEvent::from_raw(&raw))
    }

    /// Receive with auto-reconnect on connection loss.
    ///
    /// If the read fails with a connection error, attempts to reconnect once
    /// before returning an error.
    pub async fn receive_or_reconnect(&mut self) -> Result<ProgressEvent> {
        match self.receive().await {
            Ok(event) => Ok(event),
            Err(Error::Connection(_)) => {
                warn!("Progress connection lost, reconnecting...");
                self.reconnect().await?;
                self.receive().await
            }
            Err(e) => Err(e),
        }
    }

    async fn reconnect(&mut self) -> Result<()> {
        let _ = self.stream.shutdown().await;
        self.stream = self.config.connect_progress().await?;
        self.handshake().await?;
        debug!("Reconnected to progress socket");
        Ok(())
    }

    async fn handshake(&mut self) -> Result<()> {
        let timeout = Duration::from_secs(5);
        let ack = match tokio::time::timeout(timeout, self.read_ack()).await {
            Ok(Ok(ack)) => ack,
            Ok(Err(e)) => return Err(e),
            Err(_) => return Err(Error::Timeout(timeout)),
        };

        if ack.magic != *PROGRESS_CONNECT_ACK_MAGIC {
            return Err(Error::Protocol(format!(
                "invalid ACK magic: {:?}",
                &ack.magic
            )));
        }

        let server_major = (ack.apiversion >> 16) & 0xFFFF;
        let client_major = (PROGRESS_API_VERSION >> 16) & 0xFFFF;
        if server_major != client_major {
            return Err(Error::VersionMismatch {
                expected: PROGRESS_API_VERSION,
                actual: ack.apiversion,
            });
        }

        debug!(
            server_version = format!("{:#x}", ack.apiversion),
            "Progress handshake OK"
        );
        Ok(())
    }

    async fn read_ack(&mut self) -> Result<RawProgressAck> {
        let mut ack = RawProgressAck {
            apiversion: 0,
            magic: [0; 4],
        };
        self.stream.read_exact(ack.as_mut_bytes()).await?;
        Ok(ack)
    }

    async fn read_raw(&mut self) -> Result<RawProgressMsg> {
        let mut msg = RawProgressMsg::new_zeroed();
        self.stream.read_exact(msg.as_mut_bytes()).await?;

        let version = { msg.apiversion };
        let server_major = (version >> 16) & 0xFFFF;
        let client_major = (PROGRESS_API_VERSION >> 16) & 0xFFFF;
        if server_major != client_major {
            return Err(Error::VersionMismatch {
                expected: PROGRESS_API_VERSION,
                actual: version,
            });
        }

        Ok(msg)
    }
}

impl Drop for ProgressClient {
    fn drop(&mut self) {
        let _ = self.stream.try_write(&[]);
    }
}
