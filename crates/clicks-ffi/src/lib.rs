//! Foreign bindings for `clicks-core`.
//!
//! The host platform owns the BLE connection and shuttles bytes:
//!
//! ```swift
//! let session = ClicksSession()
//! session.enqueueSync()
//! pump()                                   // write every frame from pollTransmit()
//! // CBPeripheralDelegate.didUpdateValueFor fff1:
//! let events = session.handleNotification(bytes: value)
//! pump()
//! ```

use std::sync::Mutex;

use clicks_core::keymap::SlotKind;
use clicks_core::registers::{self, RegisterKind};
use clicks_core::{gatt, hosts, keymap, HostKind, KeyAction, Request, Session, SessionEvent};

uniffi::setup_scaffolding!();

#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum ClicksError {
    #[error("{message}")]
    Invalid { message: String },
    #[error("state has not been read from the keyboard yet")]
    NotSynced,
}

impl From<clicks_core::ModelError> for ClicksError {
    fn from(e: clicks_core::ModelError) -> Self {
        match e {
            clicks_core::ModelError::NotSynced => ClicksError::NotSynced,
            other => ClicksError::Invalid { message: other.to_string() },
        }
    }
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct GattUuids {
    pub service: String,
    pub notify: String,
    pub write: String,
    pub battery_service: String,
    pub battery_level: String,
    pub device_info_service: String,
    pub model_number: String,
    pub serial_number: String,
    pub firmware_revision: String,
    pub name_prefix: String,
}

#[uniffi::export]
pub fn gatt_uuids() -> GattUuids {
    GattUuids {
        service: gatt::SERVICE.into(),
        notify: gatt::NOTIFY.into(),
        write: gatt::WRITE.into(),
        battery_service: gatt::BATTERY_SERVICE.into(),
        battery_level: gatt::BATTERY_LEVEL.into(),
        device_info_service: gatt::DEVICE_INFO_SERVICE.into(),
        model_number: gatt::MODEL_NUMBER.into(),
        serial_number: gatt::SERIAL_NUMBER.into(),
        firmware_revision: gatt::FIRMWARE_REVISION.into(),
        name_prefix: gatt::NAME_PREFIX.into(),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum FfiKeyAction {
    Key { usage: u8 },
    Consumer { usage: u8 },
    Combo { modifier: u8, usage: u8 },
}

impl From<KeyAction> for FfiKeyAction {
    fn from(a: KeyAction) -> Self {
        match a {
            KeyAction::Key { usage } => FfiKeyAction::Key { usage },
            KeyAction::Consumer { usage } => FfiKeyAction::Consumer { usage },
            KeyAction::Combo { modifier, usage } => FfiKeyAction::Combo { modifier, usage },
        }
    }
}

impl From<FfiKeyAction> for KeyAction {
    fn from(a: FfiKeyAction) -> Self {
        match a {
            FfiKeyAction::Key { usage } => KeyAction::Key { usage },
            FfiKeyAction::Consumer { usage } => KeyAction::Consumer { usage },
            FfiKeyAction::Combo { modifier, usage } => KeyAction::Combo { modifier, usage },
        }
    }
}

/// Parses `"Ctrl+Space"`, `"Escape"`, `"Play/Pause"`, `"key:0x29"`, `"consumer:0xcd"`.
#[uniffi::export]
pub fn parse_key_action(text: String) -> Result<FfiKeyAction, ClicksError> {
    text.parse::<KeyAction>()
        .map(Into::into)
        .map_err(|e| ClicksError::Invalid { message: e.to_string() })
}

#[uniffi::export]
pub fn describe_key_action(action: FfiKeyAction) -> String {
    KeyAction::from(action).describe()
}

#[uniffi::export]
pub fn factory_key_actions() -> Vec<Option<FfiKeyAction>> {
    keymap::FACTORY_DEFAULTS.iter().map(|a| a.map(Into::into)).collect()
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct FlagBitInfo {
    pub bit: u8,
    pub id: String,
    pub label: String,
    pub note: String,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum FfiRegisterKind {
    Level { min: u16, max: u16, step: u16, unit: String },
    Duration { unit_ms: u32, presets: Vec<u16> },
    Flags { bits: Vec<FlagBitInfo> },
    ReadOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum FfiSlotKind {
    /// Tab/Gemini/Alt/Backspace: a key or modifier+key combo.
    Remap,
    /// Sym + digit: a single key or media key.
    Shortcut,
}

impl From<SlotKind> for FfiSlotKind {
    fn from(k: SlotKind) -> Self {
        match k {
            SlotKind::Remap => FfiSlotKind::Remap,
            SlotKind::Shortcut => FfiSlotKind::Shortcut,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum FfiHostKind {
    Empty,
    Unknown,
    Ios,
    Android,
}

impl From<HostKind> for FfiHostKind {
    fn from(k: HostKind) -> Self {
        match k {
            HostKind::Empty => FfiHostKind::Empty,
            HostKind::Unknown => FfiHostKind::Unknown,
            HostKind::Ios => FfiHostKind::Ios,
            HostKind::Android => FfiHostKind::Android,
        }
    }
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct RegisterValue {
    pub id: String,
    pub addr: u8,
    pub label: String,
    pub note: String,
    pub kind: FfiRegisterKind,
    pub value: Option<u32>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct KeySlotValue {
    pub index: u8,
    pub kind: FfiSlotKind,
    /// e.g. "Tab key", "Sym + 3".
    pub label: String,
    /// What the key does while disabled.
    pub native: String,
    pub enabled: bool,
    pub action: Option<FfiKeyAction>,
    pub description: Option<String>,
    /// Choices the official app offers; `None` entries mean "disabled".
    pub presets: Vec<Option<FfiKeyAction>>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct HostValue {
    /// 1-based.
    pub slot: u8,
    pub kind: FfiHostKind,
    pub name: Option<String>,
    /// Custom name, else "iOS"/"Android"; empty for an empty slot.
    pub display_name: String,
    pub active: bool,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct KeyboardSnapshot {
    pub synced: bool,
    pub settings: Vec<RegisterValue>,
    pub info: Vec<RegisterValue>,
    pub key_slots: Vec<KeySlotValue>,
    pub hosts: Vec<HostValue>,
    pub wireless_charging: Option<bool>,
    pub status: Option<Vec<u8>>,
}

#[derive(Debug, Clone, uniffi::Enum)]
pub enum FfiSessionEvent {
    /// A request completed and the snapshot changed.
    Applied { opcode: u8 },
    Rejected { opcode: u8, status: u8 },
    Stray { opcode: u8 },
    Unsolicited { payload: Vec<u8> },
    FrameError { message: String },
}

impl From<SessionEvent> for FfiSessionEvent {
    fn from(e: SessionEvent) -> Self {
        match e {
            SessionEvent::Applied { request, .. } => FfiSessionEvent::Applied { opcode: request.opcode() },
            SessionEvent::Rejected { request, status } => FfiSessionEvent::Rejected { opcode: request.opcode(), status },
            SessionEvent::Stray(r) => FfiSessionEvent::Stray { opcode: r.opcode },
            SessionEvent::Unsolicited(payload) => FfiSessionEvent::Unsolicited { payload },
            SessionEvent::FrameError(e) => FfiSessionEvent::FrameError { message: e.to_string() },
        }
    }
}

fn register_value(info: &registers::RegisterInfo, value: Option<u32>) -> RegisterValue {
    RegisterValue {
        id: info.id.into(),
        addr: info.addr,
        label: info.label.into(),
        note: info.note.into(),
        kind: match info.kind {
            RegisterKind::Level { min, max, step, unit } => FfiRegisterKind::Level { min, max, step, unit: unit.into() },
            RegisterKind::Duration { unit_ms, presets } => FfiRegisterKind::Duration { unit_ms, presets: presets.to_vec() },
            RegisterKind::Flags { bits } => FfiRegisterKind::Flags {
                bits: bits
                    .iter()
                    .map(|b| FlagBitInfo { bit: b.bit, id: b.id.into(), label: b.label.into(), note: b.note.into() })
                    .collect(),
            },
            RegisterKind::ReadOnly => FfiRegisterKind::ReadOnly,
        },
        value,
    }
}

/// Thread-safe wrapper around [`clicks_core::Session`].
#[derive(uniffi::Object, Default)]
pub struct ClicksSession {
    inner: Mutex<Session>,
}

impl ClicksSession {
    fn with<T>(&self, f: impl FnOnce(&mut Session) -> T) -> T {
        f(&mut self.inner.lock().unwrap_or_else(|p| p.into_inner()))
    }
}

#[uniffi::export]
impl ClicksSession {
    #[uniffi::constructor]
    pub fn new() -> Self {
        Self::default()
    }

    /// Queues the full read sequence (settings, key slots, host table).
    pub fn enqueue_sync(&self) {
        self.with(|s| s.enqueue_sync());
    }

    pub fn enqueue_host_table(&self) {
        self.with(|s| s.enqueue(clicks_core::model::host_table_plan()));
    }

    pub fn set_register(&self, id: String, value: u32) -> Result<(), ClicksError> {
        self.with(|s| s.enqueue_with(|st| st.set_register(&id, value)))?;
        Ok(())
    }

    pub fn set_flag(&self, bit: u8, on: bool) -> Result<(), ClicksError> {
        self.with(|s| s.enqueue_with(|st| st.set_flag(bit, on)))?;
        Ok(())
    }

    pub fn set_wireless_charging(&self, on: bool) {
        self.with(|s| {
            let reqs = s.projected_state().set_wireless_charging(on);
            s.enqueue(reqs);
        });
    }

    /// `slot` is 1-based; an empty name restores the default label.
    pub fn set_host_name(&self, slot: u8, name: String) -> Result<(), ClicksError> {
        self.with(|s| s.enqueue_with(|st| st.set_host_name(slot, &name)))?;
        Ok(())
    }

    /// Forgets a paired host (fails for the active one).
    pub fn remove_host(&self, slot: u8) -> Result<(), ClicksError> {
        self.with(|s| s.enqueue_with(|st| st.remove_host(slot)))?;
        Ok(())
    }

    /// `None` disables the slot.
    pub fn set_key_slot(&self, index: u8, action: Option<FfiKeyAction>) -> Result<(), ClicksError> {
        self.with(|s| s.enqueue_with(|st| st.set_key_slot(index, action.map(Into::into))))?;
        Ok(())
    }

    pub fn reset_key_slots(&self) {
        self.with(|s| {
            let reqs = s.projected_state().reset_key_slots();
            s.enqueue(reqs);
        });
    }

    /// Queues an unmodelled request (`opcode` + raw args).
    pub fn enqueue_raw(&self, opcode: u8, args: Vec<u8>) {
        self.with(|s| s.enqueue([Request::Raw { opcode, args }]));
    }

    /// Next frame to write to the `fff2` characteristic, or `None` while a
    /// request is in flight or the queue is empty.
    pub fn poll_transmit(&self) -> Option<Vec<u8>> {
        self.with(|s| s.poll_transmit())
    }

    /// Feed every `fff1` notification here.
    pub fn handle_notification(&self, bytes: Vec<u8>) -> Vec<FfiSessionEvent> {
        self.with(|s| s.handle_notification(&bytes)).into_iter().map(Into::into).collect()
    }

    /// Call when no response arrived in time (the official app's cadence suggests ~2–3 s).
    /// Returns whether a request was abandoned.
    pub fn handle_timeout(&self) -> bool {
        self.with(|s| s.handle_timeout()).is_some()
    }

    pub fn pending(&self) -> u32 {
        self.with(|s| s.pending() as u32)
    }

    /// Call after a disconnect.
    pub fn reset(&self) {
        self.with(|s| s.reset());
    }

    pub fn snapshot(&self) -> KeyboardSnapshot {
        self.with(|s| {
            let st = s.state();
            KeyboardSnapshot {
                synced: st.is_synced(),
                settings: registers::SETTINGS.iter().map(|r| register_value(r, st.register(r))).collect(),
                info: registers::INFO.iter().map(|r| register_value(r, st.register(r))).collect(),
                key_slots: st
                    .key_slots()
                    .into_iter()
                    .map(|k| {
                        let info = &keymap::SLOTS[k.index as usize];
                        KeySlotValue {
                            index: k.index,
                            kind: k.kind.into(),
                            label: k.label.into(),
                            native: info.native.into(),
                            enabled: k.enabled,
                            action: k.action.map(Into::into),
                            description: k.description,
                            presets: info.presets.iter().map(|p| p.map(Into::into)).collect(),
                        }
                    })
                    .collect(),
                hosts: st
                    .hosts()
                    .into_iter()
                    .map(|h| HostValue {
                        slot: h.slot,
                        kind: h.kind.into(),
                        display_name: h.display_name(),
                        name: h.name,
                        active: h.active,
                    })
                    .collect(),
                wireless_charging: st.wireless_charging(),
                status: st.status().map(<[u8]>::to_vec),
            }
        })
    }
}

#[uniffi::export]
pub fn host_table_len() -> u16 {
    hosts::TABLE_LEN
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct FirmwareRecord {
    pub model: String,
    pub version: String,
    pub url: String,
    pub sha256: String,
}

impl From<clicks_core::firmware::FirmwareInfo> for FirmwareRecord {
    fn from(f: clicks_core::firmware::FirmwareInfo) -> Self {
        FirmwareRecord { model: f.model, version: f.version, url: f.url, sha256: f.sha256 }
    }
}

impl From<FirmwareRecord> for clicks_core::firmware::FirmwareInfo {
    fn from(f: FirmwareRecord) -> Self {
        clicks_core::firmware::FirmwareInfo { model: f.model, version: f.version, url: f.url, sha256: f.sha256 }
    }
}

/// Manifest the official app polls for Power Keyboard firmware.
#[uniffi::export]
pub fn firmware_manifest_url() -> String {
    clicks_core::firmware::MANIFEST_URL.into()
}

#[uniffi::export]
pub fn parse_firmware_manifest(json: String) -> Result<Vec<FirmwareRecord>, ClicksError> {
    clicks_core::firmware::parse_manifest(&json)
        .map(|m| m.into_iter().map(Into::into).collect())
        .map_err(|e| ClicksError::Invalid { message: e.to_string() })
}

/// Newest image for `model` (GATT 2a24) newer than `installed` (2a26).
#[uniffi::export]
pub fn select_firmware_update(manifest: Vec<FirmwareRecord>, model: String, installed: String) -> Option<FirmwareRecord> {
    let m: Vec<clicks_core::firmware::FirmwareInfo> = manifest.into_iter().map(Into::into).collect();
    clicks_core::firmware::select_update(&m, &model, &installed).cloned().map(Into::into)
}

#[uniffi::export]
pub fn latest_firmware(manifest: Vec<FirmwareRecord>, model: String) -> Option<FirmwareRecord> {
    let m: Vec<clicks_core::firmware::FirmwareInfo> = manifest.into_iter().map(Into::into).collect();
    clicks_core::firmware::latest(&m, &model).cloned().map(Into::into)
}

/// Checks a downloaded image against the manifest's SHA-256.
#[uniffi::export]
pub fn verify_firmware(image: Vec<u8>, sha256: String) -> bool {
    clicks_core::firmware::verify(&image, &sha256)
}
