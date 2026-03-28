//! Public, idiomatic Rust types for the SWUpdate IPC protocol.
//!
//! These types are what users interact with. Conversion to/from the wire
//! format ([`crate::wire`]) happens inside the client implementations.

use crate::wire;

// ---------------------------------------------------------------------------
// Source — where the update originates
// ---------------------------------------------------------------------------

/// Source that triggered the update.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    Unknown,
    Webserver,
    Suricatta,
    Downloader,
    Local,
    ChunksDownloader,
}

impl Source {
    pub(crate) fn to_wire(self) -> i32 {
        match self {
            Self::Unknown => 0,
            Self::Webserver => 1,
            Self::Suricatta => 2,
            Self::Downloader => 3,
            Self::Local => 4,
            Self::ChunksDownloader => 5,
        }
    }

    #[cfg(test)]
    fn from_wire(v: i32) -> Self {
        match v {
            1 => Self::Webserver,
            2 => Self::Suricatta,
            3 => Self::Downloader,
            4 => Self::Local,
            5 => Self::ChunksDownloader,
            _ => Self::Unknown,
        }
    }
}

// ---------------------------------------------------------------------------
// RunMode — execution mode for install requests
// ---------------------------------------------------------------------------

/// Execution mode for an install request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RunMode {
    #[default]
    Default,
    DryRun,
    Install,
}

impl RunMode {
    pub(crate) fn to_wire(self) -> i32 {
        match self {
            Self::Default => 0,
            Self::DryRun => 1,
            Self::Install => 2,
        }
    }
}

// ---------------------------------------------------------------------------
// UpdateState — bootloader update state (GET/SET_UPDATE_STATE)
// ---------------------------------------------------------------------------

/// Bootloader update state, used with `get_update_state` / `set_update_state`.
///
/// Maps to `RECOVERY_STATUS` in SWUpdate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateState {
    Idle,
    Start,
    Run,
    Success,
    Failure,
    Download,
    Done,
    Subprocess,
    Progress,
}

impl UpdateState {
    pub(crate) fn to_wire(self) -> i32 {
        match self {
            Self::Idle => 0,
            Self::Start => 1,
            Self::Run => 2,
            Self::Success => 3,
            Self::Failure => 4,
            Self::Download => 5,
            Self::Done => 6,
            Self::Subprocess => 7,
            Self::Progress => 8,
        }
    }

    pub(crate) fn from_wire(v: i32) -> Option<Self> {
        match v {
            0 => Some(Self::Idle),
            1 => Some(Self::Start),
            2 => Some(Self::Run),
            3 => Some(Self::Success),
            4 => Some(Self::Failure),
            5 => Some(Self::Download),
            6 => Some(Self::Done),
            7 => Some(Self::Subprocess),
            8 => Some(Self::Progress),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// SubprocessCmd — suricatta subprocess commands
// ---------------------------------------------------------------------------

/// Subprocess command for `subprocess_cmd()`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubprocessCmd {
    Activation,
    Config,
    Enable,
    GetStatus,
    SetDownloadUrl,
}

impl SubprocessCmd {
    pub(crate) fn to_wire(self) -> i32 {
        match self {
            Self::Activation => 0,
            Self::Config => 1,
            Self::Enable => 2,
            Self::GetStatus => 3,
            Self::SetDownloadUrl => 4,
        }
    }
}

// ---------------------------------------------------------------------------
// ProgressEvent — high-level progress from the progress socket
// ---------------------------------------------------------------------------

/// A progress event from SWUpdate's progress socket.
#[derive(Debug, Clone, PartialEq)]
pub enum ProgressEvent {
    /// Update has started.
    Started,
    /// Downloading image data.
    Downloading { percent: u32, bytes_total: u64 },
    /// Installing a step.
    Installing {
        step: u32,
        total_steps: u32,
        percent: u32,
        image: String,
        handler: String,
    },
    /// Update completed successfully.
    Success,
    /// Update failed.
    Failed(String),
    /// Idle — no update in progress.
    Idle,
}

impl ProgressEvent {
    /// Convert from a raw progress message.
    pub(crate) fn from_raw(raw: &wire::RawProgressMsg) -> Self {
        // Access packed fields by copying to locals
        let status = { raw.status };
        let dwl_percent = { raw.dwl_percent };
        let dwl_bytes = { raw.dwl_bytes };
        let nsteps = { raw.nsteps };
        let cur_step = { raw.cur_step };
        let cur_percent = { raw.cur_percent };
        let infolen = { raw.infolen };

        let image = wire::cstr_from_bytes(&raw.cur_image);
        let handler = wire::cstr_from_bytes(&raw.hnd_name);
        let info_len = (infolen as usize).min(wire::PRINFOSIZE);
        let info = wire::cstr_from_bytes(&raw.info[..info_len]);

        match wire::RecoveryStatus::from_u32(status) {
            Some(wire::RecoveryStatus::Idle) => Self::Idle,
            Some(wire::RecoveryStatus::Start) => Self::Started,
            Some(wire::RecoveryStatus::Download) => Self::Downloading {
                percent: dwl_percent,
                bytes_total: dwl_bytes,
            },
            Some(wire::RecoveryStatus::Run | wire::RecoveryStatus::Progress) => Self::Installing {
                step: cur_step,
                total_steps: nsteps,
                percent: cur_percent,
                image,
                handler,
            },
            Some(wire::RecoveryStatus::Success | wire::RecoveryStatus::Done) => Self::Success,
            Some(wire::RecoveryStatus::Failure) => Self::Failed(info),
            Some(wire::RecoveryStatus::Subprocess) => Self::Installing {
                step: cur_step,
                total_steps: nsteps,
                percent: cur_percent,
                image,
                handler,
            },
            None => Self::Failed(format!("unknown status: {status}")),
        }
    }

    /// Whether this event indicates the update is complete (success or failure).
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Success | Self::Failed(_))
    }
}

// ---------------------------------------------------------------------------
// InstallRequest — builder for REQ_INSTALL
// ---------------------------------------------------------------------------

/// Configuration for an install request sent to SWUpdate.
#[derive(Debug, Clone)]
pub struct InstallRequest {
    pub(crate) source: Source,
    pub(crate) mode: RunMode,
    pub(crate) info: String,
    pub(crate) software_set: Option<String>,
    pub(crate) running_mode: Option<String>,
    pub(crate) len: Option<u64>,
    pub(crate) store_swu: bool,
}

impl Default for InstallRequest {
    fn default() -> Self {
        Self {
            source: Source::Local,
            mode: RunMode::Default,
            info: String::new(),
            software_set: None,
            running_mode: None,
            len: None,
            store_swu: true,
        }
    }
}

impl InstallRequest {
    /// Create a new install request with default settings (local source).
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the update source.
    pub fn source(mut self, source: Source) -> Self {
        self.source = source;
        self
    }

    /// Set the execution mode (default, dry-run, or install).
    pub fn mode(mut self, mode: RunMode) -> Self {
        self.mode = mode;
        self
    }

    /// Set the info string (typically the firmware description).
    pub fn info(mut self, info: impl Into<String>) -> Self {
        self.info = info.into();
        self
    }

    /// Set the software set name.
    pub fn software_set(mut self, set: impl Into<String>) -> Self {
        self.software_set = Some(set.into());
        self
    }

    /// Set the running mode name.
    pub fn running_mode(mut self, mode: impl Into<String>) -> Self {
        self.running_mode = Some(mode.into());
        self
    }

    /// Set the expected image length in bytes.
    pub fn len(mut self, len: u64) -> Self {
        self.len = Some(len);
        self
    }

    /// Whether to store the SWU file (default: true).
    pub fn store_swu(mut self, store: bool) -> Self {
        self.store_swu = store;
        self
    }

    /// Convert to wire format.
    pub(crate) fn to_raw(&self) -> wire::RawSwupdateRequest {
        let mut req = wire::RawSwupdateRequest::zeroed();
        req.apiversion = wire::SWUPDATE_API_VERSION;
        req.source = self.source.to_wire();
        req.dry_run = self.mode.to_wire();
        req.len = self.len.unwrap_or(0) as usize;
        wire::write_cstr(&mut req.info, &self.info);
        if let Some(ref set) = self.software_set {
            wire::write_cstr(&mut req.software_set, set);
        }
        if let Some(ref mode) = self.running_mode {
            wire::write_cstr(&mut req.running_mode, mode);
        }
        req.disable_store_swu = if self.store_swu { 0 } else { 1 };
        req
    }
}

// ---------------------------------------------------------------------------
// Response types for query operations
// ---------------------------------------------------------------------------

/// Current update status returned by `get_status()`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateStatus {
    pub current: UpdateState,
    pub last_result: UpdateState,
    pub error: i32,
    pub description: String,
}

/// Hardware revision info returned by `get_hw_revision()`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HwRevision {
    pub boardname: String,
    pub revision: String,
}

/// SWUpdate variable (namespace, name, value).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SwupdateVar {
    pub namespace: String,
    pub name: String,
    pub value: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_request_defaults() {
        let req = InstallRequest::new();
        assert_eq!(req.source, Source::Local);
        assert_eq!(req.mode, RunMode::Default);
        assert!(req.store_swu);
    }

    #[test]
    fn install_request_builder() {
        let req = InstallRequest::new()
            .source(Source::Suricatta)
            .mode(RunMode::DryRun)
            .info("test firmware v1.0")
            .software_set("main")
            .len(1024)
            .store_swu(false);

        assert_eq!(req.source, Source::Suricatta);
        assert_eq!(req.mode, RunMode::DryRun);
        assert_eq!(req.info, "test firmware v1.0");
        assert_eq!(req.software_set.as_deref(), Some("main"));
        assert_eq!(req.len, Some(1024));
        assert!(!req.store_swu);
    }

    #[test]
    fn install_request_to_raw_roundtrip() {
        let req = InstallRequest::new()
            .source(Source::Local)
            .info("my firmware");

        let raw = req.to_raw();
        assert_eq!(raw.apiversion, wire::SWUPDATE_API_VERSION);
        assert_eq!(raw.source, Source::Local.to_wire());
        assert_eq!(wire::cstr_from_bytes(&raw.info), "my firmware");
        assert_eq!(raw.disable_store_swu, 0); // store_swu = true → disable = 0
    }

    #[test]
    fn progress_event_terminal() {
        assert!(ProgressEvent::Success.is_terminal());
        assert!(ProgressEvent::Failed("oops".into()).is_terminal());
        assert!(!ProgressEvent::Idle.is_terminal());
        assert!(!ProgressEvent::Started.is_terminal());
    }

    #[test]
    fn source_roundtrip() {
        for src in [
            Source::Unknown,
            Source::Webserver,
            Source::Suricatta,
            Source::Downloader,
            Source::Local,
            Source::ChunksDownloader,
        ] {
            assert_eq!(Source::from_wire(src.to_wire()), src);
        }
    }

    #[test]
    fn update_state_roundtrip() {
        for state in [
            UpdateState::Idle,
            UpdateState::Start,
            UpdateState::Run,
            UpdateState::Success,
            UpdateState::Failure,
            UpdateState::Download,
            UpdateState::Done,
            UpdateState::Subprocess,
            UpdateState::Progress,
        ] {
            assert_eq!(UpdateState::from_wire(state.to_wire()), Some(state));
        }
    }
}
