//! Paired host slots.
//!
//! The keyboard remembers [`SLOT_COUNT`] hosts. Each slot has
//!
//! * a 2-bit [`HostKind`] packed into registers `0x0D` (slots 1–4), `0x0E`
//!   (5–8) and `0x0F` (9), lowest slot in the lowest bits;
//! * an optional user-chosen name: a 32-byte record in the name table, read with
//!   [`crate::protocol::Request::ReadBlock`] and written with
//!   [`crate::protocol::Request::WriteBlock`]:
//!   `[name_len, name (UTF-8, zero padded to 30 bytes), xor]`, where `xor` is
//!   the XOR of the first 31 bytes. Records failing the check (e.g. erased
//!   `0xFF` flash) have no name.
//!
//! Register `0x0C` holds the active slot (1-based).

use crate::protocol::Request;

pub const SLOT_COUNT: u8 = 9;
pub const RECORD_LEN: usize = 32;
pub const NAME_CAPACITY: usize = 30;
pub const TABLE_LEN: u16 = SLOT_COUNT as u16 * RECORD_LEN as u16;
/// The official app reads the table in three blocks of three records.
pub const READ_CHUNK: u8 = 3 * RECORD_LEN as u8;

pub const ACTIVE_ADDR: u8 = 0x0C;
pub const KIND_ADDR: u8 = 0x0D;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub enum HostKind {
    Empty,
    /// Value 1; never produced by the official app.
    Unknown,
    /// Value 2; also what a freshly paired host starts as.
    Ios,
    /// Value 3; the official app sets this for the phone it runs on.
    Android,
}

impl HostKind {
    pub fn from_bits(bits: u8) -> Self {
        match bits & 3 {
            0 => HostKind::Empty,
            1 => HostKind::Unknown,
            2 => HostKind::Ios,
            _ => HostKind::Android,
        }
    }

    pub fn bits(self) -> u8 {
        match self {
            HostKind::Empty => 0,
            HostKind::Unknown => 1,
            HostKind::Ios => 2,
            HostKind::Android => 3,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            HostKind::Empty => "Empty",
            HostKind::Unknown => "Paired",
            HostKind::Ios => "iOS",
            HostKind::Android => "Android",
        }
    }
}

/// Register holding `slot`'s kind (slot is 1-based) and the bit shift within it.
pub fn kind_location(slot: u8) -> (u8, u8) {
    let i = slot - 1;
    (KIND_ADDR + i / 4, (i % 4) * 2)
}

/// Byte offset of `slot`'s name record (slot is 1-based).
pub fn record_offset(slot: u8) -> u16 {
    (slot as u16 - 1) * RECORD_LEN as u16
}

fn checksum(record: &[u8]) -> u8 {
    record[..RECORD_LEN - 1].iter().fold(0, |a, b| a ^ b)
}

/// Decodes a name record; `None` for empty or invalid records.
pub fn parse_record(record: &[u8]) -> Option<String> {
    if record.len() != RECORD_LEN || checksum(record) != record[RECORD_LEN - 1] {
        return None;
    }
    let len = record[0] as usize;
    if len == 0 || len > NAME_CAPACITY {
        return None;
    }
    Some(String::from_utf8_lossy(&record[1..1 + len]).into_owned())
}

/// Encodes `name`, truncated at a character boundary to [`NAME_CAPACITY`]
/// bytes. An empty name clears the record.
pub fn encode_record(name: &str) -> [u8; RECORD_LEN] {
    let mut rec = [0u8; RECORD_LEN];
    let mut len = 0;
    for c in name.chars() {
        let n = c.len_utf8();
        if len + n > NAME_CAPACITY {
            break;
        }
        c.encode_utf8(&mut rec[1 + len..]);
        len += n;
    }
    rec[0] = len as u8;
    rec[RECORD_LEN - 1] = checksum(&rec);
    rec
}

pub fn write_name(slot: u8, name: &str) -> Request {
    Request::WriteBlock { offset: record_offset(slot), data: encode_record(name).to_vec() }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub struct Host {
    /// 1-based, as printed on the keyboard.
    pub slot: u8,
    pub kind: HostKind,
    pub name: Option<String>,
    pub active: bool,
}

impl Host {
    /// What the official app shows: the custom name, else the kind.
    pub fn display_name(&self) -> String {
        match (&self.name, self.kind) {
            (Some(n), _) => n.clone(),
            (None, HostKind::Empty) => String::new(),
            (None, kind) => kind.label().to_owned(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_captured_record() {
        let mut rec = [0u8; RECORD_LEN];
        rec[0] = 13;
        rec[1..14].copy_from_slice(b"iPhone 15 Pro");
        rec[31] = 0x71;
        assert_eq!(parse_record(&rec).as_deref(), Some("iPhone 15 Pro"));
        assert_eq!(encode_record("iPhone 15 Pro"), rec);
        assert_eq!(parse_record(&[0xFF; RECORD_LEN]), None);
    }

    #[test]
    fn truncates_on_char_boundary() {
        let name = "가".repeat(11); // 33 bytes
        let rec = encode_record(&name);
        assert_eq!(rec[0], 30);
        assert_eq!(parse_record(&rec), Some("가".repeat(10)));
        let empty = encode_record("");
        assert_eq!(empty, [0; RECORD_LEN]);
        assert_eq!(parse_record(&empty), None);
    }

    #[test]
    fn kind_layout() {
        assert_eq!(kind_location(1), (0x0D, 0));
        assert_eq!(kind_location(4), (0x0D, 6));
        assert_eq!(kind_location(5), (0x0E, 0));
        assert_eq!(kind_location(9), (0x0F, 0));
        assert_eq!(write_name(2, "").payload()[..3], [0x28, 0x00, 0x20]);
    }
}
