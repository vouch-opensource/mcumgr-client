// Copyright © 2023 Vouch.io LLC

use anyhow::{Error, Result};
use base64::{engine::general_purpose, Engine as _};
use byteorder::{BigEndian, WriteBytesExt};
use crc16::*;
use hex;
use lazy_static::lazy_static;
use log::debug;
use rand::{thread_rng, Rng};
use serde_cbor;
use std::cmp::min;
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

pub fn encode_request(
    linelength: usize,
    op: NmpOp,
    group: NmpGroup,
    id: u8,
    body: &Vec<u8>,
    seq_id: u8,
) -> Result<Vec<u8>, Error> {
    // create request
    let mut request_header = NmpHdr::new_req(op, group, id);
    request_header.seq = seq_id;
    request_header.len = body.len() as u16;
    debug!("request header: {:?}", request_header);
    let mut serialized = request_header.serialize()?;
    serialized.extend(body);
    debug!("serialized: {}", hex::encode(&serialized));

    // calculate CRC16 of it and append to the request
    let checksum = State::<XMODEM>::calculate(&serialized);
    serialized.write_u16::<BigEndian>(checksum)?;

    // prepend chunk length
    let mut len: Vec<u8> = Vec::new();
    len.write_u16::<BigEndian>(serialized.len() as u16)?;
    serialized.splice(0..0, len);
    debug!(
        "encoded with packet length and checksum: {}",
        hex::encode(&serialized)
    );

    // convert to base64
    let base64_data: Vec<u8> = general_purpose::STANDARD.encode(&serialized).into_bytes();
    debug!("encoded: {}", String::from_utf8(base64_data.clone())?);
    let mut data = Vec::<u8>::new();

    // transfer in blocks of max linelength bytes per line
    let mut written = 0;
    let totlen = base64_data.len();
    while written < totlen {
        // start designator
        if written == 0 {
            data.extend_from_slice(&[6, 9]);
        } else {
            // TODO: add a configurable sleep for slower devices
            // thread::sleep(Duration::from_millis(20));
            data.extend_from_slice(&[4, 20]);
        }
        let write_len = min(linelength - 4, totlen - written);
        data.extend_from_slice(&base64_data[written..written + write_len]);
        data.push(b'\n');
        written += write_len;
    }

    Ok(data)
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
