//! Binary protocol types matching SWUpdate's C headers exactly.
//!
//! This module is crate-internal. Users interact with the idiomatic types in
//! [`crate::types`]; conversion happens inside the client implementations.
//!
//! ## Layout strategy
//!
//! - `progress_msg` is `__packed__` in C → `#[repr(C, packed)]` + zerocopy
//! - `ipc_message` uses a C union (`msgdata`) → `#[repr(C)]` union in Rust
//! - Individual union variant structs are `#[repr(C)]` + `Copy`
//! - All unsafe is contained in this module's helper functions

use std::mem;

use zerocopy::{FromBytes, Immutable, IntoBytes, KnownLayout};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

pub const IPC_MAGIC: i32 = 0x14052001;
pub const SWUPDATE_API_VERSION: u32 = 0x1;

pub const PROGRESS_API_MAJOR: u32 = 2;
pub const PROGRESS_API_MINOR: u32 = 0;
pub const PROGRESS_API_PATCH: u32 = 0;
pub const PROGRESS_API_VERSION: u32 = (PROGRESS_API_MAJOR & 0xFFFF) << 16
    | (PROGRESS_API_MINOR & 0xFF) << 8
    | (PROGRESS_API_PATCH & 0xFF);

pub const PROGRESS_CONNECT_ACK_MAGIC: &[u8; 4] = b"ACK\0";

/// Size of the info buffer in progress messages.
pub const PRINFOSIZE: usize = 2048;

// ---------------------------------------------------------------------------
// Message type enum (matches C `msgtype`)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum MsgType {
    ReqInstall = 0,
    Ack = 1,
    Nack = 2,
    GetStatus = 3,
    PostUpdate = 4,
    SwupdateSubprocess = 5,
    SetAesKey = 6,
    SetUpdateState = 7,
    GetUpdateState = 8,
    ReqInstallExt = 9,
    SetVersionsRange = 10,
    NotifyStream = 11,
    GetHwRevision = 12,
    SetSwupdateVars = 13,
    GetSwupdateVars = 14,
    SetDeltaUrl = 15,
}

impl MsgType {
    pub fn from_i32(v: i32) -> Option<Self> {
        match v {
            0 => Some(Self::ReqInstall),
            1 => Some(Self::Ack),
            2 => Some(Self::Nack),
            3 => Some(Self::GetStatus),
            4 => Some(Self::PostUpdate),
            5 => Some(Self::SwupdateSubprocess),
            6 => Some(Self::SetAesKey),
            7 => Some(Self::SetUpdateState),
            8 => Some(Self::GetUpdateState),
            9 => Some(Self::ReqInstallExt),
            10 => Some(Self::SetVersionsRange),
            11 => Some(Self::NotifyStream),
            12 => Some(Self::GetHwRevision),
            13 => Some(Self::SetSwupdateVars),
            14 => Some(Self::GetSwupdateVars),
            15 => Some(Self::SetDeltaUrl),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// RECOVERY_STATUS (matches C enum in swupdate_status.h)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum RecoveryStatus {
    Idle = 0,
    Start = 1,
    Run = 2,
    Success = 3,
    Failure = 4,
    Download = 5,
    Done = 6,
    Subprocess = 7,
    Progress = 8,
}

impl RecoveryStatus {
    pub fn from_u32(v: u32) -> Option<Self> {
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

// C enum representations (sourcetype, run_type, cmd) are handled in
// crate::types with idiomatic Rust enums. The wire representation uses
// plain i32/u32 fields in the repr(C) structs.

// ---------------------------------------------------------------------------
// progress_msg — two layouts depending on swupdate version
//
// swupdate >= 2025.12: struct is __attribute__((__packed__)) → 2408 bytes
// swupdate <= 2025.05: struct is unpacked (natural alignment) → 2416 bytes
// The __packed__ attribute was added upstream in commit 485fd2be (June 2025)
// without bumping PROGRESS_API_VERSION.
// ---------------------------------------------------------------------------

pub const PROGRESS_MSG_SIZE_PACKED: usize = 2408;
pub const PROGRESS_MSG_SIZE_UNPACKED: usize = 2416;

/// Progress message layout, determined by the installed swupdate version.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgressLayout {
    /// swupdate >= 2025.12: `__attribute__((__packed__))`, 2408 bytes
    Packed,
    /// swupdate <= 2025.05: natural alignment, 2416 bytes
    Unpacked,
}

impl ProgressLayout {
    pub fn msg_size(self) -> usize {
        match self {
            Self::Packed => PROGRESS_MSG_SIZE_PACKED,
            Self::Unpacked => PROGRESS_MSG_SIZE_UNPACKED,
        }
    }
}

/// Packed progress message (swupdate >= 2025.12).
#[derive(FromBytes, IntoBytes, Immutable, KnownLayout, Clone)]
#[repr(C, packed)]
pub struct RawProgressMsg {
    pub apiversion: u32,
    pub status: u32,
    pub dwl_percent: u32,
    pub dwl_bytes: u64,
    pub nsteps: u32,
    pub cur_step: u32,
    pub cur_percent: u32,
    pub cur_image: [u8; 256],
    pub hnd_name: [u8; 64],
    pub source: u32,
    pub infolen: u32,
    pub info: [u8; PRINFOSIZE],
}

const _: () = assert!(mem::size_of::<RawProgressMsg>() == PROGRESS_MSG_SIZE_PACKED);

/// Unpacked progress message (swupdate <= 2025.05).
///
/// Identical fields but `repr(C)` — the compiler inserts 4 bytes of alignment
/// padding before `dwl_bytes` (u64) and 4 bytes of tail padding, totalling
/// 2416 bytes on the wire.
#[derive(FromBytes, IntoBytes, Immutable, KnownLayout, Clone)]
#[repr(C)]
pub struct RawProgressMsgUnpacked {
    pub apiversion: u32,
    pub status: u32,
    pub dwl_percent: u32,
    /// Alignment padding before dwl_bytes (u64 requires 8-byte alignment).
    pub _pad_align: u32,
    pub dwl_bytes: u64,
    pub nsteps: u32,
    pub cur_step: u32,
    pub cur_percent: u32,
    pub cur_image: [u8; 256],
    pub hnd_name: [u8; 64],
    pub source: u32,
    pub infolen: u32,
    pub info: [u8; PRINFOSIZE],
    /// Tail padding to match C struct size (8-byte alignment of overall struct).
    pub _pad_tail: u32,
}

const _: () = assert!(mem::size_of::<RawProgressMsgUnpacked>() == PROGRESS_MSG_SIZE_UNPACKED);

// ---------------------------------------------------------------------------
// progress_connect_ack
// ---------------------------------------------------------------------------

#[derive(FromBytes, IntoBytes, Immutable, KnownLayout, Clone)]
#[repr(C)]
pub struct RawProgressAck {
    pub apiversion: u32,
    pub magic: [u8; 4],
}

const _: () = assert!(mem::size_of::<RawProgressAck>() == 8);

// ---------------------------------------------------------------------------
// swupdate_request — embedded in instmsg variant
// ---------------------------------------------------------------------------

#[derive(Clone, Copy)]
#[repr(C)]
pub struct RawSwupdateRequest {
    pub apiversion: u32,
    pub source: i32,  // sourcetype enum (C int)
    pub dry_run: i32, // enum run_type (C int)
    pub len: usize,   // size_t
    pub info: [u8; 512],
    pub software_set: [u8; 256],
    pub running_mode: [u8; 256],
    pub disable_store_swu: u8, // C bool — use u8 to avoid UB on arbitrary values
}

// ---------------------------------------------------------------------------
// msgdata union variant structs
// ---------------------------------------------------------------------------

#[derive(Clone, Copy)]
#[repr(C)]
pub struct RawStatusData {
    pub current: i32,
    pub last_result: i32,
    pub error: i32,
    pub desc: [u8; 2048],
}

#[derive(Clone, Copy)]
#[repr(C)]
pub struct RawNotifyData {
    pub status: i32,
    pub error: i32,
    pub level: i32,
    pub msg: [u8; 2048],
}

#[derive(Clone, Copy)]
#[repr(C)]
pub struct RawInstMsgData {
    pub req: RawSwupdateRequest,
    pub len: u32, // unsigned int
    pub buf: [u8; 2048],
}

#[derive(Clone, Copy)]
#[repr(C)]
pub struct RawProcMsgData {
    pub source: i32, // sourcetype enum
    pub cmd: i32,
    pub timeout: i32,
    pub len: u32,
    pub buf: [u8; 2048],
}

#[derive(Clone, Copy)]
#[repr(C)]
pub struct RawAesKeyData {
    pub key_ascii: [u8; 65],
    pub ivt_ascii: [u8; 33],
}

#[derive(Clone, Copy)]
#[repr(C)]
pub struct RawVersionsData {
    pub minimum_version: [u8; 256],
    pub maximum_version: [u8; 256],
    pub current_version: [u8; 256],
    pub update_type: [u8; 256],
}

#[derive(Clone, Copy)]
#[repr(C)]
pub struct RawRevisionsData {
    pub boardname: [u8; 256],
    pub revision: [u8; 256],
}

#[derive(Clone, Copy)]
#[repr(C)]
pub struct RawVarsData {
    pub varnamespace: [u8; 256],
    pub varname: [u8; 256],
    pub varvalue: [u8; 256],
}

#[derive(Clone, Copy)]
#[repr(C)]
pub struct RawDwlUrlData {
    pub filename: [u8; 256],
    pub url: [u8; 1024],
}

// ---------------------------------------------------------------------------
// msgdata union
// ---------------------------------------------------------------------------

#[repr(C)]
pub union RawMsgData {
    pub msg: [u8; 128],
    pub status: RawStatusData,
    pub notify: RawNotifyData,
    pub instmsg: RawInstMsgData,
    pub procmsg: RawProcMsgData,
    pub aeskeymsg: RawAesKeyData,
    pub versions: RawVersionsData,
    pub revisions: RawRevisionsData,
    pub vars: RawVarsData,
    pub dwl_url: RawDwlUrlData,
}

// SAFETY: All union variants are POD types. Bitwise copy is always valid.
impl Copy for RawMsgData {}
impl Clone for RawMsgData {
    fn clone(&self) -> Self {
        *self
    }
}

// ---------------------------------------------------------------------------
// ipc_message — the top-level wire message
// ---------------------------------------------------------------------------

#[derive(Clone, Copy)]
#[repr(C)]
pub struct RawIpcMessage {
    pub magic: i32,
    pub msg_type: i32,
    pub data: RawMsgData,
}

// ---------------------------------------------------------------------------
// Constructors and helpers
// ---------------------------------------------------------------------------

impl RawMsgData {
    /// Create a zero-initialized msgdata (all bytes zero).
    pub fn zeroed() -> Self {
        // SAFETY: all-zero is valid for every union variant (all POD types)
        unsafe { mem::zeroed() }
    }
}

impl RawIpcMessage {
    /// Create a zero-initialized IPC message.
    pub fn zeroed() -> Self {
        // SAFETY: all-zero is valid for all fields
        unsafe { mem::zeroed() }
    }

    /// Create a new message with magic set and type specified.
    pub fn new(msg_type: MsgType) -> Self {
        let mut msg = Self::zeroed();
        msg.magic = IPC_MAGIC;
        msg.msg_type = msg_type as i32;
        msg
    }

    /// Validate the magic number.
    pub fn is_valid(&self) -> bool {
        self.magic == IPC_MAGIC
    }

    /// Parse the message type.
    pub fn msg_type(&self) -> Option<MsgType> {
        MsgType::from_i32(self.msg_type)
    }
}

impl RawSwupdateRequest {
    pub fn zeroed() -> Self {
        // SAFETY: all-zero is valid
        unsafe { mem::zeroed() }
    }
}

// ---------------------------------------------------------------------------
// Safe I/O helpers — these contain ALL the unsafe in the crate
// ---------------------------------------------------------------------------

/// Read a `RawIpcMessage` from a byte buffer.
///
/// The buffer must be exactly `size_of::<RawIpcMessage>()` bytes.
///
/// # Safety rationale
/// `RawIpcMessage` is `#[repr(C)]` with all-POD fields. Every bit pattern is
/// valid (no `bool`, no references, no enums with restricted values — all enum
/// fields are stored as plain integers). The `Box` guarantees proper alignment.
pub fn ipc_message_from_bytes(buf: &[u8]) -> Option<Box<RawIpcMessage>> {
    if buf.len() != mem::size_of::<RawIpcMessage>() {
        return None;
    }
    let mut msg = Box::new(RawIpcMessage::zeroed());
    // SAFETY: `msg` is a Box with proper alignment for RawIpcMessage.
    // We copy exactly size_of bytes from buf into it. All bit patterns
    // are valid for RawIpcMessage (all fields are integers or byte arrays).
    unsafe {
        std::ptr::copy_nonoverlapping(
            buf.as_ptr(),
            &mut *msg as *mut RawIpcMessage as *mut u8,
            mem::size_of::<RawIpcMessage>(),
        );
    }
    Some(msg)
}

/// Serialize a `RawIpcMessage` to bytes.
pub fn ipc_message_to_bytes(msg: &RawIpcMessage) -> &[u8] {
    // SAFETY: `RawIpcMessage` is repr(C) with all-POD fields.
    // Reading it as bytes is always valid.
    unsafe {
        std::slice::from_raw_parts(
            msg as *const RawIpcMessage as *const u8,
            mem::size_of::<RawIpcMessage>(),
        )
    }
}

// ---------------------------------------------------------------------------
// C string helpers
// ---------------------------------------------------------------------------

/// Extract a Rust `String` from a null-padded C byte array.
pub fn cstr_from_bytes(bytes: &[u8]) -> String {
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).into_owned()
}

/// Write a Rust string into a fixed-size null-padded C byte array.
/// Truncates if the string is too long, always null-terminates.
pub fn write_cstr(dst: &mut [u8], src: &str) {
    let bytes = src.as_bytes();
    let copy_len = bytes.len().min(dst.len().saturating_sub(1));
    dst[..copy_len].copy_from_slice(&bytes[..copy_len]);
    // Rest is already zero from zeroed() initialization
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_msg_packed_size() {
        assert_eq!(mem::size_of::<RawProgressMsg>(), PROGRESS_MSG_SIZE_PACKED);
    }

    #[test]
    fn progress_msg_unpacked_size() {
        assert_eq!(
            mem::size_of::<RawProgressMsgUnpacked>(),
            PROGRESS_MSG_SIZE_UNPACKED
        );
    }

    #[test]
    fn progress_ack_size() {
        assert_eq!(mem::size_of::<RawProgressAck>(), 8);
    }

    #[test]
    fn ipc_message_union_largest_is_instmsg() {
        // instmsg should be the largest variant
        assert!(mem::size_of::<RawInstMsgData>() >= mem::size_of::<RawStatusData>());
        assert!(mem::size_of::<RawInstMsgData>() >= mem::size_of::<RawNotifyData>());
        assert!(mem::size_of::<RawInstMsgData>() >= mem::size_of::<RawProcMsgData>());
        assert!(mem::size_of::<RawInstMsgData>() >= mem::size_of::<RawAesKeyData>());
        assert!(mem::size_of::<RawInstMsgData>() >= mem::size_of::<RawVersionsData>());
        assert!(mem::size_of::<RawInstMsgData>() >= mem::size_of::<RawRevisionsData>());
        assert!(mem::size_of::<RawInstMsgData>() >= mem::size_of::<RawVarsData>());
        assert!(mem::size_of::<RawInstMsgData>() >= mem::size_of::<RawDwlUrlData>());
    }

    #[test]
    fn msgdata_size_matches_largest_variant() {
        // The union size should equal the largest variant's size (with alignment)
        assert_eq!(
            mem::size_of::<RawMsgData>(),
            mem::size_of::<RawInstMsgData>()
        );
    }

    #[test]
    fn ipc_message_magic() {
        let msg = RawIpcMessage::new(MsgType::GetStatus);
        assert!(msg.is_valid());
        assert_eq!(msg.msg_type(), Some(MsgType::GetStatus));
    }

    #[test]
    fn cstr_roundtrip() {
        let mut buf = [0u8; 256];
        write_cstr(&mut buf, "hello world");
        assert_eq!(cstr_from_bytes(&buf), "hello world");
    }

    #[test]
    fn cstr_truncation() {
        let mut buf = [0u8; 5];
        write_cstr(&mut buf, "hello world");
        assert_eq!(cstr_from_bytes(&buf), "hell"); // 4 chars + null
    }

    #[test]
    fn ipc_message_roundtrip() {
        let mut msg = RawIpcMessage::new(MsgType::ReqInstall);
        // SAFETY: we just set up the message, accessing instmsg is valid
        unsafe {
            msg.data.instmsg.req.apiversion = SWUPDATE_API_VERSION;
            msg.data.instmsg.req.source = 4 /* SOURCE_LOCAL */;
            write_cstr(&mut msg.data.instmsg.req.info, "test firmware");
        }

        let bytes = ipc_message_to_bytes(&msg);
        let msg2 = ipc_message_from_bytes(bytes).unwrap();

        assert!(msg2.is_valid());
        assert_eq!(msg2.msg_type(), Some(MsgType::ReqInstall));
        unsafe {
            assert_eq!(msg2.data.instmsg.req.apiversion, SWUPDATE_API_VERSION);
            assert_eq!(msg2.data.instmsg.req.source, 4 /* SOURCE_LOCAL */);
            assert_eq!(
                cstr_from_bytes(&msg2.data.instmsg.req.info),
                "test firmware"
            );
        }
    }
}
