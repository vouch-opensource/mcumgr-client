// Copyright © 2023-2024 Vouch.io LLC

use base64::engine::{general_purpose::STANDARD, Engine};
use byteorder::{BigEndian, ByteOrder};
use crc16::State;
use crc16::XMODEM;
use hex;
use serialport::DataBits;
use serialport::FlowControl;
use serialport::Parity;
use serialport::SerialPort;
use serialport::StopBits;
use std::io::Cursor;
use std::io::{Read, Write};
use std::thread;
use std::time::Duration;

use crate::nmp_hdr::*;
use crate::transfer::encode_request;

pub struct TestSerialPort {
    data: Vec<u8>,
    position: usize,
    total_len: u32,
    images: Vec<ImageStateEntry>,
}

impl TestSerialPort {
    pub fn new() -> TestSerialPort {
        TestSerialPort {
            data: Vec::new(),
            position: 0,
            total_len: 0,
            images: vec![ImageStateEntry {
                image: 1,
                slot: 0,
                version: "1.0.0".to_string(),
                hash: hex::decode(
                    "61ddbce8f52e53715f57b360a5af0700ba17122114c94a11b86d9097f7e09cc3",
                )
                .unwrap(),
                bootable: false,
                pending: false,
                confirmed: false,
                active: true,
                permanent: false,
            }],
        }
    }
}

impl Read for TestSerialPort {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let available_data = &self.data[self.position..];
        let bytes_to_read = std::cmp::min(available_data.len(), buf.len());
        buf[..bytes_to_read].copy_from_slice(&available_data[..bytes_to_read]);
        self.position += bytes_to_read;
        Ok(bytes_to_read)
    }
}

impl Write for TestSerialPort {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
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
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "wrong checksum",
            ));
        }

        let mut request_cursor = Cursor::new(&data);
        let request_header = NmpHdr::deserialize(&mut request_cursor).unwrap();
        // let header_len: usize = 8;
        // let request_body = data[header_len..].to_vec();

        match request_header.id {
            id if id == NmpIdImage::State as u8 => {
                if request_header.op == NmpOp::Read {
                    let state_response = ImageStateRsp {
                        images: self.images.clone(),
                        split_status: None,
                    };
                    let body = serde_cbor::to_vec(&state_response).unwrap();
                    let (encoded_response, _) = encode_request(
                        100,
                        NmpOp::ReadRsp,
                        NmpGroup::Image,
                        NmpIdImage::State,
                        &body,
                        request_header.seq,
                    )
                    .unwrap();
                    self.data.extend_from_slice(&encoded_response);
                } else if request_header.op == NmpOp::Write {
                    // let request: ImageStateReq = serde_cbor::from_slice(request_body.as_slice()).unwrap();
                    let body = serde_cbor::to_vec(&serde_cbor::Value::Null).unwrap();
                    let (encoded_response, _) = encode_request(
                        100,
                        NmpOp::WriteRsp,
                        NmpGroup::Image,
                        NmpIdImage::Erase,
                        &body,
                        request_header.seq,
                    )
                    .unwrap();
                    self.data.extend_from_slice(&encoded_response);
                }
            }
            id if id == NmpIdImage::Upload as u8 => {
                let body_start = request_cursor.position() as usize;
                let body_end = data.len();
                let body = &data[body_start..body_end];

                let image_upload_req: ImageUploadReq = serde_cbor::from_slice(body).unwrap();
                if image_upload_req.off == 0 {
                    self.total_len = image_upload_req.len.unwrap();
                }
                let mut off_value = image_upload_req.off + data.len() as u32;
                if off_value > self.total_len {
                    off_value = self.total_len;
                }

                let mut response_map = std::collections::BTreeMap::new();
                response_map.insert("rc", 0);
                response_map.insert("off", off_value);

                let cbor_body = serde_cbor::to_vec(&response_map).unwrap();
                let (encoded_response, _) = encode_request(
                    4096,
                    NmpOp::WriteRsp,
                    NmpGroup::Image,
                    NmpIdImage::State,
                    &cbor_body,
                    request_header.seq,
                )
                .unwrap();
                self.data.extend_from_slice(&encoded_response);
            }
            id if id == NmpIdImage::Erase as u8 => {
                // let request: ImageEraseReq = serde_cbor::from_slice(request_body.as_slice()).unwrap();
                let body = serde_cbor::to_vec(&serde_cbor::Value::Null).unwrap();
                let (encoded_response, _) = encode_request(
                    100,
                    NmpOp::WriteRsp,
                    NmpGroup::Image,
                    NmpIdImage::Erase,
                    &body,
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

        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl SerialPort for TestSerialPort {
    fn name(&self) -> Option<String> {
        Some("test".to_string())
    }

    fn baud_rate(&self) -> serialport::Result<u32> {
        Ok(115200)
    }

    fn data_bits(&self) -> serialport::Result<DataBits> {
        Ok(DataBits::Eight)
    }

    fn flow_control(&self) -> serialport::Result<FlowControl> {
        Ok(FlowControl::None)
    }

    fn parity(&self) -> serialport::Result<Parity> {
        Ok(Parity::None)
    }

    fn stop_bits(&self) -> serialport::Result<StopBits> {
        Ok(StopBits::One)
    }

    fn timeout(&self) -> Duration {
        Duration::from_secs(1)
    }

    fn set_baud_rate(&mut self, _baud_rate: u32) -> serialport::Result<()> {
        Ok(())
    }

    fn set_data_bits(&mut self, _data_bits: serialport::DataBits) -> serialport::Result<()> {
        Ok(())
    }

    fn set_flow_control(
        &mut self,
        _flow_control: serialport::FlowControl,
    ) -> serialport::Result<()> {
        Ok(())
    }

    fn set_parity(&mut self, _parity: serialport::Parity) -> serialport::Result<()> {
        Ok(())
    }

    fn set_stop_bits(&mut self, _stop_bits: serialport::StopBits) -> serialport::Result<()> {
        Ok(())
    }

    fn set_timeout(&mut self, _timeout: Duration) -> serialport::Result<()> {
        Ok(())
    }

    fn write_request_to_send(&mut self, _level: bool) -> serialport::Result<()> {
        Ok(())
    }

    fn write_data_terminal_ready(&mut self, _level: bool) -> serialport::Result<()> {
        Ok(())
    }

    fn read_clear_to_send(&mut self) -> serialport::Result<bool> {
        Ok(true)
    }

    fn read_data_set_ready(&mut self) -> serialport::Result<bool> {
        Ok(true)
    }

    fn read_ring_indicator(&mut self) -> serialport::Result<bool> {
        Ok(true)
    }

    fn read_carrier_detect(&mut self) -> serialport::Result<bool> {
        Ok(true)
    }

    fn bytes_to_read(&self) -> serialport::Result<u32> {
        Ok(0)
    }

    fn bytes_to_write(&self) -> serialport::Result<u32> {
        Ok(0)
    }

    fn clear(&self, _buffer_to_clear: serialport::ClearBuffer) -> serialport::Result<()> {
        Ok(())
    }

    fn try_clone(&self) -> serialport::Result<Box<dyn SerialPort>> {
        unimplemented!()
    }

    fn set_break(&self) -> serialport::Result<()> {
        Ok(())
    }

    fn clear_break(&self) -> serialport::Result<()> {
        Ok(())
    }
}
