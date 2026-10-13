//! Protocol core for the Clicks Power Keyboard configuration service.
//!
//! Everything here is transport-agnostic ("sans-IO"): it turns intents into
//! bytes for the write characteristic and bytes from the notify characteristic
//! into state. BLE plumbing lives in `clicks-ble` (btleplug) or in the host
//! platform (e.g. CoreBluetooth via the `clicks-ffi` bindings).

pub mod firmware;
pub mod frame;
pub mod hosts;
pub mod keymap;
pub mod model;
pub mod protocol;
pub mod registers;
pub mod session;
#[cfg(feature = "serde")]
pub mod view;

pub use keymap::KeyAction;
pub use hosts::{Host, HostKind};
pub use model::{KeySlot, KeyboardState, ModelError};
pub use protocol::{Message, Request, Response};
pub use session::{Session, SessionEvent};

pub mod gatt {
    //! GATT layout. The keyboard is also a regular HID-over-GATT keyboard; the
    //! configuration channel is a separate vendor service.

    /// Vendor configuration service.
    pub const SERVICE: &str = "0000fff0-0000-1000-8000-00805f9b34fb";
    /// Notify: framed responses from the keyboard.
    pub const NOTIFY: &str = "0000fff1-0000-1000-8000-00805f9b34fb";
    /// Write (with response): framed requests to the keyboard.
    pub const WRITE: &str = "0000fff2-0000-1000-8000-00805f9b34fb";

    pub const BATTERY_SERVICE: &str = "0000180f-0000-1000-8000-00805f9b34fb";
    pub const BATTERY_LEVEL: &str = "00002a19-0000-1000-8000-00805f9b34fb";

    pub const DEVICE_INFO_SERVICE: &str = "0000180a-0000-1000-8000-00805f9b34fb";
    pub const MANUFACTURER_NAME: &str = "00002a29-0000-1000-8000-00805f9b34fb";
    pub const MODEL_NUMBER: &str = "00002a24-0000-1000-8000-00805f9b34fb";
    pub const SERIAL_NUMBER: &str = "00002a25-0000-1000-8000-00805f9b34fb";
    pub const HARDWARE_REVISION: &str = "00002a27-0000-1000-8000-00805f9b34fb";
    pub const FIRMWARE_REVISION: &str = "00002a26-0000-1000-8000-00805f9b34fb";
    pub const SOFTWARE_REVISION: &str = "00002a28-0000-1000-8000-00805f9b34fb";

    /// Advertised/GAP name is `"Power Keyboard-XXXX"` (last 4 hex digits of the address).
    pub const NAME_PREFIX: &str = "Power Keyboard";
}
