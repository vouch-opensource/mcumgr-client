// Copyright © 2023 Vouch.io LLC

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

    /// maximum timeout in seconds
    #[arg(short, long, default_value_t = 10)]
    pub timeout: u32,

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

    /// upload a file to the device
    Upload { filename: PathBuf },
}
