use anyhow::{bail, Context, Error, Result};
use btleplug::api::{
    Central, CentralEvent, Characteristic, Manager as _, Peripheral as _, ScanFilter,
    ValueNotification, WriteType,
};
use btleplug::platform::{Manager, Peripheral};
use futures::StreamExt;
use log::debug;
use std::pin::Pin;
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

// Groups async state so that transceive() can borrow it separately from the runtime.
struct BleConnection {
    peripheral: Peripheral,
    smp_char: Characteristic,
    notifications: Pin<Box<dyn futures::Stream<Item = ValueNotification> + Send>>,
}

pub struct BleTransport {
    rt: tokio::runtime::Runtime,
    // Wrapped in Option so Drop can take it and destroy it inside block_on,
    // which is required because MessageStream::drop calls Handle::current().
    conn: Option<BleConnection>,
    seq: u8,
    timeout_ms: u32,
    mtu: usize,
}

impl Drop for BleTransport {
    fn drop(&mut self) {
        if let Some(conn) = self.conn.take() {
            self.rt.block_on(async move { drop(conn) });
        }
    }
}

impl BleTransport {
    pub fn new(specs: &BleSpecs) -> Result<Self, Error> {
        if specs.address.is_none() && specs.name.is_none() {
            bail!("Either --ble-address or --ble-name must be provided");
        }

        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .context("Failed to build tokio runtime")?;

        let conn = rt.block_on(Self::connect_async(specs))?;

        Ok(BleTransport {
            rt,
            conn: Some(conn),
            seq: 0,
            timeout_ms: specs.timeout_s * 1000,
            mtu: specs.mtu,
        })
    }

    async fn connect_async(specs: &BleSpecs) -> Result<BleConnection, Error> {
        let manager = Manager::new().await.context("Failed to initialize BLE manager")?;
        let adapters = manager.adapters().await.context("Failed to list BLE adapters")?;
        let adapter = adapters
            .into_iter()
            .next()
            .ok_or_else(|| anyhow::anyhow!("No Bluetooth adapter found"))?;

        let target_addr = specs.address.clone();
        let target_name = specs.name.clone();
        let scan_timeout = specs.scan_timeout_s;

        debug!("Starting BLE device discovery (timeout: {}s)...", scan_timeout);

        adapter
            .start_scan(ScanFilter::default())
            .await
            .context("Failed to start BLE scan")?;

        let mut events = adapter
            .events()
            .await
            .context("Failed to subscribe to adapter events")?;

        let peripheral = timeout(
            Duration::from_secs(scan_timeout as u64),
            async {
                while let Some(event) = events.next().await {
                    // DeviceUpdated fires when advertisement data (e.g. name) arrives after discovery
                    let id = match event {
                        CentralEvent::DeviceDiscovered(id) | CentralEvent::DeviceUpdated(id) => id,
                        _ => continue,
                    };

                    let Ok(p) = adapter.peripheral(&id).await else {
                        continue;
                    };

                    // MAC address matching — not available on macOS (CoreBluetooth hides MACs)
                    #[cfg(not(target_os = "macos"))]
                    if let Some(ref addr) = target_addr {
                        if p.address().to_string().eq_ignore_ascii_case(addr) {
                            debug!("Found target BLE device at {}", p.address());
                            return Some(p);
                        }
                    }

                    if let Some(ref name) = target_name {
                        if let Ok(Some(props)) = p.properties().await {
                            if let Some(ref n) = props.local_name {
                                if n.contains(name.as_str()) {
                                    debug!("Found BLE device '{}' at {}", n, p.address());
                                    return Some(p);
                                }
                            }
                        }
                    }
                }
                None
            },
        )
        .await
        .ok()
        .flatten()
        .ok_or_else(|| {
            anyhow::anyhow!(
                "BLE device not found within {}s — ensure the device is advertising",
                scan_timeout
            )
        })?;

        adapter.stop_scan().await.ok();

        if !peripheral.is_connected().await.unwrap_or(false) {
            debug!("Connecting to {}...", peripheral.address());
            peripheral
                .connect()
                .await
                .with_context(|| format!("Failed to connect to {}", peripheral.address()))?;
            debug!("Connected to {}", peripheral.address());
        } else {
            debug!("Already connected to {}", peripheral.address());
        }

        peripheral
            .discover_services()
            .await
            .context("Failed to discover GATT services")?;

        let smp_char = peripheral
            .services()
            .into_iter()
            .find(|s| s.uuid == SMP_SERVICE_UUID)
            .and_then(|s| s.characteristics.into_iter().find(|c| c.uuid == SMP_CHAR_UUID))
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "SMP characteristic not found — ensure the device has BLE SMP service enabled"
                )
            })?;

        debug!("Found SMP characteristic");

        // Subscribe once; the resulting stream is reused across all transceive() calls.
        peripheral
            .subscribe(&smp_char)
            .await
            .context("Failed to subscribe to SMP characteristic notifications")?;

        let notifications = peripheral
            .notifications()
            .await
            .context("Failed to open BLE notification stream")?;

        Ok(BleConnection {
            peripheral,
            smp_char,
            notifications,
        })
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
            byte0,
            0,
            (len >> 8) as u8,
            (len & 0xFF) as u8,
            (group.0 >> 8) as u8,
            (group.0 & 0xFF) as u8,
            seq,
            id,
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
        Ok(NmpHdr {
            op,
            flags: 0,
            len,
            group: NmpGroup(group_val),
            seq,
            id,
        })
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

        let timeout_ms = self.timeout_ms;
        // Borrow rt and conn as disjoint fields so the async block can capture conn.
        let rt = &self.rt;
        let conn = self.conn.as_mut().unwrap();

        let response = rt.block_on(async {
            conn.peripheral
                .write(&conn.smp_char, &packet, WriteType::WithoutResponse)
                .await
                .context("Failed to write to SMP characteristic")?;

            // Collect the first notification that carries an SMP response.
            // The stream may deliver notifications for other characteristics if any were
            // subscribed by the platform; filter by UUID to be safe.
            let first = timeout(
                Duration::from_millis(timeout_ms as u64),
                async {
                    loop {
                        match conn.notifications.next().await {
                            Some(n) if n.uuid == SMP_CHAR_UUID => break Some(n.value),
                            Some(_) => continue,
                            None => break None,
                        }
                    }
                },
            )
            .await
            .map_err(|_| anyhow::anyhow!("BLE response timeout after {}ms", timeout_ms))?
            .ok_or_else(|| anyhow::anyhow!("BLE notification stream closed"))?;

            if first.len() < 8 {
                bail!("BLE first notification too short: {} bytes", first.len());
            }

            let total_payload = ((first[2] as usize) << 8) | (first[3] as usize);
            let mut payload = first[8..].to_vec();

            // Accumulate additional notifications if the SMP payload is fragmented across ATT MTU
            while payload.len() < total_payload {
                let fragment = timeout(
                    Duration::from_millis(timeout_ms as u64),
                    async {
                        loop {
                            match conn.notifications.next().await {
                                Some(n) if n.uuid == SMP_CHAR_UUID => break Some(n.value),
                                Some(_) => continue,
                                None => break None,
                            }
                        }
                    },
                )
                .await
                .map_err(|_| anyhow::anyhow!("BLE fragment timeout after {}ms", timeout_ms))?
                .ok_or_else(|| anyhow::anyhow!("BLE notification stream closed mid-packet"))?;

                payload.extend_from_slice(&fragment);
            }

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
