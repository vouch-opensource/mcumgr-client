// Copyright © 2023 Vouch.io LLC

use btleplug::api::ValueNotification;
use clap::Parser;
use log::{error, info, LevelFilter};
use serialport::available_ports;
use simplelog::{ColorChoice, Config, SimpleLogger, TermLogger, TerminalMode};
use std::env;
use std::process;

use btleplug::api::Characteristic;
use btleplug::api::{
    bleuuid::uuid_from_u16, Central, CentralEvent, Manager as _, Peripheral as _, ScanFilter,
    WriteType,
};
use btleplug::platform::{Adapter, Manager, Peripheral};
use futures::stream::StreamExt;
use rand::{thread_rng, Rng};
use std::error::Error;
use std::thread;
use std::time::Duration;
use tokio::runtime::Runtime;
use tokio::time;
use uuid::Uuid;

pub mod cli;
pub mod default;
pub mod image;
pub mod nmp_hdr;
pub mod test_serial_port;
pub mod transfer;

use crate::cli::*;
use crate::default::*;
use crate::image::*;

async fn connect_light() -> Result<Peripheral, Box<dyn Error>> {
    let manager = Manager::new().await.unwrap();

    // get the first bluetooth adapter
    let adapters = manager.adapters().await.unwrap();
    let central = adapters.into_iter().nth(0).unwrap();

    let desired_name = "VKA";

    let mut found_light = None;
    let mut events = central.events().await.unwrap();

    // start scanning for devices
    central.start_scan(ScanFilter::default()).await.unwrap();

    while let Some(event) = events.next().await {
        if let CentralEvent::DeviceDiscovered(addr) = event {
            let peripheral = central.peripheral(&addr).await.unwrap();
            if let Some(properties) = peripheral.properties().await.unwrap() {
                if let Some(name) = properties.local_name {
                    if name == desired_name {
                        found_light = Some(peripheral);
                        break;
                    }
                }
            }
        }
    }

    let light = found_light
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::Other, "Device not found"))?;

    // connect to the device
    light.connect().await?;
    println!("VKA connected");
    Ok(light)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    const MAX_ATTEMPTS: u32 = 3;

    let mut light = None;

    for attempt in 1..=MAX_ATTEMPTS {
        match connect_light().await {
            Ok(l) => {
                light = Some(l);
                break;
            }
            Err(e) if attempt < MAX_ATTEMPTS => {
                println!("Attempt #{} failed, retrying...", attempt);
                time::sleep(Duration::from_secs(1)).await;
            }
            Err(e) => {
                println!("Attempt #{} failed, giving up.", attempt);
                return Err(e);
            }
        }
    }

    let light = light.expect("Unable to establish connection");

    // Discover services and characteristics
    light.discover_services().await.unwrap();

    // Get a list of the peripheral's characteristics.
    let characteristics = light.characteristics();

    // Filter the characteristics to find the one you're interested in.
    let char_uuid = Uuid::parse_str("DA2E7828-FBCE-4E01-AE9E-261174997C48").unwrap();
    let desired_char = characteristics
        .into_iter()
        .find(|c| c.uuid == char_uuid)
        .unwrap();

    // Subscribe to notifications from the characteristic.
    light.subscribe(&desired_char).await.unwrap();

    // Write bytes to the characteristic.
    let bytes_to_write = vec![0x00, 0x00, 0x00, 0x01, 0x00, 0x01, 0x80, 0x00, 0xa0]; // Replace with the bytes you want to write
    light
        .write(&desired_char, &bytes_to_write, WriteType::WithoutResponse)
        .await
        .unwrap();

    let mut notification_stream = light.notifications().await.unwrap();
    loop {
        match notification_stream.next().await {
            Some(ValueNotification { uuid, value, .. }) if uuid == desired_char.uuid => {
                println!("Received data: {:?}", value);
                break;
            }
            _ => {}
        }
        time::sleep(Duration::from_millis(100)).await;
    }

    return Ok(());

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

    // execute command
    let result = match &cli.command {
        Commands::List => list(&cli),
        Commands::Upload { filename } => upload(&cli, filename),
        Commands::Echo { message } => echo(&cli, message),
        Commands::Reset => reset(&cli),
    };

    // show error, if failed
    if let Err(e) = result {
        error!("Error: {}", e);
        process::exit(1);
    }

    Ok(())
}
