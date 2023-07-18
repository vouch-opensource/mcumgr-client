// Copyright © 2023 Vouch.io LLC

use anyhow::bail;
use anyhow::Result;
use base64::{engine::general_purpose, Engine as _};
use byteorder::{BigEndian, ByteOrder};
use crc16::*;
use log::debug;
use serialport::SerialPort;
use std::io::Read;
use std::io::Write;

use crate::interface::Interface;

fn expect_byte(interface: &mut dyn Interface, b: u8) -> Result<(), anyhow::Error> {
    let read = interface.read_byte()?;
    if read != b {
        bail!("read error, expected: {}, read: {}", b, read);
    }
    Ok(())
}

pub fn serial_port_read_and_decode(interface: &mut dyn Interface) -> Result<Vec<u8>> {
    // read result
    let mut bytes_read = 0;
    let mut expected_len = 0;
    let mut result: Vec<u8> = Vec::new();
    loop {
        // first wait for the chunk start marker
        if bytes_read == 0 {
            expect_byte(&mut *interface, 6)?;
            expect_byte(&mut *interface, 9)?;
        } else {
            expect_byte(&mut *interface, 4)?;
            expect_byte(&mut *interface, 20)?;
        }

        // next read until newline
        loop {
            let b = interface.read_byte()?;
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

pub struct SerialPortInterface {
    serial_port: Box<dyn SerialPort>,
}

impl SerialPortInterface {
    pub fn new(serial_port: Box<dyn SerialPort>) -> Self {
        SerialPortInterface { serial_port }
    }
}

impl Interface for SerialPortInterface {
    fn bytes_to_read(&self) -> Result<u32, serialport::Error> {
        self.serial_port.bytes_to_read()
    }

    fn read_byte(self: &mut SerialPortInterface) -> Result<u8, serialport::Error> {
        let mut byte = [0u8];
        self.serial_port.read(&mut byte)?;
        Ok(byte[0])
    }

    fn write_all(&mut self, buf: &[u8]) -> Result<(), std::io::Error> {
        self.serial_port.write_all(buf)
    }

    fn read_and_decode(&mut self) -> Result<Vec<u8>> {
        let data = serial_port_read_and_decode(self)?;

        Ok(data)
    }

    fn encode(&mut self, buf: &[u8]) -> std::result::Result<Vec<u8>, anyhow::Error> {
        todo!()
    }
}
