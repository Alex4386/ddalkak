//! Firmware update discovery, as done by the official app.
//!
//! The vendor publishes a JSON manifest per product line; the Power Keyboard's
//! is [`MANIFEST_URL`]. An update is the highest `firmwareRevision` for the
//! keyboard's model number (GATT `2a24`, e.g. `CP-1000`) that is newer than its
//! firmware revision (`2a26`). Images are verified with SHA-256 before use.
//!
//! Flashing (opcodes `A0`–`A3`, see `docs/PROTOCOL.md`) is intentionally not
//! implemented.

use std::cmp::Ordering;

/// Manifest for BLE keyboards. `firmwares-a.json` covers the USB cases.
pub const MANIFEST_URL: &str = "https://xinyi1.clicks.tech/firmwares-p.json";

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FirmwareInfo {
    pub model: String,
    #[cfg_attr(feature = "serde", serde(rename = "firmwareRevision"))]
    pub version: String,
    #[cfg_attr(feature = "serde", serde(rename = "file"))]
    pub url: String,
    #[cfg_attr(feature = "serde", serde(rename = "sha256sum"))]
    pub sha256: String,
}

/// Compares dotted versions numerically (`1.1.10 > 1.1.9`, `1.1 == 1.1.0`).
/// Non-numeric components count as 0.
pub fn compare_versions(a: &str, b: &str) -> Ordering {
    let parts = |s: &str| s.trim().split('.').map(|p| p.trim().parse::<u64>().unwrap_or(0)).collect::<Vec<_>>();
    let (a, b) = (parts(a), parts(b));
    (0..a.len().max(b.len()))
        .map(|i| a.get(i).unwrap_or(&0).cmp(b.get(i).unwrap_or(&0)))
        .find(|o| o.is_ne())
        .unwrap_or(Ordering::Equal)
}

/// Newest image for `model` that is newer than `current`.
pub fn select_update<'a>(manifest: &'a [FirmwareInfo], model: &str, current: &str) -> Option<&'a FirmwareInfo> {
    manifest
        .iter()
        .filter(|f| f.model.trim() == model.trim())
        .filter(|f| compare_versions(&f.version, current).is_gt())
        .max_by(|a, b| compare_versions(&a.version, &b.version))
}

/// Newest image for `model`, regardless of what is installed.
pub fn latest<'a>(manifest: &'a [FirmwareInfo], model: &str) -> Option<&'a FirmwareInfo> {
    manifest
        .iter()
        .filter(|f| f.model.trim() == model.trim())
        .max_by(|a, b| compare_versions(&a.version, &b.version))
}

#[cfg(feature = "firmware")]
pub fn parse_manifest(json: &str) -> Result<Vec<FirmwareInfo>, serde_json::Error> {
    serde_json::from_str(json)
}

/// Whether `image` matches the manifest's hex SHA-256.
#[cfg(feature = "firmware")]
pub fn verify(image: &[u8], sha256_hex: &str) -> bool {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(image);
    let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
    hex.eq_ignore_ascii_case(sha256_hex.trim())
}

/// Suggested file name, e.g. `CP-1000-1.1.2.bin`.
pub fn file_name(info: &FirmwareInfo) -> String {
    let safe = |s: &str| s.chars().map(|c| if c.is_ascii_alphanumeric() || c == '.' || c == '-' { c } else { '_' }).collect::<String>();
    format!("{}-{}.bin", safe(&info.model), safe(&info.version))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fw(model: &str, version: &str) -> FirmwareInfo {
        FirmwareInfo { model: model.into(), version: version.into(), url: String::new(), sha256: String::new() }
    }

    #[test]
    fn versions() {
        assert!(compare_versions("1.1.10", "1.1.9").is_gt());
        assert!(compare_versions("1.1", "1.1.0").is_eq());
        assert!(compare_versions("1.0.9", "1.1.0").is_lt());
    }

    #[test]
    fn selects_like_the_official_app() {
        let m = [fw("CP-1000", "1.0.8"), fw("CP-1000", "1.1.2"), fw("CP-1000", "1.1.0"), fw("CA-1100", "9.0")];
        assert_eq!(select_update(&m, "CP-1000", "1.1.0").map(|f| f.version.as_str()), Some("1.1.2"));
        assert_eq!(select_update(&m, "CP-1000", "1.1.2"), None);
        assert_eq!(select_update(&m, "CP-2000", "0.1"), None);
        assert_eq!(latest(&m, "CP-1000").map(|f| f.version.as_str()), Some("1.1.2"));
        assert_eq!(file_name(&m[1]), "CP-1000-1.1.2.bin");
    }

    #[cfg(feature = "firmware")]
    #[test]
    fn parses_published_manifest() {
        let json = r#"[{
            "model": "CP-1000",
            "firmwareRevision": "1.1.2",
            "file": "https://xinyi1.clicks.tech/CP-1000/CP-1000-V1.12.bin",
            "sha256sum": "212db31a98a2ab1a25fedf4399d1a513830abb43db7d2634037a9052d1432e78"
        }]"#;
        let m = parse_manifest(json).unwrap();
        assert_eq!(m[0].version, "1.1.2");
        assert!(m[0].url.ends_with("V1.12.bin"));
        assert!(verify(b"abc", "BA7816BF8F01CFEA414140DE5DAE2223B00361A396177A9CB410FF61F20015AD"));
        assert!(!verify(b"abd", "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"));
    }
}
