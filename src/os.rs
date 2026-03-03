// Copyright © 2026 Rudis Laboratories LLC

use anyhow::{bail, Error, Result};
use log::{debug, info};

use crate::nmp_hdr::*;
use crate::transfer::Transport;
use crate::util::get_rc;

/// Send an echo request to the device
pub fn echo(transport: &mut dyn Transport, message: &str) -> Result<String, Error> {
    info!("send echo request: {}", message);

    let req = EchoReq {
        d: message.to_string(),
    };
    let body = serde_cbor::to_vec(&req)?;

    let (_response_header, response_body) = transport.transceive(
        NmpOp::Write,
        NmpGroup::Default,
        NmpIdDef::Echo.to_u8(),
        &body,
    )?;

    debug!("response_body: {}", serde_json::to_string_pretty(&response_body)?);

    let rsp: EchoRsp = serde_cbor::value::from_value(response_body)
        .map_err(|e| anyhow::format_err!("unexpected answer from device | {}", e))?;

    Ok(rsp.r)
}

/// Get task/thread statistics from the device
pub fn taskstat(transport: &mut dyn Transport) -> Result<TaskStatRsp, Error> {
    info!("send taskstat request");

    let body: Vec<u8> =
        serde_cbor::to_vec(&std::collections::BTreeMap::<String, String>::new()).unwrap();

    let (_response_header, response_body) = transport.transceive(
        NmpOp::Read,
        NmpGroup::Default,
        NmpIdDef::TaskStat.to_u8(),
        &body,
    )?;

    debug!("response_body: {}", serde_json::to_string_pretty(&response_body)?);

    let rsp: TaskStatRsp = serde_cbor::value::from_value(response_body)
        .map_err(|e| anyhow::format_err!("unexpected answer from device | {}", e))?;

    Ok(rsp)
}

/// Get MCUmgr parameters from the device
pub fn mcumgr_params(transport: &mut dyn Transport) -> Result<McumgrParamsRsp, Error> {
    info!("send mcumgr_params request");

    let body: Vec<u8> =
        serde_cbor::to_vec(&std::collections::BTreeMap::<String, String>::new()).unwrap();

    let (_response_header, response_body) = transport.transceive(
        NmpOp::Read,
        NmpGroup::Default,
        NmpIdDef::McumgrParams.to_u8(),
        &body,
    )?;

    debug!("response_body: {}", serde_json::to_string_pretty(&response_body)?);

    let rsp: McumgrParamsRsp = serde_cbor::value::from_value(response_body)
        .map_err(|e| anyhow::format_err!("unexpected answer from device | {}", e))?;

    Ok(rsp)
}

/// Get OS/application information from the device
///
/// Format specifiers:
/// - s: Kernel name
/// - n: Node name
/// - r: Kernel release
/// - v: Kernel version
/// - b: Build date and time
/// - m: Machine
/// - p: Processor
/// - i: Hardware platform
/// - o: Operating system
/// - a: All fields
pub fn os_info(transport: &mut dyn Transport, format: Option<&str>) -> Result<String, Error> {
    info!("send os_info request");

    let req = OsInfoReq {
        format: format.map(|s| s.to_string()),
    };
    let body = serde_cbor::to_vec(&req)?;

    let (_response_header, response_body) = transport.transceive(
        NmpOp::Read,
        NmpGroup::Default,
        NmpIdDef::Info.to_u8(),
        &body,
    )?;

    debug!("response_body: {}", serde_json::to_string_pretty(&response_body)?);

    // Check for rc error
    if let Some(rc) = get_rc(&response_body) {
        if rc != 0 {
            bail!("Error from device: rc={}", rc);
        }
    }

    let rsp: OsInfoRsp = serde_cbor::value::from_value(response_body)
        .map_err(|e| anyhow::format_err!("unexpected answer from device | {}", e))?;

    Ok(rsp.output)
}

/// Get bootloader information from the device
///
/// Query options:
/// - None: Get basic bootloader info (name)
/// - Some("mode"): Get MCUboot mode information
pub fn bootloader_info(transport: &mut dyn Transport, query: Option<&str>) -> Result<BootloaderInfoRsp, Error> {
    info!("send bootloader_info request");

    let req = BootloaderInfoReq {
        query: query.map(|s| s.to_string()),
    };
    let body = serde_cbor::to_vec(&req)?;

    let (_response_header, response_body) = transport.transceive(
        NmpOp::Read,
        NmpGroup::Default,
        NmpIdDef::BootloaderInfo.to_u8(),
        &body,
    )?;

    debug!("response_body: {}", serde_json::to_string_pretty(&response_body)?);

    // Check for rc error
    if let Some(rc) = get_rc(&response_body) {
        if rc != 0 {
            bail!("Error from device: rc={}", rc);
        }
    }

    let rsp: BootloaderInfoRsp = serde_cbor::value::from_value(response_body)
        .map_err(|e| anyhow::format_err!("unexpected answer from device | {}", e))?;

    Ok(rsp)
}

/// MCUboot mode names for display
pub fn mcuboot_mode_name(mode: i32) -> &'static str {
    match mode {
        0 => "Single application",
        1 => "Swap using scratch partition",
        2 => "Overwrite (upgrade-only)",
        3 => "Swap without scratch",
        4 => "Direct XIP without revert",
        5 => "Direct XIP with revert",
        6 => "RAM loader",
        7 => "Firmware loader",
        8 => "RAM load with network core",
        9 => "Swap using move",
        _ => "Unknown",
    }
}
