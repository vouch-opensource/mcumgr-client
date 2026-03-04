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

/// Execute a shell command on the device
///
/// The command is passed as a vector of strings (argv style).
/// Returns the output and return code from the device.
pub fn shell_exec(specs: &SerialSpecs, argv: Vec<String>) -> Result<ShellExecRsp, Error> {
    info!("send shell exec request: {:?}", argv);

    let mut port = open_port(specs)?;

    let req = ShellExecReq { argv };
    let body = serde_cbor::to_vec(&req)?;

    let (data, request_header) = encode_request(
        specs.linelength,
        NmpOp::Write,
        NmpGroup::Shell,
        NmpIdShell::Exec,
        &body,
        next_seq_id(),
    )?;

    let (response_header, response_body) = transceive(&mut *port, &data)?;

    if !check_answer(&request_header, &response_header) {
        bail!("wrong answer types");
    }

    debug!("response_body: {}", serde_json::to_string_pretty(&response_body)?);

    let rsp: ShellExecRsp = serde_cbor::value::from_value(response_body)
        .map_err(|e| anyhow::format_err!("unexpected answer from device | {}", e))?;

    Ok(rsp)
}

// ==================== Transport-based versions ====================

/// Execute a shell command using a transport
pub fn shell_exec_transport(transport: &mut dyn Transport, argv: Vec<String>) -> Result<ShellExecRsp, Error> {
    info!("send shell exec request: {:?}", argv);

    let req = ShellExecReq { argv };
    let body = serde_cbor::to_vec(&req)?;

    let (_response_header, response_body) = transport.transceive(
        NmpOp::Write,
        NmpGroup::Shell,
        NmpIdShell::Exec.to_u8(),
        &body,
    )?;

    debug!("response_body: {}", serde_json::to_string_pretty(&response_body)?);

    let rsp: ShellExecRsp = serde_cbor::value::from_value(response_body)
        .map_err(|e| anyhow::format_err!("unexpected answer from device | {}", e))?;

    Ok(rsp)
}
