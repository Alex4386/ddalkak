# Clicks Power Keyboard — configuration protocol

Reconstructed from an Android HCI snoop log of the official companion app
(`com.clicks.companionapp`) talking to a *Power Keyboard* (model `CP-1000`,
firmware `1.1.0`), then cross-checked against the decompiled app, which
supplied the names, units and the opcodes the capture didn't exercise.
Nothing here is vendor documentation.

## Transport

The keyboard is a normal BLE HID keyboard (HID vendor ID `0x07D7`) named
`Power Keyboard-XXXX` (last two address bytes). Configuration uses a
separate vendor service:

| UUID   | Role                                             |
|--------|--------------------------------------------------|
| `fff0` | service                                          |
| `fff1` | notify — responses (CCCD written `01 00`)        |
| `fff2` | write with response — requests                   |
| `180f`/`2a19` | battery level (read + notify)             |
| `180a` | device information: model `CP-1000`, serial, firmware `1.1.0` |

The official app finds keyboards among *bonded* devices whose name starts with
`Power keyboard` (case-insensitive), negotiates MTU 517 (the keyboard settles
on 248), writes with response and keeps exactly one request in flight
(3 s timeout). Each response arrives as one notification.

## Framing

```
FF  LEN  PAYLOAD…  CRC
```

* `LEN` = payload length + 1 (it counts the CRC byte).
* `CRC` = CRC-8, poly `0x1D` reflected (`0xB8`), init `0xFF`, no final XOR,
  over `LEN` and `PAYLOAD`. Verified against all 83 frames in the capture.

Request payload: `OPCODE ARGS…`.
Response payload: `02 OPCODE STATUS DATA…` (`STATUS 00` = OK).

## Opcodes

| Op   | Request args                     | Response data          | Notes |
|------|----------------------------------|------------------------|-------|
| `24` | `ADDR DATA…` (little-endian)     | —                      | write register |
| `25` | `ADDR LEN`                       | `LEN` bytes, LE        | read register |
| `26` | `MASK(2) VALUE(2)` big-endian    | —                      | set control bits; bit 2 = wireless charging |
| `27` | —                                | 4 bytes, e.g. `00 00 00 02` | status; bytes 2–3 = control bits (BE) |
| `28` | `OFFSET(2) RECORD(32)`           | —                      | write a host-name record |
| `29` | `OFFSET(2) LEN`                  | `LEN` bytes            | read host-name table |
| `A1` | `IMAGE_LEN` u32 BE               | —                      | firmware update: begin |
| `A2` | `TOKEN(4) CHUNK…`                | `TOKEN(4) …`           | firmware update: data; see below |
| `A3` | `IMAGE_LEN` u32 BE               | —                      | firmware update: finish |
| `A0` | —                                | (may not answer)       | firmware update: reboot |

The capture's lone `26 00 04 00 04` was the user enabling wireless charging.

## Registers

| Addr      | Size | Name (official app)          | Encoding |
|-----------|------|------------------------------|----------|
| `02`      | 1    | Backlight brightness         | 0–255; the app shows 0–100 % (`round(255·p)`) |
| `03`      | 1    | Power reserved for keyboard  | percent 0–50, step 10; 0 = all power to wireless charging |
| `04`      | 2    | Backlight dim timeout        | **ms**; app offers 2/5/15/30/45/60 s |
| `06`      | 2    | Idle timeout                 | **s** before disconnecting to save power; 2/5/15/30/45/60 min |
| `08`      | 1    | Feature switches             | bits below |
| `0C`      | 1    | Active host slot             | 1-based |
| `0D`–`0F` | 1 each | Host types                 | 2 bits per slot (below) |
| `12`–`13` | 2    | Key-slot enable mask         | LE, bit *n* = slot *n* |
| `16 + 4n` | 2    | Key slot *n*, *n* = 0…12     | below |

Factory values: `02`=`91`, `03`=30, `04`=5000, `06`=300, `08`=`05`, mask `FC 1F`.

### Feature switches (`08`)

| Bit | Name               | Behaviour |
|-----|--------------------|-----------|
| 0   | Backlight          | |
| 1   | Sym Lock           | double-tap Sym locks the symbol layer; Shift or Sym exits |
| 2   | Caps Lock          | double-tap Shift locks capitals |
| 3   | Soft return        | exposed only by the official iOS app |
| 4   | Cursor mode        | Sym + Meta turns WASD/IJKL into arrows until another key |
| 5   | Backlight shortcut | Sym + mic toggles the backlight |

### Key slots

13 slots at `0x16, 0x1A, … 0x46` (stride 4; only the first two bytes are
used). A disabled slot (mask bit clear) gives the key its normal behaviour.

| Slot | Addr | Key           | Official choices |
|------|------|---------------|------------------|
| 0    | `16` | Tab           | Tab, Ctrl+Space |
| 1    | `1A` | Gemini        | Gemini, `-`, Alt, Alt+D/K/S/. |
| 2    | `1E` | Alt           | Alt, Alt+D (voice), Alt+K (soft keyboard), Alt+S (symbols), Alt+. (emoji) |
| 3    | `22` | Backspace     | not in the UI; factory GUI+Left |
| 4–12 | `26`–`46` | Sym + 1 … Sym + 9 | Escape, Volume ±/Mute, Play/Pause, Next, Prev, Page Up/Down, Home, End, `[`, `]` |

The two groups use different value layouts:

| Slots | Layout | Examples |
|-------|--------|----------|
| 0–3 (remap) | `[modifier, key]`, either may be `00` | `E0 2C` Ctrl+Space, `E2 00` Alt, `00 2D` `-`, `E3 50` GUI+Left |
| 4–12 (Sym shortcuts) | `[usage, page]`, page `00` keyboard / `FF` consumer | `29 00` Esc, `80 00` Vol+, `CD FF` Play/Pause |

`FF FF` is an erased slot. Choosing the "native" option clears the mask bit
without touching the value; any other choice sets the bit, then writes the
value:

```
24 12 FD        enable slot 0 (mask FC → FD)
24 16 E0 2C     slot 0 = Ctrl+Space
24 12 FC        disable slot 0 again
```

### Paired hosts

Nine host slots, switched on the keyboard with power key + slot number.

* **Type** — 2 bits per slot: `0D` holds slots 1–4, `0E` 5–8, `0F` slot 9,
  lowest slot in the lowest bits. `0` empty, `2` iOS (also what a fresh
  pairing reports), `3` Android, `1` never used by the app. After connecting,
  the official app flips its own slot from `2` to `3`.
* **Name** — 32-byte records, slot *s* at offset `32·(s−1)`:
  `[len, UTF-8 name zero-padded to 30 bytes, xor]` where `xor` is the XOR of
  the first 31 bytes. Erased flash (`FF…`) fails the check and means "no
  name"; the app then shows the type. Read with `29` in three 96-byte blocks,
  written one record at a time with `28`.
* **Remove device** — write an empty record (`len 0`, all zero) and set the
  type to `0`. The app refuses this for the active slot.

Captured state: `0C`=`02`, `0D`=`8E`, `0E`=`0B` → slot 1 iOS ("iPhone 15 Pro"),
slot 2 Android (active, the capturing phone), slot 4 iOS, slot 5 Android,
slot 6 iOS.

### Firmware update

The official app polls `https://xinyi1.clicks.tech/firmwares-p.json` (BLE
keyboards; `firmwares-a.json` is for the USB cases):

```json
[{ "model": "CP-1000", "firmwareRevision": "1.1.2",
   "file": "https://xinyi1.clicks.tech/CP-1000/CP-1000-V1.12.bin",
   "sha256sum": "212db31a…2e78" }]
```

It offers the highest `firmwareRevision` (compared numerically per dotted
component) for the keyboard's model (`2a24`) that is newer than its `2a26`
revision, and checks the SHA-256 after downloading. Published so far:
1.0.8, 1.0.9, 1.1.0, 1.1.2. `clicks firmware` and the desktop app do the same
check and can download and verify an image; they don't flash it.

Flashing over BLE:

1. `A1 len` (u32 BE) — begin, expect status `00`.
2. Split the image into `MTU − 11`-byte chunks. Send `A2 token chunk` for each,
   starting with token `00 00 00 00`; each response's data starts with the
   token for the next chunk.
3. `A3 len` — finish.
4. `A0` — reboot (300 ms timeout; no answer expected).

## Firmware image notes

From `CP-1000-V1.12.bin` (unencrypted, 175 044 bytes):

* WCH **CH585** RISC-V BLE SoC (`CH585_BLE_LIB_V1.4.4`); disassembles with
  `llvm-objdump --triple=riscv32 --mattr=+m,+a,+c,+zicsr,+xwchc`.
* The image is linked at flash `0x3000` (the reset handler copies
  file `0x204–0x476c` to RAM `0x20000200` and `.data` from file `0x2a5a4`
  to `0x20004768`); presumably a bootloader occupies `0x0000–0x2FFF`.
* Frame parser at file `0xa20c` (CRC-8 at `0xa186`), opcode dispatcher at
  `0xa812`. Besides the opcodes above it handles `03`, `08` and `09`, which
  the app never sends: `09 MASK VALUE` toggles three internal switches
  (bits 0–2), `08` takes up to 15 bytes and runs them through a table with an
  XOR check. Their purpose is unknown — don't send them.
* Registers live in RAM at `0x20005644 + addr`. Writing `0C` (active host)
  also copies that host's 2-bit type into a status byte the key handling
  reads, so iOS/Android behaviour follows the type of the active slot.
* One HID report map (no report IDs): keyboard with LED output (Caps Lock),
  modifiers declared as `E0–E3` only, 4 key slots, then a 16-bit consumer
  field starting with AL Keyboard Layout (`0x1AE`), Globe (`0x29D`) and
  Voice Command (`0xCF`).

## Connect sequence used by the official app

```
25 08 01   25 06 02   25 02 01   25 04 02   27
25 03 01   25 12 01   25 13 01   25 16 02 … 25 46 02 (13 slots)
25 0C 01   25 0D 01   25 0E 01   25 0F 01
29 0000 60   29 0060 60   29 00C0 60
```

The official app then reads `2a26`/`2a24`/`2a25` and battery, and (on an
Android host) rewrites the active slot's type as described above.
`clicks_core::model::sync_plan()` reproduces the vendor-service part
byte-for-byte (see the
`replays_captured_sync` test).
