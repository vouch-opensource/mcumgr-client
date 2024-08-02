// Copyright © 2023-2024 Vouch.io LLC

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

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// device name
    #[arg(short, long, default_value = "")]
    device: String,

    /// verbose mode
    #[arg(short, long)]
    verbose: bool,

    /// initial timeout in seconds
    #[arg(short = 't', long = "initial_timeout", default_value_t = 60)]
    initial_timeout_s: u32,

    /// subsequent timeout in msec
    #[arg(short = 'u', long = "subsequent_timeout", default_value_t = 200)]
    subsequent_timeout_ms: u32,

    // number of retry per packet
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

#[derive(Subcommand)]
enum Commands {
    /// list slots on the device
    List,

    /// reset the device
    Reset,

    /// upload a file to the device
    Upload {
        filename: PathBuf,

        /// slot number
        #[arg(short, long, default_value_t = 1)]
        slot: u8,
    },

    Test {
        hash: String,
        #[arg(short, long)]
        confirm: Option<bool>,
    },
    Erase {
        #[arg(short, long)]
        slot: Option<u32>,
    },
}

fn main() {
    // show program name, version and copyright
    let name = env!("CARGO_PKG_NAME");
    let version = env!("CARGO_PKG_VERSION");
    println!("{} {}, Copyright © 2024 Vouch.io LLC", name, version);
    println!("");

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

    // if no device is specified, try to auto detect it
    if cli.device.is_empty() {
        let mut bootloaders = Vec::new();
        match available_ports() {
            Ok(ports) => {
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
            Err(_) => {}
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
                        println!("Error listing serial ports: {}", e);
                        process::exit(1);
                    }
                }
            }
        }
    }

    let specs = SerialSpecs::from(&cli);

    // execute command
    let result = match &cli.command {
        Commands::List => || -> Result<(), Error> {
            let v = list(&specs)?;
            print!("response: {}", serde_json::to_string_pretty(&v)?);
            Ok(())
        }(),
        Commands::Reset => reset(&specs),
        Commands::Upload { filename, slot } => || -> Result<(), Error> {
            // create a progress bar
            let pb = ProgressBar::new(1 as u64);
            pb.set_style(ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({eta})")
            .unwrap().progress_chars("=> "));

            upload(
                &specs,
                filename,
                *slot,
                Some(|offset, total| {
                    if let Some(l) = pb.length() {
                        if l != total {
                            pb.set_length(total as u64)
                        }
                    }

                    pb.set_position(offset as u64);

                    if offset >= total {
                        pb.finish_with_message("upload complete");
                    }
                }),
            )
        }(),
        Commands::Test { hash, confirm } => || -> Result<(), Error> { 
            test(&specs, hex::decode(hash)?, *confirm)
        }(),
        Commands::Erase { slot } => erase(&specs, *slot),
    };

    // show error, if failed
    if let Err(e) = result {
        error!("Error: {}", e);
        process::exit(1);
    }
}
