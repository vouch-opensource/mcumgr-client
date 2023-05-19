// Copyright © 2023 Vouch.io LLC

use clap::Parser;
use log::{error, info, LevelFilter};
use serialport::available_ports;
use simplelog::{ColorChoice, Config, SimpleLogger, TermLogger, TerminalMode};
use std::process;

pub mod cli;
pub mod image;
pub mod nmp_hdr;
pub mod test_serial_port;
pub mod transfer;

use crate::cli::*;
use crate::image::*;

fn main() {
    // show program name, version and copyright
    let name = env!("CARGO_PKG_NAME");
    let version = env!("CARGO_PKG_VERSION");
    println!("{} {}, Copyright © 2023 Vouch.io LLC", name, version);
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

    // if no device is specified, list all devices and use it, if there is only one device
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

    // execute command
    let result = match &cli.command {
        Commands::List => list(&cli),
        Commands::Upload { filename } => upload(&cli, filename),
    };

    // show error, if failed
    if let Err(e) = result {
        error!("Error: {}", e);
        process::exit(1);
    }
}
