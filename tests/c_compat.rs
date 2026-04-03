//! C compatibility tests.
//!
//! These tests compile a small C program using the vendored SWUpdate headers
//! and compare struct sizes, field offsets, and serialized bytes against the
//! Rust `#[repr(C)]` types.
//!
//! Two variants are built: one with the packed progress_ipc.h (swupdate >=
//! 2025.12) and one with the unpacked version (swupdate <= 2025.05).

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;
use std::sync::LazyLock;

/// Compiled binary for the packed layout variant.
static C_BINARY_PACKED: LazyLock<PathBuf> = LazyLock::new(|| compile_c_harness("packed"));

/// Compiled binary for the unpacked layout variant.
static C_BINARY_UNPACKED: LazyLock<PathBuf> = LazyLock::new(|| compile_c_harness("unpacked"));

/// Compile the C test harness for a given progress_msg layout variant.
///
/// `variant` is "packed" or "unpacked", selecting the corresponding
/// `swupdate_headers/{variant}/progress_ipc.h`.
fn compile_c_harness(variant: &str) -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let c_source = manifest_dir.join("tests/c_compat/compat_test.c");
    let headers = manifest_dir.join("tests/c_compat/swupdate_headers");
    let variant_headers = headers.join(variant);
    let out_dir = PathBuf::from(std::env::var("CARGO_TARGET_TMPDIR").unwrap_or_else(|_| {
        manifest_dir
            .join("target/c_compat_test")
            .display()
            .to_string()
    }));
    std::fs::create_dir_all(&out_dir).unwrap();
    let binary = out_dir.join(format!("compat_test_{variant}"));

    let status = Command::new("cc")
        .args([
            c_source.to_str().unwrap(),
            "-o",
            binary.to_str().unwrap(),
            // Variant-specific header first (provides progress_ipc.h)
            &format!("-I{}", variant_headers.display()),
            // Shared headers (network_ipc.h, swupdate_status.h)
            &format!("-I{}", headers.display()),
            "-Wall",
            "-Wextra",
            "-Werror",
        ])
        .status()
        .expect("failed to compile C test harness — is `cc` installed?");

    assert!(status.success(), "C compilation failed for variant {variant}");
    binary
}

/// Run the C harness with a command and return stdout as bytes.
fn run_harness(binary: &PathBuf, cmd: &str) -> Vec<u8> {
    let output = Command::new(binary)
        .arg(cmd)
        .output()
        .expect("failed to run C test harness");

    assert!(
        output.status.success(),
        "C harness failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    output.stdout
}

/// Parse key=value layout output from the C harness.
fn parse_layout(stdout: &[u8]) -> HashMap<String, usize> {
    String::from_utf8_lossy(stdout)
        .lines()
        .filter_map(|line| {
            let (key, val) = line.split_once('=')?;
            Some((key.to_string(), val.parse().ok()?))
        })
        .collect()
}

macro_rules! check_offset {
    ($layout:expr, $struct:ty, $field:ident, $key:expr) => {
        let rust_offset = std::mem::offset_of!($struct, $field);
        let c_offset = $layout[$key];
        assert_eq!(
            rust_offset, c_offset,
            "{}: Rust={rust_offset} C={c_offset}",
            $key
        );
    };
}

// ---------------------------------------------------------------------------
// Packed progress_msg (swupdate >= 2025.12)
// ---------------------------------------------------------------------------

#[test]
fn progress_msg_packed_layout_matches_c() {
    let binary = &*C_BINARY_PACKED;
    let layout = parse_layout(&run_harness(binary, "layout"));

    let rust_size = std::mem::size_of::<swupdate_ipc::wire::RawProgressMsg>();
    let c_size = layout["sizeof_progress_msg"];
    assert_eq!(
        rust_size, c_size,
        "progress_msg packed size: Rust={rust_size} C={c_size}"
    );

    use swupdate_ipc::wire::RawProgressMsg;
    check_offset!(layout, RawProgressMsg, apiversion, "offsetof_progress_msg_apiversion");
    check_offset!(layout, RawProgressMsg, status, "offsetof_progress_msg_status");
    check_offset!(layout, RawProgressMsg, dwl_percent, "offsetof_progress_msg_dwl_percent");
    check_offset!(layout, RawProgressMsg, dwl_bytes, "offsetof_progress_msg_dwl_bytes");
    check_offset!(layout, RawProgressMsg, nsteps, "offsetof_progress_msg_nsteps");
    check_offset!(layout, RawProgressMsg, cur_step, "offsetof_progress_msg_cur_step");
    check_offset!(layout, RawProgressMsg, cur_percent, "offsetof_progress_msg_cur_percent");
    check_offset!(layout, RawProgressMsg, cur_image, "offsetof_progress_msg_cur_image");
    check_offset!(layout, RawProgressMsg, hnd_name, "offsetof_progress_msg_hnd_name");
    check_offset!(layout, RawProgressMsg, source, "offsetof_progress_msg_source");
    check_offset!(layout, RawProgressMsg, infolen, "offsetof_progress_msg_infolen");
    check_offset!(layout, RawProgressMsg, info, "offsetof_progress_msg_info");
}

#[test]
fn progress_msg_packed_roundtrip_with_c() {
    let binary = &*C_BINARY_PACKED;
    let c_bytes = run_harness(binary, "progress");

    assert_eq!(
        c_bytes.len(),
        std::mem::size_of::<swupdate_ipc::wire::RawProgressMsg>(),
        "C serialized {} bytes, expected {}",
        c_bytes.len(),
        std::mem::size_of::<swupdate_ipc::wire::RawProgressMsg>()
    );

    use zerocopy::FromBytes;
    let msg = swupdate_ipc::wire::RawProgressMsg::read_from_bytes(&c_bytes)
        .expect("failed to deserialize packed progress_msg from C bytes");

    assert_eq!({ msg.apiversion }, 0x00020000);
    assert_eq!({ msg.status }, 3); // SUCCESS
    assert_eq!({ msg.dwl_percent }, 100);
    assert_eq!({ msg.dwl_bytes }, 1048576);
    assert_eq!({ msg.nsteps }, 2);
    assert_eq!({ msg.cur_step }, 2);
    assert_eq!({ msg.cur_percent }, 100);
    assert_eq!(swupdate_ipc::wire::cstr_from_bytes(&msg.cur_image), "rootfs.img");
    assert_eq!(swupdate_ipc::wire::cstr_from_bytes(&msg.hnd_name), "raw");
    assert_eq!({ msg.source }, 4); // SOURCE_LOCAL
    assert_eq!({ msg.infolen }, 4);
    assert_eq!(swupdate_ipc::wire::cstr_from_bytes(&msg.info[..4]), "done");

    use zerocopy::IntoBytes;
    assert_eq!(msg.as_bytes(), &c_bytes[..]);
}

// ---------------------------------------------------------------------------
// Unpacked progress_msg (swupdate <= 2025.05)
// ---------------------------------------------------------------------------

#[test]
fn progress_msg_unpacked_layout_matches_c() {
    let binary = &*C_BINARY_UNPACKED;
    let layout = parse_layout(&run_harness(binary, "layout"));

    let rust_size = std::mem::size_of::<swupdate_ipc::wire::RawProgressMsgUnpacked>();
    let c_size = layout["sizeof_progress_msg"];
    assert_eq!(
        rust_size, c_size,
        "progress_msg unpacked size: Rust={rust_size} C={c_size}"
    );

    use swupdate_ipc::wire::RawProgressMsgUnpacked;
    check_offset!(layout, RawProgressMsgUnpacked, apiversion, "offsetof_progress_msg_apiversion");
    check_offset!(layout, RawProgressMsgUnpacked, status, "offsetof_progress_msg_status");
    check_offset!(layout, RawProgressMsgUnpacked, dwl_percent, "offsetof_progress_msg_dwl_percent");
    check_offset!(layout, RawProgressMsgUnpacked, dwl_bytes, "offsetof_progress_msg_dwl_bytes");
    check_offset!(layout, RawProgressMsgUnpacked, nsteps, "offsetof_progress_msg_nsteps");
    check_offset!(layout, RawProgressMsgUnpacked, cur_step, "offsetof_progress_msg_cur_step");
    check_offset!(layout, RawProgressMsgUnpacked, cur_percent, "offsetof_progress_msg_cur_percent");
    check_offset!(layout, RawProgressMsgUnpacked, cur_image, "offsetof_progress_msg_cur_image");
    check_offset!(layout, RawProgressMsgUnpacked, hnd_name, "offsetof_progress_msg_hnd_name");
    check_offset!(layout, RawProgressMsgUnpacked, source, "offsetof_progress_msg_source");
    check_offset!(layout, RawProgressMsgUnpacked, infolen, "offsetof_progress_msg_infolen");
    check_offset!(layout, RawProgressMsgUnpacked, info, "offsetof_progress_msg_info");
}

#[test]
fn progress_msg_unpacked_roundtrip_with_c() {
    let binary = &*C_BINARY_UNPACKED;
    let c_bytes = run_harness(binary, "progress");

    assert_eq!(
        c_bytes.len(),
        std::mem::size_of::<swupdate_ipc::wire::RawProgressMsgUnpacked>(),
        "C serialized {} bytes, expected {}",
        c_bytes.len(),
        std::mem::size_of::<swupdate_ipc::wire::RawProgressMsgUnpacked>()
    );

    use zerocopy::FromBytes;
    let msg = swupdate_ipc::wire::RawProgressMsgUnpacked::read_from_bytes(&c_bytes)
        .expect("failed to deserialize unpacked progress_msg from C bytes");

    assert_eq!(msg.apiversion, 0x00020000);
    assert_eq!(msg.status, 3); // SUCCESS
    assert_eq!(msg.dwl_percent, 100);
    assert_eq!(msg.dwl_bytes, 1048576);
    assert_eq!(msg.nsteps, 2);
    assert_eq!(msg.cur_step, 2);
    assert_eq!(msg.cur_percent, 100);
    assert_eq!(swupdate_ipc::wire::cstr_from_bytes(&msg.cur_image), "rootfs.img");
    assert_eq!(swupdate_ipc::wire::cstr_from_bytes(&msg.hnd_name), "raw");
    assert_eq!(msg.source, 4); // SOURCE_LOCAL
    assert_eq!(msg.infolen, 4);
    assert_eq!(swupdate_ipc::wire::cstr_from_bytes(&msg.info[..4]), "done");

    use zerocopy::IntoBytes;
    assert_eq!(msg.as_bytes(), &c_bytes[..]);
}

// ---------------------------------------------------------------------------
// progress_connect_ack (same for both variants)
// ---------------------------------------------------------------------------

#[test]
fn progress_ack_size_matches_c() {
    let binary = &*C_BINARY_PACKED;
    let layout = parse_layout(&run_harness(binary, "layout"));

    let rust_size = std::mem::size_of::<swupdate_ipc::wire::RawProgressAck>();
    let c_size = layout["sizeof_progress_connect_ack"];
    assert_eq!(
        rust_size, c_size,
        "progress_connect_ack size: Rust={rust_size} C={c_size}"
    );
}

// ---------------------------------------------------------------------------
// ipc_message and swupdate_request (unaffected by progress_msg layout)
// ---------------------------------------------------------------------------

#[test]
fn ipc_message_layout_matches_c() {
    let binary = &*C_BINARY_PACKED;
    let layout = parse_layout(&run_harness(binary, "layout"));

    let rust_size = std::mem::size_of::<swupdate_ipc::wire::RawIpcMessage>();
    let c_size = layout["sizeof_ipc_message"];
    assert_eq!(
        rust_size, c_size,
        "ipc_message size: Rust={rust_size} C={c_size}"
    );

    let rust_msgdata = std::mem::size_of::<swupdate_ipc::wire::RawMsgData>();
    let c_msgdata = layout["sizeof_msgdata"];
    assert_eq!(
        rust_msgdata, c_msgdata,
        "msgdata size: Rust={rust_msgdata} C={c_msgdata}"
    );
}

#[test]
fn swupdate_request_layout_matches_c() {
    let binary = &*C_BINARY_PACKED;
    let layout = parse_layout(&run_harness(binary, "layout"));

    let rust_size = std::mem::size_of::<swupdate_ipc::wire::RawSwupdateRequest>();
    let c_size = layout["sizeof_swupdate_request"];
    assert_eq!(
        rust_size, c_size,
        "swupdate_request size: Rust={rust_size} C={c_size}"
    );

    use swupdate_ipc::wire::RawSwupdateRequest;
    check_offset!(layout, RawSwupdateRequest, apiversion, "offsetof_swupdate_request_apiversion");
    check_offset!(layout, RawSwupdateRequest, source, "offsetof_swupdate_request_source");
    check_offset!(layout, RawSwupdateRequest, dry_run, "offsetof_swupdate_request_dry_run");
    check_offset!(layout, RawSwupdateRequest, len, "offsetof_swupdate_request_len");
    check_offset!(layout, RawSwupdateRequest, info, "offsetof_swupdate_request_info");
    check_offset!(layout, RawSwupdateRequest, software_set, "offsetof_swupdate_request_software_set");
    check_offset!(layout, RawSwupdateRequest, running_mode, "offsetof_swupdate_request_running_mode");
    check_offset!(layout, RawSwupdateRequest, disable_store_swu, "offsetof_swupdate_request_disable_store_swu");
}

#[test]
fn ipc_message_roundtrip_with_c() {
    let binary = &*C_BINARY_PACKED;
    let c_bytes = run_harness(binary, "ipc");

    let expected_size = std::mem::size_of::<swupdate_ipc::wire::RawIpcMessage>();
    assert_eq!(
        c_bytes.len(),
        expected_size,
        "C serialized {} bytes, expected {expected_size}",
        c_bytes.len(),
    );

    let msg = swupdate_ipc::wire::ipc_message_from_bytes(&c_bytes)
        .expect("failed to deserialize ipc_message from C bytes");

    assert_eq!(msg.magic, 0x14052001);
    assert_eq!(msg.msg_type, 0); // REQ_INSTALL

    // SAFETY: we know the C code set msg_type=REQ_INSTALL and populated instmsg
    unsafe {
        assert_eq!(msg.data.instmsg.req.apiversion, 0x1);
        assert_eq!(msg.data.instmsg.req.source, 4); // SOURCE_LOCAL
        assert_eq!(msg.data.instmsg.req.dry_run, 0); // RUN_DEFAULT
        assert_eq!(
            swupdate_ipc::wire::cstr_from_bytes(&msg.data.instmsg.req.info),
            "test firmware"
        );
    }

    let rust_bytes = swupdate_ipc::wire::ipc_message_to_bytes(&msg);
    assert_eq!(rust_bytes, &c_bytes[..]);
}
