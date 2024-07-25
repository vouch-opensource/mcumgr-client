// Copyright © 2023-2024 Vouch.io LLC

use anyhow::{bail, Context, Error, Result};
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
use std::time::Duration;

use crate::nmp_hdr::*;
use crate::test_serial_port::TestSerialPort;

pub struct SerialSpecs {
    pub device: String,
    pub initial_timeout_s: u32,
    pub subsequent_timeout_ms: u32,
    pub nb_retry: u32,
    pub linelength: usize,
    pub mtu: usize,
    pub baudrate: u32
}

fn read_byte(port: &mut dyn SerialPort) -> Result<u8, Error> {
    let mut byte = [0u8];
    port.read(&mut byte)?;
    Ok(byte[0])
}

fn expect_byte(port: &mut dyn SerialPort, b: u8) -> Result<(), Error> {
    let read = read_byte(port)?;
    if read != b {
        bail!("read error, expected: {}, read: {}", b, read);
    }
    Ok(())
}

pub fn open_port(specs: &SerialSpecs) -> Result<Box<dyn SerialPort>, Error> {
    if specs.device.to_lowercase() == "test" {
        Ok(Box::new(TestSerialPort::new()))
    } else {
        serialport::new(&specs.device, specs.baudrate)
            .timeout(Duration::from_secs(specs.initial_timeout_s as u64))
            .open()
            .with_context(|| format!("failed to open serial port {}", &specs.device))
    }
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
    id: impl NmpId,
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
    port: &mut dyn SerialPort,
    data: &Vec<u8>,
) -> Result<(NmpHdr, serde_cbor::Value), Error> {
    // empty input buffer
    let to_read = port.bytes_to_read()?;
    for _ in 0..to_read {
        read_byte(&mut *port)?;
    }

    // write request
    port.write_all(data)?;

    // read result
    let mut bytes_read = 0;
    let mut expected_len = 0;
    let mut result: Vec<u8> = Vec::new();
    loop {
        // first wait for the chunk start marker
        if bytes_read == 0 {
            expect_byte(&mut *port, 6)?;
            expect_byte(&mut *port, 9)?;
        } else {
            expect_byte(&mut *port, 4)?;
            expect_byte(&mut *port, 20)?;
        }

        // next read until newline
        loop {
            let b = read_byte(&mut *port)?;
            if b == 0xa {
                break;
            } else {
                result.push(b);
                bytes_read += 1;
            }
        }

        // try to extract length
        let decoded: Vec<u8> = general_purpose::STANDARD.decode(&result)?;
        if expected_len == 0 {
            let len = BigEndian::read_u16(&decoded);
            if len > 0 {
                expected_len = len as usize;
            }
            debug!("expected length: {}", expected_len);
        }

        // stop when done
        if (decoded.len() - 2) >= expected_len {
            break;
        }
    }

    // decode base64
    debug!("result string: {}", String::from_utf8(result.clone())?);
    let decoded: Vec<u8> = general_purpose::STANDARD.decode(&result)?;

    // verify length: must be the decoded length, minus the 2 bytes to encode the length
    let len = BigEndian::read_u16(&decoded) as usize;
    if len != decoded.len() - 2 {
        bail!("wrong chunk length");
    }

    // verify checksum
    let data = decoded[2..decoded.len() - 2].to_vec();
    let read_checksum = BigEndian::read_u16(&decoded[decoded.len() - 2..]);
    let calculated_checksum = State::<XMODEM>::calculate(&data);
    if read_checksum != calculated_checksum {
        bail!("wrong checksum");
    }

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
