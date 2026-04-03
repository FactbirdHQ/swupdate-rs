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
/// Maps to `update_state_t` in SWUpdate (state.h). These use ASCII character
/// values on the wire (`'0'` through `'7'`), stored as single-character strings
/// in the bootloader environment (e.g., `ustate=1` in grub.env).
///
/// Not to be confused with `RECOVERY_STATUS` which is used in progress messages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateState {
    /// `STATE_OK` — no update pending, system is confirmed good.
    Ok,
    /// `STATE_INSTALLED` — new image installed, awaiting validation.
    Installed,
    /// `STATE_TESTING` — currently running self-test.
    Testing,
    /// `STATE_FAILED` — update failed.
    Failed,
    /// `STATE_NOT_AVAILABLE` — no state information available.
    NotAvailable,
    /// `STATE_ERROR` — error during update.
    Error,
    /// `STATE_WAIT` — waiting for update.
    Wait,
    /// `STATE_IN_PROGRESS` — update in progress.
    InProgress,
}

impl UpdateState {
    /// Convert to the wire representation (ASCII character as i32).
    pub(crate) fn to_wire(self) -> i32 {
        match self {
            Self::Ok => b'0' as i32,
            Self::Installed => b'1' as i32,
            Self::Testing => b'2' as i32,
            Self::Failed => b'3' as i32,
            Self::NotAvailable => b'4' as i32,
            Self::Error => b'5' as i32,
            Self::Wait => b'6' as i32,
            Self::InProgress => b'7' as i32,
        }
    }

    /// Parse from wire representation (ASCII character as i32).
    pub(crate) fn from_wire(v: i32) -> Option<Self> {
        match v {
            v if v == b'0' as i32 => Some(Self::Ok),
            v if v == b'1' as i32 => Some(Self::Installed),
            v if v == b'2' as i32 => Some(Self::Testing),
            v if v == b'3' as i32 => Some(Self::Failed),
            v if v == b'4' as i32 => Some(Self::NotAvailable),
            v if v == b'5' as i32 => Some(Self::Error),
            v if v == b'6' as i32 => Some(Self::Wait),
            v if v == b'7' as i32 => Some(Self::InProgress),
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

/// Intermediate representation with all fields extracted from wire format.
/// Used to share parsing logic between packed and unpacked layouts.
pub(crate) struct ProgressFields {
    pub status: u32,
    pub dwl_percent: u32,
    pub dwl_bytes: u64,
    pub nsteps: u32,
    pub cur_step: u32,
    pub cur_percent: u32,
    pub cur_image: String,
    pub hnd_name: String,
    pub info: String,
}

impl From<&wire::RawProgressMsg> for ProgressFields {
    fn from(raw: &wire::RawProgressMsg) -> Self {
        // Block-scope copies required for packed struct field access
        let info_len = ({ raw.infolen } as usize).min(wire::PRINFOSIZE);
        Self {
            status: { raw.status },
            dwl_percent: { raw.dwl_percent },
            dwl_bytes: { raw.dwl_bytes },
            nsteps: { raw.nsteps },
            cur_step: { raw.cur_step },
            cur_percent: { raw.cur_percent },
            cur_image: wire::cstr_from_bytes(&raw.cur_image),
            hnd_name: wire::cstr_from_bytes(&raw.hnd_name),
            info: wire::cstr_from_bytes(&raw.info[..info_len]),
        }
    }
}

impl From<&wire::RawProgressMsgUnpacked> for ProgressFields {
    fn from(raw: &wire::RawProgressMsgUnpacked) -> Self {
        let info_len = (raw.infolen as usize).min(wire::PRINFOSIZE);
        Self {
            status: raw.status,
            dwl_percent: raw.dwl_percent,
            dwl_bytes: raw.dwl_bytes,
            nsteps: raw.nsteps,
            cur_step: raw.cur_step,
            cur_percent: raw.cur_percent,
            cur_image: wire::cstr_from_bytes(&raw.cur_image),
            hnd_name: wire::cstr_from_bytes(&raw.hnd_name),
            info: wire::cstr_from_bytes(&raw.info[..info_len]),
        }
    }
}

impl ProgressEvent {
    /// Convert from a packed raw progress message (swupdate >= 2025.12).
    pub(crate) fn from_raw(raw: &wire::RawProgressMsg) -> Self {
        Self::from_fields(ProgressFields::from(raw))
    }

    /// Convert from an unpacked raw progress message (swupdate <= 2025.05).
    pub(crate) fn from_raw_unpacked(raw: &wire::RawProgressMsgUnpacked) -> Self {
        Self::from_fields(ProgressFields::from(raw))
    }

    fn from_fields(f: ProgressFields) -> Self {
        match wire::RecoveryStatus::from_u32(f.status) {
            Some(wire::RecoveryStatus::Idle) => Self::Idle,
            Some(wire::RecoveryStatus::Start) => Self::Started,
            Some(wire::RecoveryStatus::Download) => Self::Downloading {
                percent: f.dwl_percent,
                bytes_total: f.dwl_bytes,
            },
            Some(wire::RecoveryStatus::Run | wire::RecoveryStatus::Progress) => Self::Installing {
                step: f.cur_step,
                total_steps: f.nsteps,
                percent: f.cur_percent,
                image: f.cur_image,
                handler: f.hnd_name,
            },
            Some(wire::RecoveryStatus::Success | wire::RecoveryStatus::Done) => Self::Success,
            Some(wire::RecoveryStatus::Failure) => Self::Failed(f.info),
            Some(wire::RecoveryStatus::Subprocess) => Self::Installing {
                step: f.cur_step,
                total_steps: f.nsteps,
                percent: f.cur_percent,
                image: f.cur_image,
                handler: f.hnd_name,
            },
            None => Self::Failed(format!("unknown status: {}", f.status)),
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

/// Recovery status, used in `get_status()` responses.
///
/// Maps to `RECOVERY_STATUS` in SWUpdate (swupdate_status.h). Not to be
/// confused with [`UpdateState`] which maps to `update_state_t`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryStatus {
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

impl RecoveryStatus {
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

/// Current update status returned by `get_status()`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateStatus {
    pub current: RecoveryStatus,
    pub last_result: RecoveryStatus,
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
            UpdateState::Ok,
            UpdateState::Installed,
            UpdateState::Testing,
            UpdateState::Failed,
            UpdateState::NotAvailable,
            UpdateState::Error,
            UpdateState::Wait,
            UpdateState::InProgress,
        ] {
            assert_eq!(UpdateState::from_wire(state.to_wire()), Some(state));
        }
    }
}
