// Copyright © 2023-2024 Vouch.io LLC

use anyhow::{bail, Error, Result};
use humantime::format_duration;
use indicatif::{ProgressBar, ProgressStyle};
use log::{debug, info, warn};
use serde_cbor;
use serde_json;
use sha2::{Digest, Sha256};
use std::fs::read;
use std::path::PathBuf;
use std::time::Duration;
use std::time::Instant;

use crate::cli::*;
use crate::nmp_hdr::*;
use crate::transfer::encode_request;
use crate::transfer::next_seq_id;
use crate::transfer::open_port;
use crate::transfer::transceive;

pub fn list(cli: &Cli) -> Result<(), Error> {
    info!("send image list request");

    // open serial port
    let mut port = open_port(cli)?;

    // send request
    let body: Vec<u8> =
        serde_cbor::to_vec(&std::collections::BTreeMap::<String, String>::new()).unwrap();
    let (data, request_header) = encode_request(
        cli.linelength,
        NmpOp::Read,
        NmpGroup::Image,
        NmpIdImage::State,
        &body,
        next_seq_id(),
    )?;
    let (response_header, response_body) = transceive(&mut *port, &data)?;

    // verify sequence id
    if response_header.seq != request_header.seq {
        bail!("wrong sequence number");
    }

    // verify response
    if response_header.op != NmpOp::ReadRsp || response_header.group != NmpGroup::Image {
        bail!("wrong response types");
    }

    // print body
    info!(
        "response: {}",
        serde_json::to_string_pretty(&response_body)?
    );

    Ok(())
}

pub fn upload(cli: &Cli, filename: &PathBuf) -> Result<(), Error> {
    let filename_string = filename.to_string_lossy();
    info!("upload file: {}", filename_string);

    // special feature: if the name contains "slot1" or "slot3", then use this slot
    let filename_lowercase = filename_string.to_lowercase();
    let mut slot = cli.slot;
    if filename_lowercase.contains(&"slot1".to_lowercase()) {
        slot = 1;
    }
    if filename_lowercase.contains(&"slot3".to_lowercase()) {
        slot = 3;
    }
    info!("flashing to slot {}", slot);

    // open serial port
    let mut port = open_port(cli)?;

    // load file
    let data = read(filename)?;
    info!("{} bytes to transfer", data.len());

    // create a progress bar
    let pb = ProgressBar::new(data.len() as u64);
    pb.set_style(ProgressStyle::default_bar()
    .template("{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({eta})")
    .unwrap().progress_chars("=> "));

    // transfer in blocks
    let mut off: usize = 0;
    let start_time = Instant::now();
    let mut sent_blocks: u32 = 0;
    let mut confirmed_blocks: u32 = 0;
    loop {
        let mut nb_retry = cli.nb_retry;
        let off_start = off;
        let mut try_length = cli.mtu;
        debug!("try_length: {}", try_length);
        let seq_id = next_seq_id();
        loop {
            // get slot
            let image_num = slot;

            // create image upload request
            if off + try_length > data.len() {
                try_length = data.len() - off;
            }
            let chunk = data[off..off + try_length].to_vec();
            let len = data.len() as u32;
            let req = if off == 0 {
                ImageUploadReq {
                    image_num,
                    off: off as u32,
                    len: Some(len),
                    data_sha: Some(Sha256::digest(&data).to_vec()),
                    upgrade: None,
                    data: chunk,
                }
            } else {
                ImageUploadReq {
                    image_num,
                    off: off as u32,
                    len: None,
                    data_sha: None,
                    upgrade: None,
                    data: chunk,
                }
            };
            debug!("req: {:?}", req);

            // convert to bytes with CBOR
            let body = serde_cbor::to_vec(&req)?;
            let (chunk, request_header) = encode_request(
                cli.linelength,
                NmpOp::Write,
                NmpGroup::Image,
                NmpIdImage::Upload,
                &body,
                seq_id,
            )?;

            // test if too long
            if chunk.len() > cli.mtu {
                let reduce = chunk.len() - cli.mtu;
                if reduce > try_length {
                    bail!("MTU too small");
                }

                // number of bytes to reduce is base64 encoded, calculate back the number of bytes
                // and then reduce a bit more for base64 filling and rounding
                try_length -= reduce * 3 / 4 + 3;
                debug!("new try_length: {}", try_length);
                continue;
            }

            // send request
            sent_blocks += 1;
            let (response_header, response_body) = match transceive(&mut *port, &chunk) {
                Ok(ret) => ret,
                Err(e) if e.to_string() == "Operation timed out" => {
                    if nb_retry == 0 {
                        return Err(e);
                    }
                    nb_retry -= 1;
                    debug!("missed answer, nb_retry: {}", nb_retry);
                    continue;
                }
                Err(e) => return Err(e),
            };

            // verify sequence id
            if response_header.seq != request_header.seq {
                bail!("wrong sequence number");
            }

            // verify response
            if response_header.op != NmpOp::WriteRsp || response_header.group != NmpGroup::Image {
                bail!("wrong response types");
            }

            // verify result code and update offset
            debug!(
                "response_body: {}",
                serde_json::to_string_pretty(&response_body)?
            );
            if let serde_cbor::Value::Map(object) = response_body {
                for (key, val) in object.iter() {
                    match key {
                        serde_cbor::Value::Text(rc_key) if rc_key == "rc" => {
                            if let serde_cbor::Value::Integer(rc) = val {
                                if *rc != 0 {
                                    bail!("rc = {}", rc);
                                }
                            }
                        }
                        serde_cbor::Value::Text(off_key) if off_key == "off" => {
                            if let serde_cbor::Value::Integer(off_val) = val {
                                off = *off_val as usize;
                            }
                        }
                        _ => (),
                    }
                }
            }
            confirmed_blocks += 1;
            break;
        }

        // next chunk, next off should have been sent from the device
        if off_start == off {
            bail!("wrong offset received");
        }
        pb.set_position(off as u64);
        //info!("{}% uploaded", 100 * off / data.len());
        if off == data.len() {
            break;
        }
    
        // The first packet was sent and the device has cleared its internal flash
        // We can now lower the timeout in case of failed transmission
        port.set_timeout(Duration::from_millis(cli.subsequent_timeout_ms as u64))?;
    }
    pb.finish_with_message("upload complete");

    let elapsed = start_time.elapsed().as_secs_f64().round();
    let elapsed_duration = Duration::from_secs(elapsed as u64);
    let formatted_duration = format_duration(elapsed_duration);
    info!("upload took {}", formatted_duration);
    if confirmed_blocks != sent_blocks {
        warn!("upload packet loss {}%", 100 - confirmed_blocks * 100 / sent_blocks);
    }

    Ok(())
}
