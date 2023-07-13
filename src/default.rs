// Copyright © 2023 Vouch.io LLC

//! Default functions
//! Currently implemented: echo and reset.

use anyhow::{bail, Error, Result};
use log::info;
use serde_cbor;
use serde_json;

use crate::cli::*;
use crate::nmp_hdr::*;
use crate::transfer::encode_request;
use crate::transfer::next_seq_id;
use crate::transfer::transceive;

pub fn echo(cli: &Cli, message: &String) -> Result<(), Error> {
    info!("echo request");

    // open serial port
    let mut port = open_port(cli)?;

    // send request
    let mut map = std::collections::BTreeMap::new();
    map.insert("d".to_string(), message);
    let body: Vec<u8> = serde_cbor::to_vec(&map).unwrap();
    let (data, request_header) = encode_request(
        cli.linelength,
        NmpOp::Write,
        NmpGroup::Default,
        NmpIdDefault::Echo as u8,
        &body,
        next_seq_id(),
    )?;
    let (response_header, response_body) = transceive(&mut *port, data)?;

    // verify sequence id
    if response_header.seq != request_header.seq {
        bail!("wrong sequence number");
    }

    // verify response
    if response_header.op != NmpOp::ReadRsp || response_header.group != NmpGroup::Default {
        bail!("wrong response types");
    }

    // print body
    info!(
        "response: {}",
        serde_json::to_string_pretty(&response_body)?
    );

    Ok(())
}

pub fn reset(cli: &Cli) -> Result<(), Error> {
    info!("send reset request");

    // open serial port
    let mut port = open_port(cli)?;

    // send request
    let body: Vec<u8> =
        serde_cbor::to_vec(&std::collections::BTreeMap::<String, String>::new()).unwrap();
    let (data, request_header) = encode_request(
        cli.linelength,
        NmpOp::Write,
        NmpGroup::Default,
        NmpIdDefault::Reset as u8,
        &body,
        next_seq_id(),
    )?;
    let (response_header, response_body) = transceive(&mut *port, data)?;

    // verify sequence id
    if response_header.seq != request_header.seq {
        bail!("wrong sequence number");
    }

    // verify response
    if response_header.op != NmpOp::WriteRsp || response_header.group != NmpGroup::Default {
        bail!("wrong response types");
    }

    // print body
    info!(
        "response: {}",
        serde_json::to_string_pretty(&response_body)?
    );

    Ok(())
}
