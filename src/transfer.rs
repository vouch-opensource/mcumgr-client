// Copyright © 2023 Vouch.io LLC

use anyhow::{bail, Error, Result};
use base64::{engine::general_purpose, Engine as _};
use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use crc16::*;
use hex;
use lazy_static::lazy_static;
use log::debug;
use rand::{thread_rng, Rng};
use serde_cbor;
use serialport::SerialPort;
use std::cmp::min;
use std::io::Cursor;
use std::sync::atomic::{AtomicU8, Ordering};

use crate::nmp_hdr::*;

fn read_byte(port: &mut dyn SerialPort) -> Result<u8, Error> {
    let mut byte = [0u8];
    port.read(&mut byte)?;
    Ok(byte[0])
}

fn read_until_newline(port: &mut dyn SerialPort) -> Result<Vec<u8>, Error> {
    let mut result: Vec<u8> = Vec::new();
    loop {
        let b = read_byte(&mut *port)?;
        if b == 0xa {
            break;
        } else {
            result.push(b);
        }
    }
    Ok(result)
}

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
    id: NmpIdImage,
    body: &Vec<u8>,
    seq_id: u8,
) -> Result<(Vec<u8>, NmpHdr), Error> {
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

    Ok((data, request_header))
}

pub fn transceive(
    //port: &mut Box<dyn SerialPort>,
    port: &mut dyn SerialPort,
    data: Vec<u8>,
) -> Result<(NmpHdr, serde_cbor::Value), Error> {
    // empty input buffer
    let to_read = port.bytes_to_read()?;
    for _ in 0..to_read {
        read_byte(&mut *port)?;
    }

    // write request
    port.write_all(&data)?;

    // next read until newline
    let mut len: usize = 0;
    let mut line: Vec<u8>;
    let mut result: Vec<u8> = Vec::new();
    loop {
        line = read_until_newline(&mut *port)?;

        // Remove '\r'
        loop {
            if line.len() > 1 && line[0] == 0xd {
                line = line[1..].to_vec();
            } else {
                break;
            }
        }

        debug!("result string: {}....", String::from_utf8(line.clone())?);

        // Skip data if the two first bytes is neither (hex) [04 14] or [06 09]
        if line.len() < 2 || ((line[0] != 0x4 || line[1] != 0x14) && (line[0] != 0x6 || line[1] != 0x9)) {
                continue;
        }

        // Decode base64
        let data_to_decode: Vec<u8> = line[2..].to_vec();
        let mut decoded: Vec<u8> = general_purpose::STANDARD.decode(&data_to_decode)?;

        // Get length from the first message, starting with bytes 0x6 and 0x9
        if line[0] == 0x6 && line[1] == 0x9 {
            if decoded.len() < 2 {
                continue
            }
            len = BigEndian::read_u16(&decoded) as usize;

            decoded = decoded[2..].to_vec();
        }

        result.append(&mut decoded);

        // Verify checksum when all data is received
        if result.len() >= len {
            let read_checksum = BigEndian::read_u16(&result[result.len() - 2..]);
            let calculated_checksum = State::<XMODEM>::calculate(&result[..result.len() - 2].to_vec());
            if read_checksum != calculated_checksum {
                bail!("wrong checksum");
            }
            // Trim away CRC bytes
            result = result[..result.len() - 2].to_vec();
            break;
        }
    }

    // read header
    let mut cursor = Cursor::new(&result);
    let response_header = NmpHdr::deserialize(&mut cursor).unwrap();
    debug!("response header: {:?}", response_header);

    debug!("cbor: {}", hex::encode(&result[8..]));

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
