# ddalkak — Clicks Power Keyboard companion

Cross-platform configuration tool for the Clicks Power Keyboard, built from a
reverse-engineered BLE protocol ([docs/PROTOCOL.md](docs/PROTOCOL.md)).

```
crates/clicks-core   sans-IO protocol: framing, CRC, registers, key slots, request queue
crates/clicks-ble    btleplug transport (Windows / macOS / Linux)
crates/clicks-cli    `clicks` command-line tool
crates/clicks-ffi    UniFFI bindings (Swift, Kotlin) around clicks-core
app/                 Tauri 2 desktop app (Rust backend + vanilla TS frontend)
swift/               Swift package: ClicksCore bindings + ClicksKit (CoreBluetooth)
```

The core never touches Bluetooth, so the same Rust code drives the Tauri app
(through btleplug) and a native Swift app (through CoreBluetooth).

## Desktop app

Prerequisites: Rust, [bun](https://bun.sh), and on Linux:

```sh
sudo apt install libwebkit2gtk-4.1-dev libgtk-3-dev librsvg2-dev \
  libayatana-appindicator3-dev libdbus-1-dev libssl-dev libxdo-dev pkg-config build-essential
```

```sh
cd app
bun install
bun run tauri dev      # or: bun run tauri build
```

Pair the keyboard with the computer first; the app looks for already-connected
keyboards before scanning. Closing the app never drops the keyboard's HID link.

## CLI

```sh
cargo run -p clicks-cli -- dump
cargo run -p clicks-cli -- set idleTimeout 3600     # ids: brightness dimTimeout idleTimeout chargeReserve
cargo run -p clicks-cli -- flag capsLock off        # backlight symLock capsLock softReturn cursorMode backlightShortcut
cargo run -p clicks-cli -- charging on
cargo run -p clicks-cli -- key 0 ctrl+space         # slot 0 = Tab key; `key 0 off` restores Tab
cargo run -p clicks-cli -- key 4 play/pause         # Sym + 1
cargo run -p clicks-cli -- host-name 1 "Work phone"
cargo run -p clicks-cli -- firmware                 # check for updates; --download --out DIR saves + verifies
cargo run -p clicks-cli -- raw 25 08 01
```

## Swift

```sh
scripts/build-apple.sh     # macOS: builds swift/ClicksCoreFFI.xcframework + bindings
```

Then add `swift/` as a local package and use either layer:

* `ClicksKit.ClicksKeyboard`: an `ObservableObject` that owns the CoreBluetooth
  connection and publishes `snapshot`, `battery`, `phase`.
* `ClicksCore.ClicksSession`: the raw sans-IO session. Feed `fff1`
  notifications to `handleNotification(bytes:)` and write every
  `pollTransmit()` frame to `fff2`.

`scripts/gen-bindings.sh` generates Swift/Kotlin sources on any OS.

## Tests

```sh
cargo test
```

`clicks-core` tests replay frames from the original capture, so encoding,
CRC and the connect sequence are checked byte-for-byte against the official app.
