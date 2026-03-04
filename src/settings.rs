// Copyright © 2026 Rudis Laboratories LLC, 2026 VeeMax BV

use anyhow::{bail, Error, Result};
use log::{debug, info};

use crate::nmp_hdr::*;
use crate::transfer::Transport;
use crate::util::check_rc;

/// Read a settings value from the device
pub fn settings_read(transport: &mut dyn Transport, name: &str, max_size: Option<u32>) -> Result<SettingsReadRsp, Error> {
    info!("read setting: {}", name);

    let req = SettingsReadReq {
        name: name.to_string(),
        max_size,
    };
    let body = serde_cbor::to_vec(&req)?;

    let (_response_header, response_body) = transport.transceive(
        NmpOp::Read,
        NmpGroup::Config,
        NmpIdConfig::Val.to_u8(),
        &body,
    )?;

    debug!("response_body: {}", serde_json::to_string_pretty(&response_body)?);

    let rsp: SettingsReadRsp = serde_cbor::value::from_value(response_body)
        .map_err(|e| anyhow::format_err!("unexpected answer from device | {}", e))?;

    if rsp.rc != 0 {
        bail!("Error from device: rc={}", rsp.rc);
    }

    Ok(rsp)
}

/// Write a settings value to the device
pub fn settings_write(transport: &mut dyn Transport, name: &str, value: Vec<u8>) -> Result<(), Error> {
    info!("write setting: {} = {:?}", name, value);

    let req = SettingsWriteReq {
        name: name.to_string(),
        val: value,
    };
    let body = serde_cbor::to_vec(&req)?;

    let (_response_header, response_body) = transport.transceive(
        NmpOp::Write,
        NmpGroup::Config,
        NmpIdConfig::Val.to_u8(),
        &body,
    )?;

    debug!("response_body: {}", serde_json::to_string_pretty(&response_body)?);

    check_rc(&response_body)?;

    info!("setting written successfully");
    Ok(())
}

/// Delete a settings value from the device
pub fn settings_delete(transport: &mut dyn Transport, name: &str) -> Result<(), Error> {
    info!("delete setting: {}", name);

    let req = SettingsDeleteReq {
        name: name.to_string(),
    };
    let body = serde_cbor::to_vec(&req)?;

    let (_response_header, response_body) = transport.transceive(
        NmpOp::Write,
        NmpGroup::Config,
        NmpIdConfig::Val.to_u8(),
        &body,
    )?;

    debug!("response_body: {}", serde_json::to_string_pretty(&response_body)?);

    check_rc(&response_body)?;

    info!("setting deleted successfully");
    Ok(())
}

/// Commit settings changes (save to persistent storage)
pub fn settings_commit(transport: &mut dyn Transport) -> Result<(), Error> {
    info!("commit settings");

    let req = SettingsCommitReq {};
    let body = serde_cbor::to_vec(&req)?;

    let (_response_header, response_body) = transport.transceive(
        NmpOp::Write,
        NmpGroup::Config,
        NmpIdConfig::Val.to_u8(),
        &body,
    )?;

    debug!("response_body: {}", serde_json::to_string_pretty(&response_body)?);

    check_rc(&response_body)?;

    info!("settings committed successfully");
    Ok(())
}

/// Load settings from persistent storage
pub fn settings_load(transport: &mut dyn Transport) -> Result<(), Error> {
    info!("load settings");

    let req = SettingsLoadReq {};
    let body = serde_cbor::to_vec(&req)?;

    let (_response_header, response_body) = transport.transceive(
        NmpOp::Read,
        NmpGroup::Config,
        NmpIdConfig::Val.to_u8(),
        &body,
    )?;

    debug!("response_body: {}", serde_json::to_string_pretty(&response_body)?);

    check_rc(&response_body)?;

    info!("settings loaded successfully");
    Ok(())
}

/// Save settings to persistent storage
pub fn settings_save(transport: &mut dyn Transport) -> Result<(), Error> {
    info!("save settings");

    let req = SettingsSaveReq {};
    let body = serde_cbor::to_vec(&req)?;

    let (_response_header, response_body) = transport.transceive(
        NmpOp::Write,
        NmpGroup::Config,
        NmpIdConfig::Val.to_u8(),
        &body,
    )?;

    debug!("response_body: {}", serde_json::to_string_pretty(&response_body)?);

    check_rc(&response_body)?;

    info!("settings saved successfully");
    Ok(())
}
