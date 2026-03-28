use std::path::{Path, PathBuf};
use std::time::Duration;

use tokio::net::UnixStream;

use crate::error::{Error, Result};

/// Default socket name for the control interface.
const CTRL_SOCKET_NAME: &str = "sockinstctrl";

/// Default socket name for the progress interface.
const PROGRESS_SOCKET_NAME: &str = "swupdateprog";

/// Configuration for connecting to SWUpdate sockets.
#[derive(Debug, Clone)]
pub struct SocketConfig {
    ctrl_path: PathBuf,
    progress_path: PathBuf,
    connect_timeout: Duration,
}

impl Default for SocketConfig {
    fn default() -> Self {
        Self {
            ctrl_path: resolve_socket_path(CTRL_SOCKET_NAME),
            progress_path: resolve_socket_path(PROGRESS_SOCKET_NAME),
            connect_timeout: Duration::from_secs(10),
        }
    }
}

impl SocketConfig {
    /// Override the control socket path.
    pub fn ctrl_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.ctrl_path = path.into();
        self
    }

    /// Override the progress socket path.
    pub fn progress_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.progress_path = path.into();
        self
    }

    /// Set the connection timeout.
    pub fn connect_timeout(mut self, timeout: Duration) -> Self {
        self.connect_timeout = timeout;
        self
    }

    /// Get the resolved control socket path.
    pub fn ctrl_socket_path(&self) -> &Path {
        &self.ctrl_path
    }

    /// Get the resolved progress socket path.
    pub fn progress_socket_path(&self) -> &Path {
        &self.progress_path
    }

    /// Get the connection timeout.
    pub fn timeout(&self) -> Duration {
        self.connect_timeout
    }

    /// Connect to the control socket.
    pub(crate) async fn connect_ctrl(&self) -> Result<UnixStream> {
        self.connect_socket(&self.ctrl_path).await
    }

    /// Connect to the progress socket.
    pub(crate) async fn connect_progress(&self) -> Result<UnixStream> {
        self.connect_socket(&self.progress_path).await
    }

    async fn connect_socket(&self, path: &Path) -> Result<UnixStream> {
        if !path.exists() {
            return Err(Error::InvalidPath(path.display().to_string()));
        }
        match tokio::time::timeout(self.connect_timeout, UnixStream::connect(path)).await {
            Ok(Ok(stream)) => Ok(stream),
            Ok(Err(e)) => Err(Error::Connection(e)),
            Err(_) => Err(Error::Timeout(self.connect_timeout)),
        }
    }
}

/// Resolve the socket path following SWUpdate's priority order:
///
/// 1. `RUNTIME_DIRECTORY` env var
/// 2. `TMPDIR` env var
/// 3. `/run/swupdate` (if it exists)
/// 4. `/tmp` (fallback)
fn resolve_socket_path(socket_name: &str) -> PathBuf {
    if let Ok(dir) = std::env::var("RUNTIME_DIRECTORY") {
        return PathBuf::from(dir).join(socket_name);
    }

    if let Ok(dir) = std::env::var("TMPDIR") {
        return PathBuf::from(dir).join(socket_name);
    }

    let run_dir = Path::new("/run/swupdate");
    if run_dir.exists() {
        return run_dir.join(socket_name);
    }

    PathBuf::from("/tmp").join(socket_name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_has_sensible_paths() {
        let config = SocketConfig::default();
        let ctrl = config.ctrl_socket_path().to_string_lossy();
        let progress = config.progress_socket_path().to_string_lossy();

        assert!(ctrl.ends_with(CTRL_SOCKET_NAME));
        assert!(progress.ends_with(PROGRESS_SOCKET_NAME));
    }

    #[test]
    fn builder_overrides() {
        let config = SocketConfig::default()
            .ctrl_path("/custom/ctrl")
            .progress_path("/custom/progress")
            .connect_timeout(Duration::from_secs(30));

        assert_eq!(config.ctrl_socket_path(), Path::new("/custom/ctrl"));
        assert_eq!(config.progress_socket_path(), Path::new("/custom/progress"));
        assert_eq!(config.timeout(), Duration::from_secs(30));
    }
}
