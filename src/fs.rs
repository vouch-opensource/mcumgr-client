// Copyright © 2026 Rudis Laboratories LLC

use anyhow::{bail, Error, Result};
use indicatif::{ProgressBar, ProgressStyle};
use log::{debug, info};
use std::fs;
use std::path::Path;

use crate::nmp_hdr::*;
use crate::transfer::Transport;
use crate::util::get_rc;

/// Download a file from the device
///
/// Downloads a file from the remote path on the device to a local file.
pub fn download(transport: &mut dyn Transport, remote_path: &str, local_path: &Path) -> Result<(), Error> {
    info!("download file: {} -> {}", remote_path, local_path.display());

    let mut file_data: Vec<u8> = Vec::new();
    let mut offset: u32 = 0;
    let mut total_len: Option<u32> = None;

    // Create progress bar
    let pb = ProgressBar::new(0);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({eta})")
            .unwrap()
            .progress_chars("=> "),
    );

    loop {
        let req = FsDownloadReq {
            name: remote_path.to_string(),
            off: offset,
        };
        let body = serde_cbor::to_vec(&req)?;

        let (_response_header, response_body) = transport.transceive(
            NmpOp::Read,
            NmpGroup::Fs,
            NmpIdFs::File.to_u8(),
            &body,
        )?;

        debug!("response_body: {}", serde_json::to_string_pretty(&response_body)?);

        // Check for rc error
        if let Some(rc) = get_rc(&response_body) {
            if rc != 0 {
                bail!("Error from device: rc={}", rc);
            }
        }

        let rsp: FsDownloadRsp = serde_cbor::value::from_value(response_body)
            .map_err(|e| anyhow::format_err!("unexpected answer from device | {}", e))?;

        // On first chunk, get the total length
        if offset == 0 {
            if let Some(len) = rsp.len {
                total_len = Some(len);
                pb.set_length(len as u64);
            }
        }

        // Append data
        file_data.extend_from_slice(&rsp.data);
        offset = rsp.off + rsp.data.len() as u32;
        pb.set_position(offset as u64);

        // Check if we're done
        if let Some(len) = total_len {
            if offset >= len {
                break;
            }
        }

        // If no data was returned, we're done
        if rsp.data.is_empty() {
            break;
        }

        // Reduce timeout for subsequent packets
        transport.set_timeout(200)?;
    }

    pb.finish_with_message("download complete");

    // Write to local file
    fs::write(local_path, &file_data)?;
    info!("downloaded {} bytes", file_data.len());

    Ok(())
}

/// Upload a file to the device
///
/// Uploads a local file to the remote path on the device.
pub fn upload(transport: &mut dyn Transport, local_path: &Path, remote_path: &str) -> Result<(), Error> {
    info!("upload file: {} -> {}", local_path.display(), remote_path);

    let file_data = fs::read(local_path)?;
    let total_len = file_data.len() as u32;
    let mut offset: u32 = 0;
    let mtu = transport.mtu();

    info!("{} bytes to transfer", total_len);

    // Create progress bar
    let pb = ProgressBar::new(total_len as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({eta})")
            .unwrap()
            .progress_chars("=> "),
    );

    while offset < total_len {
        // Calculate chunk size based on MTU
        let mut chunk_size = mtu;
        if offset + chunk_size as u32 > total_len {
            chunk_size = (total_len - offset) as usize;
        }

        let chunk = file_data[offset as usize..(offset as usize + chunk_size)].to_vec();

        let req = FsUploadReq {
            name: remote_path.to_string(),
            off: offset,
            data: chunk,
            len: if offset == 0 { Some(total_len) } else { None },
        };
        let body = serde_cbor::to_vec(&req)?;

        let (_response_header, response_body) = transport.transceive(
            NmpOp::Write,
            NmpGroup::Fs,
            NmpIdFs::File.to_u8(),
            &body,
        )?;

        debug!("response_body: {}", serde_json::to_string_pretty(&response_body)?);

        // Check for rc error
        if let Some(rc) = get_rc(&response_body) {
            if rc != 0 {
                bail!("Error from device: rc={}", rc);
            }
        }

        let rsp: FsUploadRsp = serde_cbor::value::from_value(response_body)
            .map_err(|e| anyhow::format_err!("unexpected answer from device | {}", e))?;

        offset = rsp.off;
        pb.set_position(offset as u64);

        // Reduce timeout for subsequent packets
        if offset > 0 {
            transport.set_timeout(200)?;
        }
    }

    pb.finish_with_message("upload complete");
    info!("uploaded {} bytes", total_len);

    Ok(())
}

/// Get file status (size) from the device
pub fn stat(transport: &mut dyn Transport, path: &str) -> Result<FsStatRsp, Error> {
    info!("stat file: {}", path);

    let req = FsStatReq {
        name: path.to_string(),
    };
    let body = serde_cbor::to_vec(&req)?;

    let (_response_header, response_body) = transport.transceive(
        NmpOp::Read,
        NmpGroup::Fs,
        NmpIdFs::FileStat.to_u8(),
        &body,
    )?;

    debug!("response_body: {}", serde_json::to_string_pretty(&response_body)?);

    let rsp: FsStatRsp = serde_cbor::value::from_value(response_body)
        .map_err(|e| anyhow::format_err!("unexpected answer from device | {}", e))?;

    if rsp.rc != 0 {
        bail!("Error from device: rc={}", rsp.rc);
    }

    Ok(rsp)
}

/// Calculate hash/checksum of a file on the device
pub fn hash(
    transport: &mut dyn Transport,
    path: &str,
    hash_type: Option<&str>,
    off: Option<u32>,
    len: Option<u32>,
) -> Result<FsHashRsp, Error> {
    info!("hash file: {}", path);

    let req = FsHashReq {
        name: path.to_string(),
        hash_type: hash_type.map(|s| s.to_string()),
        off,
        len,
    };
    let body = serde_cbor::to_vec(&req)?;

    let (_response_header, response_body) = transport.transceive(
        NmpOp::Read,
        NmpGroup::Fs,
        NmpIdFs::FileHash.to_u8(),
        &body,
    )?;

    debug!("response_body: {}", serde_json::to_string_pretty(&response_body)?);

    let rsp: FsHashRsp = serde_cbor::value::from_value(response_body)
        .map_err(|e| anyhow::format_err!("unexpected answer from device | {}", e))?;

    if rsp.rc != 0 {
        bail!("Error from device: rc={}", rsp.rc);
    }

    Ok(rsp)
}
