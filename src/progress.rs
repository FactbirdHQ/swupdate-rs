use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;
use tracing::{debug, info, warn};
use zerocopy::{FromZeros, IntoBytes};

use crate::error::{Error, Result};
use crate::socket::SocketConfig;
use crate::types::ProgressEvent;
use crate::wire::{
    ProgressLayout, RawProgressAck, RawProgressMsg, RawProgressMsgUnpacked,
    PROGRESS_API_VERSION, PROGRESS_CONNECT_ACK_MAGIC,
};

/// Handle for receiving progress events from SWUpdate.
///
/// Created via [`Installation::progress()`](crate::Installation::progress).
/// Each call opens a new independent connection to the progress socket —
/// like subscribing to a channel. Multiple handles can coexist.
pub struct ProgressClient {
    stream: UnixStream,
    config: SocketConfig,
    layout: ProgressLayout,
}

impl ProgressClient {
    /// Connect to the progress socket and perform the handshake.
    ///
    /// Typically created via [`Installation::progress()`](crate::Installation::progress),
    /// but can also be constructed directly for standalone monitoring.
    pub async fn connect(config: &SocketConfig) -> Result<Self> {
        let layout = detect_layout().await;
        info!(?layout, "Detected swupdate progress message layout");
        let stream = config.connect_progress().await?;
        debug!(path = %config.progress_socket_path().display(), "Connected to progress socket");

        let mut client = Self {
            stream,
            config: config.clone(),
            layout,
        };
        client.handshake().await?;
        Ok(client)
    }

    /// Receive the next progress event.
    ///
    /// Blocks until a message arrives.
    pub async fn receive(&mut self) -> Result<ProgressEvent> {
        match self.layout {
            ProgressLayout::Packed => {
                let raw = self.read_raw_packed().await?;
                Ok(ProgressEvent::from_raw(&raw))
            }
            ProgressLayout::Unpacked => {
                let raw = self.read_raw_unpacked().await?;
                Ok(ProgressEvent::from_raw_unpacked(&raw))
            }
        }
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

    async fn read_raw_packed(&mut self) -> Result<RawProgressMsg> {
        let mut msg = RawProgressMsg::new_zeroed();
        self.stream.read_exact(msg.as_mut_bytes()).await?;
        Ok(msg)
    }

    async fn read_raw_unpacked(&mut self) -> Result<RawProgressMsgUnpacked> {
        let mut msg = RawProgressMsgUnpacked::new_zeroed();
        self.stream.read_exact(msg.as_mut_bytes()).await?;
        Ok(msg)
    }
}

impl Drop for ProgressClient {
    fn drop(&mut self) {
        let _ = self.stream.try_write(&[]);
    }
}

/// Detect progress message layout from the installed swupdate version.
///
/// Packed layout (`__attribute__((__packed__))`) was added upstream in commit
/// `485fd2be` (June 2025), after the v2025.05 release. Therefore:
/// - swupdate >= 2025.12 → Packed (2408 bytes)
/// - swupdate <= 2025.05 → Unpacked (2416 bytes)
///
/// Defaults to Packed if the version cannot be determined.
async fn detect_layout() -> ProgressLayout {
    let output: Option<std::process::Output> = tokio::process::Command::new("swupdate")
        .arg("--version")
        .output()
        .await
        .ok();

    let version_str = output
        .as_ref()
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
        .unwrap_or_default();

    match parse_swupdate_version(&version_str) {
        Some((year, month)) if (year, month) >= (2025, 12) => ProgressLayout::Packed,
        Some((year, month)) => {
            info!(
                year,
                month, "swupdate <= 2025.05 detected, using unpacked progress layout"
            );
            ProgressLayout::Unpacked
        }
        None => {
            debug!("Could not determine swupdate version, defaulting to packed layout");
            ProgressLayout::Packed
        }
    }
}

/// Parse "SWUpdate vYYYY.MM" or "SWUpdate vYYYY.MM.P" into (year, month).
pub(crate) fn parse_swupdate_version(output: &str) -> Option<(u32, u32)> {
    // Look for "vYYYY.MM" pattern in the output
    let version_part = output
        .split_whitespace()
        .find(|s| s.starts_with('v') && s.contains('.'))?;

    let without_v = version_part.strip_prefix('v')?;
    let mut parts = without_v.split('.');
    let year: u32 = parts.next()?.parse().ok()?;
    let month: u32 = parts.next()?.parse().ok()?;
    Some((year, month))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_version_full() {
        assert_eq!(
            parse_swupdate_version("SWUpdate v2024.12.1"),
            Some((2024, 12))
        );
    }

    #[test]
    fn parse_version_no_patch() {
        assert_eq!(
            parse_swupdate_version("SWUpdate v2025.05"),
            Some((2025, 5))
        );
    }

    #[test]
    fn parse_version_future() {
        assert_eq!(
            parse_swupdate_version("SWUpdate v2025.12"),
            Some((2025, 12))
        );
    }

    #[test]
    fn parse_version_empty() {
        assert_eq!(parse_swupdate_version(""), None);
    }

    #[test]
    fn parse_version_garbage() {
        assert_eq!(parse_swupdate_version("not a version"), None);
    }

    #[test]
    fn layout_from_old_version() {
        match parse_swupdate_version("SWUpdate v2024.12.1") {
            Some((year, month)) => assert!((year, month) < (2025, 12)),
            None => panic!("should parse"),
        }
    }

    #[test]
    fn layout_from_new_version() {
        match parse_swupdate_version("SWUpdate v2025.12") {
            Some((year, month)) => assert!((year, month) >= (2025, 12)),
            None => panic!("should parse"),
        }
    }

    #[tokio::test]
    async fn packed_receive() {
        let (mut server, client_stream) = UnixStream::pair().unwrap();
        let mut client = ProgressClient {
            stream: client_stream,
            config: SocketConfig::default(),
            layout: ProgressLayout::Packed,
        };

        let mut msg = RawProgressMsg::new_zeroed();
        msg.apiversion = PROGRESS_API_VERSION;
        msg.status = 3; // Success
        msg.dwl_percent = 100;

        use tokio::io::AsyncWriteExt;
        server.write_all(msg.as_bytes()).await.unwrap();

        let event = client.receive().await.unwrap();
        assert_eq!(event, ProgressEvent::Success);
    }

    #[tokio::test]
    async fn unpacked_receive() {
        let (mut server, client_stream) = UnixStream::pair().unwrap();
        let mut client = ProgressClient {
            stream: client_stream,
            config: SocketConfig::default(),
            layout: ProgressLayout::Unpacked,
        };

        let mut msg = RawProgressMsgUnpacked::new_zeroed();
        msg.apiversion = PROGRESS_API_VERSION;
        msg.status = 3; // Success
        msg.dwl_percent = 100;

        use tokio::io::AsyncWriteExt;
        server.write_all(msg.as_bytes()).await.unwrap();

        let event = client.receive().await.unwrap();
        assert_eq!(event, ProgressEvent::Success);
    }
}
