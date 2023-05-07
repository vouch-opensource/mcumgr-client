// Copyright © 2023 Vouch.io LLC

use clap::Parser;
use log::{error, LevelFilter};
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
    let cli = Cli::parse();

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
