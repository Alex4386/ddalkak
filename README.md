# ddalkak — Clicks Power Keyboard companion

Cross-platform configuration tool for the Clicks Power Keyboard, built from a
reverse-engineered BLE protocol ([docs/PROTOCOL.md](docs/PROTOCOL.md)).

```
crates/clicks-core   sans-IO protocol: framing, CRC, registers, key slots, request queue
crates/clicks-ble    btleplug transport (Windows / macOS / Linux)
crates/clicks-cli    `clicks` command-line tool
crates/clicks-ffi    UniFFI bindings (Swift, Kotlin) around clicks-core
crates/clicks-wasm   wasm-bindgen bindings around clicks-core, for the browser
web/                 the UI (React + shadcn/ui + ReUI), shared by the browser and the desktop app
app/                 Tauri 2 desktop shell (Rust backend; loads web/)
swift/               Swift package: ClicksCore bindings + ClicksKit (CoreBluetooth)
```

The core never touches Bluetooth, so the same Rust code drives the Tauri app
(through btleplug), the browser (WebAssembly + Web Bluetooth) and a native
Swift app (through CoreBluetooth).

## Web app

The same UI runs in the browser over Web Bluetooth, with `clicks-core`
compiled to WebAssembly. Prerequisites: Rust with the wasm target, a matching
`wasm-bindgen` CLI, and [bun](https://bun.sh):

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.129 --locked   # must match crates/clicks-wasm
cd web
bun install
bun run dev        # http://localhost:1420
bun run build      # static site in web/dist (needs HTTPS when hosted)
```

Browser support and limits:

* Chrome or Edge on Windows, macOS, ChromeOS and Android. On Linux, turn on
  `chrome://flags/#enable-web-bluetooth` first. Firefox and Safari don't
  implement Web Bluetooth.
* The picker only lists keyboards the browser can see. On Linux the
  already-connected keyboard shows up; on macOS, Windows and Android the
  browser only finds devices that are advertising.
* The serial number (`2a25`) is on the Web Bluetooth blocklist, so the web app
  doesn't show it.
* The firmware server sends no CORS headers, so the browser can't check for
  updates; use the desktop app or `clicks firmware`.
* Leaving the page never drops the keyboard's HID connection: the app stops
  notifications instead of disconnecting, and Chrome leaves paired devices
  connected.

## Desktop app

Prerequisites: Rust, [bun](https://bun.sh), and on Linux:

```sh
sudo apt install libwebkit2gtk-4.1-dev libgtk-3-dev librsvg2-dev \
  libayatana-appindicator3-dev libdbus-1-dev libssl-dev libxdo-dev pkg-config build-essential
```

```sh
(cd web && bun install)    # the UI; also needs the wasm prerequisites above
cd app
bun install
bun run tauri dev          # or: bun run tauri build
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
