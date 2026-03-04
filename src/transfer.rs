// Copyright © 2023-2024 Vouch.io LLC, 2026 Rudis Laboratories LLC

use anyhow::{bail, Context, Error, Result};
use base64::{engine::general_purpose, Engine as _};
use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use crc16::*;
use lazy_static::lazy_static;
use log::debug;
use rand::{thread_rng, Rng};
use serialport::SerialPort;
use std::cmp::min;
use std::io::Cursor;
use std::net::{SocketAddr, ToSocketAddrs, UdpSocket};
use std::sync::atomic::{AtomicU8, Ordering};
use std::time::Duration;

use crate::nmp_hdr::*;
use crate::test_serial_port::TestSerialPort;

/// Trait for SMP transport implementations
pub trait Transport {
    /// Send an SMP request and receive a response
    fn transceive(
        &mut self,
        op: NmpOp,
        group: NmpGroup,
        id: u8,
        body: &[u8],
    ) -> Result<(NmpHdr, serde_cbor::Value), Error>;

    /// Set the timeout for subsequent operations
    fn set_timeout(&mut self, timeout_ms: u32) -> Result<(), Error>;

    /// Get the MTU for this transport
    fn mtu(&self) -> usize;

    /// Get the line length for this transport (for serial framing)
    fn linelength(&self) -> usize;
}

/// Connection specification - either serial or UDP
#[derive(Debug, Clone)]
pub enum ConnSpec {
    Serial(SerialSpecs),
    Udp(UdpSpecs),
}

impl ConnSpec {
    /// Check if this is a UDP connection
    pub fn is_udp(&self) -> bool {
        matches!(self, ConnSpec::Udp(_))
    }

    /// Check if this is a serial connection
    pub fn is_serial(&self) -> bool {
        matches!(self, ConnSpec::Serial(_))
    }

    /// Open a transport connection based on this spec
    pub fn open(&self) -> Result<Box<dyn Transport>, Error> {
        match self {
            ConnSpec::Serial(specs) => {
                let transport = SerialTransport::new(specs)?;
                Ok(Box::new(transport))
            }
            ConnSpec::Udp(specs) => {
                let transport = UdpTransport::new(specs)?;
                Ok(Box::new(transport))
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct SerialSpecs {
    pub device: String,
    pub initial_timeout_s: u32,
    pub subsequent_timeout_ms: u32,
    pub nb_retry: u32,
    pub linelength: usize,
    pub mtu: usize,
    pub baudrate: u32,
}

/// UDP connection specification
#[derive(Debug, Clone)]
pub struct UdpSpecs {
    pub host: String,
    pub port: u16,
    pub timeout_s: u32,
    pub mtu: usize,
}

impl Default for UdpSpecs {
    fn default() -> Self {
        UdpSpecs {
            host: String::new(),
            port: 1337,
            timeout_s: 5,
            mtu: 1024,
        }
    }
}

/// Serial transport wrapper that implements Transport trait
pub struct SerialTransport {
    port: Box<dyn SerialPort>,
    specs: SerialSpecs,
}

impl SerialTransport {
    pub fn new(specs: &SerialSpecs) -> Result<Self, Error> {
        let port = open_port(specs)?;
        Ok(SerialTransport {
            port,
            specs: SerialSpecs {
                device: specs.device.clone(),
                initial_timeout_s: specs.initial_timeout_s,
                subsequent_timeout_ms: specs.subsequent_timeout_ms,
                nb_retry: specs.nb_retry,
                linelength: specs.linelength,
                mtu: specs.mtu,
                baudrate: specs.baudrate,
            },
        })
    }
}

impl Transport for SerialTransport {
    fn transceive(
        &mut self,
        op: NmpOp,
        group: NmpGroup,
        id: u8,
        body: &[u8],
    ) -> Result<(NmpHdr, serde_cbor::Value), Error> {
        let seq_id = next_seq_id();
        let body_vec = body.to_vec();

        // Create a temporary NmpId wrapper
        struct TempId(u8);
        impl NmpId for TempId {
            fn to_u8(&self) -> u8 {
                self.0
            }
        }

        let (data, request_header) = encode_request(
            self.specs.linelength,
            op,
            group,
            TempId(id),
            &body_vec,
            seq_id,
        )?;

        let (response_header, response_body) = transceive(&mut *self.port, &data)?;

        // Verify sequence id
        if response_header.seq != request_header.seq {
            bail!("wrong sequence number");
        }

        // Verify response type
        let expected_op_type = match request_header.op {
            NmpOp::Read => NmpOp::ReadRsp,
            NmpOp::Write => NmpOp::WriteRsp,
            _ => bail!("unexpected request op type"),
        };

        if response_header.op != expected_op_type || response_header.group != request_header.group {
            bail!("wrong response types");
        }

        Ok((response_header, response_body))
    }

    fn set_timeout(&mut self, timeout_ms: u32) -> Result<(), Error> {
        self.port
            .set_timeout(Duration::from_millis(timeout_ms as u64))?;
        Ok(())
    }

    fn mtu(&self) -> usize {
        self.specs.mtu
    }

    fn linelength(&self) -> usize {
        self.specs.linelength
    }
}

/// UDP transport for SMP over network
pub struct UdpTransport {
    socket: UdpSocket,
    addr: SocketAddr,
    seq: u8,
    mtu: usize,
}

impl UdpTransport {
    pub fn new(config: &UdpSpecs) -> Result<Self, Error> {
        let addr_str = format!("{}:{}", config.host, config.port);
        let addr: SocketAddr = addr_str
            .to_socket_addrs()
            .with_context(|| format!("Failed to resolve address: {addr_str}"))?
            .next()
            .ok_or_else(|| anyhow::anyhow!("No address found for: {addr_str}"))?;

        let socket = UdpSocket::bind("0.0.0.0:0")
            .with_context(|| "Failed to bind UDP socket")?;

        socket
            .set_read_timeout(Some(Duration::from_secs(config.timeout_s as u64)))
            .with_context(|| "Failed to set socket timeout")?;

        socket
            .set_write_timeout(Some(Duration::from_secs(config.timeout_s as u64)))
            .with_context(|| "Failed to set socket write timeout")?;

        Ok(UdpTransport {
            socket,
            addr,
            seq: 0,
            mtu: config.mtu,
        })
    }

    fn next_seq(&mut self) -> u8 {
        let seq = self.seq;
        self.seq = self.seq.wrapping_add(1);
        seq
    }

    /// Encode SMP v2 header for UDP transport
    /// Byte 0: Res(3 bits) | Ver(2 bits) | OP(3 bits)
    /// Byte 1: Flags
    /// Bytes 2-3: Data Length (big-endian)
    /// Bytes 4-5: Group ID (big-endian)
    /// Byte 6: Sequence Number
    /// Byte 7: Command ID
    fn encode_header(&self, op: NmpOp, group: NmpGroup, id: u8, len: u16, seq: u8) -> [u8; 8] {
        let version: u8 = 1; // SMP v2
        let byte0 = ((version & 0x03) << 3) | (op as u8 & 0x07);
        let flags: u8 = 0;
        let group_u16 = group as u16;

        [
            byte0,
            flags,
            (len >> 8) as u8,
            (len & 0xFF) as u8,
            (group_u16 >> 8) as u8,
            (group_u16 & 0xFF) as u8,
            seq,
            id,
        ]
    }

    /// Decode SMP v2 header from UDP response
    fn decode_header(&self, data: &[u8]) -> Result<NmpHdr, Error> {
        if data.len() < 8 {
            bail!("Response too short: {} bytes", data.len());
        }

        let byte0 = data[0];
        let op_val = byte0 & 0x07;
        let _version = (byte0 >> 3) & 0x03;
        let _flags = data[1];
        let len = ((data[2] as u16) << 8) | (data[3] as u16);
        let group_val = ((data[4] as u16) << 8) | (data[5] as u16);
        let seq = data[6];
        let id = data[7];

        let op = match op_val {
            0 => NmpOp::Read,
            1 => NmpOp::ReadRsp,
            2 => NmpOp::Write,
            3 => NmpOp::WriteRsp,
            _ => bail!("Unknown op: {}", op_val),
        };

        let group = num::FromPrimitive::from_u16(group_val)
            .ok_or_else(|| anyhow::anyhow!("Unknown group: {}", group_val))?;

        Ok(NmpHdr {
            op,
            flags: 0,
            len,
            group,
            seq,
            id,
        })
    }
}

impl Transport for UdpTransport {
    fn transceive(
        &mut self,
        op: NmpOp,
        group: NmpGroup,
        id: u8,
        body: &[u8],
    ) -> Result<(NmpHdr, serde_cbor::Value), Error> {
        let seq = self.next_seq();

        // Build packet: header + CBOR body
        let header = self.encode_header(op, group, id, body.len() as u16, seq);
        let mut packet = Vec::with_capacity(8 + body.len());
        packet.extend_from_slice(&header);
        packet.extend_from_slice(body);

        debug!("UDP TX: {} bytes to {}", packet.len(), self.addr);
        debug!("UDP TX header: {:02x?}", &header);

        // Send packet
        self.socket
            .send_to(&packet, self.addr)
            .with_context(|| "Failed to send UDP packet")?;

        // Receive response
        let mut buf = [0u8; 4096];
        let (len, _src) = self.socket
            .recv_from(&mut buf)
            .with_context(|| "Failed to receive UDP response")?;

        debug!("UDP RX: {} bytes", len);

        if len < 8 {
            bail!("Response too short: {} bytes", len);
        }

        // Parse header
        let response_header = self.decode_header(&buf[..len])?;
        debug!("UDP RX header: {:?}", response_header);

        // Verify sequence number
        if response_header.seq != seq {
            bail!(
                "Sequence mismatch: expected {}, got {}",
                seq,
                response_header.seq
            );
        }

        // Verify response type
        let expected_op_type = match op {
            NmpOp::Read => NmpOp::ReadRsp,
            NmpOp::Write => NmpOp::WriteRsp,
            _ => bail!("unexpected request op type"),
        };

        if response_header.op != expected_op_type || response_header.group != group {
            bail!("wrong response types");
        }

        // Parse CBOR body
        let cbor_data = &buf[8..len];
        debug!("UDP RX CBOR: {} bytes", cbor_data.len());

        let body: serde_cbor::Value = if cbor_data.is_empty() {
            serde_cbor::Value::Map(std::collections::BTreeMap::new())
        } else {
            serde_cbor::from_slice(cbor_data)
                .with_context(|| "Failed to parse CBOR response")?
        };

        Ok((response_header, body))
    }

    fn set_timeout(&mut self, timeout_ms: u32) -> Result<(), Error> {
        self.socket
            .set_read_timeout(Some(Duration::from_millis(timeout_ms as u64)))
            .with_context(|| "Failed to set socket timeout")?;
        Ok(())
    }

    fn mtu(&self) -> usize {
        self.mtu
    }

    fn linelength(&self) -> usize {
        // Not used for UDP, but return a reasonable value
        self.mtu
    }
}

fn read_byte(port: &mut dyn SerialPort) -> Result<u8, Error> {
    let mut byte = [0u8];
    port.read_exact(&mut byte)?;
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
    data: &[u8],
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

        for _ in 0..u8::MAX {
            let id = next_seq_id();
            assert!(ids.insert(id), "Duplicate ID: {id}");
        }

        // Check wrapping behavior
        let wrapped_id = next_seq_id();
        assert_eq!(
            wrapped_id, initial_id,
            "Wrapped ID does not match initial ID"
        );
    }
}
