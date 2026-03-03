// Copyright © 2026 Rudis Laboratories LLC

use log::debug;

use crate::nmp_hdr::{NmpHdr, NmpOp};

/// Verify that a response header matches the expected request header.
pub fn check_answer(request_header: &NmpHdr, response_header: &NmpHdr) -> bool {
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

/// Extract the "rc" (return code) field from a CBOR response map.
pub fn get_rc(response_body: &serde_cbor::Value) -> Option<i64> {
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
