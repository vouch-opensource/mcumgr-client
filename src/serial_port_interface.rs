// Copyright © 2023 Vouch.io LLC

use anyhow::Result;
use serialport::Error;
use serialport::SerialPort;
use std::io::Read;
use std::io::Write;

use crate::interface::Interface;

pub struct SerialPortInterface {
    serial_port: Box<dyn SerialPort>,
}

impl SerialPortInterface {
    pub fn new(serial_port: Box<dyn SerialPort>) -> Self {
        SerialPortInterface { serial_port }
    }
}

impl Interface for SerialPortInterface {
    fn bytes_to_read(&self) -> Result<u32, Error> {
        self.serial_port.bytes_to_read()
    }

    fn read_byte(self: &mut SerialPortInterface) -> Result<u8, Error> {
        let mut byte = [0u8];
        self.serial_port.read(&mut byte)?;
        Ok(byte[0])
    }

    fn write_all(&mut self, buf: &[u8]) -> Result<(), std::io::Error> {
        self.serial_port.write_all(buf)
    }
}
