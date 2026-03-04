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

/// Read a settings value from the device
pub fn settings_read(specs: &SerialSpecs, name: &str, max_size: Option<u32>) -> Result<SettingsReadRsp, Error> {
    info!("read setting: {}", name);

    let mut port = open_port(specs)?;

    let req = SettingsReadReq {
        name: name.to_string(),
        max_size,
    };
    let body = serde_cbor::to_vec(&req)?;

    let (data, request_header) = encode_request(
        specs.linelength,
        NmpOp::Read,
        NmpGroup::Config,
        NmpIdConfig::Val,
        &body,
        next_seq_id(),
    )?;

    let (response_header, response_body) = transceive(&mut *port, &data)?;

    if !check_answer(&request_header, &response_header) {
        bail!("wrong answer types");
    }

    debug!("response_body: {}", serde_json::to_string_pretty(&response_body)?);

    let rsp: SettingsReadRsp = serde_cbor::value::from_value(response_body)
        .map_err(|e| anyhow::format_err!("unexpected answer from device | {}", e))?;

    if rsp.rc != 0 {
        bail!("Error from device: rc={}", rsp.rc);
    }

    Ok(rsp)
}

/// Write a settings value to the device
pub fn settings_write(specs: &SerialSpecs, name: &str, value: Vec<u8>) -> Result<(), Error> {
    info!("write setting: {} = {:?}", name, value);

    let mut port = open_port(specs)?;

    let req = SettingsWriteReq {
        name: name.to_string(),
        val: value,
    };
    let body = serde_cbor::to_vec(&req)?;

    let (data, request_header) = encode_request(
        specs.linelength,
        NmpOp::Write,
        NmpGroup::Config,
        NmpIdConfig::Val,
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

    info!("setting written successfully");
    Ok(())
}

/// Delete a settings value from the device
pub fn settings_delete(specs: &SerialSpecs, name: &str) -> Result<(), Error> {
    info!("delete setting: {}", name);

    let mut port = open_port(specs)?;

    let req = SettingsDeleteReq {
        name: name.to_string(),
    };
    let body = serde_cbor::to_vec(&req)?;

    let (data, request_header) = encode_request(
        specs.linelength,
        NmpOp::Write,  // Delete uses Write op
        NmpGroup::Config,
        NmpIdConfig::Val,
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

    info!("setting deleted successfully");
    Ok(())
}

/// Commit settings changes (save to persistent storage)
pub fn settings_commit(specs: &SerialSpecs) -> Result<(), Error> {
    info!("commit settings");

    let mut port = open_port(specs)?;

    let req = SettingsCommitReq {};
    let body = serde_cbor::to_vec(&req)?;

    let (data, request_header) = encode_request(
        specs.linelength,
        NmpOp::Write,
        NmpGroup::Config,
        NmpIdConfig::Val,
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

    info!("settings committed successfully");
    Ok(())
}

/// Load settings from persistent storage
pub fn settings_load(specs: &SerialSpecs) -> Result<(), Error> {
    info!("load settings");

    let mut port = open_port(specs)?;

    let req = SettingsLoadReq {};
    let body = serde_cbor::to_vec(&req)?;

    let (data, request_header) = encode_request(
        specs.linelength,
        NmpOp::Read,
        NmpGroup::Config,
        NmpIdConfig::Val,
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

    info!("settings loaded successfully");
    Ok(())
}

/// Save settings to persistent storage
pub fn settings_save(specs: &SerialSpecs) -> Result<(), Error> {
    info!("save settings");

    let mut port = open_port(specs)?;

    let req = SettingsSaveReq {};
    let body = serde_cbor::to_vec(&req)?;

    let (data, request_header) = encode_request(
        specs.linelength,
        NmpOp::Write,
        NmpGroup::Config,
        NmpIdConfig::Val,
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

    info!("settings saved successfully");
    Ok(())
}

// ==================== Transport-based versions ====================

/// Read a settings value using a transport
pub fn settings_read_transport(transport: &mut dyn Transport, name: &str, max_size: Option<u32>) -> Result<SettingsReadRsp, Error> {
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

/// Write a settings value using a transport
pub fn settings_write_transport(transport: &mut dyn Transport, name: &str, value: Vec<u8>) -> Result<(), Error> {
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

    // Check for rc error
    if let Some(rc) = get_rc(&response_body) {
        if rc != 0 {
            bail!("Error from device: rc={}", rc);
        }
    }

    info!("setting written successfully");
    Ok(())
}

/// Delete a settings value using a transport
pub fn settings_delete_transport(transport: &mut dyn Transport, name: &str) -> Result<(), Error> {
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

    // Check for rc error
    if let Some(rc) = get_rc(&response_body) {
        if rc != 0 {
            bail!("Error from device: rc={}", rc);
        }
    }

    info!("setting deleted successfully");
    Ok(())
}

/// Commit settings using a transport
pub fn settings_commit_transport(transport: &mut dyn Transport) -> Result<(), Error> {
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

    // Check for rc error
    if let Some(rc) = get_rc(&response_body) {
        if rc != 0 {
            bail!("Error from device: rc={}", rc);
        }
    }

    info!("settings committed successfully");
    Ok(())
}

/// Load settings using a transport
pub fn settings_load_transport(transport: &mut dyn Transport) -> Result<(), Error> {
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

    // Check for rc error
    if let Some(rc) = get_rc(&response_body) {
        if rc != 0 {
            bail!("Error from device: rc={}", rc);
        }
    }

    info!("settings loaded successfully");
    Ok(())
}

/// Save settings using a transport
pub fn settings_save_transport(transport: &mut dyn Transport) -> Result<(), Error> {
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

    // Check for rc error
    if let Some(rc) = get_rc(&response_body) {
        if rc != 0 {
            bail!("Error from device: rc={}", rc);
        }
    }

    info!("settings saved successfully");
    Ok(())
}
