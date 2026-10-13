//! Serializable snapshot of [`KeyboardState`] for UIs. The Tauri app and the
//! WebAssembly build both hand this to the same frontend, so its JSON shape is
//! shared.

use serde::Serialize;

use crate::keymap::{self, KeyAction, Os};
use crate::registers::{self, RegisterInfo};
use crate::{Host, KeySlot, KeyboardState};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegisterView {
    #[serde(flatten)]
    pub info: RegisterInfo,
    pub value: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SlotView {
    #[serde(flatten)]
    pub slot: KeySlot,
    pub addr: u8,
    pub native: &'static str,
    pub factory: Option<String>,
    /// Official choices as parseable names; `None` = disabled.
    pub presets: Vec<Option<String>>,
}

/// The keyboard's two key tables; it uses the active host's.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyTables {
    pub ios: Vec<SlotView>,
    pub android: Vec<SlotView>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HostView {
    #[serde(flatten)]
    pub host: Host,
    pub display_name: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub synced: bool,
    pub settings: Vec<RegisterView>,
    pub info: Vec<RegisterView>,
    pub key_slots: KeyTables,
    /// Which table the keyboard is using: the active host's.
    pub active_os: Option<Os>,
    pub hosts: Vec<HostView>,
    pub wireless_charging: Option<bool>,
    pub status: Option<Vec<u8>>,
}

pub fn snapshot(state: &KeyboardState) -> Snapshot {
    let reg = |r: &RegisterInfo| RegisterView { info: *r, value: state.register(r) };
    Snapshot {
        synced: state.is_synced(),
        settings: registers::SETTINGS.iter().map(reg).collect(),
        info: registers::INFO.iter().map(reg).collect(),
        key_slots: KeyTables { ios: slot_views(state, Os::Ios), android: slot_views(state, Os::Android) },
        active_os: state.active_os(),
        hosts: state.hosts().into_iter().map(|host| HostView { display_name: host.display_name(), host }).collect(),
        wireless_charging: state.wireless_charging(),
        status: state.status().map(<[u8]>::to_vec),
    }
}

fn slot_views(state: &KeyboardState, os: Os) -> Vec<SlotView> {
    state
        .key_slots(os)
        .into_iter()
        .map(|slot| {
            let info = &keymap::SLOTS[slot.index as usize];
            SlotView {
                addr: keymap::slot_addr(os, slot.index),
                native: info.native,
                factory: keymap::factory_defaults(os)[slot.index as usize].map(KeyAction::describe),
                presets: info.presets(os).iter().map(|p| p.map(KeyAction::describe)).collect(),
                slot,
            }
        })
        .collect()
}

/// Names a key-slot editor can offer as suggestions.
pub fn key_names() -> Vec<String> {
    let keys: Vec<&str> = (0..=0xFFu8)
        .filter(|&u| !keymap::is_modifier(u))
        .filter_map(keymap::keyboard_usage_name)
        .collect();
    let consumer = (0..=0xFFu8).filter_map(keymap::consumer_usage_name).filter(|n| !keys.contains(n));
    keys.iter().copied().chain(consumer).map(str::to_owned).collect()
}
