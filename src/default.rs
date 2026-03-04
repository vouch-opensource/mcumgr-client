// Copyright © 2023-2024 Vouch.io LLC, 2026 Rudis Laboratories LLC

use anyhow::{bail, Error, Result};
use log::debug;
use log::info;

use crate::nmp_hdr::*;
use crate::transfer::encode_request;
use crate::transfer::next_seq_id;
use crate::transfer::open_port;
use crate::transfer::transceive;
use crate::transfer::SerialSpecs;
use crate::transfer::Transport;

pub fn reset(specs: &SerialSpecs) -> Result<(), Error> {
    info!("send reset request");

    // open serial port
    let mut port = open_port(specs)?;

    // send request
    let body = Vec::new();
    let (data, request_header) = encode_request(
        specs.linelength,
        NmpOp::Write,
        NmpGroup::Default,
        NmpIdDef::Reset,
        &body,
        next_seq_id(),
    )?;
    let (response_header, response_body) = transceive(&mut *port, &data)?;
    
    // verify sequence id
    if response_header.seq != request_header.seq {
        bail!("wrong sequence number");
    }

    // verify response
    if response_header.op != NmpOp::WriteRsp || response_header.group != NmpGroup::Default {
        bail!("wrong response types");
    }

    // verify result code
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
                        } else {
                            info!("reset complete");
                        }
                    }
                }
                _ => (),
            }
        }
    }

    Ok(())
}

// ==================== Transport-based versions ====================

/// Reset the device using a transport
pub fn reset_transport(transport: &mut dyn Transport) -> Result<(), Error> {
    info!("send reset request");

    let body = Vec::new();
    let (_response_header, response_body) = transport.transceive(
        NmpOp::Write,
        NmpGroup::Default,
        NmpIdDef::Reset.to_u8(),
        &body,
    )?;

    // verify result code
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
                        } else {
                            info!("reset complete");
                        }
                    }
                }
                _ => (),
            }
        }
    }

    Ok(())
}
