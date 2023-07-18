// Copyright © 2023 Vouch.io LLC

//! Default functions
//! Currently implemented: echo and reset.

use anyhow::{bail, Error, Result};
use log::info;
use serde_cbor;
use serde_json;

use crate::cli::*;
use crate::nmp_hdr::*;
use crate::transfer::create_request;
use crate::transfer::next_seq_id;
use crate::transfer::transceive;

pub fn echo(cli: &Cli, message: &String) -> Result<(), Error> {
    info!("echo request");

    // open serial port
    let mut interface = open_port(cli)?;

    // send request
    let mut map = std::collections::BTreeMap::new();
    map.insert("d".to_string(), message);
    let body: Vec<u8> = serde_cbor::to_vec(&map).unwrap();
    let seq_id = next_seq_id();
    let data = create_request(
        NmpOp::Write,
        NmpGroup::Default,
        NmpIdDefault::Echo as u8,
        &body,
        seq_id,
    )?;
    let data = interface.encode(&data, cli.linelength)?;
    let (response_header, response_body) = transceive(&mut *interface, data)?;

    // verify sequence id
    if response_header.seq != seq_id {
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
    let mut interface = open_port(cli)?;

    // send request
    let seq_id = next_seq_id();
    let body: Vec<u8> =
        serde_cbor::to_vec(&std::collections::BTreeMap::<String, String>::new()).unwrap();
    let data = create_request(
        NmpOp::Write,
        NmpGroup::Default,
        NmpIdDefault::Reset as u8,
        &body,
        seq_id,
    )?;
    let data = interface.encode(&data, cli.linelength)?;
    let (response_header, response_body) = transceive(&mut *interface, data)?;

    // verify sequence id
    if response_header.seq != seq_id {
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
