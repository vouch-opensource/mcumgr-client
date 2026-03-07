// Copyright © 2026 Rudis Laboratories LLC, 2026 VeeMax BV

use anyhow::{Error, Result};
use log::{debug, info};

use crate::nmp_hdr::*;
use crate::transfer::Transport;

/// Execute a shell command on the device
///
/// The command is passed as a vector of strings (argv style).
/// Returns the output and return code from the device.
pub fn shell_exec(transport: &mut dyn Transport, argv: Vec<String>) -> Result<ShellExecRsp, Error> {
    info!("send shell exec request: {:?}", argv);

    let req = ShellExecReq { argv };
    let body = serde_cbor::to_vec(&req)?;

    let (_response_header, response_body) = transport.transceive(
        NmpOp::Write,
        NmpGroup::SHELL,
        NmpIdShell::Exec.to_u8(),
        &body,
    )?;

    debug!("response_body: {}", serde_json::to_string_pretty(&response_body)?);

    let rsp: ShellExecRsp = serde_cbor::value::from_value(response_body)
        .map_err(|e| anyhow::format_err!("unexpected answer from device | {}", e))?;

    Ok(rsp)
}
