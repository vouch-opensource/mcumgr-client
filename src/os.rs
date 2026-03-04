// Copyright © 2026 Rudis Laboratories LLC

use anyhow::{bail, Error, Result};
use log::{debug, info};

use crate::nmp_hdr::*;
use crate::transfer::encode_request;
use crate::transfer::next_seq_id;
use crate::transfer::open_port;
use crate::transfer::transceive;
use crate::transfer::SerialSpecs;
use crate::transfer::Transport;

fn get_rc(response_body: &serde_cbor::Value) -> Option<i32> {
    if let serde_cbor::Value::Map(object) = response_body {
        for (key, val) in object.iter() {
            if let serde_cbor::Value::Text(rc_key) = key {
                if rc_key == "rc" {
                    if let serde_cbor::Value::Integer(rc) = val {
                        return Some(*rc as i32);
                    }
                }
            }
        }
    }
    None
}

fn check_answer(request_header: &NmpHdr, response_header: &NmpHdr) -> bool {
    // verify sequence id
    if response_header.seq != request_header.seq {
        debug!("wrong sequence number");
        return false;
    }

    let expected_op_type = match request_header.op {
        NmpOp::Read => NmpOp::ReadRsp,
        NmpOp::Write => NmpOp::WriteRsp,
        _ => return false,
    };

    // verify response
    if response_header.op != expected_op_type || response_header.group != request_header.group {
        debug!("wrong response types");
        return false;
    }

    true
}

/// Send an echo request to the device
pub fn echo(specs: &SerialSpecs, message: &str) -> Result<String, Error> {
    info!("send echo request: {}", message);

    let mut port = open_port(specs)?;

    let req = EchoReq {
        d: message.to_string(),
    };
    let body = serde_cbor::to_vec(&req)?;

    let (data, request_header) = encode_request(
        specs.linelength,
        NmpOp::Write,
        NmpGroup::Default,
        NmpIdDef::Echo,
        &body,
        next_seq_id(),
    )?;

    let (response_header, response_body) = transceive(&mut *port, &data)?;

    if !check_answer(&request_header, &response_header) {
        bail!("wrong answer types");
    }

    debug!("response_body: {}", serde_json::to_string_pretty(&response_body)?);

    let rsp: EchoRsp = serde_cbor::value::from_value(response_body)
        .map_err(|e| anyhow::format_err!("unexpected answer from device | {}", e))?;

    Ok(rsp.r)
}

/// Get task/thread statistics from the device
pub fn taskstat(specs: &SerialSpecs) -> Result<TaskStatRsp, Error> {
    info!("send taskstat request");

    let mut port = open_port(specs)?;

    let body: Vec<u8> =
        serde_cbor::to_vec(&std::collections::BTreeMap::<String, String>::new()).unwrap();

    let (data, request_header) = encode_request(
        specs.linelength,
        NmpOp::Read,
        NmpGroup::Default,
        NmpIdDef::TaskStat,
        &body,
        next_seq_id(),
    )?;

    let (response_header, response_body) = transceive(&mut *port, &data)?;

    if !check_answer(&request_header, &response_header) {
        bail!("wrong answer types");
    }

    debug!("response_body: {}", serde_json::to_string_pretty(&response_body)?);

    let rsp: TaskStatRsp = serde_cbor::value::from_value(response_body)
        .map_err(|e| anyhow::format_err!("unexpected answer from device | {}", e))?;

    Ok(rsp)
}

/// Get MCUmgr parameters from the device
pub fn mcumgr_params(specs: &SerialSpecs) -> Result<McumgrParamsRsp, Error> {
    info!("send mcumgr_params request");

    let mut port = open_port(specs)?;

    let body: Vec<u8> =
        serde_cbor::to_vec(&std::collections::BTreeMap::<String, String>::new()).unwrap();

    let (data, request_header) = encode_request(
        specs.linelength,
        NmpOp::Read,
        NmpGroup::Default,
        NmpIdDef::McumgrParams,
        &body,
        next_seq_id(),
    )?;

    let (response_header, response_body) = transceive(&mut *port, &data)?;

    if !check_answer(&request_header, &response_header) {
        bail!("wrong answer types");
    }

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
pub fn os_info(specs: &SerialSpecs, format: Option<&str>) -> Result<String, Error> {
    info!("send os_info request");

    let mut port = open_port(specs)?;

    let req = OsInfoReq {
        format: format.map(|s| s.to_string()),
    };
    let body = serde_cbor::to_vec(&req)?;

    let (data, request_header) = encode_request(
        specs.linelength,
        NmpOp::Read,
        NmpGroup::Default,
        NmpIdDef::Info,
        &body,
        next_seq_id(),
    )?;

    let (response_header, response_body) = transceive(&mut *port, &data)?;

    if !check_answer(&request_header, &response_header) {
        bail!("wrong answer types");
    }

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
pub fn bootloader_info(specs: &SerialSpecs, query: Option<&str>) -> Result<BootloaderInfoRsp, Error> {
    info!("send bootloader_info request");

    let mut port = open_port(specs)?;

    let req = BootloaderInfoReq {
        query: query.map(|s| s.to_string()),
    };
    let body = serde_cbor::to_vec(&req)?;

    let (data, request_header) = encode_request(
        specs.linelength,
        NmpOp::Read,
        NmpGroup::Default,
        NmpIdDef::BootloaderInfo,
        &body,
        next_seq_id(),
    )?;

    let (response_header, response_body) = transceive(&mut *port, &data)?;

    if !check_answer(&request_header, &response_header) {
        bail!("wrong answer types");
    }

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

// ==================== Transport-based versions ====================

/// Send an echo request using a transport
pub fn echo_transport(transport: &mut dyn Transport, message: &str) -> Result<String, Error> {
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

/// Get task/thread statistics using a transport
pub fn taskstat_transport(transport: &mut dyn Transport) -> Result<TaskStatRsp, Error> {
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

/// Get MCUmgr parameters using a transport
pub fn mcumgr_params_transport(transport: &mut dyn Transport) -> Result<McumgrParamsRsp, Error> {
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

/// Get OS/application information using a transport
pub fn os_info_transport(transport: &mut dyn Transport, format: Option<&str>) -> Result<String, Error> {
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

/// Get bootloader information using a transport
pub fn bootloader_info_transport(transport: &mut dyn Transport, query: Option<&str>) -> Result<BootloaderInfoRsp, Error> {
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
