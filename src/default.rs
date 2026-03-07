// Copyright © 2023-2024 Vouch.io LLC, 2026 Rudis Laboratories LLC, 2026 VeeMax BV

use anyhow::{Error, Result};
use log::debug;
use log::info;

use crate::nmp_hdr::*;
use crate::transfer::Transport;
use crate::util::check_rc;

/// Reset the device
pub fn reset(transport: &mut dyn Transport) -> Result<(), Error> {
    info!("send reset request");

    let body = Vec::new();
    let (_response_header, response_body) = transport.transceive(
        NmpOp::Write,
        NmpGroup::DEFAULT,
        NmpIdDef::Reset.to_u8(),
        &body,
    )?;

    // verify result code
    debug!(
        "response_body: {}",
        serde_json::to_string_pretty(&response_body)?
    );
    check_rc(&response_body)?;
    info!("reset complete");

    Ok(())
}
