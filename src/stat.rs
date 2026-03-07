// Copyright © 2026 Rudis Laboratories LLC, 2026 VeeMax BV

use anyhow::{bail, Error, Result};
use log::{debug, info};

use crate::nmp_hdr::*;
use crate::transfer::Transport;
use crate::util::empty_cbor_body;

/// List available statistics groups on the device
pub fn stat_list(transport: &mut dyn Transport) -> Result<StatListRsp, Error> {
    info!("send stat list request");

    let body = empty_cbor_body();

    let (_response_header, response_body) = transport.transceive(
        NmpOp::Read,
        NmpGroup::STAT,
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

/// Read statistics from a specific group
pub fn stat_read(transport: &mut dyn Transport, name: &str) -> Result<StatReadRsp, Error> {
    info!("send stat read request: {}", name);

    let req = StatReadReq {
        name: name.to_string(),
    };
    let body = serde_cbor::to_vec(&req)?;

    let (_response_header, response_body) = transport.transceive(
        NmpOp::Read,
        NmpGroup::STAT,
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
