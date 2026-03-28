use std::mem;
use std::path::Path;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;
use tracing::{debug, info};

use crate::error::{Error, Result};
use crate::socket::SocketConfig;
use crate::types::{
    HwRevision, InstallRequest, RecoveryStatus, Source, SubprocessCmd, SwupdateVar, UpdateState,
    UpdateStatus,
};
use crate::wire::{self, ipc_message_from_bytes, ipc_message_to_bytes, MsgType, RawIpcMessage};

/// Client for SWUpdate's control IPC socket.
///
/// Provides typed methods for all message types in the protocol.
pub struct ControlClient {
    stream: UnixStream,
    config: SocketConfig,
}

/// Handle for an in-progress installation.
///
/// Returned by [`ControlClient::install()`]. The stream methods live here —
/// you cannot stream image data without first sending an install request.
///
/// Borrows the `ControlClient` mutably, so no other commands can be sent
/// while the installation is in progress. Dropping the handle releases the
/// client for reuse.
///
/// ```rust,ignore
/// let mut ctrl = ControlClient::connect_default().await?;
///
/// // Typestate enforces: install() → stream → done
/// ctrl.install(&req).await?
///     .stream(&mut reader).await?;
///
/// // Client is available again for queries
/// ctrl.get_status().await?;
/// ```
pub struct Installation<'a> {
    client: &'a mut ControlClient,
}

impl ControlClient {
    /// Connect using default socket paths.
    pub async fn connect_default() -> Result<Self> {
        Self::connect(SocketConfig::default()).await
    }

    /// Connect with a custom socket configuration.
    pub async fn connect(config: SocketConfig) -> Result<Self> {
        let stream = config.connect_ctrl().await?;
        debug!(path = %config.ctrl_socket_path().display(), "Connected to control socket");
        Ok(Self { stream, config })
    }

    /// Reconnect to the control socket.
    pub async fn reconnect(&mut self) -> Result<()> {
        let _ = self.stream.shutdown().await;
        self.stream = self.config.connect_ctrl().await?;
        debug!("Reconnected to control socket");
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Installation
    // -----------------------------------------------------------------------

    /// Send an install request (REQ_INSTALL) and return a handle for streaming.
    ///
    /// The returned [`Installation`] holds a mutable borrow on this client.
    /// Use its [`stream()`](Installation::stream) or
    /// [`stream_file()`](Installation::stream_file) method to send image data.
    pub async fn install(&mut self, req: &InstallRequest) -> Result<Installation<'_>> {
        let mut msg = RawIpcMessage::new(MsgType::ReqInstall);
        msg.data.instmsg.req = req.to_raw();
        msg.data.instmsg.len = 0;
        self.send_and_expect_ack(&msg).await?;
        Ok(Installation { client: self })
    }

    // -----------------------------------------------------------------------
    // Status & state
    // -----------------------------------------------------------------------

    /// Query the current update status (GET_STATUS).
    pub async fn get_status(&mut self) -> Result<UpdateStatus> {
        let msg = RawIpcMessage::new(MsgType::GetStatus);
        let resp = self.send_and_receive(&msg).await?;

        // SAFETY: all union fields are POD valid for any bit pattern.
        // The message was received from swupdate which populates this variant.
        let (current, last_result, error, desc) = unsafe {
            let d = &resp.data.status;
            (
                d.current,
                d.last_result,
                d.error,
                wire::cstr_from_bytes(&d.desc),
            )
        };

        let current = RecoveryStatus::from_wire(current)
            .ok_or_else(|| Error::Protocol(format!("invalid status: {current}")))?;
        let last_result = RecoveryStatus::from_wire(last_result)
            .ok_or_else(|| Error::Protocol(format!("invalid last_result: {last_result}")))?;

        Ok(UpdateStatus {
            current,
            last_result,
            error,
            description: desc,
        })
    }

    /// Query the bootloader update state (GET_UPDATE_STATE).
    pub async fn get_update_state(&mut self) -> Result<UpdateState> {
        let msg = RawIpcMessage::new(MsgType::GetUpdateState);
        let resp = self.send_and_receive(&msg).await?;

        // SAFETY: reading msg variant (plain byte array)
        let val = unsafe {
            i32::from_ne_bytes([
                resp.data.msg[0],
                resp.data.msg[1],
                resp.data.msg[2],
                resp.data.msg[3],
            ])
        };
        UpdateState::from_wire(val)
            .ok_or_else(|| Error::Protocol(format!("invalid update state: {val}")))
    }

    /// Set the bootloader update state (SET_UPDATE_STATE).
    pub async fn set_update_state(&mut self, state: UpdateState) -> Result<()> {
        let mut msg = RawIpcMessage::new(MsgType::SetUpdateState);
        // SAFETY: writing to msg variant (plain byte array), message is zero-initialized
        unsafe {
            msg.data.msg[..4].copy_from_slice(&state.to_wire().to_ne_bytes());
        }
        self.send_and_expect_ack(&msg).await
    }

    // -----------------------------------------------------------------------
    // Post-update
    // -----------------------------------------------------------------------

    /// Send a post-update notification (POST_UPDATE).
    pub async fn post_update(&mut self) -> Result<()> {
        let msg = RawIpcMessage::new(MsgType::PostUpdate);
        self.send_and_expect_ack(&msg).await
    }

    // -----------------------------------------------------------------------
    // Configuration
    // -----------------------------------------------------------------------

    /// Set AES encryption key and IV (SET_AES_KEY).
    pub async fn set_aes_key(&mut self, key: &str, iv: &str) -> Result<()> {
        let mut msg = RawIpcMessage::new(MsgType::SetAesKey);
        // SAFETY: message is zero-initialized, all fields are POD
        unsafe {
            wire::write_cstr(&mut msg.data.aeskeymsg.key_ascii, key);
            wire::write_cstr(&mut msg.data.aeskeymsg.ivt_ascii, iv);
        }
        self.send_and_expect_ack(&msg).await
    }

    /// Set allowed version range (SET_VERSIONS_RANGE).
    pub async fn set_versions_range(
        &mut self,
        minimum: &str,
        maximum: &str,
        current: &str,
        update_type: &str,
    ) -> Result<()> {
        let mut msg = RawIpcMessage::new(MsgType::SetVersionsRange);
        // SAFETY: message is zero-initialized, all fields are POD
        unsafe {
            wire::write_cstr(&mut msg.data.versions.minimum_version, minimum);
            wire::write_cstr(&mut msg.data.versions.maximum_version, maximum);
            wire::write_cstr(&mut msg.data.versions.current_version, current);
            wire::write_cstr(&mut msg.data.versions.update_type, update_type);
        }
        self.send_and_expect_ack(&msg).await
    }

    /// Query hardware revision (GET_HW_REVISION).
    pub async fn get_hw_revision(&mut self) -> Result<HwRevision> {
        let msg = RawIpcMessage::new(MsgType::GetHwRevision);
        let resp = self.send_and_receive(&msg).await?;

        // SAFETY: response populates revisions variant. All fields are POD.
        let (boardname, revision) = unsafe {
            let d = &resp.data.revisions;
            (
                wire::cstr_from_bytes(&d.boardname),
                wire::cstr_from_bytes(&d.revision),
            )
        };
        Ok(HwRevision {
            boardname,
            revision,
        })
    }

    // -----------------------------------------------------------------------
    // Variables
    // -----------------------------------------------------------------------

    /// Set a SWUpdate variable (SET_SWUPDATE_VARS).
    pub async fn set_swupdate_var(
        &mut self,
        namespace: &str,
        name: &str,
        value: &str,
    ) -> Result<()> {
        let mut msg = RawIpcMessage::new(MsgType::SetSwupdateVars);
        // SAFETY: message is zero-initialized, all fields are POD
        unsafe {
            wire::write_cstr(&mut msg.data.vars.varnamespace, namespace);
            wire::write_cstr(&mut msg.data.vars.varname, name);
            wire::write_cstr(&mut msg.data.vars.varvalue, value);
        }
        self.send_and_expect_ack(&msg).await
    }

    /// Get a SWUpdate variable (GET_SWUPDATE_VARS).
    pub async fn get_swupdate_var(&mut self, namespace: &str, name: &str) -> Result<SwupdateVar> {
        let mut msg = RawIpcMessage::new(MsgType::GetSwupdateVars);
        // SAFETY: message is zero-initialized, all fields are POD
        unsafe {
            wire::write_cstr(&mut msg.data.vars.varnamespace, namespace);
            wire::write_cstr(&mut msg.data.vars.varname, name);
        }
        let resp = self.send_and_receive(&msg).await?;

        // SAFETY: response populates vars variant. All fields are POD.
        let (ns, n, v) = unsafe {
            let d = &resp.data.vars;
            (
                wire::cstr_from_bytes(&d.varnamespace),
                wire::cstr_from_bytes(&d.varname),
                wire::cstr_from_bytes(&d.varvalue),
            )
        };
        Ok(SwupdateVar {
            namespace: ns,
            name: n,
            value: v,
        })
    }

    // -----------------------------------------------------------------------
    // Subprocess (suricatta)
    // -----------------------------------------------------------------------

    /// Send a subprocess command (SWUPDATE_SUBPROCESS).
    pub async fn subprocess_cmd(
        &mut self,
        source: Source,
        cmd: SubprocessCmd,
        timeout: i32,
    ) -> Result<()> {
        let mut msg = RawIpcMessage::new(MsgType::SwupdateSubprocess);
        msg.data.procmsg.source = source.to_wire();
        msg.data.procmsg.cmd = cmd.to_wire();
        msg.data.procmsg.timeout = timeout;
        msg.data.procmsg.len = 0;
        self.send_and_expect_ack(&msg).await
    }

    // -----------------------------------------------------------------------
    // Delta URL
    // -----------------------------------------------------------------------

    /// Set delta update URL (SET_DELTA_URL).
    pub async fn set_delta_url(&mut self, filename: &str, url: &str) -> Result<()> {
        let mut msg = RawIpcMessage::new(MsgType::SetDeltaUrl);
        // SAFETY: message is zero-initialized, all fields are POD
        unsafe {
            wire::write_cstr(&mut msg.data.dwl_url.filename, filename);
            wire::write_cstr(&mut msg.data.dwl_url.url, url);
        }
        self.send_and_expect_ack(&msg).await
    }

    // -----------------------------------------------------------------------
    // Internal helpers
    // -----------------------------------------------------------------------

    /// Send a message and wait for ACK.
    async fn send_and_expect_ack(&mut self, msg: &RawIpcMessage) -> Result<()> {
        let resp = self.send_and_receive(msg).await?;
        match resp.msg_type() {
            Some(MsgType::Ack) => Ok(()),
            Some(MsgType::Nack) => Err(Error::Rejected),
            other => Err(Error::Protocol(format!("expected ACK/NACK, got {other:?}"))),
        }
    }

    /// Send a message and receive the response.
    async fn send_and_receive(&mut self, msg: &RawIpcMessage) -> Result<Box<RawIpcMessage>> {
        let bytes = ipc_message_to_bytes(msg);
        self.stream.write_all(bytes).await?;

        let mut buf = vec![0u8; mem::size_of::<RawIpcMessage>()];
        self.stream.read_exact(&mut buf).await?;

        ipc_message_from_bytes(&buf).ok_or_else(|| Error::Protocol("response size mismatch".into()))
    }
}

// ---------------------------------------------------------------------------
// Installation — typestate for the install → stream sequence
// ---------------------------------------------------------------------------

impl<'a> Installation<'a> {
    /// Subscribe to progress events for this installation.
    ///
    /// Opens a new connection to the progress socket. Each call returns an
    /// independent handle — like subscribing to a channel. The returned
    /// [`ProgressClient`] is fully owned and can be moved to another task.
    ///
    /// ```rust,ignore
    /// let mut install = ctrl.install(&req).await?;
    /// let mut progress = install.progress().await?;
    ///
    /// tokio::try_join!(
    ///     install.stream(&mut reader),
    ///     async {
    ///         loop {
    ///             let event = progress.receive().await?;
    ///             if event.is_terminal() { break; }
    ///         }
    ///         Ok::<_, swupdate_ipc::Error>(())
    ///     },
    /// )?;
    /// ```
    pub async fn progress(&self) -> Result<crate::progress::ProgressClient> {
        crate::progress::ProgressClient::connect(&self.client.config).await
    }

    /// Stream image data from any async reader to SWUpdate.
    ///
    /// Reads from `reader` until EOF, forwarding chunks directly to the
    /// control socket. Returns the number of bytes streamed.
    ///
    /// # Example: stream from reqwest
    ///
    /// ```rust,ignore
    /// let resp = reqwest::get("https://example.com/firmware.swu").await?;
    /// let reader = tokio_util::io::StreamReader::new(
    ///     resp.bytes_stream().map(|r| r.map_err(|e|
    ///         std::io::Error::new(std::io::ErrorKind::Other, e)
    ///     ))
    /// );
    /// tokio::pin!(reader);
    /// ctrl.install(&req).await?.stream(&mut reader).await?;
    /// ```
    pub async fn stream(&mut self, reader: &mut (dyn tokio::io::AsyncRead + Unpin)) -> Result<u64> {
        let mut buf = [0u8; 8192];
        let mut total: u64 = 0;

        loop {
            let n = reader.read(&mut buf).await?;
            if n == 0 {
                break;
            }
            self.client.stream.write_all(&buf[..n]).await?;
            total += n as u64;
        }

        info!(bytes = total, "Image streamed to swupdate");
        Ok(total)
    }

    /// Stream an image file to SWUpdate.
    ///
    /// Convenience wrapper around [`stream()`](Self::stream) that opens a file.
    pub async fn stream_file(&mut self, path: impl AsRef<Path>) -> Result<u64> {
        let path = path.as_ref();
        let mut file = tokio::fs::File::open(path)
            .await
            .map_err(|e| Error::Protocol(format!("failed to open {}: {e}", path.display())))?;
        self.stream(&mut file).await
    }
}

impl Drop for ControlClient {
    fn drop(&mut self) {
        // Best-effort shutdown — ignore errors since we're in Drop.
        let _ = self.stream.try_write(&[]);
    }
}
