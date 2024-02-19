// Copyright © 2023-2024 Vouch.io LLC

use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    /// device name
    #[arg(short, long, default_value = "")]
    pub device: String,

    /// slot number
    #[arg(short, long, default_value_t = 1)]
    pub slot: u8,

    /// verbose mode
    #[arg(short, long)]
    pub verbose: bool,

    /// initial timeout in seconds
    #[arg(short = 'i', long = "initial_timeout", default_value_t = 60)]
    pub initial_timeout_s: u32,

    /// subsequent timeout in msec
    #[arg(short = 't', long = "subsequent_timeout", default_value_t = 200)]
    pub subsequent_timeout_ms: u32,

    // number of retry per packet
    #[arg(long, default_value_t = 4)]
    pub nb_retry: u32,

    /// maximum length per line
    #[arg(short, long, default_value_t = 128)]
    pub linelength: usize,

    /// maximum length per request
    #[arg(short, long, default_value_t = 512)]
    pub mtu: usize,

    /// baudrate
    #[arg(short, long, default_value_t = 115_200)]
    pub baudrate: u32,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// list slots on the device
    List,

    /// reset the device
    Reset,

    /// upload a file to the device
    Upload { filename: PathBuf },
}
