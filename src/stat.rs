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

/// List available statistics groups on the device
pub fn stat_list(specs: &SerialSpecs) -> Result<StatListRsp, Error> {
    info!("send stat list request");

    let mut port = open_port(specs)?;

    let body: Vec<u8> =
        serde_cbor::to_vec(&std::collections::BTreeMap::<String, String>::new()).unwrap();

    let (data, request_header) = encode_request(
        specs.linelength,
        NmpOp::Read,
        NmpGroup::Stat,
        NmpIdStat::List,
        &body,
        next_seq_id(),
    )?;

    let (response_header, response_body) = transceive(&mut *port, &data)?;

    if !check_answer(&request_header, &response_header) {
        bail!("wrong answer types");
    }

    debug!("response_body: {}", serde_json::to_string_pretty(&response_body)?);

    let rsp: StatListRsp = serde_cbor::value::from_value(response_body)
        .map_err(|e| anyhow::format_err!("unexpected answer from device | {}", e))?;

    if rsp.rc != 0 {
        bail!("Error from device: rc={}", rsp.rc);
    }

    Ok(rsp)
}

/// Read statistics from a specific group
pub fn stat_read(specs: &SerialSpecs, name: &str) -> Result<StatReadRsp, Error> {
    info!("send stat read request: {}", name);

    let mut port = open_port(specs)?;

    let req = StatReadReq {
        name: name.to_string(),
    };
    let body = serde_cbor::to_vec(&req)?;

    let (data, request_header) = encode_request(
        specs.linelength,
        NmpOp::Read,
        NmpGroup::Stat,
        NmpIdStat::Read,
        &body,
        next_seq_id(),
    )?;

    let (response_header, response_body) = transceive(&mut *port, &data)?;

    if !check_answer(&request_header, &response_header) {
        bail!("wrong answer types");
    }

    debug!("response_body: {}", serde_json::to_string_pretty(&response_body)?);

    let rsp: StatReadRsp = serde_cbor::value::from_value(response_body)
        .map_err(|e| anyhow::format_err!("unexpected answer from device | {}", e))?;

    if rsp.rc != 0 {
        bail!("Error from device: rc={}", rsp.rc);
    }

    Ok(rsp)
}

// ==================== Transport-based versions ====================

/// List available statistics groups using a transport
pub fn stat_list_transport(transport: &mut dyn Transport) -> Result<StatListRsp, Error> {
    info!("send stat list request");

    let body: Vec<u8> =
        serde_cbor::to_vec(&std::collections::BTreeMap::<String, String>::new()).unwrap();

    let (_response_header, response_body) = transport.transceive(
        NmpOp::Read,
        NmpGroup::Stat,
        NmpIdStat::List.to_u8(),
        &body,
    )?;

    debug!("response_body: {}", serde_json::to_string_pretty(&response_body)?);

    let rsp: StatListRsp = serde_cbor::value::from_value(response_body)
        .map_err(|e| anyhow::format_err!("unexpected answer from device | {}", e))?;

    if rsp.rc != 0 {
        bail!("Error from device: rc={}", rsp.rc);
    }

    Ok(rsp)
}

/// Read statistics from a specific group using a transport
pub fn stat_read_transport(transport: &mut dyn Transport, name: &str) -> Result<StatReadRsp, Error> {
    info!("send stat read request: {}", name);

    let req = StatReadReq {
        name: name.to_string(),
    };
    let body = serde_cbor::to_vec(&req)?;

    let (_response_header, response_body) = transport.transceive(
        NmpOp::Read,
        NmpGroup::Stat,
        NmpIdStat::Read.to_u8(),
        &body,
    )?;

    debug!("response_body: {}", serde_json::to_string_pretty(&response_body)?);

    let rsp: StatReadRsp = serde_cbor::value::from_value(response_body)
        .map_err(|e| anyhow::format_err!("unexpected answer from device | {}", e))?;

    if rsp.rc != 0 {
        bail!("Error from device: rc={}", rsp.rc);
    }

    Ok(rsp)
}
