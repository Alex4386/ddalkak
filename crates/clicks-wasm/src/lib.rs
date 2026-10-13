//! WebAssembly bindings for `clicks-core`.
//!
//! The page owns the Web Bluetooth connection and shuttles bytes, exactly like
//! the Swift/Kotlin hosts of `clicks-ffi`:
//!
//! ```js
//! const session = new ClicksSession();
//! session.enqueueSync();
//! let frame;
//! while ((frame = session.pollTransmit())) {
//!   await fff2.writeValueWithResponse(frame);
//!   // fff1 `characteristicvaluechanged` → session.handleNotification(bytes)
//! }
//! ```
//!
//! Structured values cross the boundary as plain JS objects with the same
//! shape the Tauri backend returns (see `clicks_core::view`).

use clicks_core::firmware::{self, FirmwareInfo};
use clicks_core::{gatt, view, KeyAction, Os, Request, Session, SessionEvent};
use serde::Serialize;
use wasm_bindgen::prelude::*;

fn to_js<T: Serialize + ?Sized>(value: &T) -> JsValue {
    value
        .serialize(&serde_wasm_bindgen::Serializer::json_compatible())
        .expect("plain data serializes")
}

fn js_err(e: impl std::fmt::Display) -> JsError {
    JsError::new(&e.to_string())
}

fn parse_os(os: &str) -> Result<Os, JsError> {
    os.parse().map_err(js_err)
}

/// Session outcome, tagged by `type` for the page.
#[derive(Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
enum Event {
    Applied { opcode: u8, status: u8, data: Vec<u8> },
    Rejected { opcode: u8, status: u8 },
    Stray { opcode: u8 },
    Unsolicited { payload: Vec<u8> },
    FrameError { message: String },
}

impl From<SessionEvent> for Event {
    fn from(e: SessionEvent) -> Self {
        match e {
            SessionEvent::Applied { response, .. } => {
                Event::Applied { opcode: response.opcode, status: response.status, data: response.data }
            }
            SessionEvent::Rejected { request, status } => Event::Rejected { opcode: request.opcode(), status },
            SessionEvent::Stray(r) => Event::Stray { opcode: r.opcode },
            SessionEvent::Unsolicited(payload) => Event::Unsolicited { payload },
            SessionEvent::FrameError(e) => Event::FrameError { message: e.to_string() },
        }
    }
}

#[wasm_bindgen]
#[derive(Default)]
pub struct ClicksSession {
    inner: Session,
}

#[wasm_bindgen]
impl ClicksSession {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self::default()
    }

    /// Queues the full read sequence (settings, key slots, host table).
    #[wasm_bindgen(js_name = enqueueSync)]
    pub fn enqueue_sync(&mut self) {
        self.inner.enqueue_sync();
    }

    #[wasm_bindgen(js_name = setRegister)]
    pub fn set_register(&mut self, id: &str, value: u32) -> Result<(), JsError> {
        self.inner.enqueue_with(|st| st.set_register(id, value)).map(drop).map_err(js_err)
    }

    #[wasm_bindgen(js_name = setFlag)]
    pub fn set_flag(&mut self, bit: u8, on: bool) -> Result<(), JsError> {
        self.inner.enqueue_with(|st| st.set_flag(bit, on)).map(drop).map_err(js_err)
    }

    #[wasm_bindgen(js_name = setWirelessCharging)]
    pub fn set_wireless_charging(&mut self, on: bool) {
        let reqs = self.inner.projected_state().set_wireless_charging(on);
        self.inner.enqueue(reqs);
    }

    /// `slot` is 1-based; an empty name restores the default label.
    #[wasm_bindgen(js_name = setHostName)]
    pub fn set_host_name(&mut self, slot: u8, name: &str) -> Result<(), JsError> {
        self.inner.enqueue_with(|st| st.set_host_name(slot, name)).map(drop).map_err(js_err)
    }

    /// Switches a paired host between the `"ios"` and `"android"` key tables.
    #[wasm_bindgen(js_name = setHostOs)]
    pub fn set_host_os(&mut self, slot: u8, os: &str) -> Result<(), JsError> {
        let os = parse_os(os)?;
        self.inner.enqueue_with(|st| st.set_host_os(slot, os)).map(drop).map_err(js_err)
    }

    /// Forgets a paired host (fails for the active one).
    #[wasm_bindgen(js_name = removeHost)]
    pub fn remove_host(&mut self, slot: u8) -> Result<(), JsError> {
        self.inner.enqueue_with(|st| st.remove_host(slot)).map(drop).map_err(js_err)
    }

    /// Edits `os`'s key table (`"ios"` or `"android"`). `action` is parsed
    /// like the CLI (`"Ctrl+Space"`, `"Play/Pause"`, `"key:0x29"`);
    /// `undefined` disables the slot.
    #[wasm_bindgen(js_name = setKeySlot)]
    pub fn set_key_slot(&mut self, os: &str, index: u8, action: Option<String>) -> Result<(), JsError> {
        let os = parse_os(os)?;
        let action = action.map(|a| a.parse::<KeyAction>()).transpose().map_err(js_err)?;
        self.inner.enqueue_with(|st| st.set_key_slot(os, index, action)).map(drop).map_err(js_err)
    }

    #[wasm_bindgen(js_name = resetKeySlots)]
    pub fn reset_key_slots(&mut self, os: &str) -> Result<(), JsError> {
        let reqs = self.inner.projected_state().reset_key_slots(parse_os(os)?);
        self.inner.enqueue(reqs);
        Ok(())
    }

    /// Queues an unmodelled request (`opcode` + raw args).
    #[wasm_bindgen(js_name = enqueueRaw)]
    pub fn enqueue_raw(&mut self, opcode: u8, args: Vec<u8>) {
        self.inner.enqueue([Request::Raw { opcode, args }]);
    }

    /// Next frame to write to `fff2`, or `undefined` while a request is in
    /// flight or the queue is empty.
    #[wasm_bindgen(js_name = pollTransmit)]
    pub fn poll_transmit(&mut self) -> Option<Vec<u8>> {
        self.inner.poll_transmit()
    }

    /// Feed every `fff1` notification here. Returns the resulting events.
    #[wasm_bindgen(js_name = handleNotification)]
    pub fn handle_notification(&mut self, bytes: &[u8]) -> JsValue {
        let events: Vec<Event> = self.inner.handle_notification(bytes).into_iter().map(Into::into).collect();
        to_js(&events)
    }

    /// Gives up on the in-flight request; returns its opcode, if any.
    #[wasm_bindgen(js_name = handleTimeout)]
    pub fn handle_timeout(&mut self) -> Option<u8> {
        self.inner.handle_timeout().map(|r| r.opcode())
    }

    /// Drops queued (not yet sent) requests.
    #[wasm_bindgen(js_name = clearQueue)]
    pub fn clear_queue(&mut self) {
        self.inner.clear_queue();
    }

    pub fn pending(&self) -> usize {
        self.inner.pending()
    }

    /// Call after a disconnect.
    pub fn reset(&mut self) {
        self.inner.reset();
    }

    /// Same shape as the Tauri backend's snapshot.
    pub fn snapshot(&self) -> JsValue {
        to_js(&view::snapshot(self.inner.state()))
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GattUuids {
    service: &'static str,
    notify: &'static str,
    write: &'static str,
    battery_service: &'static str,
    battery_level: &'static str,
    device_info_service: &'static str,
    model_number: &'static str,
    serial_number: &'static str,
    firmware_revision: &'static str,
    software_revision: &'static str,
    name_prefix: &'static str,
}

#[wasm_bindgen(js_name = gattUuids)]
pub fn gatt_uuids() -> JsValue {
    to_js(&GattUuids {
        service: gatt::SERVICE,
        notify: gatt::NOTIFY,
        write: gatt::WRITE,
        battery_service: gatt::BATTERY_SERVICE,
        battery_level: gatt::BATTERY_LEVEL,
        device_info_service: gatt::DEVICE_INFO_SERVICE,
        model_number: gatt::MODEL_NUMBER,
        serial_number: gatt::SERIAL_NUMBER,
        firmware_revision: gatt::FIRMWARE_REVISION,
        software_revision: gatt::SOFTWARE_REVISION,
        name_prefix: gatt::NAME_PREFIX,
    })
}

/// Names the key-slot editor offers as suggestions.
#[wasm_bindgen(js_name = keyNames)]
pub fn key_names() -> Vec<String> {
    view::key_names()
}

#[wasm_bindgen(js_name = firmwareManifestUrl)]
pub fn firmware_manifest_url() -> String {
    firmware::MANIFEST_URL.into()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FirmwareStatus {
    model: String,
    installed: Option<String>,
    latest: Option<FirmwareInfo>,
    update: Option<FirmwareInfo>,
}

/// Same shape as the Tauri backend's `check_firmware` result.
#[wasm_bindgen(js_name = firmwareStatus)]
pub fn firmware_status(manifest_json: &str, model: &str, installed: Option<String>) -> Result<JsValue, JsError> {
    let manifest =
        firmware::parse_manifest(manifest_json).map_err(|e| js_err(format!("unexpected manifest format: {e}")))?;
    Ok(to_js(&FirmwareStatus {
        latest: firmware::latest(&manifest, model).cloned(),
        update: installed.as_deref().and_then(|v| firmware::select_update(&manifest, model, v)).cloned(),
        model: model.into(),
        installed,
    }))
}

/// Checks a downloaded image against the manifest's SHA-256.
#[wasm_bindgen(js_name = verifyFirmware)]
pub fn verify_firmware(image: &[u8], sha256: &str) -> bool {
    firmware::verify(image, sha256)
}

#[wasm_bindgen(js_name = firmwareFileName)]
pub fn firmware_file_name(model: String, version: String) -> String {
    firmware::file_name(&FirmwareInfo { model, version, url: String::new(), sha256: String::new() })
}
