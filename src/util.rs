// Copyright © 2026 Rudis Laboratories LLC, 2026 VeeMax BV

use anyhow::{bail, Result};

/// Extract the "rc" (return code) field from a CBOR response map.
fn get_rc(response_body: &serde_cbor::Value) -> Option<i64> {
    if let serde_cbor::Value::Map(object) = response_body {
        for (key, val) in object.iter() {
            if let serde_cbor::Value::Text(rc_key) = key {
                if rc_key == "rc" {
                    if let serde_cbor::Value::Integer(rc) = val {
                        return Some(*rc as i64);
                    }
                }
            }
        }
    }
    None
}

/// Check the "rc" field in a CBOR response and bail if non-zero.
pub fn check_rc(response_body: &serde_cbor::Value) -> Result<()> {
    if let Some(rc) = get_rc(response_body) {
        if rc != 0 {
            bail!("device returned error: rc={}", rc);
        }
    }
    Ok(())
}

/// Create an empty CBOR map body for requests with no parameters.
pub fn empty_cbor_body() -> Vec<u8> {
    serde_cbor::to_vec(&std::collections::BTreeMap::<String, String>::new()).unwrap()
}
