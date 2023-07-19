// Copyright © 2023 Vouch.io LLC

use serialport::Error;
use async_trait::async_trait;

#[async_trait]
pub trait Interface: Send {
    fn bytes_to_read(&self) -> Result<u32, Error>;

    async fn read_byte(&mut self) -> Result<u8, Error>;

    async fn write_all(&mut self, buf: &[u8]) -> Result<(), anyhow::Error>;

    async fn read_and_decode(&mut self) -> Result<Vec<u8>, anyhow::Error>;

    fn encode(&mut self, buf: &[u8], linelength: usize) -> Result<Vec<u8>, anyhow::Error>;
}
