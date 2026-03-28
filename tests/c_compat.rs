//! C compatibility tests.
//!
//! These tests compile a small C program using the vendored SWUpdate headers
//! and compare struct sizes, field offsets, and serialized bytes against the
//! Rust `#[repr(C)]` types.

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;
use std::sync::LazyLock;

/// Compiled binary path, shared across all tests.
static C_BINARY: LazyLock<PathBuf> = LazyLock::new(compile_c_harness);

/// Compile the C test harness and return the path to the binary.
fn compile_c_harness() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let c_source = manifest_dir.join("tests/c_compat/compat_test.c");
    let headers = manifest_dir.join("tests/c_compat/swupdate_headers");
    let out_dir = PathBuf::from(std::env::var("CARGO_TARGET_TMPDIR").unwrap_or_else(|_| {
        manifest_dir
            .join("target/c_compat_test")
            .display()
            .to_string()
    }));
    std::fs::create_dir_all(&out_dir).unwrap();
    let binary = out_dir.join("compat_test");

    let status = Command::new("cc")
        .args([
            c_source.to_str().unwrap(),
            "-o",
            binary.to_str().unwrap(),
            &format!("-I{}", headers.display()),
            "-Wall",
            "-Wextra",
            "-Werror",
        ])
        .status()
        .expect("failed to compile C test harness — is `cc` installed?");

    assert!(status.success(), "C compilation failed");
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

#[test]
fn progress_msg_layout_matches_c() {
    let binary = &*C_BINARY;
    let layout = parse_layout(&run_harness(binary, "layout"));

    let rust_size = std::mem::size_of::<swupdate_ipc::wire::RawProgressMsg>();
    let c_size = layout["sizeof_progress_msg"];
    assert_eq!(
        rust_size, c_size,
        "progress_msg size: Rust={rust_size} C={c_size}"
    );

    // Verify field offsets using Rust's offset_of! (stable since 1.77)
    macro_rules! check_offset {
        ($struct:ty, $field:ident, $key:expr) => {
            let rust_offset = std::mem::offset_of!($struct, $field);
            let c_offset = layout[$key];
            assert_eq!(
                rust_offset, c_offset,
                "{}: Rust={rust_offset} C={c_offset}",
                $key
            );
        };
    }

    use swupdate_ipc::wire::RawProgressMsg;
    check_offset!(
        RawProgressMsg,
        apiversion,
        "offsetof_progress_msg_apiversion"
    );
    check_offset!(RawProgressMsg, status, "offsetof_progress_msg_status");
    check_offset!(
        RawProgressMsg,
        dwl_percent,
        "offsetof_progress_msg_dwl_percent"
    );
    check_offset!(RawProgressMsg, dwl_bytes, "offsetof_progress_msg_dwl_bytes");
    check_offset!(RawProgressMsg, nsteps, "offsetof_progress_msg_nsteps");
    check_offset!(RawProgressMsg, cur_step, "offsetof_progress_msg_cur_step");
    check_offset!(
        RawProgressMsg,
        cur_percent,
        "offsetof_progress_msg_cur_percent"
    );
    check_offset!(RawProgressMsg, cur_image, "offsetof_progress_msg_cur_image");
    check_offset!(RawProgressMsg, hnd_name, "offsetof_progress_msg_hnd_name");
    check_offset!(RawProgressMsg, source, "offsetof_progress_msg_source");
    check_offset!(RawProgressMsg, infolen, "offsetof_progress_msg_infolen");
    check_offset!(RawProgressMsg, info, "offsetof_progress_msg_info");
}

#[test]
fn progress_ack_size_matches_c() {
    let binary = &*C_BINARY;
    let layout = parse_layout(&run_harness(binary, "layout"));

    let rust_size = std::mem::size_of::<swupdate_ipc::wire::RawProgressAck>();
    let c_size = layout["sizeof_progress_connect_ack"];
    assert_eq!(
        rust_size, c_size,
        "progress_connect_ack size: Rust={rust_size} C={c_size}"
    );
}

#[test]
fn ipc_message_layout_matches_c() {
    let binary = &*C_BINARY;
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
    let binary = &*C_BINARY;
    let layout = parse_layout(&run_harness(binary, "layout"));

    let rust_size = std::mem::size_of::<swupdate_ipc::wire::RawSwupdateRequest>();
    let c_size = layout["sizeof_swupdate_request"];
    assert_eq!(
        rust_size, c_size,
        "swupdate_request size: Rust={rust_size} C={c_size}"
    );

    macro_rules! check_offset {
        ($struct:ty, $field:ident, $key:expr) => {
            let rust_offset = std::mem::offset_of!($struct, $field);
            let c_offset = layout[$key];
            assert_eq!(
                rust_offset, c_offset,
                "{}: Rust={rust_offset} C={c_offset}",
                $key
            );
        };
    }

    use swupdate_ipc::wire::RawSwupdateRequest;
    check_offset!(
        RawSwupdateRequest,
        apiversion,
        "offsetof_swupdate_request_apiversion"
    );
    check_offset!(
        RawSwupdateRequest,
        source,
        "offsetof_swupdate_request_source"
    );
    check_offset!(
        RawSwupdateRequest,
        dry_run,
        "offsetof_swupdate_request_dry_run"
    );
    check_offset!(RawSwupdateRequest, len, "offsetof_swupdate_request_len");
    check_offset!(RawSwupdateRequest, info, "offsetof_swupdate_request_info");
    check_offset!(
        RawSwupdateRequest,
        software_set,
        "offsetof_swupdate_request_software_set"
    );
    check_offset!(
        RawSwupdateRequest,
        running_mode,
        "offsetof_swupdate_request_running_mode"
    );
    check_offset!(
        RawSwupdateRequest,
        disable_store_swu,
        "offsetof_swupdate_request_disable_store_swu"
    );
}

#[test]
fn progress_msg_roundtrip_with_c() {
    let binary = &*C_BINARY;
    let c_bytes = run_harness(binary, "progress");

    assert_eq!(
        c_bytes.len(),
        std::mem::size_of::<swupdate_ipc::wire::RawProgressMsg>(),
        "C serialized {} bytes, expected {}",
        c_bytes.len(),
        std::mem::size_of::<swupdate_ipc::wire::RawProgressMsg>()
    );

    // Deserialize in Rust
    use zerocopy::FromBytes;
    let msg = swupdate_ipc::wire::RawProgressMsg::read_from_bytes(&c_bytes)
        .expect("failed to deserialize progress_msg from C bytes");

    // Verify known values (packed field access requires copying)
    assert_eq!({ msg.apiversion }, 0x00020000);
    assert_eq!({ msg.status }, 3); // SUCCESS
    assert_eq!({ msg.dwl_percent }, 100);
    assert_eq!({ msg.dwl_bytes }, 1048576);
    assert_eq!({ msg.nsteps }, 2);
    assert_eq!({ msg.cur_step }, 2);
    assert_eq!({ msg.cur_percent }, 100);
    assert_eq!(
        swupdate_ipc::wire::cstr_from_bytes(&msg.cur_image),
        "rootfs.img"
    );
    assert_eq!(swupdate_ipc::wire::cstr_from_bytes(&msg.hnd_name), "raw");
    assert_eq!({ msg.source }, 4); // SOURCE_LOCAL
    assert_eq!({ msg.infolen }, 4);
    assert_eq!(swupdate_ipc::wire::cstr_from_bytes(&msg.info[..4]), "done");

    // Serialize back in Rust and compare byte-for-byte
    use zerocopy::IntoBytes;
    assert_eq!(msg.as_bytes(), &c_bytes[..]);
}

#[test]
fn ipc_message_roundtrip_with_c() {
    let binary = &*C_BINARY;
    let c_bytes = run_harness(binary, "ipc");

    let expected_size = std::mem::size_of::<swupdate_ipc::wire::RawIpcMessage>();
    assert_eq!(
        c_bytes.len(),
        expected_size,
        "C serialized {} bytes, expected {expected_size}",
        c_bytes.len(),
    );

    // Deserialize in Rust
    let msg = swupdate_ipc::wire::ipc_message_from_bytes(&c_bytes)
        .expect("failed to deserialize ipc_message from C bytes");

    assert_eq!(msg.magic, 0x14052001);
    assert_eq!(msg.msg_type, 0); // REQ_INSTALL

    // Check instmsg fields
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

    // Serialize back and compare bytes
    let rust_bytes = swupdate_ipc::wire::ipc_message_to_bytes(&msg);
    assert_eq!(rust_bytes, &c_bytes[..]);
}
