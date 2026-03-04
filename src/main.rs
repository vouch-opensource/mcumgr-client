// Copyright © 2023-2024 Vouch.io LLC, 2026 Rudis Laboratories LLC

use anyhow::{Error, Result};
use clap::{Parser, Subcommand};
use indicatif::{ProgressBar, ProgressStyle};
use log::{error, info, LevelFilter};
use serialport::available_ports;
use simplelog::{ColorChoice, Config, SimpleLogger, TermLogger, TerminalMode};
use std::env;
use std::path::PathBuf;
use std::process;

use mcumgr_client::*;

/// Format bytes to human-readable string
fn format_bytes(size: u32) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB"];
    let mut size = size as f64;
    for unit in UNITS {
        if size < 1024.0 {
            return format!("{size:.1} {unit}");
        }
        size /= 1024.0;
    }
    format!("{size:.1} TB")
}

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// device name (serial port)
    #[arg(short, long, default_value = "")]
    device: String,

    /// UDP host (use instead of --device for UDP connection)
    #[arg(long)]
    host: Option<String>,

    /// UDP port (default: 1337)
    #[arg(long, default_value_t = 1337)]
    port: u16,

    /// verbose mode
    #[arg(short, long)]
    verbose: bool,

    /// initial timeout in seconds
    #[arg(short = 't', long = "initial_timeout", default_value_t = 60)]
    initial_timeout_s: u32,

    /// subsequent timeout in msec
    #[arg(short = 'u', long = "subsequent_timeout", default_value_t = 200)]
    subsequent_timeout_ms: u32,

    // number of retries per packet
    #[arg(long, default_value_t = 4)]
    nb_retry: u32,

    /// maximum length per line
    #[arg(short, long, default_value_t = 128)]
    linelength: usize,

    /// maximum length per request
    #[arg(short, long, default_value_t = 512)]
    mtu: usize,

    /// baudrate
    #[arg(short, long, default_value_t = 115_200)]
    baudrate: u32,

    #[command(subcommand)]
    command: Commands,
}

impl From<&Cli> for SerialSpecs {
    fn from(cli: &Cli) -> SerialSpecs {
        SerialSpecs {
            device: cli.device.clone(),
            initial_timeout_s: cli.initial_timeout_s,
            subsequent_timeout_ms: cli.subsequent_timeout_ms,
            nb_retry: cli.nb_retry,
            linelength: cli.linelength,
            mtu: cli.mtu,
            baudrate: cli.baudrate,
        }
    }
}

impl Cli {
    fn is_udp(&self) -> bool {
        self.host.is_some()
    }

    fn udp_specs(&self) -> UdpSpecs {
        UdpSpecs {
            host: self.host.clone().unwrap_or_default(),
            port: self.port,
            timeout_s: self.initial_timeout_s,
            mtu: self.mtu,
        }
    }
}

#[derive(Subcommand)]
enum Commands {
    // ============== Image Management ==============
    /// list slots on the device
    List,

    /// upload a firmware file to the device
    Upload {
        filename: PathBuf,

        /// slot number
        #[arg(short, long, default_value_t = 1)]
        slot: u8,
    },

    /// mark an image for testing or confirm it
    Test {
        hash: String,
        #[arg(short, long)]
        confirm: Option<bool>,
    },

    /// erase an image slot
    Erase {
        #[arg(short, long)]
        slot: Option<u32>,
    },

    // ============== OS/Default Management ==============
    /// reset the device
    Reset,

    /// send an echo request to the device
    Echo {
        /// message to echo
        #[arg(default_value = "hello")]
        message: String,
    },

    /// get task/thread statistics
    Taskstat,

    /// get MCUmgr parameters (buffer size, count)
    McumgrParams,

    /// get OS/application information
    OsInfo {
        /// format string (s=kernel, n=node, r=release, v=version, b=build, m=machine, p=processor, i=platform, o=os, a=all)
        #[arg(short, long, default_value = "a")]
        format: String,
    },

    /// get bootloader information
    BootloaderInfo {
        /// query type (e.g., "mode" for MCUboot mode)
        #[arg(short, long)]
        query: Option<String>,
    },

    /// get chip hardware ID (custom extension using os-info 'h' format)
    Hwid,

    // ============== Shell Management ==============
    /// execute a shell command on the device
    Shell {
        /// command and arguments to execute
        #[arg(trailing_var_arg = true)]
        command: Vec<String>,
    },

    // ============== File System Management ==============
    /// download a file from the device
    FsDownload {
        /// remote file path on device
        remote_path: String,

        /// local file path to save to
        local_path: PathBuf,
    },

    /// upload a file to the device
    FsUpload {
        /// local file path to upload
        local_path: PathBuf,

        /// remote file path on device
        remote_path: String,
    },

    /// get file status (size) from the device
    FsStat {
        /// file path on device
        path: String,
    },

    /// calculate hash/checksum of a file on the device
    FsHash {
        /// file path on device
        path: String,

        /// hash type (e.g., "sha256", "crc32")
        #[arg(short = 't', long)]
        hash_type: Option<String>,
    },

    // ============== Statistics Management ==============
    /// list available statistics groups
    StatList,

    /// read statistics from a specific group
    StatRead {
        /// statistics group name
        name: String,
    },

    // ============== Settings/Config Management ==============
    /// read a settings value
    SettingsRead {
        /// setting name/key
        name: String,

        /// maximum size of value to read
        #[arg(short, long)]
        max_size: Option<u32>,
    },

    /// write a settings value
    SettingsWrite {
        /// setting name/key
        name: String,

        /// value to write (hex string)
        value: String,
    },

    /// delete a settings value
    SettingsDelete {
        /// setting name/key
        name: String,
    },

    /// commit settings to persistent storage
    SettingsCommit,

    /// load settings from persistent storage
    SettingsLoad,

    /// save settings to persistent storage
    SettingsSave,
}

fn main() {
    // show program name and version
    let name = env!("CARGO_PKG_NAME");
    let version = env!("CARGO_PKG_VERSION");
    println!("{name} {version}");
    println!();

    // parse command line arguments
    let mut cli = Cli::parse();

    // initialize the logger with the desired level filter based on the verbose flag
    let level_filter = if cli.verbose {
        LevelFilter::Debug
    } else {
        LevelFilter::Info
    };
    TermLogger::init(
        level_filter,
        Config::default(),
        TerminalMode::Mixed,
        ColorChoice::Auto,
    )
    .unwrap_or_else(|_| SimpleLogger::init(LevelFilter::Info, Default::default()).unwrap());

    // Check if using UDP or serial connection
    let use_udp = cli.is_udp();

    // If using serial, auto-detect device if not specified
    if !use_udp && cli.device.is_empty() {
        let mut bootloaders = Vec::new();
        if let Ok(ports) = available_ports() {
            for port in ports {
                let name = port.port_name;
                // on Mac, use only special names
                if env::consts::OS == "macos" {
                    if name.contains("cu.usbmodem") {
                        bootloaders.push(name);
                    }
                } else {
                    bootloaders.push(name);
                }
            }
        }

        // if there is one bootloader device, then use it
        if bootloaders.len() == 1 {
            cli.device = bootloaders[0].clone();
            info!(
                "One bootloader device found, setting device to: {}",
                cli.device
            );
        } else {
            // otherwise print all devices, and use a device, if there is only one device
            if cli.device.is_empty() {
                match available_ports() {
                    Ok(ports) => match ports.len() {
                        0 => {
                            error!("No serial port found.");
                            process::exit(1);
                        }
                        1 => {
                            cli.device = ports[0].port_name.clone();
                            info!(
                                "Only one serial port found, setting device to: {}",
                                cli.device
                            );
                        }
                        _ => {
                            error!("More than one serial port found, please specify one:");
                            for p in ports {
                                println!("{}", p.port_name);
                            }
                            process::exit(1);
                        }
                    },
                    Err(e) => {
                        println!("Error listing serial ports: {e}");
                        process::exit(1);
                    }
                }
            }
        }
    }

    let specs = SerialSpecs::from(&cli);

    // execute command
    let result = if use_udp {
        // UDP transport mode
        let udp_specs = cli.udp_specs();
        info!("Using UDP transport: {}:{}", udp_specs.host, udp_specs.port);
        execute_command_udp(&cli.command, &udp_specs)
    } else {
        // Serial transport mode
        execute_command_serial(&cli.command, &specs)
    };

    // show error, if failed
    if let Err(e) = result {
        error!("Error: {}", e);
        process::exit(1);
    }
}

fn execute_command_serial(command: &Commands, specs: &SerialSpecs) -> Result<(), Error> {
    match command {
        // ============== Image Management ==============
        Commands::List => {
            let v = list(specs)?;
            print!("response: {}", serde_json::to_string_pretty(&v)?);
            Ok(())
        }

        Commands::Upload { filename, slot } => {
            // create a progress bar
            let pb = ProgressBar::new(1_u64);
            pb.set_style(ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({eta})")
            .unwrap().progress_chars("=> "));

            upload(
                specs,
                filename,
                *slot,
                Some(|offset, total| {
                    if let Some(l) = pb.length() {
                        if l != total {
                            pb.set_length(total)
                        }
                    }

                    pb.set_position(offset);

                    if offset >= total {
                        pb.finish_with_message("upload complete");
                    }
                }),
            )
        }

        Commands::Test { hash, confirm } => {
            test(specs, hex::decode(hash)?, *confirm)
        }

        Commands::Erase { slot } => erase(specs, *slot),

        // ============== OS/Default Management ==============
        Commands::Reset => reset(specs),

        Commands::Echo { message } => {
            let response = echo(specs, message)?;
            println!("Echo response: {response}");
            Ok(())
        }

        Commands::Taskstat => {
            let stats = taskstat(specs)?;
            println!("Task Statistics:");
            println!("{:<24} {:>5} {:>6} {:>10} {:>10}", "Task", "Prio", "State", "Stack Use", "Stack Size");
            println!("{}", "-".repeat(59));
            for (name, info) in stats.tasks.iter() {
                println!(
                    "{:<24} {:>5} {:>6} {:>10} {:>10}",
                    name, info.prio, info.state, info.stkuse, info.stksiz
                );
            }
            Ok(())
        }

        Commands::McumgrParams => {
            let params = mcumgr_params(specs)?;
            println!("MCUmgr Parameters:");
            println!("  Buffer size:  {}", format_bytes(params.buf_size));
            println!("  Buffer count: {}", params.buf_count);
            Ok(())
        }

        Commands::OsInfo { format } => {
            let info = os_info(specs, Some(format))?;
            println!("OS Information:");
            println!("{info}");
            Ok(())
        }

        Commands::BootloaderInfo { query } => {
            let info = bootloader_info(specs, query.as_deref())?;
            println!("Bootloader Information:");
            println!("  Bootloader: {}", info.bootloader);
            if let Some(mode) = info.mode {
                println!("  Mode: {} ({})", mode, mcuboot_mode_name(mode));
            }
            if let Some(no_downgrade) = info.no_downgrade {
                println!("  Downgrade Prevention: {}", if no_downgrade { "Enabled" } else { "Disabled" });
            }
            Ok(())
        }

        Commands::Hwid => {
            let info = os_info(specs, Some("h"))?;
            // Parse "hwid:XXXX" format
            if let Some(stripped) = info.strip_prefix("hwid:") {
                println!("Hardware ID: {}", stripped.trim().to_uppercase());
            } else if !info.is_empty() {
                println!("Hardware ID: {}", info.trim().to_uppercase());
            } else {
                println!("Hardware ID: (not available - custom hook may not be present)");
            }
            Ok(())
        }

        // ============== Shell Management ==============
        Commands::Shell { command } => {
            if command.is_empty() {
                return Err(anyhow::anyhow!("No command provided"));
            }
            let result = shell_exec(specs, command.clone())?;
            if !result.o.is_empty() {
                print!("{}", result.o);
            }
            if result.rc != 0 {
                info!("Command exited with code: {}", result.rc);
            }
            Ok(())
        }

        // ============== File System Management ==============
        Commands::FsDownload { remote_path, local_path } => {
            fs_download(specs, remote_path, local_path)
        }

        Commands::FsUpload { local_path, remote_path } => {
            fs_upload(specs, local_path, remote_path)
        }

        Commands::FsStat { path } => {
            let result = fs_stat(specs, path)?;
            println!("File: {path}");
            println!("  Size: {} ({} bytes)", format_bytes(result.len), result.len);
            Ok(())
        }

        Commands::FsHash { path, hash_type } => {
            let result = fs_hash(specs, path, hash_type.as_deref(), None, None)?;
            println!("File: {path}");
            println!("  Type:   {}", result.hash_type);
            println!("  Offset: {}", result.off);
            println!("  Length: {}", result.len);
            println!("  Hash:   {}", hex::encode(&result.output));
            Ok(())
        }

        // ============== Statistics Management ==============
        Commands::StatList => {
            let result = stat_list(specs)?;
            println!("Available statistics groups:");
            for name in result.stat_list {
                println!("  {name}");
            }
            Ok(())
        }

        Commands::StatRead { name } => {
            let result = stat_read(specs, name)?;
            println!("Statistics for '{}':", result.name);
            for (field, value) in result.fields.iter() {
                println!("  {field}: {value}");
            }
            Ok(())
        }

        // ============== Settings/Config Management ==============
        Commands::SettingsRead { name, max_size } => {
            let result = settings_read(specs, name, *max_size)?;
            println!("Setting '{}': {}", name, hex::encode(&result.val));
            // Try to also print as string if it's valid UTF-8
            if let Ok(s) = std::str::from_utf8(&result.val) {
                if s.chars().all(|c| c.is_ascii_graphic() || c.is_ascii_whitespace()) {
                    println!("  (as string): {s}");
                }
            }
            Ok(())
        }

        Commands::SettingsWrite { name, value } => {
            let bytes = hex::decode(value)
                .map_err(|e| anyhow::anyhow!("Invalid hex value: {}", e))?;
            settings_write(specs, name, bytes)?;
            println!("Setting '{name}' written successfully");
            Ok(())
        }

        Commands::SettingsDelete { name } => {
            settings_delete(specs, name)?;
            println!("Setting '{name}' deleted successfully");
            Ok(())
        }

        Commands::SettingsCommit => {
            settings_commit(specs)?;
            println!("Settings committed successfully");
            Ok(())
        }

        Commands::SettingsLoad => {
            settings_load(specs)?;
            println!("Settings loaded successfully");
            Ok(())
        }

        Commands::SettingsSave => {
            settings_save(specs)?;
            println!("Settings saved successfully");
            Ok(())
        }
    }
}

fn execute_command_udp(command: &Commands, specs: &UdpSpecs) -> Result<(), Error> {
    // Create UDP transport
    let mut transport = UdpTransport::new(specs)?;

    match command {
        // ============== Image Management ==============
        Commands::List => {
            let v = list_transport(&mut transport)?;
            print!("response: {}", serde_json::to_string_pretty(&v)?);
            Ok(())
        }

        Commands::Upload { filename, slot } => {
            // create a progress bar
            let pb = ProgressBar::new(1_u64);
            pb.set_style(ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({eta})")
            .unwrap().progress_chars("=> "));

            upload_image_transport(
                &mut transport,
                filename,
                *slot,
                Some(|offset: u64, total: u64| {
                    if let Some(l) = pb.length() {
                        if l != total {
                            pb.set_length(total)
                        }
                    }

                    pb.set_position(offset);

                    if offset >= total {
                        pb.finish_with_message("upload complete");
                    }
                }),
            )
        }

        Commands::Test { hash, confirm } => {
            test_transport(&mut transport, hex::decode(hash)?, *confirm)
        }

        Commands::Erase { slot } => erase_transport(&mut transport, *slot),

        // ============== OS/Default Management ==============
        Commands::Reset => reset_transport(&mut transport),

        Commands::Echo { message } => {
            let response = echo_transport(&mut transport, message)?;
            println!("Echo response: {response}");
            Ok(())
        }

        Commands::Taskstat => {
            let stats = taskstat_transport(&mut transport)?;
            println!("Task Statistics:");
            println!("{:<24} {:>5} {:>6} {:>10} {:>10}", "Task", "Prio", "State", "Stack Use", "Stack Size");
            println!("{}", "-".repeat(59));
            for (name, info) in stats.tasks.iter() {
                println!(
                    "{:<24} {:>5} {:>6} {:>10} {:>10}",
                    name, info.prio, info.state, info.stkuse, info.stksiz
                );
            }
            Ok(())
        }

        Commands::McumgrParams => {
            let params = mcumgr_params_transport(&mut transport)?;
            println!("MCUmgr Parameters:");
            println!("  Buffer size:  {}", format_bytes(params.buf_size));
            println!("  Buffer count: {}", params.buf_count);
            Ok(())
        }

        Commands::OsInfo { format } => {
            let info = os_info_transport(&mut transport, Some(format))?;
            println!("OS Information:");
            println!("{info}");
            Ok(())
        }

        Commands::BootloaderInfo { query } => {
            let info = bootloader_info_transport(&mut transport, query.as_deref())?;
            println!("Bootloader Information:");
            println!("  Bootloader: {}", info.bootloader);
            if let Some(mode) = info.mode {
                println!("  Mode: {} ({})", mode, mcuboot_mode_name(mode));
            }
            if let Some(no_downgrade) = info.no_downgrade {
                println!("  Downgrade Prevention: {}", if no_downgrade { "Enabled" } else { "Disabled" });
            }
            Ok(())
        }

        Commands::Hwid => {
            let info = os_info_transport(&mut transport, Some("h"))?;
            // Parse "hwid:XXXX" format
            if let Some(stripped) = info.strip_prefix("hwid:") {
                println!("Hardware ID: {}", stripped.trim().to_uppercase());
            } else if !info.is_empty() {
                println!("Hardware ID: {}", info.trim().to_uppercase());
            } else {
                println!("Hardware ID: (not available - custom hook may not be present)");
            }
            Ok(())
        }

        // ============== Shell Management ==============
        Commands::Shell { command } => {
            if command.is_empty() {
                return Err(anyhow::anyhow!("No command provided"));
            }
            let result = shell_exec_transport(&mut transport, command.clone())?;
            if !result.o.is_empty() {
                print!("{}", result.o);
            }
            if result.rc != 0 {
                info!("Command exited with code: {}", result.rc);
            }
            Ok(())
        }

        // ============== File System Management ==============
        Commands::FsDownload { remote_path, local_path } => {
            download_transport(&mut transport, remote_path, local_path)
        }

        Commands::FsUpload { local_path, remote_path } => {
            upload_transport(&mut transport, local_path, remote_path)
        }

        Commands::FsStat { path } => {
            let result = stat_transport(&mut transport, path)?;
            println!("File: {path}");
            println!("  Size: {} ({} bytes)", format_bytes(result.len), result.len);
            Ok(())
        }

        Commands::FsHash { path, hash_type } => {
            let result = hash_transport(&mut transport, path, hash_type.as_deref(), None, None)?;
            println!("File: {path}");
            println!("  Type:   {}", result.hash_type);
            println!("  Offset: {}", result.off);
            println!("  Length: {}", result.len);
            println!("  Hash:   {}", hex::encode(&result.output));
            Ok(())
        }

        // ============== Statistics Management ==============
        Commands::StatList => {
            let result = stat_list_transport(&mut transport)?;
            println!("Available statistics groups:");
            for name in result.stat_list {
                println!("  {name}");
            }
            Ok(())
        }

        Commands::StatRead { name } => {
            let result = stat_read_transport(&mut transport, name)?;
            println!("Statistics for '{}':", result.name);
            for (field, value) in result.fields.iter() {
                println!("  {field}: {value}");
            }
            Ok(())
        }

        // ============== Settings/Config Management ==============
        Commands::SettingsRead { name, max_size } => {
            let result = settings_read_transport(&mut transport, name, *max_size)?;
            println!("Setting '{}': {}", name, hex::encode(&result.val));
            // Try to also print as string if it's valid UTF-8
            if let Ok(s) = std::str::from_utf8(&result.val) {
                if s.chars().all(|c| c.is_ascii_graphic() || c.is_ascii_whitespace()) {
                    println!("  (as string): {s}");
                }
            }
            Ok(())
        }

        Commands::SettingsWrite { name, value } => {
            let bytes = hex::decode(value)
                .map_err(|e| anyhow::anyhow!("Invalid hex value: {}", e))?;
            settings_write_transport(&mut transport, name, bytes)?;
            println!("Setting '{name}' written successfully");
            Ok(())
        }

        Commands::SettingsDelete { name } => {
            settings_delete_transport(&mut transport, name)?;
            println!("Setting '{name}' deleted successfully");
            Ok(())
        }

        Commands::SettingsCommit => {
            settings_commit_transport(&mut transport)?;
            println!("Settings committed successfully");
            Ok(())
        }

        Commands::SettingsLoad => {
            settings_load_transport(&mut transport)?;
            println!("Settings loaded successfully");
            Ok(())
        }

        Commands::SettingsSave => {
            settings_save_transport(&mut transport)?;
            println!("Settings saved successfully");
            Ok(())
        }
    }
}
