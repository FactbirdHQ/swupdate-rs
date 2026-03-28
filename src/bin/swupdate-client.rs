use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};
use swupdate_ipc::{
    ControlClient, InstallRequest, ProgressEvent, RunMode, SocketConfig, Source, SubprocessCmd,
    UpdateState,
};
use tracing::{error, info};

#[derive(Parser)]
#[command(name = "swupdate-client", about = "CLI client for SWUpdate IPC")]
struct Cli {
    /// Control socket path override
    #[arg(long, global = true)]
    ctrl_socket: Option<PathBuf>,

    /// Progress socket path override
    #[arg(long, global = true)]
    progress_socket: Option<PathBuf>,

    /// Enable verbose (debug) logging
    #[arg(short, long, global = true)]
    verbose: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Install a .swu image (REQ_INSTALL + stream)
    Install {
        /// Path to the .swu file (omit or use - for stdin)
        file: Option<PathBuf>,

        /// Monitor progress during installation
        #[arg(short, long)]
        progress: bool,

        /// Update source
        #[arg(short, long, default_value = "local")]
        source: SourceArg,

        /// Dry run (don't actually install)
        #[arg(short, long)]
        dry_run: bool,

        /// Info string
        #[arg(short, long, default_value = "")]
        info: String,
    },

    /// Monitor progress events (connect to progress socket)
    Progress,

    /// Query current update status (GET_STATUS)
    GetStatus,

    /// Query bootloader update state (GET_UPDATE_STATE)
    GetState,

    /// Set bootloader update state (SET_UPDATE_STATE)
    SetState {
        /// State to set
        state: StateArg,
    },

    /// Send post-update notification (POST_UPDATE)
    PostUpdate,

    /// Set AES encryption key (SET_AES_KEY)
    SetAesKey {
        /// AES key (hex ASCII, 64 chars)
        key: String,
        /// Initialization vector (hex ASCII, 32 chars)
        iv: String,
    },

    /// Set allowed version range (SET_VERSIONS_RANGE)
    SetVersionsRange {
        /// Minimum version
        #[arg(long)]
        min: String,
        /// Maximum version
        #[arg(long)]
        max: String,
        /// Current version
        #[arg(long)]
        current: String,
        /// Update type
        #[arg(long, default_value = "")]
        update_type: String,
    },

    /// Query hardware revision (GET_HW_REVISION)
    GetHwRevision,

    /// Set a SWUpdate variable (SET_SWUPDATE_VARS)
    SetVar {
        /// Variable namespace
        namespace: String,
        /// Variable name
        name: String,
        /// Variable value
        value: String,
    },

    /// Get a SWUpdate variable (GET_SWUPDATE_VARS)
    GetVar {
        /// Variable namespace
        namespace: String,
        /// Variable name
        name: String,
    },

    /// Send subprocess command (SWUPDATE_SUBPROCESS)
    Subprocess {
        /// Update source
        #[arg(long, default_value = "local")]
        source: SourceArg,
        /// Command to send
        cmd: SubprocessCmdArg,
        /// Timeout in seconds
        #[arg(long, default_value = "30")]
        timeout: i32,
    },

    /// Set delta update URL (SET_DELTA_URL)
    SetDeltaUrl {
        /// Artifact filename
        filename: String,
        /// Download URL
        url: String,
    },
}

#[derive(Clone, ValueEnum)]
enum SourceArg {
    Unknown,
    Webserver,
    Suricatta,
    Downloader,
    Local,
    ChunksDownloader,
}

impl From<SourceArg> for Source {
    fn from(s: SourceArg) -> Self {
        match s {
            SourceArg::Unknown => Source::Unknown,
            SourceArg::Webserver => Source::Webserver,
            SourceArg::Suricatta => Source::Suricatta,
            SourceArg::Downloader => Source::Downloader,
            SourceArg::Local => Source::Local,
            SourceArg::ChunksDownloader => Source::ChunksDownloader,
        }
    }
}

#[derive(Clone, ValueEnum)]
enum StateArg {
    Ok,
    Installed,
    Testing,
    Failed,
    NotAvailable,
    Error,
    Wait,
    InProgress,
}

impl From<StateArg> for UpdateState {
    fn from(s: StateArg) -> Self {
        match s {
            StateArg::Ok => UpdateState::Ok,
            StateArg::Installed => UpdateState::Installed,
            StateArg::Testing => UpdateState::Testing,
            StateArg::Failed => UpdateState::Failed,
            StateArg::NotAvailable => UpdateState::NotAvailable,
            StateArg::Error => UpdateState::Error,
            StateArg::Wait => UpdateState::Wait,
            StateArg::InProgress => UpdateState::InProgress,
        }
    }
}

#[derive(Clone, ValueEnum)]
enum SubprocessCmdArg {
    Activation,
    Config,
    Enable,
    GetStatus,
    SetDownloadUrl,
}

impl From<SubprocessCmdArg> for SubprocessCmd {
    fn from(s: SubprocessCmdArg) -> Self {
        match s {
            SubprocessCmdArg::Activation => SubprocessCmd::Activation,
            SubprocessCmdArg::Config => SubprocessCmd::Config,
            SubprocessCmdArg::Enable => SubprocessCmd::Enable,
            SubprocessCmdArg::GetStatus => SubprocessCmd::GetStatus,
            SubprocessCmdArg::SetDownloadUrl => SubprocessCmd::SetDownloadUrl,
        }
    }
}

fn init_tracing(verbose: bool) {
    use tracing_subscriber::EnvFilter;
    let filter = if verbose {
        EnvFilter::new("debug")
    } else {
        EnvFilter::from_default_env().add_directive("swupdate_ipc=info".parse().unwrap())
    };
    tracing_subscriber::fmt().with_env_filter(filter).init();
}

fn build_config(cli: &Cli) -> SocketConfig {
    let mut config = SocketConfig::default();
    if let Some(ref path) = cli.ctrl_socket {
        config = config.ctrl_path(path);
    }
    if let Some(ref path) = cli.progress_socket {
        config = config.progress_path(path);
    }
    config
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    init_tracing(cli.verbose);

    if let Err(e) = run(cli).await {
        error!("{e}");
        std::process::exit(1);
    }
}

async fn run(cli: Cli) -> swupdate_ipc::Result<()> {
    let config = build_config(&cli);

    match cli.command {
        Command::Install {
            file,
            progress,
            source,
            dry_run,
            info: info_str,
        } => {
            let mut ctrl = ControlClient::connect(config).await?;

            let mut req = InstallRequest::new().source(source.into());
            if !info_str.is_empty() {
                req = req.info(info_str);
            }
            if dry_run {
                req = req.mode(RunMode::DryRun);
            }

            let mut install = ctrl.install(&req).await?;
            println!("Install request accepted");

            if progress {
                let mut progress_client = install.progress().await?;

                let stream_handle = async {
                    let bytes = match file {
                        Some(path) => install.stream_file(&path).await?,
                        None => {
                            let mut stdin = tokio::io::stdin();
                            install.stream(&mut stdin).await?
                        }
                    };
                    println!("Streamed {bytes} bytes");
                    Ok::<_, swupdate_ipc::Error>(())
                };

                let progress_handle = async {
                    loop {
                        let event = progress_client.receive().await?;
                        print_progress(&event);
                        if event.is_terminal() {
                            break;
                        }
                    }
                    Ok::<_, swupdate_ipc::Error>(())
                };

                tokio::try_join!(stream_handle, progress_handle)?;
            } else {
                let bytes = match file {
                    Some(path) => install.stream_file(&path).await?,
                    None => {
                        let mut stdin = tokio::io::stdin();
                        install.stream(&mut stdin).await?
                    }
                };
                println!("Streamed {bytes} bytes");
            }
        }

        Command::Progress => {
            let mut progress_client = swupdate_ipc::ProgressClient::connect(&config).await?;
            println!("Connected to progress socket, waiting for events...");
            loop {
                let event = progress_client.receive().await?;
                print_progress(&event);
                if event.is_terminal() {
                    break;
                }
            }
        }

        Command::GetStatus => {
            let mut ctrl = ControlClient::connect(config).await?;
            let status = ctrl.get_status().await?;
            println!("Current:     {:?}", status.current);
            println!("Last result: {:?}", status.last_result);
            println!("Error:       {}", status.error);
            if !status.description.is_empty() {
                println!("Description: {}", status.description);
            }
        }

        Command::GetState => {
            let mut ctrl = ControlClient::connect(config).await?;
            let state = ctrl.get_update_state().await?;
            println!("{state:?}");
        }

        Command::SetState { state } => {
            let mut ctrl = ControlClient::connect(config).await?;
            ctrl.set_update_state(state.into()).await?;
            println!("State set");
        }

        Command::PostUpdate => {
            let mut ctrl = ControlClient::connect(config).await?;
            ctrl.post_update().await?;
            println!("Post-update sent");
        }

        Command::SetAesKey { key, iv } => {
            let mut ctrl = ControlClient::connect(config).await?;
            ctrl.set_aes_key(&key, &iv).await?;
            println!("AES key set");
        }

        Command::SetVersionsRange {
            min,
            max,
            current,
            update_type,
        } => {
            let mut ctrl = ControlClient::connect(config).await?;
            ctrl.set_versions_range(&min, &max, &current, &update_type)
                .await?;
            println!("Versions range set");
        }

        Command::GetHwRevision => {
            let mut ctrl = ControlClient::connect(config).await?;
            let rev = ctrl.get_hw_revision().await?;
            println!("Board: {}", rev.boardname);
            println!("Revision: {}", rev.revision);
        }

        Command::SetVar {
            namespace,
            name,
            value,
        } => {
            let mut ctrl = ControlClient::connect(config).await?;
            ctrl.set_swupdate_var(&namespace, &name, &value).await?;
            println!("Variable set");
        }

        Command::GetVar { namespace, name } => {
            let mut ctrl = ControlClient::connect(config).await?;
            let var = ctrl.get_swupdate_var(&namespace, &name).await?;
            println!("{}", var.value);
        }

        Command::Subprocess {
            source,
            cmd,
            timeout,
        } => {
            let mut ctrl = ControlClient::connect(config).await?;
            ctrl.subprocess_cmd(source.into(), cmd.into(), timeout)
                .await?;
            println!("Subprocess command sent");
        }

        Command::SetDeltaUrl { filename, url } => {
            let mut ctrl = ControlClient::connect(config).await?;
            ctrl.set_delta_url(&filename, &url).await?;
            println!("Delta URL set");
        }
    }

    Ok(())
}

fn print_progress(event: &ProgressEvent) {
    match event {
        ProgressEvent::Started => info!("Started"),
        ProgressEvent::Downloading {
            percent,
            bytes_total,
        } => info!("Downloading: {percent}% ({bytes_total} bytes total)"),
        ProgressEvent::Installing {
            step,
            total_steps,
            percent,
            image,
            handler,
        } => info!("Installing [{handler}] {image}: step {step}/{total_steps} ({percent}%)"),
        ProgressEvent::Success => info!("Success"),
        ProgressEvent::Failed(msg) => error!("Failed: {msg}"),
        ProgressEvent::Idle => {}
    }
}
