# swupdate-ipc

Async Rust client for [SWUpdate](https://sbabic.github.io/swupdate/)'s IPC interface.

Communicates with the SWUpdate daemon over Unix domain sockets using the binary
protocol defined in `network_ipc.h` and `progress_ipc.h`. Wire types are
`#[repr(C)]` structs verified byte-for-byte against the vendored C headers.

## Usage

```rust
use swupdate_ipc::{ControlClient, InstallRequest, Source};

async fn install_firmware() -> swupdate_ipc::Result<()> {
    let mut ctrl = ControlClient::connect_default().await?;

    let req = InstallRequest::new()
        .source(Source::Local)
        .info("firmware v2.0");

    // install() returns a typestate handle — streaming is only available on it
    let mut install = ctrl.install(&req).await?;

    // Optionally subscribe to progress (opens a separate socket)
    let mut progress = install.progress().await?;

    // Stream image data and monitor progress concurrently
    tokio::try_join!(
        install.stream_file("/path/to/image.swu"),
        async {
            loop {
                let event = progress.receive().await?;
                println!("{event:?}");
                if event.is_terminal() { break; }
            }
            Ok(())
        },
    )?;
    // swupdate reboots into the new image

    Ok(())
}
```

After reboot, confirm the update from the new image:

```rust
async fn confirm_update() -> swupdate_ipc::Result<()> {
    let mut ctrl = ControlClient::connect_default().await?;

    if ctrl.get_update_state().await? == UpdateState::Start {
        // New image booted successfully — tell the bootloader to keep it
        ctrl.set_update_state(UpdateState::Idle).await?;
    }
    Ok(())
}
```

`stream()` also accepts any `AsyncRead`, so you can stream directly from
an HTTP response without an intermediate file:

```rust
let resp = reqwest::get("https://example.com/firmware.swu").await?;
let reader = tokio_util::io::StreamReader::new(
    resp.bytes_stream().map(|r| r.map_err(|e|
        std::io::Error::new(std::io::ErrorKind::Other, e)
    ))
);
tokio::pin!(reader);
install.stream(&mut reader).await?;
```

## Full protocol support

All IPC message types are implemented across three types:

**`ControlClient`** — commands and queries:

| Method | IPC Message |
|--------|-------------|
| `install()` → `Installation` | `REQ_INSTALL` |
| `get_status()` | `GET_STATUS` |
| `get_update_state()` | `GET_UPDATE_STATE` |
| `set_update_state()` | `SET_UPDATE_STATE` |
| `post_update()` | `POST_UPDATE` |
| `set_aes_key()` | `SET_AES_KEY` |
| `set_versions_range()` | `SET_VERSIONS_RANGE` |
| `get_hw_revision()` | `GET_HW_REVISION` |
| `set_swupdate_var()` | `SET_SWUPDATE_VARS` |
| `get_swupdate_var()` | `GET_SWUPDATE_VARS` |
| `subprocess_cmd()` | `SWUPDATE_SUBPROCESS` |
| `set_delta_url()` | `SET_DELTA_URL` |

**`Installation`** — returned by `install()`, gates streaming:

| Method | Description |
|--------|-------------|
| `stream()` | Stream from any `AsyncRead` |
| `stream_file()` | Stream from a file path |
| `progress()` → `ProgressClient` | Subscribe to progress events |

**`ProgressClient`** — returned by `progress()`:

| Method | Description |
|--------|-------------|
| `receive()` | Next `ProgressEvent` (blocking) |
| `receive_or_reconnect()` | Same, with auto-reconnect |

## C compatibility

The `#[repr(C)]` wire types in `src/wire.rs` are verified against vendored
SWUpdate headers via integration tests that:

1. Compile a C program with the actual struct definitions
2. Compare `sizeof` and `offsetof` for every struct and field
3. Serialize known messages in C, deserialize in Rust, and verify byte-for-byte

Run with: `cargo test --test c_compat`

## Protocol version

Targets SWUpdate API version `0x1` (control) and progress API `2.0.0`.

SWUpdate's IPC is a local-only protocol over Unix domain sockets. The wire
format uses native byte order and platform-specific sizes (e.g., `size_t`).
This crate must be compiled for the same target as the SWUpdate daemon.

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT) at your option.
