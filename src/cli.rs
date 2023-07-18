// Copyright © 2023 Vouch.io LLC

use anyhow::{Context, Error};
use clap::{Parser, Subcommand};
use std::sync::mpsc;
use std::{path::PathBuf, time::Duration, thread};

use crate::{
    interface::Interface, serial_port_interface::SerialPortInterface,
    test_serial_port_interface::TestSerialPortInterface, bluetooth_interface::BluetoothInterface,
};

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
    #[arg(short, long, default_value_t = 60)]
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

    /// echo a message
    Echo { message: String },

    /// reset the device
    Reset,
}

pub fn open_port(cli: &Cli) -> Result<Box<dyn Interface>, Error> {
    match cli.device.to_lowercase().as_str() {
        "test" => Ok(Box::new(TestSerialPortInterface::new())),
        "bluetooth" => {
            let (tx, rx) = mpsc::channel();
            thread::spawn(move || {
                let rt = tokio::runtime::Runtime::new().unwrap();
                rt.block_on(async {
                    let result = BluetoothInterface::new().await;
                    let _ = tx.send(result);
                });
            });
            let bluetooth_interface = rx.recv().unwrap().with_context(|| format!("Failed to open Bluetooth interface"))?;
            Ok(Box::new(bluetooth_interface))
        },
        _ => {
            let serial_port = serialport::new(&cli.device, cli.baudrate)
                .timeout(Duration::from_secs(cli.timeout as u64))
                .open()
                .with_context(|| format!("failed to open serial port {}", &cli.device))?;
            Ok(Box::new(SerialPortInterface::new(serial_port)))
        }
    }
}
