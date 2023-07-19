// Copyright © 2023 Vouch.io LLC

use anyhow::bail;
use anyhow::Result;
use base64::{engine::general_purpose, Engine as _};
use byteorder::WriteBytesExt;
use byteorder::{BigEndian, ByteOrder};
use crc16::*;
use log::debug;
use serialport::SerialPort;
use std::cmp::min;
use std::io::Read;
use std::io::Write;
use async_trait::async_trait;

use crate::interface::Interface;

async fn expect_byte(interface: &mut dyn Interface, b: u8) -> Result<(), anyhow::Error> {
    let read = interface.read_byte().await?;
    if read != b {
        bail!("read error, expected: {}, read: {}", b, read);
    }
    Ok(())
}

pub async fn serial_port_read_and_decode(interface: &mut dyn Interface) -> Result<Vec<u8>> {
    // read result
    let mut bytes_read = 0;
    let mut expected_len = 0;
    let mut result: Vec<u8> = Vec::new();
    loop {
        // first wait for the chunk start marker
        if bytes_read == 0 {
            expect_byte(&mut *interface, 6).await?;
            expect_byte(&mut *interface, 9).await?;
        } else {
            expect_byte(&mut *interface, 4).await?;
            expect_byte(&mut *interface, 20).await?;
        }

        // next read until newline
        loop {
            let b = interface.read_byte().await?;
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
        if decoded.len() >= expected_len {
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

    Ok(data)
}

pub fn serial_port_encode(data: &[u8], linelength: usize) -> Result<Vec<u8>> {
    // calculate CRC16 of it and append to the request
    let mut serialized = data.to_vec();
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

pub struct SerialPortInterface {
    serial_port: Box<dyn SerialPort>,
}

impl SerialPortInterface {
    pub fn new(serial_port: Box<dyn SerialPort>) -> Self {
        SerialPortInterface { serial_port }
    }
}

#[async_trait]
impl Interface for SerialPortInterface {
    fn bytes_to_read(&self) -> Result<u32, serialport::Error> {
        self.serial_port.bytes_to_read()
    }

    async fn read_byte(self: &mut SerialPortInterface) -> Result<u8, serialport::Error> {
        let mut byte = [0u8];
        self.serial_port.read(&mut byte)?;
        Ok(byte[0])
    }

    async fn write_all(&mut self, buf: &[u8]) -> Result<(), anyhow::Error> {
        self.serial_port.write_all(buf).map_err(anyhow::Error::from)
    }

    async fn read_and_decode(&mut self) -> Result<Vec<u8>> {
        serial_port_read_and_decode(&mut *self).await
    }

    fn encode(
        &mut self,
        buf: &[u8],
        linelength: usize,
    ) -> std::result::Result<Vec<u8>, anyhow::Error> {
        serial_port_encode(buf, linelength)
    }
}
