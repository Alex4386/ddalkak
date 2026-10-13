//! Device state mirror plus the request sequences that read and change it.
//!
//! Transports (btleplug, CoreBluetooth, ...) send each [`Request`], wait for the
//! [`Response`] it [`matches`](Request::matches), and feed both to
//! [`KeyboardState::apply`].

use std::collections::BTreeMap;

use crate::hosts::{self, Host, HostKind};
use crate::keymap::{self, KeyAction, Os, SlotKind, SLOT_COUNT};
use crate::protocol::{le_value, Request, Response, CONTROL_WIRELESS_CHARGING};
use crate::registers::{self, RegisterInfo};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct KeyboardState {
    /// Register bytes by address, as last read or successfully written.
    bytes: BTreeMap<u8, u8>,
    status: Option<Vec<u8>>,
    host_table: Vec<Option<u8>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub struct KeySlot {
    pub os: Os,
    pub index: u8,
    pub kind: SlotKind,
    pub label: &'static str,
    pub enabled: bool,
    pub action: Option<KeyAction>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelError {
    UnknownRegister(String),
    OutOfRange { id: &'static str, value: u32 },
    SlotOutOfRange(u8),
    UnsupportedAction { slot: &'static str, action: KeyAction },
    HostOutOfRange(u8),
    /// Only bonded hosts can switch OS; the firmware drops the bond bit from
    /// writes, so an empty slot can't be made into one.
    HostNotPaired(u8),
    /// The official app refuses to clear the slot the keyboard is using.
    ActiveHost(u8),
    /// The enable mask must be read before a slot can be toggled.
    NotSynced,
}

impl std::fmt::Display for ModelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ModelError::UnknownRegister(id) => write!(f, "unknown register {id:?}"),
            ModelError::OutOfRange { id, value } => write!(f, "{value} is out of range for {id}"),
            ModelError::SlotOutOfRange(i) => write!(f, "key slot {i} does not exist"),
            ModelError::UnsupportedAction { slot, action } => {
                let what = match action {
                    KeyAction::Combo { .. } => "key combos",
                    KeyAction::Consumer { .. } => "media keys",
                    KeyAction::Key { .. } => "this key",
                };
                write!(f, "{slot} cannot be assigned {what}")
            }
            ModelError::HostOutOfRange(s) => {
                write!(f, "host slot {s} does not exist (1–{})", hosts::SLOT_COUNT)
            }
            ModelError::HostNotPaired(s) => write!(f, "host slot {s} has no paired device"),
            ModelError::ActiveHost(s) => {
                write!(f, "host slot {s} is in use; switch to another host first")
            }
            ModelError::NotSynced => write!(f, "state has not been read from the keyboard yet"),
        }
    }
}

impl std::error::Error for ModelError {}

/// The reads the official app performs right after connecting, in the same
/// order, then the iOS key table, which the Android app never reads.
pub fn sync_plan() -> Vec<Request> {
    let reg = |r: &RegisterInfo| Request::ReadRegister { addr: r.addr, len: r.len };
    let mut plan = vec![
        reg(&registers::FEATURES),
        reg(&registers::IDLE_TIMEOUT),
        reg(&registers::BRIGHTNESS),
        reg(&registers::DIM_TIMEOUT),
        Request::ReadStatus,
        reg(&registers::CHARGE_RESERVE),
    ];
    plan.extend(key_table_plan(Os::Android));
    plan.extend(registers::INFO.iter().map(reg));
    plan.extend(host_table_plan());
    plan.extend(key_table_plan(Os::Ios));
    plan
}

pub fn key_table_plan(os: Os) -> Vec<Request> {
    let mask = os.mask_addr();
    let mut plan = vec![Request::ReadRegister { addr: mask, len: 1 }, Request::ReadRegister { addr: mask + 1, len: 1 }];
    plan.extend((0..SLOT_COUNT).map(|i| Request::ReadRegister {
        addr: keymap::slot_addr(os, i),
        len: keymap::SLOT_VALUE_LEN,
    }));
    plan
}

pub fn host_table_plan() -> Vec<Request> {
    (0..hosts::TABLE_LEN)
        .step_by(hosts::READ_CHUNK as usize)
        .map(|offset| Request::ReadBlock { offset, len: hosts::READ_CHUNK })
        .collect()
}

impl KeyboardState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Records the outcome of `request`. Failed responses are ignored.
    pub fn apply(&mut self, request: &Request, response: &Response) {
        if !response.is_ok() || !request.matches(response) {
            return;
        }
        match request {
            Request::ReadRegister { addr, .. } => self.store(*addr, &response.data),
            Request::WriteRegister { addr, data } => self.store(*addr, data),
            Request::ReadStatus => self.status = Some(response.data.clone()),
            Request::SetControl { mask, value } => {
                if let Some(bits) = self.control_bits() {
                    let next = (bits & !mask) | (value & mask);
                    if let Some(s) = &mut self.status {
                        s[2..4].copy_from_slice(&next.to_be_bytes());
                    }
                }
            }
            Request::ReadBlock { offset, .. } => self.store_block(*offset, &response.data),
            Request::WriteBlock { offset, data } => self.store_block(*offset, data),
            Request::Raw { .. } => {}
        }
    }

    fn store_block(&mut self, offset: u16, data: &[u8]) {
        let start = offset as usize;
        let end = start + data.len();
        if self.host_table.len() < end {
            self.host_table.resize(end, None);
        }
        for (slot, &b) in self.host_table[start..end].iter_mut().zip(data) {
            *slot = Some(b);
        }
    }

    fn store(&mut self, addr: u8, data: &[u8]) {
        for (i, &b) in data.iter().enumerate() {
            self.bytes.insert(addr.wrapping_add(i as u8), b);
        }
    }

    pub fn byte(&self, addr: u8) -> Option<u8> {
        self.bytes.get(&addr).copied()
    }

    fn read(&self, addr: u8, len: u8) -> Option<Vec<u8>> {
        (0..len).map(|i| self.byte(addr.wrapping_add(i))).collect()
    }

    pub fn register(&self, info: &RegisterInfo) -> Option<u32> {
        self.read(info.addr, info.len).map(|d| le_value(&d))
    }

    pub fn register_by_id(&self, id: &str) -> Option<u32> {
        registers::by_id(id).and_then(|r| self.register(r))
    }

    pub fn status(&self) -> Option<&[u8]> {
        self.status.as_deref()
    }

    /// The bits [`Request::SetControl`] changes: the last two status bytes.
    pub fn control_bits(&self) -> Option<u16> {
        match self.status.as_deref()? {
            [_, _, hi, lo, ..] => Some(u16::from_be_bytes([*hi, *lo])),
            _ => None,
        }
    }

    pub fn wireless_charging(&self) -> Option<bool> {
        self.control_bits().map(|b| b & CONTROL_WIRELESS_CHARGING != 0)
    }

    pub fn key_slot(&self, os: Os, index: u8) -> Option<KeySlot> {
        if index >= SLOT_COUNT {
            return None;
        }
        let info = keymap::slot(index)?;
        let (mask_addr, bit) = keymap::enable_bit(os, index);
        let enabled = self.byte(mask_addr)? & (1 << bit) != 0;
        let raw = self.read(keymap::slot_addr(os, index), keymap::SLOT_VALUE_LEN)?;
        let action = KeyAction::from_bytes([raw[0], raw[1]]);
        Some(KeySlot {
            os,
            index,
            kind: info.kind,
            label: info.label,
            enabled,
            action,
            description: action.map(KeyAction::describe),
        })
    }

    pub fn key_slots(&self, os: Os) -> Vec<KeySlot> {
        (0..SLOT_COUNT).filter_map(|i| self.key_slot(os, i)).collect()
    }

    pub fn active_host(&self) -> Option<u8> {
        self.byte(hosts::ACTIVE_ADDR)
    }

    /// The key table in use right now: the active host's.
    pub fn active_os(&self) -> Option<Os> {
        self.active_host().and_then(|slot| self.host_kind(slot)).map(HostKind::os)
    }

    pub fn host_kind(&self, slot: u8) -> Option<HostKind> {
        let (addr, shift) = hosts::kind_location(slot);
        self.byte(addr).map(|b| HostKind::from_bits(b >> shift))
    }

    fn host_name(&self, slot: u8) -> Option<String> {
        let start = hosts::record_offset(slot) as usize;
        let rec: Option<Vec<u8>> = self.host_table.get(start..start + hosts::RECORD_LEN)?.iter().copied().collect();
        hosts::parse_record(&rec?)
    }

    /// Every host slot whose type register has been read.
    pub fn hosts(&self) -> Vec<Host> {
        let active = self.active_host();
        (1..=hosts::SLOT_COUNT)
            .filter_map(|slot| {
                Some(Host { slot, kind: self.host_kind(slot)?, name: self.host_name(slot), active: active == Some(slot) })
            })
            .collect()
    }

    pub fn is_synced(&self) -> bool {
        sync_plan().iter().all(|req| match req {
            Request::ReadRegister { addr, len } => self.read(*addr, *len).is_some(),
            Request::ReadStatus => self.status.is_some(),
            Request::ReadBlock { offset, len } => {
                let end = *offset as usize + *len as usize;
                self.host_table.len() >= end && self.host_table[*offset as usize..end].iter().all(Option::is_some)
            }
            _ => true,
        })
    }

    /// Requests that set a register from [`registers::SETTINGS`].
    pub fn set_register(&self, id: &str, value: u32) -> Result<Vec<Request>, ModelError> {
        let info = registers::SETTINGS
            .iter()
            .find(|r| r.id == id)
            .ok_or_else(|| ModelError::UnknownRegister(id.to_owned()))?;
        let max = if info.len == 1 { 0xFF } else { 0xFFFF };
        let in_range = match info.kind {
            registers::RegisterKind::Level { min, max, .. } => (min as u32..=max as u32).contains(&value),
            _ => value <= max,
        };
        if !in_range {
            return Err(ModelError::OutOfRange { id: info.id, value });
        }
        Ok(vec![match info.len {
            1 => Request::write_u8(info.addr, value as u8),
            _ => Request::write_u16(info.addr, value as u16),
        }])
    }

    /// Turns a [`registers::FEATURES`] bit on or off.
    pub fn set_flag(&self, bit: u8, on: bool) -> Result<Vec<Request>, ModelError> {
        if bit > 7 {
            return Err(ModelError::OutOfRange { id: registers::FEATURES.id, value: bit as u32 });
        }
        let addr = registers::FEATURES.addr;
        let current = self.byte(addr).ok_or(ModelError::NotSynced)?;
        let next = if on { current | (1 << bit) } else { current & !(1 << bit) };
        Ok(vec![Request::write_u8(addr, next)])
    }

    pub fn set_wireless_charging(&self, on: bool) -> Vec<Request> {
        let mask = CONTROL_WIRELESS_CHARGING;
        vec![Request::SetControl { mask, value: if on { mask } else { 0 } }]
    }

    fn check_host(slot: u8) -> Result<(), ModelError> {
        match slot {
            1..=hosts::SLOT_COUNT => Ok(()),
            _ => Err(ModelError::HostOutOfRange(slot)),
        }
    }

    /// Names a host slot; an empty name restores the default label.
    pub fn set_host_name(&self, slot: u8, name: &str) -> Result<Vec<Request>, ModelError> {
        Self::check_host(slot)?;
        Ok(vec![hosts::write_name(slot, name.trim())])
    }

    pub fn set_host_kind(&self, slot: u8, kind: HostKind) -> Result<Vec<Request>, ModelError> {
        Self::check_host(slot)?;
        let (addr, shift) = hosts::kind_location(slot);
        let current = self.byte(addr).ok_or(ModelError::NotSynced)?;
        Ok(vec![Request::write_u8(addr, (current & !(3 << shift)) | (kind.bits() << shift))])
    }

    /// Switches a paired host between the iOS and Android key tables. Only
    /// bit 0 changes: clearing the bond bit would unpair the host, and the
    /// firmware refuses to set it again over the protocol.
    pub fn set_host_os(&self, slot: u8, os: Os) -> Result<Vec<Request>, ModelError> {
        Self::check_host(slot)?;
        let kind = self.host_kind(slot).ok_or(ModelError::NotSynced)?;
        if !kind.is_paired() {
            return Err(ModelError::HostNotPaired(slot));
        }
        self.set_host_kind(slot, HostKind::paired(os))
    }

    /// Forgets a paired host, like the official app's "Remove Device": clears
    /// its name, then marks the slot empty.
    pub fn remove_host(&self, slot: u8) -> Result<Vec<Request>, ModelError> {
        Self::check_host(slot)?;
        if self.active_host() == Some(slot) {
            return Err(ModelError::ActiveHost(slot));
        }
        let mut reqs = self.set_host_name(slot, "")?;
        reqs.extend(self.set_host_kind(slot, HostKind::Empty)?);
        Ok(reqs)
    }

    /// Assigns (`Some`) or disables (`None`) a key slot. Mirrors the official
    /// app: the enable mask byte is written first, then the slot value.
    pub fn set_key_slot(&self, os: Os, index: u8, action: Option<KeyAction>) -> Result<Vec<Request>, ModelError> {
        let info = keymap::slot(index).ok_or(ModelError::SlotOutOfRange(index))?;
        let (mask_addr, bit) = keymap::enable_bit(os, index);
        let mask = self.byte(mask_addr).ok_or(ModelError::NotSynced)?;
        match action {
            None => Ok(vec![Request::write_u8(mask_addr, mask & !(1 << bit))]),
            Some(action) => {
                let bytes =
                    action.encode(info.kind).ok_or(ModelError::UnsupportedAction { slot: info.label, action })?;
                Ok(vec![
                    Request::write_u8(mask_addr, mask | (1 << bit)),
                    Request::WriteRegister { addr: keymap::slot_addr(os, index), data: bytes.to_vec() },
                ])
            }
        }
    }

    /// Restores every slot of `os`'s table to [`keymap::factory_defaults`].
    pub fn reset_key_slots(&self, os: Os) -> Vec<Request> {
        let mut mask: u16 = 0;
        let mut out = Vec::new();
        for (i, default) in keymap::factory_defaults(os).iter().enumerate() {
            let bytes = default.and_then(|a| a.encode(keymap::SLOTS[i].kind)).unwrap_or(KeyAction::UNASSIGNED);
            if default.is_some() {
                mask |= 1 << i;
            }
            out.push(Request::WriteRegister { addr: keymap::slot_addr(os, i as u8), data: bytes.to_vec() });
        }
        let [lo, hi] = mask.to_le_bytes();
        let hi_addr = os.mask_addr() + 1;
        let hi = (self.byte(hi_addr).unwrap_or(0) & !0x1F) | hi;
        out.push(Request::write_u8(os.mask_addr(), lo));
        out.push(Request::write_u8(hi_addr, hi));
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame;
    use crate::protocol::Message;

    fn hex(s: &str) -> Vec<u8> {
        (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap()).collect()
    }

    /// Request/response pairs from the capture, in order.
    const SESSION: &[(&str, &str)] = &[
        ("ff0425080111", "ff050225000573"),
        ("ff0425060269", "ff060225002c01a8"),
        ("ff042502016b", "ff05022500913d"),
        ("ff042504023e", "ff0602250088135f"),
        ("ff0227b6", "ff0802270000000002f9"),
        ("ff04250301f8", "ff050225001e7b"),
        ("ff0425120131", "ff05022500fc79"),
        ("ff04251301a2", "ff050225001f1f"),
        ("ff0425160233", "ff06022500ffff5a"),
        ("ff04251a02b0", "ff06022500ffff5a"),
        ("ff04251e021e", "ff06022500e20ea9"),
        ("ff0425220273", "ff06022500e350eb"),
        ("ff04252602dd", "ff060225002900f1"),
        ("ff04252a025e", "ff060225008000c9"),
        ("ff04252e02f0", "ff0602250081005a"),
        ("ff0425320229", "ff060225007f004b"),
        ("ff0425360287", "ff06022500cdffe3"),
        ("ff04253a0204", "ff06022500b5ff39"),
        ("ff04253e02aa", "ff06022500b6fffd"),
        ("ff04254202de", "ff060225004b000b"),
        ("ff0425460270", "ff060225004e0036"),
        ("ff04250c01bf", "ff05022500023e"),
        ("ff04250d012c", "ff050225008ed4"),
        ("ff04250e01e8", "ff050225000be9"),
        ("ff04250f017b", "ff0502250000f6"),
    ];

    fn response(wire: &str) -> Response {
        match Message::parse(&frame::decode(&hex(wire)).unwrap().unwrap()) {
            Message::Response(r) => r,
            other => panic!("{other:?}"),
        }
    }

    fn req_len(req: &Request) -> usize {
        match req {
            Request::ReadRegister { len, .. } => *len as usize,
            _ => 0,
        }
    }

    fn synced_state() -> KeyboardState {
        let plan = sync_plan();
        let mut state = KeyboardState::new();
        for (i, (req_wire, resp_wire)) in SESSION.iter().enumerate() {
            assert_eq!(plan[i].encode().unwrap(), hex(req_wire), "plan step {i}");
            state.apply(&plan[i], &response(resp_wire));
        }
        state
    }

    #[test]
    fn replays_captured_sync() {
        let state = synced_state();
        assert_eq!(state.register(&registers::FEATURES), Some(0x05));
        assert_eq!(state.register(&registers::IDLE_TIMEOUT), Some(300));
        assert_eq!(state.register(&registers::BRIGHTNESS), Some(0x91));
        assert_eq!(state.register(&registers::DIM_TIMEOUT), Some(5000));
        assert_eq!(state.register(&registers::CHARGE_RESERVE), Some(30));
        assert_eq!(state.status(), Some(&[0, 0, 0, 2][..]));
        assert_eq!(state.wireless_charging(), Some(false));
        let slots = state.key_slots(Os::Android);
        assert_eq!(slots.len(), 13);
        for slot in &slots {
            assert_eq!(slot.action, keymap::FACTORY_DEFAULTS[slot.index as usize]);
            assert_eq!(slot.enabled, slot.action.is_some());
        }
        assert!(!state.is_synced(), "host table not read yet");
    }

    #[test]
    fn key_slot_writes_match_official_app() {
        let state = synced_state();
        let reqs = state
            .set_key_slot(Os::Android, 0, Some(KeyAction::Combo { modifier: 0xE0, usage: 0x2C }))
            .unwrap();
        let wire: Vec<_> = reqs.iter().map(|r| r.encode().unwrap()).collect();
        assert_eq!(wire, vec![hex("ff042412fdbd"), hex("ff052416e02c49")]);
        assert_eq!(state.set_key_slot(Os::Android, 0, None).unwrap()[0].encode().unwrap(), hex("ff042412fcd9"));
        let reqs =
            state.set_key_slot(Os::Android, 1, Some(KeyAction::Combo { modifier: 0xE2, usage: 0x07 })).unwrap();
        assert_eq!(reqs[0].encode().unwrap(), hex("ff042412fe11"));
        assert_eq!(reqs[1].encode().unwrap(), hex("ff05241ae2073b"));
    }

    #[test]
    fn setting_writes_match_official_app() {
        let state = synced_state();
        let enc = |r: Vec<Request>| r[0].encode().unwrap();
        assert_eq!(enc(state.set_register("idleTimeout", 3600).unwrap()), hex("ff052406100e5a"));
        assert_eq!(enc(state.set_register("chargeReserve", 50).unwrap()), hex("ff042403326e"));
        assert_eq!(enc(state.set_register("brightness", 0xe5).unwrap()), hex("ff042402e543"));
        assert_eq!(enc(state.set_flag(4, true).unwrap()), hex("ff04240815e4"));
        assert_eq!(enc(state.set_flag(5, true).unwrap()), hex("ff04240825dd"));
        assert!(state.set_register("chargeReserve", 60).is_err());
    }

    #[test]
    fn host_table_completes_sync() {
        let mut state = synced_state();
        let mut block = vec![0xFFu8; 0x60];
        block[..14].copy_from_slice(b"\x0diPhone 15 Pro");
        block[14..31].fill(0);
        block[31] = 0x71;
        for (i, req) in host_table_plan().iter().enumerate() {
            let data = if i == 0 { block.clone() } else { vec![0xFF; 0x60] };
            state.apply(req, &Response { opcode: 0x29, status: 0, data });
        }
        assert!(!state.is_synced(), "iOS key table not read yet");
        for req in key_table_plan(Os::Ios) {
            state.apply(&req, &Response { opcode: 0x25, status: 0, data: vec![0xFF; req_len(&req)] });
        }
        assert!(state.is_synced());
        let hosts = state.hosts();
        let kinds: Vec<_> = hosts.iter().map(|h| h.kind).collect();
        use HostKind::*;
        // 0x0D = 0x8E, 0x0E = 0x0B, 0x0F = 0x00 in the capture.
        assert_eq!(kinds, [Ios, Android, Empty, Ios, Android, Ios, Empty, Empty, Empty]);
        assert_eq!(hosts[0].name.as_deref(), Some("iPhone 15 Pro"));
        assert_eq!(hosts[1].display_name(), "Android");
        assert!(hosts[1].active && !hosts[0].active);

        assert_eq!(state.active_os(), Some(Os::Android));
        // Slot 1 is iOS: only bit 0 of its pair changes (0x8E -> 0x8F).
        assert_eq!(state.set_host_os(1, Os::Android).unwrap(), vec![Request::write_u8(0x0D, 0x8F)]);
        assert_eq!(state.set_host_os(2, Os::Ios).unwrap(), vec![Request::write_u8(0x0D, 0x8A)]);
        assert_eq!(state.set_host_os(3, Os::Android), Err(ModelError::HostNotPaired(3)));

        let reqs = state.remove_host(4).unwrap();
        assert_eq!(reqs[1], Request::write_u8(0x0D, 0x0E));
        assert_eq!(state.remove_host(2), Err(ModelError::ActiveHost(2)));
        for r in state.set_host_name(1, "Laptop").unwrap() {
            state.apply(&r, &Response { opcode: r.opcode(), status: 0, data: vec![] });
        }
        assert_eq!(state.hosts()[0].name.as_deref(), Some("Laptop"));
    }

    #[test]
    fn wireless_charging_matches_official_app() {
        let mut state = synced_state();
        let req = &state.set_wireless_charging(true)[0];
        assert_eq!(req.encode().unwrap(), hex("ff062600040004b4"));
        state.apply(req, &Response { opcode: 0x26, status: 0, data: vec![] });
        assert_eq!(state.wireless_charging(), Some(true));
        assert_eq!(state.status(), Some(&[0, 0, 0, 6][..]));
    }

    #[test]
    fn shortcut_slots_reject_combos() {
        let state = synced_state();
        let combo = KeyAction::Combo { modifier: 0xE0, usage: 0x2C };
        assert!(matches!(state.set_key_slot(Os::Android, 4, Some(combo)), Err(ModelError::UnsupportedAction { .. })));
        let reqs = state.set_key_slot(Os::Android, 1, Some(KeyAction::Key { usage: 0x2D })).unwrap();
        assert_eq!(reqs[1], Request::WriteRegister { addr: 0x1A, data: vec![0x00, 0x2D] });
    }

    #[test]
    fn ios_table_is_interleaved() {
        let mut state = synced_state();
        assert_eq!(state.set_key_slot(Os::Ios, 0, None), Err(ModelError::NotSynced));
        // Factory iOS mask, then slot 5 (Sym + 2) = media Volume Up.
        state.apply(
            &Request::ReadRegister { addr: 0x10, len: 2 },
            &Response { opcode: 0x25, status: 0, data: vec![0xF0, 0x1F] },
        );
        state.apply(
            &Request::ReadRegister { addr: 0x28, len: 2 },
            &Response { opcode: 0x25, status: 0, data: vec![0xE9, 0xFF] },
        );
        let slot = state.key_slot(Os::Ios, 5).unwrap();
        assert!(slot.enabled);
        assert_eq!(slot.action, keymap::IOS_FACTORY_DEFAULTS[5]);
        assert_eq!(state.key_slot(Os::Android, 5).unwrap().action, keymap::FACTORY_DEFAULTS[5]);
        let reqs = state.set_key_slot(Os::Ios, 0, Some(KeyAction::Key { usage: 0x2B })).unwrap();
        assert_eq!(
            reqs,
            vec![Request::write_u8(0x10, 0xF1), Request::WriteRegister { addr: 0x14, data: vec![0x00, 0x2B] }]
        );
        let reset = state.reset_key_slots(Os::Ios);
        assert_eq!(reset[5], Request::WriteRegister { addr: 0x28, data: vec![0xE9, 0xFF] });
        assert_eq!(&reset[13..], &[Request::write_u8(0x10, 0xF0), Request::write_u8(0x11, 0x1F)]);
    }
}
