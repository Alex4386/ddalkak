import ClicksCore
import CoreBluetooth
import Foundation

/// CoreBluetooth transport for `ClicksSession`.
///
/// The keyboard is normally already connected to the system as a HID device and
/// doesn't advertise, so `refresh()` asks CoreBluetooth for connected peripherals
/// exposing the vendor service before falling back to a scan.
@MainActor
public final class ClicksKeyboard: NSObject, ObservableObject {
    public enum Phase: Equatable {
        case unavailable(String)
        case idle
        case scanning
        case connecting
        case syncing(done: Int, total: Int)
        case ready
    }

    @Published public private(set) var phase: Phase = .idle
    @Published public private(set) var candidates: [CBPeripheral] = []
    @Published public private(set) var connected: CBPeripheral?
    @Published public private(set) var snapshot: KeyboardSnapshot
    @Published public private(set) var battery: Int?
    @Published public private(set) var model: String?
    @Published public private(set) var serial: String?
    @Published public private(set) var firmware: String?
    @Published public private(set) var lastError: String?

    public let session = ClicksSession()
    public var responseTimeout: TimeInterval = 3

    private let uuids = gattUuids()
    private lazy var serviceUUID = CBUUID(string: uuids.service)
    private lazy var notifyUUID = CBUUID(string: uuids.notify)
    private lazy var writeUUID = CBUUID(string: uuids.write)
    private lazy var batteryServiceUUID = CBUUID(string: uuids.batteryService)
    private lazy var batteryUUID = CBUUID(string: uuids.batteryLevel)
    private lazy var infoServiceUUID = CBUUID(string: uuids.deviceInfoService)
    private lazy var infoUUIDs: [CBUUID: ReferenceWritableKeyPath<ClicksKeyboard, String?>] = [
        CBUUID(string: uuids.modelNumber): \.model,
        CBUUID(string: uuids.serialNumber): \.serial,
        CBUUID(string: uuids.firmwareRevision): \.firmware,
    ]

    private var central: CBCentralManager!
    private var writeChar: CBCharacteristic?
    private var writeType: CBCharacteristicWriteType = .withResponse
    private var timeoutTask: Task<Void, Never>?
    private var syncTotal = 0

    public override init() {
        snapshot = ClicksSession().snapshot()
        super.init()
        central = CBCentralManager(delegate: self, queue: .main)
    }

    // MARK: Discovery

    public func refresh(scanFor seconds: TimeInterval = 4) {
        guard central.state == .poweredOn else { return }
        let known = central.retrieveConnectedPeripherals(withServices: [serviceUUID])
        merge(known)
        phase = .scanning
        central.scanForPeripherals(withServices: nil)
        Task { [weak self] in
            try? await Task.sleep(nanoseconds: UInt64(seconds * 1_000_000_000))
            guard let self, self.phase == .scanning else { return }
            self.central.stopScan()
            self.phase = .idle
        }
    }

    private func merge(_ peripherals: [CBPeripheral]) {
        for p in peripherals where !candidates.contains(where: { $0.identifier == p.identifier }) {
            candidates.append(p)
        }
    }

    public func connect(_ peripheral: CBPeripheral) {
        central.stopScan()
        disconnect()
        connected = peripheral
        peripheral.delegate = self
        phase = .connecting
        central.connect(peripheral)
    }

    /// Releases this app's handle. The OS keeps the HID link up.
    public func disconnect() {
        timeoutTask?.cancel()
        if let p = connected { central.cancelPeripheralConnection(p) }
        connected = nil
        writeChar = nil
        session.reset()
        snapshot = session.snapshot()
        battery = nil
        phase = .idle
    }

    // MARK: Commands

    public func sync() {
        session.enqueueSync()
        syncTotal = Int(session.pending())
        pump()
    }

    public func setRegister(_ id: String, _ value: UInt32) { run { try self.session.setRegister(id: id, value: value) } }
    public func setFlag(_ bit: UInt8, _ on: Bool) { run { try self.session.setFlag(bit: bit, on: on) } }
    public func setKeySlot(_ os: FfiOs, _ index: UInt8, _ action: FfiKeyAction?) {
        run { try self.session.setKeySlot(os: os, index: index, action: action) }
    }
    public func resetKeySlots(_ os: FfiOs) { run { self.session.resetKeySlots(os: os) } }
    public func setWirelessCharging(_ on: Bool) { run { self.session.setWirelessCharging(on: on) } }
    /// `slot` is 1-based; an empty name restores the default label.
    public func setHostName(_ slot: UInt8, _ name: String) { run { try self.session.setHostName(slot: slot, name: name) } }
    /// Switches a paired host between the iOS and Android key tables.
    public func setHostOs(_ slot: UInt8, _ os: FfiOs) { run { try self.session.setHostOs(slot: slot, os: os) } }
    public func removeHost(_ slot: UInt8) { run { try self.session.removeHost(slot: slot) } }

    private func run(_ body: () throws -> Void) {
        do {
            try body()
            pump()
        } catch {
            lastError = error.localizedDescription
        }
    }

    private func pump() {
        guard let p = connected, let c = writeChar, let frame = session.pollTransmit() else { return }
        p.writeValue(frame, for: c, type: writeType)
        timeoutTask?.cancel()
        timeoutTask = Task { [weak self, responseTimeout] in
            try? await Task.sleep(nanoseconds: UInt64(responseTimeout * 1_000_000_000))
            guard let self, !Task.isCancelled else { return }
            if self.session.handleTimeout() { self.lastError = "keyboard did not respond" }
            self.pump()
        }
    }

    private func handle(_ value: Data) {
        let events = session.handleNotification(bytes: value)
        guard !events.isEmpty else { return }
        timeoutTask?.cancel()
        for case let .rejected(opcode, status) in events {
            lastError = String(format: "opcode 0x%02x rejected (0x%02x)", opcode, status)
        }
        snapshot = session.snapshot()
        let pending = Int(session.pending())
        if case .syncing = phase {
            phase = pending == 0 ? .ready : .syncing(done: syncTotal - pending, total: syncTotal)
        }
        pump()
    }
}

extension ClicksKeyboard: CBCentralManagerDelegate {
    nonisolated public func centralManagerDidUpdateState(_ central: CBCentralManager) {
        MainActor.assumeIsolated {
            switch central.state {
            case .poweredOn: phase = .idle; refresh()
            case .unauthorized: phase = .unavailable("Bluetooth permission denied")
            case .unsupported: phase = .unavailable("Bluetooth LE unsupported")
            default: phase = .unavailable("Bluetooth is off")
            }
        }
    }

    nonisolated public func centralManager(
        _ central: CBCentralManager, didDiscover peripheral: CBPeripheral,
        advertisementData: [String: Any], rssi RSSI: NSNumber
    ) {
        MainActor.assumeIsolated {
            let name = peripheral.name ?? advertisementData[CBAdvertisementDataLocalNameKey] as? String ?? ""
            let services = advertisementData[CBAdvertisementDataServiceUUIDsKey] as? [CBUUID] ?? []
            if name.hasPrefix(uuids.namePrefix) || services.contains(serviceUUID) { merge([peripheral]) }
        }
    }

    nonisolated public func centralManager(_ central: CBCentralManager, didConnect peripheral: CBPeripheral) {
        MainActor.assumeIsolated {
            peripheral.discoverServices([serviceUUID, batteryServiceUUID, infoServiceUUID])
        }
    }

    nonisolated public func centralManager(
        _ central: CBCentralManager, didFailToConnect peripheral: CBPeripheral, error: Error?
    ) {
        MainActor.assumeIsolated {
            lastError = error?.localizedDescription ?? "connection failed"
            disconnect()
        }
    }

    nonisolated public func centralManager(
        _ central: CBCentralManager, didDisconnectPeripheral peripheral: CBPeripheral, error: Error?
    ) {
        MainActor.assumeIsolated {
            guard peripheral.identifier == connected?.identifier else { return }
            if let error { lastError = error.localizedDescription }
            disconnect()
        }
    }
}

extension ClicksKeyboard: CBPeripheralDelegate {
    nonisolated public func peripheral(_ peripheral: CBPeripheral, didDiscoverServices error: Error?) {
        MainActor.assumeIsolated {
            for s in peripheral.services ?? [] {
                switch s.uuid {
                case serviceUUID: peripheral.discoverCharacteristics([notifyUUID, writeUUID], for: s)
                case batteryServiceUUID: peripheral.discoverCharacteristics([batteryUUID], for: s)
                case infoServiceUUID: peripheral.discoverCharacteristics(Array(infoUUIDs.keys), for: s)
                default: break
                }
            }
        }
    }

    nonisolated public func peripheral(
        _ peripheral: CBPeripheral, didDiscoverCharacteristicsFor service: CBService, error: Error?
    ) {
        MainActor.assumeIsolated {
            for c in service.characteristics ?? [] {
                switch c.uuid {
                case notifyUUID:
                    peripheral.setNotifyValue(true, for: c)
                case writeUUID:
                    writeChar = c
                    writeType = c.properties.contains(.write) ? .withResponse : .withoutResponse
                case batteryUUID:
                    peripheral.readValue(for: c)
                    if c.properties.contains(.notify) { peripheral.setNotifyValue(true, for: c) }
                default:
                    if infoUUIDs[c.uuid] != nil { peripheral.readValue(for: c) }
                }
            }
        }
    }

    nonisolated public func peripheral(
        _ peripheral: CBPeripheral, didUpdateNotificationStateFor characteristic: CBCharacteristic, error: Error?
    ) {
        MainActor.assumeIsolated {
            guard characteristic.uuid == notifyUUID, error == nil, case .connecting = phase else { return }
            phase = .syncing(done: 0, total: 1)
            sync()
        }
    }

    nonisolated public func peripheral(
        _ peripheral: CBPeripheral, didUpdateValueFor characteristic: CBCharacteristic, error: Error?
    ) {
        MainActor.assumeIsolated {
            guard error == nil, let value = characteristic.value else { return }
            switch characteristic.uuid {
            case notifyUUID: handle(value)
            case batteryUUID: battery = value.first.map(Int.init)
            default:
                if let key = infoUUIDs[characteristic.uuid] {
                    self[keyPath: key] = String(decoding: value, as: UTF8.self)
                        .trimmingCharacters(in: CharacterSet(charactersIn: "\0"))
                }
            }
        }
    }
}
