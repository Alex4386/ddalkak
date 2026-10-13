//! Request/response layer on top of [`crate::frame`].
//!
//! Requests are `[opcode, args...]`. Every request is answered with a single
//! notification `[0x02, opcode, status, data...]`.

use crate::frame::{self, FrameError};

pub mod opcode {
    /// `[addr, data...]` — little-endian data, 1 or 2 bytes in practice.
    pub const WRITE_REGISTER: u8 = 0x24;
    /// `[addr, len]` — response data is `len` bytes, little-endian.
    pub const READ_REGISTER: u8 = 0x25;
    /// `[mask_hi, mask_lo, value_hi, value_lo]` — sets the control bits in
    /// `mask` to `value`. Bit 2 enables wireless charging pass-through.
    pub const SET_CONTROL: u8 = 0x26;
    /// No args. Response carries 4 bytes; the last two are the control bits
    /// (big-endian) that [`SET_CONTROL`] changes.
    pub const READ_STATUS: u8 = 0x27;
    /// `[offset_hi, offset_lo, data...]` — writes a host-name record.
    pub const WRITE_BLOCK: u8 = 0x28;
    /// `[offset_hi, offset_lo, len]` — reads `len` bytes of the host-name table.
    pub const READ_BLOCK: u8 = 0x29;

    // Firmware update, taken from the official app; not used by this crate.
    // Chunks are `MTU - 11` bytes and every data response carries the 4-byte
    // token that prefixes the next chunk (the first chunk uses `00 00 00 00`).
    /// No args. Reboots into the new image; the keyboard may not answer.
    pub const OTA_REBOOT: u8 = 0xA0;
    /// `[image_len u32 BE]`.
    pub const OTA_BEGIN: u8 = 0xA1;
    /// `[token(4), chunk...]` — response data starts with the next token.
    pub const OTA_DATA: u8 = 0xA2;
    /// `[image_len u32 BE]`.
    pub const OTA_FINISH: u8 = 0xA3;
}

/// Control bit, see [`opcode::SET_CONTROL`].
pub const CONTROL_WIRELESS_CHARGING: u16 = 0x0004;

/// Marker byte that prefixes every response payload.
pub const RESPONSE_MARKER: u8 = 0x02;
pub const STATUS_OK: u8 = 0x00;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(tag = "type", rename_all = "camelCase"))]
pub enum Request {
    WriteRegister { addr: u8, data: Vec<u8> },
    ReadRegister { addr: u8, len: u8 },
    ReadStatus,
    SetControl { mask: u16, value: u16 },
    ReadBlock { offset: u16, len: u8 },
    WriteBlock { offset: u16, data: Vec<u8> },
    /// Escape hatch for opcodes that are not modelled yet.
    Raw { opcode: u8, args: Vec<u8> },
}

impl Request {
    pub fn write_u8(addr: u8, value: u8) -> Self {
        Request::WriteRegister { addr, data: vec![value] }
    }

    pub fn write_u16(addr: u8, value: u16) -> Self {
        Request::WriteRegister { addr, data: value.to_le_bytes().to_vec() }
    }

    pub fn opcode(&self) -> u8 {
        match self {
            Request::WriteRegister { .. } => opcode::WRITE_REGISTER,
            Request::ReadRegister { .. } => opcode::READ_REGISTER,
            Request::ReadStatus => opcode::READ_STATUS,
            Request::SetControl { .. } => opcode::SET_CONTROL,
            Request::ReadBlock { .. } => opcode::READ_BLOCK,
            Request::WriteBlock { .. } => opcode::WRITE_BLOCK,
            Request::Raw { opcode, .. } => *opcode,
        }
    }

    pub fn payload(&self) -> Vec<u8> {
        let mut p = vec![self.opcode()];
        match self {
            Request::WriteRegister { addr, data } => {
                p.push(*addr);
                p.extend_from_slice(data);
            }
            Request::ReadRegister { addr, len } => p.extend_from_slice(&[*addr, *len]),
            Request::ReadStatus => {}
            Request::SetControl { mask, value } => {
                p.extend_from_slice(&mask.to_be_bytes());
                p.extend_from_slice(&value.to_be_bytes());
            }
            Request::ReadBlock { offset, len } => {
                p.extend_from_slice(&offset.to_be_bytes());
                p.push(*len);
            }
            Request::WriteBlock { offset, data } => {
                p.extend_from_slice(&offset.to_be_bytes());
                p.extend_from_slice(data);
            }
            Request::Raw { args, .. } => p.extend_from_slice(args),
        }
        p
    }

    /// Full frame ready to be written to the `fff2` characteristic.
    pub fn encode(&self) -> Result<Vec<u8>, FrameError> {
        frame::encode(&self.payload())
    }

    /// Whether the request changes device state (and so can be projected).
    pub fn is_write(&self) -> bool {
        matches!(self, Request::WriteRegister { .. } | Request::SetControl { .. } | Request::WriteBlock { .. })
    }

    /// Whether `response` is the answer to this request.
    pub fn matches(&self, response: &Response) -> bool {
        response.opcode == self.opcode()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Response {
    pub opcode: u8,
    pub status: u8,
    pub data: Vec<u8>,
}

impl Response {
    pub fn is_ok(&self) -> bool {
        self.status == STATUS_OK
    }
}

/// A decoded notification payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Message {
    Response(Response),
    /// Anything that doesn't follow the response layout (no unsolicited
    /// messages were seen in the capture, but don't drop them silently).
    Other(Vec<u8>),
}

impl Message {
    pub fn parse(payload: &[u8]) -> Self {
        match payload {
            [RESPONSE_MARKER, opcode, status, data @ ..] => {
                Message::Response(Response { opcode: *opcode, status: *status, data: data.to_vec() })
            }
            _ => Message::Other(payload.to_vec()),
        }
    }
}

/// Little-endian helper for register values of 1–4 bytes.
pub fn le_value(data: &[u8]) -> u32 {
    data.iter().take(4).rev().fold(0, |acc, &b| (acc << 8) | b as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_like_the_official_app() {
        let cases: &[(Request, &[u8])] = &[
            (Request::ReadRegister { addr: 0x08, len: 1 }, &[0xff, 0x04, 0x25, 0x08, 0x01, 0x11]),
            (Request::ReadStatus, &[0xff, 0x02, 0x27, 0xb6]),
            (Request::ReadBlock { offset: 0x00c0, len: 0x60 }, &[0xff, 0x05, 0x29, 0x00, 0xc0, 0x60, 0x93]),
            (Request::write_u16(0x04, 2000), &[0xff, 0x05, 0x24, 0x04, 0xd0, 0x07, 0xa0]),
            (Request::write_u8(0x12, 0xfd), &[0xff, 0x04, 0x24, 0x12, 0xfd, 0xbd]),
            (
                Request::SetControl { mask: CONTROL_WIRELESS_CHARGING, value: CONTROL_WIRELESS_CHARGING },
                &[0xff, 0x06, 0x26, 0x00, 0x04, 0x00, 0x04, 0xb4],
            ),
        ];
        for (req, wire) in cases {
            assert_eq!(req.encode().unwrap(), *wire, "{req:?}");
        }
    }

    #[test]
    fn parses_responses() {
        let payload = frame::decode(&[0xff, 0x06, 0x02, 0x25, 0x00, 0x2c, 0x01, 0xa8]).unwrap().unwrap();
        let Message::Response(r) = Message::parse(&payload) else { panic!() };
        assert_eq!((r.opcode, r.status), (0x25, 0));
        assert_eq!(le_value(&r.data), 300);
        assert!(Request::ReadRegister { addr: 6, len: 2 }.matches(&r));
    }
}
