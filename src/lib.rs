//! Async Rust client for [SWUpdate](https://sbabic.github.io/swupdate/)'s IPC interface.
//!
//! Communicates with the SWUpdate daemon over Unix domain sockets using the
//! binary protocol defined in `network_ipc.h` and `progress_ipc.h`.
//!
//! # Two clients
//!
//! - [`ControlClient`] — sends commands to the control socket (install,
//!   query status, set bootloader state, etc.)
//! - [`ProgressClient`] — receives progress events from the progress socket
//!
//! # Example
//!
//! ```rust,no_run
//! use swupdate_ipc::{ControlClient, InstallRequest, Source};
//!
//! # async fn example() -> swupdate_ipc::Result<()> {
//! let mut ctrl = ControlClient::connect_default().await?;
//!
//! let req = InstallRequest::new()
//!     .source(Source::Local)
//!     .info("firmware v2.0");
//!
//! // install() returns an Installation handle
//! let mut install = ctrl.install(&req).await?;
//!
//! // Optionally subscribe to progress (separate socket, owned handle)
//! let mut progress = install.progress().await?;
//!
//! // Stream and monitor concurrently
//! # let mut reader: &[u8] = &[];
//! tokio::try_join!(
//!     install.stream_file("/path/to/image.swu"),
//!     async {
//!         loop {
//!             let event = progress.receive().await?;
//!             println!("{event:?}");
//!             if event.is_terminal() { break; }
//!         }
//!         Ok(())
//!     },
//! )?;
//! # Ok(())
//! # }
//! ```

mod control;
mod error;
mod progress;
mod socket;
mod types;
/// Low-level wire protocol types matching SWUpdate's C headers.
///
/// These are `#[repr(C)]` structs used for binary-compatible I/O.
/// Most users should use the high-level types from the crate root instead.
#[doc(hidden)]
pub mod wire;

pub use control::{ControlClient, Installation};
pub use error::{Error, Result};
pub use progress::ProgressClient;
pub use socket::SocketConfig;
// ProgressClient is public (for the receive() API) but only constructable
// through Installation::progress() — not independently.
pub use types::{
    HwRevision, InstallRequest, ProgressEvent, RecoveryStatus, RunMode, Source, SubprocessCmd,
    SwupdateVar, UpdateState, UpdateStatus,
};
