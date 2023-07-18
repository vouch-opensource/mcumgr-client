// Copyright © 2023 Vouch.io LLC

use serialport::Error;

pub trait Interface {
    fn bytes_to_read(&self) -> Result<u32, Error>;

    fn read_byte(&mut self) -> Result<u8, Error>;

    fn write_all(&mut self, buf: &[u8]) -> Result<(), std::io::Error>;
}
