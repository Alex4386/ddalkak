//! Register map. Names, units and presets come from the official Android app
//! (`com.clicks.companionapp`); `note` records anything worth knowing.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(feature = "serde", serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase"))]
pub enum RegisterKind {
    /// Unsigned value edited with a slider. `unit` is a display suffix.
    Level { min: u16, max: u16, step: u16, unit: &'static str },
    /// Duration; `unit_ms` is 1 for milliseconds, 1000 for seconds.
    /// `presets` are the steps the official app offers, in register units.
    Duration { unit_ms: u32, presets: &'static [u16] },
    /// Bit field of independent switches.
    Flags { bits: &'static [FlagBit] },
    /// Read on connect; changed through dedicated helpers, not directly.
    ReadOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub struct FlagBit {
    pub bit: u8,
    pub id: &'static str,
    pub label: &'static str,
    pub note: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub struct RegisterInfo {
    pub id: &'static str,
    pub addr: u8,
    pub len: u8,
    pub kind: RegisterKind,
    pub label: &'static str,
    pub note: &'static str,
}

pub const BRIGHTNESS: RegisterInfo = RegisterInfo {
    id: "brightness",
    addr: 0x02,
    len: 1,
    kind: RegisterKind::Level { min: 0, max: 255, step: 1, unit: "" },
    label: "Brightness",
    note: "stored as 0–255; shown as a percentage",
};

pub const CHARGE_RESERVE: RegisterInfo = RegisterInfo {
    id: "chargeReserve",
    addr: 0x03,
    len: 1,
    kind: RegisterKind::Level { min: 0, max: 50, step: 10, unit: "%" },
    label: "Keyboard battery reserve",
    note: "phone charging stops when the keyboard is down to this much; 0 shares all of it",
};

pub const DIM_TIMEOUT: RegisterInfo = RegisterInfo {
    id: "dimTimeout",
    addr: 0x04,
    len: 2,
    kind: RegisterKind::Duration { unit_ms: 1, presets: &[2000, 5000, 15000, 30000, 45000, 60000] },
    label: "Dim backlight after",
    note: "",
};

pub const IDLE_TIMEOUT: RegisterInfo = RegisterInfo {
    id: "idleTimeout",
    addr: 0x06,
    len: 2,
    kind: RegisterKind::Duration { unit_ms: 1000, presets: &[120, 300, 900, 1800, 2700, 3600] },
    label: "Sleep after",
    note: "drops the connection when unused to save power; press any key to wake it",
};

pub const FEATURES: RegisterInfo = RegisterInfo {
    id: "features",
    addr: 0x08,
    len: 1,
    kind: RegisterKind::Flags {
        bits: &[
            FlagBit { bit: 0, id: "backlight", label: "Backlight", note: "" },
            FlagBit {
                bit: 5,
                id: "backlightShortcut",
                label: "Backlight toggle key",
                note: "turn the light on or off with Sym + mic",
            },
            FlagBit {
                bit: 1,
                id: "symLock",
                label: "Symbol lock",
                note: "tap Sym twice to stay on the symbol layer",
            },
            FlagBit {
                bit: 2,
                id: "capsLock",
                label: "Caps lock",
                note: "tap Shift twice to type in capitals",
            },
            FlagBit {
                bit: 4,
                id: "cursorMode",
                label: "Arrow-key mode",
                note: "Sym + Meta turns WASD and IJKL into arrows; any other key ends it",
            },
            FlagBit { bit: 3, id: "softReturn", label: "Soft line break", note: "offered by the iOS app only" },
        ],
    },
    label: "Keyboard features",
    note: "",
};

pub const ACTIVE_HOST: RegisterInfo = RegisterInfo {
    id: "activeHost",
    addr: 0x0C,
    len: 1,
    kind: RegisterKind::ReadOnly,
    label: "Active host slot",
    note: "1-based",
};
pub const HOST_KINDS_1_4: RegisterInfo = RegisterInfo {
    id: "hostKinds1",
    addr: 0x0D,
    len: 1,
    kind: RegisterKind::ReadOnly,
    label: "Host types 1–4",
    note: "2 bits per slot, slot 1 in the low bits",
};
pub const HOST_KINDS_5_8: RegisterInfo = RegisterInfo {
    id: "hostKinds5",
    addr: 0x0E,
    len: 1,
    kind: RegisterKind::ReadOnly,
    label: "Host types 5–8",
    note: "2 bits per slot, slot 5 in the low bits",
};
pub const HOST_KINDS_9: RegisterInfo = RegisterInfo {
    id: "hostKinds9",
    addr: 0x0F,
    len: 1,
    kind: RegisterKind::ReadOnly,
    label: "Host type 9",
    note: "low 2 bits",
};

pub const SETTINGS: &[RegisterInfo] = &[BRIGHTNESS, DIM_TIMEOUT, IDLE_TIMEOUT, CHARGE_RESERVE, FEATURES];
pub const INFO: &[RegisterInfo] = &[ACTIVE_HOST, HOST_KINDS_1_4, HOST_KINDS_5_8, HOST_KINDS_9];

pub fn by_id(id: &str) -> Option<&'static RegisterInfo> {
    SETTINGS.iter().chain(INFO).find(|r| r.id == id)
}

pub fn by_addr(addr: u8) -> Option<&'static RegisterInfo> {
    SETTINGS.iter().chain(INFO).find(|r| r.addr == addr)
}

/// Looks up a [`FEATURES`] bit by id (`"capsLock"`) or number (`"2"`).
pub fn feature_bit(name: &str) -> Option<&'static FlagBit> {
    let RegisterKind::Flags { bits } = FEATURES.kind else { unreachable!() };
    bits.iter().find(|b| b.id.eq_ignore_ascii_case(name) || name.parse() == Ok(b.bit))
}
