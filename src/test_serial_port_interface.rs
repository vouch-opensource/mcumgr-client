// Copyright © 2023 Vouch.io LLC

use base64::engine::{general_purpose::STANDARD, Engine};
use byteorder::{BigEndian, ByteOrder};
use crc16::State;
use crc16::XMODEM;
use serialport::Error;
use std::io::Cursor;
use std::thread;
use std::time::Duration;

use crate::interface::Interface;
use crate::nmp_hdr::*;
use crate::transfer::encode_request;

pub struct TestSerialPortInterface {
    data: Vec<u8>,
    position: usize,
}

impl TestSerialPortInterface {
    pub fn new() -> TestSerialPortInterface {
        TestSerialPortInterface {
            data: Vec::new(),
            position: 0,
        }
    }
}

impl Interface for TestSerialPortInterface {
    fn bytes_to_read(&self) -> Result<u32, Error> {
        Ok((self.data.len() - self.position) as u32)
    }

    fn read_byte(self: &mut TestSerialPortInterface) -> Result<u8, Error> {
        let b = self.data[self.position];
        self.position += 1;
        Ok(b)
    }

    fn write_all(&mut self, buf: &[u8]) -> Result<(), std::io::Error> {
        let mut cursor = Cursor::new(buf);
        let mut received_data = Vec::new();

        while cursor.position() < buf.len() as u64 {
            let _marker = byteorder::ReadBytesExt::read_u16::<BigEndian>(&mut cursor).unwrap();
            let base64_end_pos = buf[cursor.position() as usize..]
                .iter()
                .position(|&x| x == b'\n')
                .unwrap()
                + cursor.position() as usize;
            let base64_data = &buf[cursor.position() as usize..base64_end_pos];
            let binary_data = STANDARD.decode(base64_data).unwrap();
            cursor.set_position(base64_end_pos as u64 + 1);

            received_data.extend_from_slice(&binary_data);
        }

        let data = received_data[2..received_data.len() - 2].to_vec();
        let read_checksum = BigEndian::read_u16(&received_data[received_data.len() - 2..]);
        let calculated_checksum = State::<XMODEM>::calculate(&data);
        if read_checksum != calculated_checksum {
            return Err(
                std::io::Error::new(std::io::ErrorKind::InvalidInput, "wrong checksum").into(),
            );
        }

        let mut request_cursor = Cursor::new(&data);
        let mut total_len = 0;
        let request_header = NmpHdr::deserialize(&mut request_cursor).unwrap();

        match request_header.id {
            id if id == NmpIdImage::State as u8 => {
                let mut map = std::collections::BTreeMap::<String, String>::new();
                let test_string = "x".repeat(1024);
                map.insert("test".to_string(), test_string);
                let body = serde_cbor::to_vec(&map).unwrap();
                let (encoded_response, _) = encode_request(
                    100,
                    NmpOp::ReadRsp,
                    NmpGroup::Image,
                    NmpIdImage::State as u8,
                    &body,
                    request_header.seq,
                )
                .unwrap();
                self.data.extend_from_slice(&encoded_response);
            }
            id if id == NmpIdImage::Upload as u8 => {
                let body_start = request_cursor.position() as usize;
                let body_end = data.len();
                let body = &data[body_start..body_end];

                let image_upload_req: ImageUploadReq = serde_cbor::from_slice(body).unwrap();
                if image_upload_req.off == 0 {
                    total_len = image_upload_req.len.unwrap();
                }
                let mut off_value = image_upload_req.off + data.len() as u32;
                if off_value > total_len {
                    off_value = total_len;
                }

                let mut response_map = std::collections::BTreeMap::new();
                response_map.insert("rc", 0);
                response_map.insert("off", off_value);

                let cbor_body = serde_cbor::to_vec(&response_map).unwrap();
                let (encoded_response, _) = encode_request(
                    4096,
                    NmpOp::WriteRsp,
                    NmpGroup::Image,
                    NmpIdImage::State as u8,
                    &cbor_body,
                    request_header.seq,
                )
                .unwrap();
                self.data.extend_from_slice(&encoded_response);
            }
            _ => {
                // Handle other cases or return an error
            }
        }

        // add some delay for simulating real transfers
        // simulating 10 kB/s
        thread::sleep(Duration::from_millis((buf.len() / 10) as u64));

        Ok(())
    }
}
