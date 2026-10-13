//! Programmable key slots.
//!
//! The keyboard exposes [`SLOT_COUNT`] slots, 4 bytes apart starting at
//! [`SLOT_BASE`]; only the first two bytes of each are used. Registers
//! `0x12`/`0x13` form a little-endian bitmask with one enable bit per slot; a
//! disabled slot falls back to the key's normal behaviour.
//!
//! | slots | [`SlotKind`]  | key                        | value                         |
//! |-------|---------------|----------------------------|-------------------------------|
//! | 0–3   | `Remap`       | Tab, Gemini, Alt, Backspace | `[modifier, key]`, either may be `00` |
//! | 4–12  | `Shortcut`    | Sym + 1 … Sym + 9          | `[usage, page]`, page `00` = keyboard, `FF` = consumer |
//!
//! Examples from the official app: `e0 2c` Ctrl+Space, `e2 00` Alt, `00 2d`
//! "-", `29 00` Escape, `cd ff` Play/Pause. `ff ff` is an erased slot.

pub const SLOT_BASE: u8 = 0x16;
pub const SLOT_STRIDE: u8 = 4;
pub const SLOT_COUNT: u8 = 13;
pub const SLOT_VALUE_LEN: u8 = 2;
pub const ENABLE_MASK_ADDR: u8 = 0x12;

pub fn slot_addr(index: u8) -> u8 {
    SLOT_BASE + index * SLOT_STRIDE
}

/// Register byte and bit holding the enable flag for `index`.
pub fn enable_bit(index: u8) -> (u8, u8) {
    (ENABLE_MASK_ADDR + index / 8, index % 8)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub enum SlotKind {
    /// Replaces a physical key with a key or a modifier+key combo.
    Remap,
    /// A Sym + digit shortcut: a single keyboard or consumer (media) usage.
    Shortcut,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub struct SlotInfo {
    pub index: u8,
    pub kind: SlotKind,
    pub label: &'static str,
    /// What the key does while the slot is disabled.
    pub native: &'static str,
    /// Choices the official app offers (`None` = disabled).
    #[cfg_attr(feature = "serde", serde(skip))]
    pub presets: &'static [Option<KeyAction>],
}

const fn key(usage: u8) -> Option<KeyAction> {
    Some(KeyAction::Key { usage })
}
const fn media(usage: u8) -> Option<KeyAction> {
    Some(KeyAction::Consumer { usage })
}
const fn alt(usage: u8) -> Option<KeyAction> {
    Some(KeyAction::Combo { modifier: 0xE2, usage })
}

const ALT_PRESETS: [Option<KeyAction>; 4] = [alt(0x07), alt(0x0E), alt(0x16), alt(0x37)];
const SHORTCUT_PRESETS: &[Option<KeyAction>] = &[
    None,
    key(0x29),
    key(0x80),
    key(0x81),
    key(0x7F),
    media(0xCD),
    media(0xB5),
    media(0xB6),
    key(0x4B),
    key(0x4E),
    key(0x4A),
    key(0x4D),
    key(0x2F),
    key(0x30),
];

const fn shortcut(index: u8, label: &'static str) -> SlotInfo {
    SlotInfo { index, kind: SlotKind::Shortcut, label, native: "Nothing", presets: SHORTCUT_PRESETS }
}

pub const SLOTS: [SlotInfo; SLOT_COUNT as usize] = [
    SlotInfo {
        index: 0,
        kind: SlotKind::Remap,
        label: "Tab key",
        native: "Tab",
        presets: &[None, Some(KeyAction::Combo { modifier: 0xE0, usage: 0x2C })],
    },
    SlotInfo {
        index: 1,
        kind: SlotKind::Remap,
        label: "Gemini key",
        native: "Gemini",
        presets: &[None, key(0x2D), key(0xE2), ALT_PRESETS[0], ALT_PRESETS[1], ALT_PRESETS[2], ALT_PRESETS[3]],
    },
    SlotInfo {
        index: 2,
        kind: SlotKind::Remap,
        label: "Alt key",
        native: "Alt",
        presets: &[key(0xE2), ALT_PRESETS[0], ALT_PRESETS[1], ALT_PRESETS[2], ALT_PRESETS[3]],
    },
    SlotInfo { index: 3, kind: SlotKind::Remap, label: "Backspace key", native: "Backspace", presets: &[None] },
    shortcut(4, "Sym + 1"),
    shortcut(5, "Sym + 2"),
    shortcut(6, "Sym + 3"),
    shortcut(7, "Sym + 4"),
    shortcut(8, "Sym + 5"),
    shortcut(9, "Sym + 6"),
    shortcut(10, "Sym + 7"),
    shortcut(11, "Sym + 8"),
    shortcut(12, "Sym + 9"),
];

pub fn slot(index: u8) -> Option<&'static SlotInfo> {
    SLOTS.get(index as usize)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(tag = "type", rename_all = "camelCase"))]
pub enum KeyAction {
    /// HID keyboard/keypad page (0x07) usage, modifiers included.
    Key { usage: u8 },
    /// HID consumer page (0x0C) usage, low byte only.
    Consumer { usage: u8 },
    /// A modifier (usage `0xE0..=0xE7`) held together with a keyboard usage.
    Combo { modifier: u8, usage: u8 },
}

impl KeyAction {
    pub const UNASSIGNED: [u8; 2] = [0xFF, 0xFF];

    /// Decodes a slot value. Both layouts can be told apart by their bytes, so
    /// this works for either [`SlotKind`].
    pub fn from_bytes(bytes: [u8; 2]) -> Option<Self> {
        match bytes {
            Self::UNASSIGNED => None,
            [usage, 0x00] => Some(KeyAction::Key { usage }),
            [usage, 0xFF] => Some(KeyAction::Consumer { usage }),
            [0x00, usage] => Some(KeyAction::Key { usage }),
            [modifier, usage] => Some(KeyAction::Combo { modifier, usage }),
        }
    }

    /// Encodes for a slot of `kind`, or `None` if such a slot can't hold it.
    pub fn encode(self, kind: SlotKind) -> Option<[u8; 2]> {
        let bytes = match (kind, self) {
            (_, KeyAction::Key { usage: 0x00 | 0xFF }) => return None,
            (SlotKind::Remap, KeyAction::Key { usage }) if is_modifier(usage) => [usage, 0x00],
            (SlotKind::Remap, KeyAction::Key { usage }) => [0x00, usage],
            (SlotKind::Remap, KeyAction::Combo { modifier, usage })
                if is_modifier(modifier) && !matches!(usage, 0x00 | 0xFF) =>
            {
                [modifier, usage]
            }
            (SlotKind::Shortcut, KeyAction::Key { usage }) => [usage, 0x00],
            (SlotKind::Shortcut, KeyAction::Consumer { usage }) if usage != 0xFF => [usage, 0xFF],
            _ => return None,
        };
        Some(bytes)
    }

    pub fn describe(self) -> String {
        match self {
            KeyAction::Key { usage } => keyboard_usage_name(usage)
                .map(str::to_owned)
                .unwrap_or_else(|| format!("Key {usage:#04x}")),
            KeyAction::Consumer { usage } => consumer_usage_name(usage)
                .map(str::to_owned)
                .unwrap_or_else(|| format!("Consumer {usage:#04x}")),
            KeyAction::Combo { modifier, usage } => format!(
                "{}+{}",
                keyboard_usage_name(modifier).unwrap_or("?"),
                keyboard_usage_name(usage).map(str::to_owned).unwrap_or_else(|| format!("{usage:#04x}"))
            ),
        }
    }
}

pub fn is_modifier(usage: u8) -> bool {
    (0xE0..=0xE7).contains(&usage)
}

/// Defaults read from a factory-fresh keyboard (firmware 1.1.0).
pub const FACTORY_DEFAULTS: [Option<KeyAction>; SLOT_COUNT as usize] = [
    None,
    None,
    alt(0x0E),
    Some(KeyAction::Combo { modifier: 0xE3, usage: 0x50 }),
    key(0x29),
    key(0x80),
    key(0x81),
    key(0x7F),
    media(0xCD),
    media(0xB5),
    media(0xB6),
    key(0x4B),
    key(0x4E),
];

pub fn keyboard_usage_name(usage: u8) -> Option<&'static str> {
    const LETTERS: [&str; 26] = [
        "A", "B", "C", "D", "E", "F", "G", "H", "I", "J", "K", "L", "M", "N", "O", "P", "Q", "R", "S", "T", "U",
        "V", "W", "X", "Y", "Z",
    ];
    const DIGITS: [&str; 10] = ["1", "2", "3", "4", "5", "6", "7", "8", "9", "0"];
    const FKEYS: [&str; 12] = ["F1", "F2", "F3", "F4", "F5", "F6", "F7", "F8", "F9", "F10", "F11", "F12"];
    Some(match usage {
        0x04..=0x1D => LETTERS[(usage - 0x04) as usize],
        0x1E..=0x27 => DIGITS[(usage - 0x1E) as usize],
        0x28 => "Enter",
        0x29 => "Escape",
        0x2A => "Backspace",
        0x2B => "Tab",
        0x2C => "Space",
        0x2D => "-",
        0x2E => "=",
        0x2F => "[",
        0x30 => "]",
        0x31 => "\\",
        0x33 => ";",
        0x34 => "'",
        0x35 => "`",
        0x36 => ",",
        0x37 => ".",
        0x38 => "/",
        0x39 => "Caps Lock",
        0x3A..=0x45 => FKEYS[(usage - 0x3A) as usize],
        0x46 => "Print Screen",
        0x47 => "Scroll Lock",
        0x48 => "Pause",
        0x49 => "Insert",
        0x4A => "Home",
        0x4B => "Page Up",
        0x4C => "Delete",
        0x4D => "End",
        0x4E => "Page Down",
        0x4F => "Right",
        0x50 => "Left",
        0x51 => "Down",
        0x52 => "Up",
        0x65 => "Menu",
        0x7F => "Mute",
        0x80 => "Volume Up",
        0x81 => "Volume Down",
        0xE0 => "Ctrl",
        0xE1 => "Shift",
        0xE2 => "Alt",
        0xE3 => "GUI",
        0xE4 => "Right Ctrl",
        0xE5 => "Right Shift",
        0xE6 => "Right Alt",
        0xE7 => "Right GUI",
        _ => return None,
    })
}

pub fn consumer_usage_name(usage: u8) -> Option<&'static str> {
    Some(match usage {
        0x30 => "Power",
        0x40 => "Menu",
        0x6F => "Brightness Up",
        0x70 => "Brightness Down",
        0xB0 => "Play",
        0xB1 => "Pause",
        0xB3 => "Fast Forward",
        0xB4 => "Rewind",
        0xB5 => "Next Track",
        0xB6 => "Previous Track",
        0xB7 => "Stop",
        0xB8 => "Eject",
        0xCD => "Play/Pause",
        0xE2 => "Mute",
        0xE9 => "Volume Up",
        0xEA => "Volume Down",
        _ => return None,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseKeyActionError(pub String);

impl std::fmt::Display for ParseKeyActionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "cannot parse key action {:?}", self.0)
    }
}

impl std::error::Error for ParseKeyActionError {}

fn parse_number(s: &str) -> Option<u8> {
    match s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        Some(hex) => u8::from_str_radix(hex, 16).ok(),
        None => s.parse().ok(),
    }
}

fn normalise(s: &str) -> String {
    s.chars().filter(|c| !c.is_whitespace()).collect::<String>().to_ascii_lowercase()
}

fn keyboard_usage_from_name(name: &str) -> Option<u8> {
    let n = normalise(name);
    let alias = match n.as_str() {
        "esc" => Some(0x29),
        "return" => Some(0x28),
        "del" => Some(0x4C),
        "pgup" => Some(0x4B),
        "pgdn" | "pgdown" => Some(0x4E),
        "control" | "lctrl" => Some(0xE0),
        "lshift" => Some(0xE1),
        "option" | "opt" | "lalt" => Some(0xE2),
        "cmd" | "command" | "win" | "meta" | "super" | "lgui" => Some(0xE3),
        "volup" => Some(0x80),
        "voldown" => Some(0x81),
        _ => None,
    };
    alias.or_else(|| (0..=0xFFu8).find(|&u| keyboard_usage_name(u).is_some_and(|k| normalise(k) == n)))
}

fn consumer_usage_from_name(name: &str) -> Option<u8> {
    let n = normalise(name);
    (0..=0xFFu8).find(|&u| consumer_usage_name(u).is_some_and(|k| normalise(k) == n))
}

impl std::str::FromStr for KeyAction {
    type Err = ParseKeyActionError;

    /// Accepts `"Escape"`, `"Ctrl+Space"`, `"Play/Pause"`, `"key:0x29"`,
    /// `"consumer:0xcd"` and `"raw:e02c"`. Keyboard-page names win over
    /// consumer-page names (`"Mute"` is keyboard usage 0x7F).
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let err = || ParseKeyActionError(s.to_owned());
        let t = s.trim();
        if let Some((kind, value)) = t.split_once(':') {
            let value = value.trim();
            let action = match normalise(kind).as_str() {
                "key" => KeyAction::Key { usage: parse_number(value).ok_or_else(err)? },
                "consumer" => KeyAction::Consumer { usage: parse_number(value).ok_or_else(err)? },
                "raw" => {
                    let v = u16::from_str_radix(value, 16).map_err(|_| err())?;
                    KeyAction::from_bytes(v.to_be_bytes()).ok_or_else(err)?
                }
                _ => return Err(err()),
            };
            return Ok(action);
        }
        if let Some((m, k)) = t.split_once('+').filter(|(m, k)| !m.trim().is_empty() && !k.trim().is_empty()) {
            let modifier = keyboard_usage_from_name(m).filter(|&u| is_modifier(u)).ok_or_else(err)?;
            let usage = keyboard_usage_from_name(k).ok_or_else(err)?;
            return Ok(KeyAction::Combo { modifier, usage });
        }
        keyboard_usage_from_name(t)
            .map(|usage| KeyAction::Key { usage })
            .or_else(|| consumer_usage_from_name(t).map(|usage| KeyAction::Consumer { usage }))
            .ok_or_else(err)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_factory_table() {
        let raw: [[u8; 2]; 13] = [
            [0xff, 0xff], [0xff, 0xff], [0xe2, 0x0e], [0xe3, 0x50], [0x29, 0x00], [0x80, 0x00], [0x81, 0x00],
            [0x7f, 0x00], [0xcd, 0xff], [0xb5, 0xff], [0xb6, 0xff], [0x4b, 0x00], [0x4e, 0x00],
        ];
        for (i, bytes) in raw.iter().enumerate() {
            assert_eq!(KeyAction::from_bytes(*bytes), FACTORY_DEFAULTS[i]);
            if let Some(a) = FACTORY_DEFAULTS[i] {
                assert_eq!(a.encode(SLOTS[i].kind), Some(*bytes));
            }
        }
        assert_eq!(FACTORY_DEFAULTS[3].unwrap().describe(), "GUI+Left");
        assert_eq!(FACTORY_DEFAULTS[8].unwrap().describe(), "Play/Pause");
    }

    #[test]
    fn encodes_official_presets() {
        let remap = |a: KeyAction| a.encode(SlotKind::Remap);
        assert_eq!(remap(KeyAction::Key { usage: 0x2D }), Some([0x00, 0x2D]));
        assert_eq!(remap(KeyAction::Key { usage: 0xE2 }), Some([0xE2, 0x00]));
        assert_eq!(remap(KeyAction::Combo { modifier: 0xE2, usage: 0x37 }), Some([0xE2, 0x37]));
        assert_eq!(remap(KeyAction::Consumer { usage: 0xCD }), None);
        assert_eq!(KeyAction::from_bytes([0x00, 0x2D]), Some(KeyAction::Key { usage: 0x2D }));
        assert_eq!(KeyAction::Combo { modifier: 0xE0, usage: 0x2C }.encode(SlotKind::Shortcut), None);
        for s in &SLOTS {
            for p in s.presets.iter().flatten() {
                let bytes = p.encode(s.kind).unwrap();
                assert_eq!(KeyAction::from_bytes(bytes), Some(*p), "{}", s.label);
                assert_eq!(p.describe().parse(), Ok(*p), "{}", p.describe());
            }
        }
    }

    #[test]
    fn parses_names() {
        let p = |s: &str| s.parse::<KeyAction>();
        assert_eq!(p("ctrl+space"), Ok(KeyAction::Combo { modifier: 0xE0, usage: 0x2C }));
        assert_eq!(p("Cmd + Left"), Ok(KeyAction::Combo { modifier: 0xE3, usage: 0x50 }));
        assert_eq!(p("esc"), Ok(KeyAction::Key { usage: 0x29 }));
        assert_eq!(p("Mute"), Ok(KeyAction::Key { usage: 0x7F }));
        assert_eq!(p("play/pause"), Ok(KeyAction::Consumer { usage: 0xCD }));
        assert_eq!(p("consumer:0xb5"), Ok(KeyAction::Consumer { usage: 0xB5 }));
        assert_eq!(p("raw:e207"), Ok(KeyAction::Combo { modifier: 0xE2, usage: 0x07 }));
        assert!(p("a+b").is_err());
        assert!(p("nonsense").is_err());
        for a in FACTORY_DEFAULTS.iter().flatten() {
            assert_eq!(p(&a.describe()), Ok(*a), "{}", a.describe());
        }
    }

    #[test]
    fn slot_layout() {
        assert_eq!(slot_addr(0), 0x16);
        assert_eq!(slot_addr(12), 0x46);
        assert_eq!(enable_bit(1), (0x12, 1));
        assert_eq!(enable_bit(9), (0x13, 1));
    }
}
