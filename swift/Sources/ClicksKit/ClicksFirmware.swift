import ClicksCore
import Foundation

/// Firmware update discovery using the vendor manifest, like the official app.
/// Installing images is not supported.
public enum ClicksFirmware {
    public enum Failure: Error { case checksumMismatch, unexpectedHost }

    public static func manifest(session: URLSession = .shared) async throws -> [FirmwareRecord] {
        let (data, _) = try await session.data(from: URL(string: firmwareManifestUrl())!)
        return try parseFirmwareManifest(json: String(decoding: data, as: UTF8.self))
    }

    /// Newest image newer than `installed`; `model`/`installed` come from
    /// `ClicksKeyboard.model` and `.firmware`.
    public static func update(model: String, installed: String, session: URLSession = .shared) async throws -> FirmwareRecord? {
        selectFirmwareUpdate(manifest: try await manifest(session: session), model: model, installed: installed)
    }

    /// Downloads `firmware` and verifies its SHA-256 before returning it.
    public static func download(_ firmware: FirmwareRecord, session: URLSession = .shared) async throws -> Data {
        guard let url = URL(string: firmware.url), url.host == "xinyi1.clicks.tech" else { throw Failure.unexpectedHost }
        let (data, _) = try await session.data(from: url)
        guard verifyFirmware(image: data, sha256: firmware.sha256) else { throw Failure.checksumMismatch }
        return data
    }
}
