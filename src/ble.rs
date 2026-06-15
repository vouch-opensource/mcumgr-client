use anyhow::{bail, Context, Error, Result};
use bluer::{AdapterEvent, Address, Session};
use bluer::gatt::remote::Characteristic;
use futures::StreamExt;
use log::debug;
use std::str::FromStr;
use tokio::time::{timeout, Duration};
use uuid::{uuid, Uuid};

use crate::nmp_hdr::{NmpGroup, NmpHdr, NmpOp};
use crate::transfer::Transport;

const SMP_SERVICE_UUID: Uuid = uuid!("8D53DC1D-1DB7-4CD3-868B-8A527460AA84");
const SMP_CHAR_UUID: Uuid = uuid!("DA2E7828-FBCE-4E01-AE9E-261174997C48");

#[derive(Debug, Clone)]
pub struct BleSpecs {
    pub address: Option<String>,
    pub name: Option<String>,
    pub scan_timeout_s: u32,
    pub timeout_s: u32,
    pub mtu: usize,
}

impl Default for BleSpecs {
    fn default() -> Self {
        BleSpecs {
            address: None,
            name: None,
            scan_timeout_s: 10,
            timeout_s: 10,
            mtu: 244,
        }
    }
}

pub struct BleTransport {
    rt: tokio::runtime::Runtime,
    // Keep session alive so the D-Bus connection and BlueZ device handle remain valid
    _session: Session,
    characteristic: Characteristic,
    seq: u8,
    timeout_ms: u32,
    mtu: usize,
}

impl BleTransport {
    pub fn new(specs: &BleSpecs) -> Result<Self, Error> {
        if specs.address.is_none() && specs.name.is_none() {
            bail!("Either --ble-address or --ble-name must be provided");
        }

        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .context("Failed to build tokio runtime")?;

        let (session, characteristic) = rt.block_on(Self::connect_async(specs))?;

        Ok(BleTransport {
            rt,
            _session: session,
            characteristic,
            seq: 0,
            timeout_ms: specs.timeout_s * 1000,
            mtu: specs.mtu,
        })
    }

    async fn connect_async(specs: &BleSpecs) -> Result<(Session, Characteristic), Error> {
        let session = Session::new().await.context("Failed to create BlueZ session")?;
        let adapter = session.default_adapter().await.context("No Bluetooth adapter found")?;
        adapter.set_powered(true).await.context("Failed to power on Bluetooth adapter")?;

        let target_addr: Option<Address> = specs.address.as_deref()
            .map(|s| Address::from_str(s))
            .transpose()
            .context("Invalid BLE address")?;
        let target_name: Option<String> = specs.name.clone();
        let scan_timeout = specs.scan_timeout_s;

        debug!("Starting BLE device discovery (timeout: {}s)...", scan_timeout);

        // Clone adapter for use inside the async move block; original is used after scan.
        let adapter_scan = adapter.clone();
        let mut discover = adapter.discover_devices().await
            .context("Failed to start BLE device discovery")?;

        let device_addr = timeout(Duration::from_secs(scan_timeout as u64), async move {
            while let Some(evt) = discover.next().await {
                if let AdapterEvent::DeviceAdded(addr) = evt {
                    // Match by address
                    if let Some(target) = target_addr {
                        if addr == target {
                            debug!("Found target BLE device at {}", addr);
                            return Some(addr);
                        }
                    }
                    // Match by name
                    if let Some(ref name) = target_name {
                        if let Ok(device) = adapter_scan.device(addr) {
                            if let Ok(Some(n)) = device.name().await {
                                if n.contains(name.as_str()) {
                                    debug!("Found BLE device '{}' at {}", n, addr);
                                    return Some(addr);
                                }
                            }
                        }
                    }
                }
            }
            None
        })
        .await
        .ok()
        .flatten()
        .ok_or_else(|| anyhow::anyhow!(
            "BLE device not found within {}s — ensure the device is advertising",
            scan_timeout
        ))?;

        let device = adapter.device(device_addr)
            .with_context(|| format!("Failed to access device {}", device_addr))?;

        if !device.is_connected().await.unwrap_or(false) {
            debug!("Connecting to {}...", device_addr);
            device.connect().await
                .with_context(|| format!("Failed to connect to {}", device_addr))?;
            debug!("Connected to {}", device_addr);
        } else {
            debug!("Already connected to {}", device_addr);
        }

        // Discover SMP GATT service and characteristic
        let mut found_char: Option<Characteristic> = None;
        'outer: for service in device.services().await.context("Failed to discover GATT services")? {
            if service.uuid().await.unwrap_or_default() == SMP_SERVICE_UUID {
                for char in service.characteristics().await
                    .context("Failed to discover GATT characteristics")?
                {
                    if char.uuid().await.unwrap_or_default() == SMP_CHAR_UUID {
                        debug!("Found SMP characteristic");
                        found_char = Some(char);
                        break 'outer;
                    }
                }
            }
        }

        let characteristic = found_char.ok_or_else(|| anyhow::anyhow!(
            "SMP characteristic not found — ensure the device has BLE SMP service enabled"
        ))?;

        Ok((session, characteristic))
    }

    fn next_seq(&mut self) -> u8 {
        let seq = self.seq;
        self.seq = self.seq.wrapping_add(1);
        seq
    }

    fn encode_smp_header(op: NmpOp, group: NmpGroup, id: u8, len: u16, seq: u8) -> [u8; 8] {
        let version: u8 = 1; // SMP v2
        let byte0 = ((version & 0x03) << 3) | (op as u8 & 0x07);
        [
            byte0, 0,
            (len >> 8) as u8, (len & 0xFF) as u8,
            (group.0 >> 8) as u8, (group.0 & 0xFF) as u8,
            seq, id,
        ]
    }

    fn decode_smp_header(data: &[u8]) -> Result<NmpHdr, Error> {
        if data.len() < 8 {
            bail!("BLE response too short: {} bytes", data.len());
        }
        let op_val = data[0] & 0x07;
        let len = ((data[2] as u16) << 8) | (data[3] as u16);
        let group_val = ((data[4] as u16) << 8) | (data[5] as u16);
        let seq = data[6];
        let id = data[7];
        let op = match op_val {
            0 => NmpOp::Read,
            1 => NmpOp::ReadRsp,
            2 => NmpOp::Write,
            3 => NmpOp::WriteRsp,
            _ => bail!("Unknown SMP op: {}", op_val),
        };
        Ok(NmpHdr { op, flags: 0, len, group: NmpGroup(group_val), seq, id })
    }
}

impl Transport for BleTransport {
    fn transceive(
        &mut self,
        op: NmpOp,
        group: NmpGroup,
        id: u8,
        body: &[u8],
    ) -> Result<(NmpHdr, serde_cbor::Value), Error> {
        let seq = self.next_seq();
        let header = Self::encode_smp_header(op, group, id, body.len() as u16, seq);
        let mut packet = Vec::with_capacity(8 + body.len());
        packet.extend_from_slice(&header);
        packet.extend_from_slice(body);

        debug!("BLE TX: {} bytes", packet.len());

        let char = self.characteristic.clone();
        let timeout_ms = self.timeout_ms;

        // Subscribe to notifications, send request, then collect response fragments.
        // BLE ATT notifications may carry partial SMP payloads if the packet exceeds ATT MTU.
        let response = self.rt.block_on(async move {
            let notify_stream = char.notify().await
                .context("Failed to subscribe to SMP notifications")?;
            tokio::pin!(notify_stream);

            char.write(&packet).await
                .context("Failed to write to SMP characteristic")?;

            // Collect the first notification to read the SMP header + partial payload
            let first = timeout(
                Duration::from_millis(timeout_ms as u64),
                notify_stream.next(),
            )
            .await
            .map_err(|_| anyhow::anyhow!("BLE response timeout after {}ms", timeout_ms))?
            .ok_or_else(|| anyhow::anyhow!("BLE notification stream closed"))?;

            if first.len() < 8 {
                bail!("BLE first notification too short: {} bytes", first.len());
            }

            let total_payload = ((first[2] as usize) << 8) | (first[3] as usize);
            let mut payload = first[8..].to_vec();

            // Accumulate additional notifications if the SMP payload is fragmented
            while payload.len() < total_payload {
                let fragment = timeout(
                    Duration::from_millis(timeout_ms as u64),
                    notify_stream.next(),
                )
                .await
                .map_err(|_| anyhow::anyhow!("BLE fragment timeout after {}ms", timeout_ms))?
                .ok_or_else(|| anyhow::anyhow!("BLE notification stream closed mid-packet"))?;

                payload.extend_from_slice(&fragment);
            }

            // Return header bytes + reassembled payload as a flat buffer
            let mut full = first[..8].to_vec();
            full.extend_from_slice(&payload[..total_payload]);
            Ok::<Vec<u8>, Error>(full)
        })?;

        debug!("BLE RX: {} bytes", response.len());

        let response_header = Self::decode_smp_header(&response)?;

        if response_header.seq != seq {
            bail!("Sequence mismatch: expected {}, got {}", seq, response_header.seq);
        }

        let expected_op = match op {
            NmpOp::Read => NmpOp::ReadRsp,
            NmpOp::Write => NmpOp::WriteRsp,
            _ => bail!("Unexpected request op type"),
        };

        if response_header.op != expected_op || response_header.group != group {
            bail!("Wrong response type");
        }

        let cbor_data = &response[8..];
        let body: serde_cbor::Value = if cbor_data.is_empty() {
            serde_cbor::Value::Map(std::collections::BTreeMap::new())
        } else {
            serde_cbor::from_slice(cbor_data).context("Failed to parse CBOR response")?
        };

        Ok((response_header, body))
    }

    fn set_timeout(&mut self, timeout_ms: u32) -> Result<(), Error> {
        self.timeout_ms = timeout_ms;
        Ok(())
    }

    fn mtu(&self) -> usize {
        self.mtu
    }

    fn linelength(&self) -> usize {
        self.mtu
    }
}
