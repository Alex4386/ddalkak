//! Wire framing used on the `fff1`/`fff2` characteristics.
//!
//! ```text
//! +------+-----+-------------------+-------+
//! | 0xFF | len | payload (len - 1) | crc8  |
//! +------+-----+-------------------+-------+
//! ```
//!
//! `len` counts the payload plus the trailing CRC byte. The CRC is CRC-8 with
//! polynomial 0x1D (reflected, i.e. 0xB8), init 0xFF, no final XOR, computed over
//! `len` and the payload (everything except the sync byte and the CRC itself).

pub const SYNC: u8 = 0xFF;

/// Largest payload a single length byte can describe.
pub const MAX_PAYLOAD: usize = 0xFE;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FrameError {
    PayloadTooLarge(usize),
    BadChecksum { expected: u8, actual: u8, frame: Vec<u8> },
    /// Bytes that did not start with [`SYNC`] and were dropped while resynchronising.
    Garbage(Vec<u8>),
}

impl std::fmt::Display for FrameError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FrameError::PayloadTooLarge(n) => write!(f, "payload of {n} bytes does not fit in a frame"),
            FrameError::BadChecksum { expected, actual, .. } => {
                write!(f, "bad checksum: expected {expected:#04x}, got {actual:#04x}")
            }
            FrameError::Garbage(bytes) => write!(f, "dropped {} unframed bytes", bytes.len()),
        }
    }
}

impl std::error::Error for FrameError {}

pub fn crc8(data: &[u8]) -> u8 {
    let mut crc = 0xFFu8;
    for &byte in data {
        crc ^= byte;
        for _ in 0..8 {
            crc = if crc & 1 != 0 { (crc >> 1) ^ 0xB8 } else { crc >> 1 };
        }
    }
    crc
}

pub fn encode(payload: &[u8]) -> Result<Vec<u8>, FrameError> {
    if payload.len() > MAX_PAYLOAD {
        return Err(FrameError::PayloadTooLarge(payload.len()));
    }
    let mut out = Vec::with_capacity(payload.len() + 3);
    out.push(SYNC);
    out.push(payload.len() as u8 + 1);
    out.extend_from_slice(payload);
    out.push(crc8(&out[1..]));
    Ok(out)
}

/// Decodes exactly one complete frame, returning its payload.
pub fn decode(frame: &[u8]) -> Option<Result<Vec<u8>, FrameError>> {
    let mut decoder = Decoder::default();
    let mut results = decoder.push(frame);
    if results.len() == 1 && decoder.buffered() == 0 {
        results.pop()
    } else {
        None
    }
}

/// Streaming decoder that reassembles frames split across (or packed into)
/// notifications. Needed when the negotiated ATT MTU is smaller than a frame.
#[derive(Debug, Default, Clone)]
pub struct Decoder {
    buf: Vec<u8>,
}

impl Decoder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn buffered(&self) -> usize {
        self.buf.len()
    }

    pub fn reset(&mut self) {
        self.buf.clear();
    }

    pub fn push(&mut self, bytes: &[u8]) -> Vec<Result<Vec<u8>, FrameError>> {
        self.buf.extend_from_slice(bytes);
        let mut out = Vec::new();
        loop {
            match self.buf.iter().position(|&b| b == SYNC) {
                None => {
                    if !self.buf.is_empty() {
                        out.push(Err(FrameError::Garbage(std::mem::take(&mut self.buf))));
                    }
                    break;
                }
                Some(0) => {}
                Some(n) => out.push(Err(FrameError::Garbage(self.buf.drain(..n).collect()))),
            }
            if self.buf.len() < 2 {
                break;
            }
            let len = self.buf[1] as usize;
            if len == 0 {
                // A zero length can't carry a CRC; skip this sync byte and resync.
                out.push(Err(FrameError::Garbage(self.buf.drain(..1).collect())));
                continue;
            }
            let total = len + 2;
            if self.buf.len() < total {
                break;
            }
            let frame: Vec<u8> = self.buf.drain(..total).collect();
            let expected = crc8(&frame[1..total - 1]);
            let actual = frame[total - 1];
            if expected == actual {
                out.push(Ok(frame[2..total - 1].to_vec()));
            } else {
                out.push(Err(FrameError::BadChecksum { expected, actual, frame }));
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(s: &str) -> Vec<u8> {
        (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap()).collect()
    }

    // Frames captured from the official Android companion app (btsnoop_hci.log).
    const CAPTURED: &[&str] = &[
        "ff0425080111",
        "ff050225000573",
        "ff0227b6",
        "ff0802270000000002f9",
        "ff052900c06093",
        "ff062600040004b4",
        "ff040226007c",
        "ff052416e02c49",
        "ff05241ae2073b",
        "ff040224002b",
        "ff06022500e350eb",
    ];

    #[test]
    fn captured_frames_roundtrip() {
        for f in CAPTURED {
            let bytes = hex(f);
            let payload = decode(&bytes).unwrap().unwrap();
            assert_eq!(encode(&payload).unwrap(), bytes, "{f}");
        }
    }

    #[test]
    fn reassembles_split_and_packed_frames() {
        let a = hex("ff050225000573");
        let b = hex("ff040224002b");
        let mut joined = a.clone();
        joined.extend(&b);
        let mut d = Decoder::new();
        assert!(d.push(&joined[..3]).is_empty());
        let out = d.push(&joined[3..]);
        assert_eq!(out, vec![Ok(hex("022500 05".replace(' ', "").as_str())), Ok(hex("022400"))]);
        assert_eq!(d.buffered(), 0);
    }

    #[test]
    fn resyncs_after_garbage_and_bad_crc() {
        let mut d = Decoder::new();
        let out = d.push(&hex("0102ff040224002cff040224002b"));
        assert!(matches!(out[0], Err(FrameError::Garbage(_))));
        assert!(matches!(out[1], Err(FrameError::BadChecksum { .. })));
        assert_eq!(out[2], Ok(hex("022400")));
    }
}
