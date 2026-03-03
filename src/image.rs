// Copyright © 2023-2024 Vouch.io LLC, 2026 Rudis Laboratories LLC

use anyhow::{bail, Error, Result};
use humantime::format_duration;
use log::{debug, info, warn};
use sha2::{Digest, Sha256};
use std::fs::read;
use std::path::PathBuf;
use std::time::Duration;
use std::time::Instant;

use crate::nmp_hdr::*;
use crate::transfer::Transport;
use crate::util::get_rc;

/// Erase an image slot
pub fn erase(transport: &mut dyn Transport, slot: Option<u32>) -> Result<(), Error> {
    info!("erase request");

    let req = ImageEraseReq { slot };
    let body = serde_cbor::to_vec(&req)?;

    let (_response_header, response_body) = transport.transceive(
        NmpOp::Write,
        NmpGroup::Image,
        NmpIdImage::Erase.to_u8(),
        &body,
    )?;

    if let Some(rc) = get_rc(&response_body) {
        if rc != 0 {
            bail!("Error from device: {}", rc);
        }
    }

    debug!("{:?}", response_body);
    Ok(())
}

/// Set image pending/confirm
pub fn test(transport: &mut dyn Transport, hash: Vec<u8>, confirm: Option<bool>) -> Result<(), Error> {
    info!("set image pending request");

    let req = ImageStateReq { hash, confirm };
    let body = serde_cbor::to_vec(&req)?;

    let (_response_header, response_body) = transport.transceive(
        NmpOp::Write,
        NmpGroup::Image,
        NmpIdImage::State.to_u8(),
        &body,
    )?;

    if let Some(rc) = get_rc(&response_body) {
        if rc != 0 {
            return Err(anyhow::format_err!("Error from device: {}", rc));
        }
    }

    debug!("{:?}", response_body);
    Ok(())
}

/// List images
pub fn list(transport: &mut dyn Transport) -> Result<ImageStateRsp, Error> {
    info!("send image list request");

    let body: Vec<u8> =
        serde_cbor::to_vec(&std::collections::BTreeMap::<String, String>::new()).unwrap();

    let (_response_header, response_body) = transport.transceive(
        NmpOp::Read,
        NmpGroup::Image,
        NmpIdImage::State.to_u8(),
        &body,
    )?;

    let ans: ImageStateRsp = serde_cbor::value::from_value(response_body)
        .map_err(|e| anyhow::format_err!("unexpected answer from device | {}", e))?;

    Ok(ans)
}

/// Upload an image with retry logic
pub fn upload_image<F>(
    transport: &mut dyn Transport,
    filename: &PathBuf,
    slot: u8,
    nb_retry: u32,
    mut progress: Option<F>,
) -> Result<(), Error>
where
    F: FnMut(u64, u64),
{
    let filename_string = filename.to_string_lossy();
    info!("upload file: {}", filename_string);

    // special feature: if the name contains "slot1" or "slot3", then use this slot
    let filename_lowercase = filename_string.to_lowercase();
    let mut slot = slot;
    if filename_lowercase.contains("slot1") {
        slot = 1;
    }
    if filename_lowercase.contains("slot3") {
        slot = 3;
    }
    info!("flashing to slot {}", slot);

    // load file
    let data = read(filename)?;
    info!("{} bytes to transfer", data.len());

    let mtu = transport.mtu();

    // transfer in blocks
    let mut off: usize = 0;
    let start_time = Instant::now();
    let mut sent_blocks: u32 = 0;
    let mut confirmed_blocks: u32 = 0;

    while off < data.len() {
        let mut retries_left = nb_retry;
        let off_start = off;
        let try_length = mtu;

        loop {
            // get slot
            let image_num = slot;

            // create image upload request
            let mut chunk_len = try_length;
            if off + chunk_len > data.len() {
                chunk_len = data.len() - off;
            }
            let chunk = data[off..off + chunk_len].to_vec();
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

            sent_blocks += 1;
            match transport.transceive(
                NmpOp::Write,
                NmpGroup::Image,
                NmpIdImage::Upload.to_u8(),
                &body,
            ) {
                Ok((_response_header, response_body)) => {
                    // verify result code and update offset
                    debug!(
                        "response_body: {}",
                        serde_json::to_string_pretty(&response_body)?
                    );

                    if let serde_cbor::Value::Map(object) = &response_body {
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
                Err(e) => {
                    if retries_left == 0 {
                        return Err(e);
                    }
                    retries_left -= 1;
                    debug!("missed answer, retries left: {}", retries_left);
                    continue;
                }
            }
        }

        // next chunk, next off should have been sent from the device
        if off_start == off {
            bail!("wrong offset received");
        }

        if let Some(ref mut f) = progress {
            f(off as u64, data.len() as u64);
        }

        if off >= data.len() {
            break;
        }

        // Reduce timeout for subsequent packets
        transport.set_timeout(200)?;
    }

    let elapsed = start_time.elapsed().as_secs_f64().round();
    let elapsed_duration = Duration::from_secs(elapsed as u64);
    let formatted_duration = format_duration(elapsed_duration);
    info!("upload took {}", formatted_duration);
    if confirmed_blocks != sent_blocks {
        warn!(
            "upload packet loss {}%",
            100 - confirmed_blocks * 100 / sent_blocks
        );
    }

    Ok(())
}
