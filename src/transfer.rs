// Copyright © 2023 Vouch.io LLC

use anyhow::{Error, Result};
use hex;
use lazy_static::lazy_static;
use log::debug;
use rand::{thread_rng, Rng};
use serde_cbor;
use std::io::Cursor;
use std::sync::atomic::{AtomicU8, Ordering};

use crate::interface::Interface;
use crate::nmp_hdr::*;

// thread-safe counter, initialized with a random value on first call
pub fn next_seq_id() -> u8 {
    lazy_static! {
        static ref COUNTER: AtomicU8 = AtomicU8::new(thread_rng().gen::<u8>());
    }
    COUNTER.fetch_add(1, Ordering::SeqCst)
}

pub fn create_request(
    op: NmpOp,
    group: NmpGroup,
    id: u8,
    body: &Vec<u8>,
    seq_id: u8,
) -> Result<Vec<u8>, Error> {
    let mut request_header = NmpHdr::new_req(op, group, id);
    request_header.seq = seq_id;
    request_header.len = body.len() as u16;
    debug!("request header: {:?}", request_header);
    let mut serialized = request_header.serialize()?;
    serialized.extend(body);
    debug!("serialized: {}", hex::encode(&serialized));
    Ok(serialized)
}

pub fn transceive(
    interface: &mut dyn Interface,
    data: Vec<u8>,
) -> Result<(NmpHdr, serde_cbor::Value), Error> {
    // empty input buffer
    let to_read = interface.bytes_to_read()?;
    for _ in 0..to_read {
        interface.read_byte()?;
    }

    // write request
    interface.write_all(&data)?;

    let data = interface.read_and_decode()?;

    // read header
    let mut cursor = Cursor::new(&data);
    let response_header = NmpHdr::deserialize(&mut cursor).unwrap();
    debug!("response header: {:?}", response_header);

    debug!("cbor: {}", hex::encode(&data[8..]));

    // decode body in CBOR format
    let body = serde_cbor::from_reader(cursor)?;

    Ok((response_header, body))
}

#[cfg(test)]
mod tests {
    use super::next_seq_id;
    use std::collections::HashSet;

    #[test]
    fn test_next_seq_id() {
        let mut ids = HashSet::new();
        let initial_id = next_seq_id();
        ids.insert(initial_id);

        for _ in 0..std::u8::MAX {
            let id = next_seq_id();
            assert!(ids.insert(id), "Duplicate ID: {}", id);
        }

        // Check wrapping behavior
        let wrapped_id = next_seq_id();
        assert_eq!(
            wrapped_id, initial_id,
            "Wrapped ID does not match initial ID"
        );
    }
}
