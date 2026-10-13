//! Desktop BLE transport for `clicks-core`, built on btleplug.
//!
//! The keyboard is usually already bonded and connected to the OS as a HID
//! keyboard and may not advertise, so discovery first asks the OS for
//! connected/known peripherals and only then falls back to scanning.
//!
//! Dropping or [`closing`](Keyboard::close) a [`Keyboard`] never disconnects
//! the link: on BlueZ that would also drop the keyboard's HID connection.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use btleplug::api::{
    Central, CentralEvent, CharPropFlags, Characteristic, Manager as _, Peripheral as _, RetrievePeripheralsOptions,
    ScanFilter, WriteType,
};
use btleplug::platform::{Adapter, Manager, Peripheral, PeripheralId};
use clicks_core::frame::Decoder;
use clicks_core::{gatt, model, KeyboardState, Message, Request, Response};
use futures::StreamExt;
use tokio::sync::{broadcast, mpsc, Mutex};
use uuid::Uuid;

pub use btleplug;
pub use clicks_core;

pub const RESPONSE_TIMEOUT: Duration = Duration::from_secs(3);

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("bluetooth: {0}")]
    Ble(#[from] btleplug::Error),
    #[error("no bluetooth adapter found")]
    NoAdapter,
    #[error("device does not expose the configuration service (fff0)")]
    MissingService,
    #[error("timed out waiting for a response to opcode {0:#04x}")]
    Timeout(u8),
    #[error("keyboard rejected opcode {opcode:#04x} with status {status:#04x}")]
    Rejected { opcode: u8, status: u8 },
    #[error("connection closed")]
    Disconnected,
    #[error(transparent)]
    Model(#[from] clicks_core::ModelError),
    #[error(transparent)]
    Frame(#[from] clicks_core::frame::FrameError),
}

pub type Result<T> = std::result::Result<T, Error>;

fn uuid(s: &str) -> Uuid {
    Uuid::parse_str(s).expect("valid uuid constant")
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    pub id: PeripheralId,
    pub name: Option<String>,
    pub address: String,
    pub connected: bool,
}

pub async fn default_adapter() -> Result<Adapter> {
    let manager = Manager::new().await?;
    manager.adapters().await?.into_iter().next().ok_or(Error::NoAdapter)
}

async fn describe(p: &Peripheral) -> Result<Candidate> {
    let props = p.properties().await?.unwrap_or_default();
    Ok(Candidate {
        id: p.id(),
        name: props.local_name.or(props.advertisement_name),
        address: p.address().to_string(),
        connected: p.is_connected().await.unwrap_or(false),
    })
}

async fn looks_like_keyboard(p: &Peripheral) -> bool {
    let Ok(Some(props)) = p.properties().await else { return false };
    let service = uuid(gatt::SERVICE);
    props.services.contains(&service)
        || p.services().iter().any(|s| s.uuid == service)
        || [props.local_name, props.advertisement_name]
            .iter()
            .flatten()
            .any(|n| n.starts_with(gatt::NAME_PREFIX))
}

/// Finds keyboards: OS-connected ones first, then anything seen while scanning
/// for `scan_for`.
pub async fn discover(adapter: &Adapter, scan_for: Duration) -> Result<Vec<(Peripheral, Candidate)>> {
    let mut found: HashMap<PeripheralId, Peripheral> = HashMap::new();

    let retrieved = adapter
        .retrieve_peripherals(RetrievePeripheralsOptions { identifiers: None, services: Some(vec![uuid(gatt::SERVICE)]) })
        .await;
    match retrieved {
        Ok(list) => found.extend(list.into_iter().map(|p| (p.id(), p))),
        Err(btleplug::Error::NotSupported(_)) => {}
        Err(e) => log::warn!("retrieve_peripherals failed: {e}"),
    }

    if !scan_for.is_zero() {
        adapter.start_scan(ScanFilter::default()).await?;
        tokio::time::sleep(scan_for).await;
        adapter.stop_scan().await?;
    }
    for p in adapter.peripherals().await? {
        if !found.contains_key(&p.id()) && looks_like_keyboard(&p).await {
            found.insert(p.id(), p);
        }
    }

    let mut out = Vec::new();
    for (_, p) in found {
        let c = describe(&p).await?;
        out.push((p, c));
    }
    out.sort_by(|a, b| b.1.connected.cmp(&a.1.connected).then(a.1.name.cmp(&b.1.name)));
    Ok(out)
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DeviceInfo {
    pub manufacturer: Option<String>,
    pub model: Option<String>,
    pub serial: Option<String>,
    pub hardware: Option<String>,
    pub firmware: Option<String>,
    pub software: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Battery(u8),
    /// A notification on the config channel that wasn't a response.
    Unsolicited(Vec<u8>),
    Disconnected,
}

pub struct Keyboard {
    peripheral: Peripheral,
    write: Characteristic,
    write_type: WriteType,
    notify: Characteristic,
    battery: Option<Characteristic>,
    responses: Mutex<mpsc::UnboundedReceiver<Response>>,
    /// Held across multi-request operations so read-modify-writes don't interleave.
    ops: Mutex<()>,
    events: broadcast::Sender<Event>,
    state: std::sync::Mutex<KeyboardState>,
    tasks: Vec<tokio::task::JoinHandle<()>>,
}

impl Keyboard {
    pub async fn open(adapter: &Adapter, peripheral: Peripheral) -> Result<Arc<Self>> {
        if !peripheral.is_connected().await? {
            peripheral.connect().await?;
        }
        peripheral.discover_services().await?;

        let chars = peripheral.characteristics();
        let find = |u: &str| chars.iter().find(|c| c.uuid == uuid(u)).cloned();
        let write = find(gatt::WRITE).ok_or(Error::MissingService)?;
        let notify = find(gatt::NOTIFY).ok_or(Error::MissingService)?;
        let battery = find(gatt::BATTERY_LEVEL);
        let write_type = if write.properties.contains(CharPropFlags::WRITE) {
            WriteType::WithResponse
        } else {
            WriteType::WithoutResponse
        };

        let mut stream = peripheral.notifications().await?;
        peripheral.subscribe(&notify).await?;
        if let Some(b) = battery.as_ref().filter(|b| b.properties.contains(CharPropFlags::NOTIFY)) {
            if let Err(e) = peripheral.subscribe(b).await {
                log::debug!("battery notifications unavailable: {e}");
            }
        }

        let (resp_tx, resp_rx) = mpsc::unbounded_channel();
        let (events, _) = broadcast::channel(32);
        let notify_uuid = notify.uuid;
        let battery_uuid = uuid(gatt::BATTERY_LEVEL);

        let ev = events.clone();
        let reader = tokio::spawn(async move {
            let mut decoder = Decoder::new();
            while let Some(n) = stream.next().await {
                if n.uuid == battery_uuid {
                    if let Some(&level) = n.value.first() {
                        let _ = ev.send(Event::Battery(level));
                    }
                    continue;
                }
                if n.uuid != notify_uuid {
                    continue;
                }
                for frame in decoder.push(&n.value) {
                    match frame {
                        Ok(payload) => match Message::parse(&payload) {
                            Message::Response(r) => {
                                let _ = resp_tx.send(r);
                            }
                            Message::Other(raw) => {
                                let _ = ev.send(Event::Unsolicited(raw));
                            }
                        },
                        Err(e) => log::warn!("dropping bad frame: {e}"),
                    }
                }
            }
            let _ = ev.send(Event::Disconnected);
        });

        let ev = events.clone();
        let id = peripheral.id();
        let mut central = adapter.events().await?;
        let watcher = tokio::spawn(async move {
            while let Some(e) = central.next().await {
                if matches!(e, CentralEvent::DeviceDisconnected(ref d) if *d == id) {
                    let _ = ev.send(Event::Disconnected);
                    break;
                }
            }
        });

        Ok(Arc::new(Self {
            peripheral,
            write,
            write_type,
            notify,
            battery,
            responses: Mutex::new(resp_rx),
            ops: Mutex::new(()),
            events,
            state: std::sync::Mutex::new(KeyboardState::new()),
            tasks: vec![reader, watcher],
        }))
    }

    pub fn peripheral(&self) -> &Peripheral {
        &self.peripheral
    }

    pub fn events(&self) -> broadcast::Receiver<Event> {
        self.events.subscribe()
    }

    pub fn state(&self) -> KeyboardState {
        self.state.lock().unwrap().clone()
    }

    /// Sends one request and waits for its response. Requests are serialised.
    pub async fn request(&self, request: &Request) -> Result<Response> {
        let mut rx = self.responses.lock().await;
        while rx.try_recv().is_ok() {} // drop stale responses from timed-out requests
        let frame = request.encode()?;
        self.peripheral.write(&self.write, &frame, self.write_type).await?;
        let response = tokio::time::timeout(RESPONSE_TIMEOUT, async {
            loop {
                match rx.recv().await {
                    Some(r) if request.matches(&r) => return Ok(r),
                    Some(r) => log::debug!("ignoring response for opcode {:#04x}", r.opcode),
                    None => return Err(Error::Disconnected),
                }
            }
        })
        .await
        .map_err(|_| Error::Timeout(request.opcode()))??;
        self.state.lock().unwrap().apply(request, &response);
        Ok(response)
    }

    /// Runs requests in order, stopping at the first rejection.
    pub async fn execute(&self, requests: &[Request]) -> Result<()> {
        for r in requests {
            let resp = self.request(r).await?;
            if !resp.is_ok() {
                return Err(Error::Rejected { opcode: resp.opcode, status: resp.status });
            }
        }
        Ok(())
    }

    /// Reads everything the official app reads on connect.
    pub async fn sync(&self, mut progress: impl FnMut(usize, usize)) -> Result<KeyboardState> {
        let _ops = self.ops.lock().await;
        let plan = model::sync_plan();
        for (i, r) in plan.iter().enumerate() {
            self.request(r).await?;
            progress(i + 1, plan.len());
        }
        Ok(self.state())
    }

    /// Computes requests from the current state and executes them.
    pub async fn update<F>(&self, build: F) -> Result<KeyboardState>
    where
        F: FnOnce(&KeyboardState) -> std::result::Result<Vec<Request>, clicks_core::ModelError>,
    {
        let _ops = self.ops.lock().await;
        let requests = build(&self.state())?;
        self.execute(&requests).await?;
        Ok(self.state())
    }

    pub async fn battery(&self) -> Result<Option<u8>> {
        match &self.battery {
            Some(c) => Ok(self.peripheral.read(c).await?.first().copied()),
            None => Ok(None),
        }
    }

    pub async fn device_info(&self) -> Result<DeviceInfo> {
        let chars = self.peripheral.characteristics();
        let read = |u: &'static str| {
            let c = chars.iter().find(|c| c.uuid == uuid(u)).cloned();
            async move {
                let c = c?;
                let v = self.peripheral.read(&c).await.ok()?;
                Some(String::from_utf8_lossy(&v).trim_end_matches('\0').to_owned())
            }
        };
        Ok(DeviceInfo {
            manufacturer: read(gatt::MANUFACTURER_NAME).await,
            model: read(gatt::MODEL_NUMBER).await,
            serial: read(gatt::SERIAL_NUMBER).await,
            hardware: read(gatt::HARDWARE_REVISION).await,
            firmware: read(gatt::FIRMWARE_REVISION).await,
            software: read(gatt::SOFTWARE_REVISION).await,
        })
    }

    /// Stops notifications without tearing down the OS-level connection.
    pub async fn close(&self) {
        let _ = self.peripheral.unsubscribe(&self.notify).await;
        if let Some(b) = &self.battery {
            let _ = self.peripheral.unsubscribe(b).await;
        }
        for t in &self.tasks {
            t.abort();
        }
    }
}

impl Drop for Keyboard {
    fn drop(&mut self) {
        for t in &self.tasks {
            t.abort();
        }
    }
}
