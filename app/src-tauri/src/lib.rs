use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use clicks_ble::btleplug::platform::{Adapter, Peripheral};
use clicks_ble::{Event, Keyboard};
use clicks_core::firmware::{self, FirmwareInfo};
use clicks_core::keymap::KeyAction;
use clicks_core::view::{self, Snapshot};
use clicks_core::Request;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::sync::{Mutex, OnceCell};

#[derive(Default)]
struct AppState {
    adapter: OnceCell<Adapter>,
    seen: Mutex<HashMap<String, Peripheral>>,
    keyboard: Mutex<Option<Arc<Keyboard>>>,
}

type CmdResult<T> = Result<T, String>;

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

impl AppState {
    async fn adapter(&self) -> CmdResult<&Adapter> {
        self.adapter.get_or_try_init(clicks_ble::default_adapter).await.map_err(err)
    }

    async fn keyboard(&self) -> CmdResult<Arc<Keyboard>> {
        self.keyboard.lock().await.clone().ok_or_else(|| "not connected".to_owned())
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DeviceDto {
    id: String,
    name: Option<String>,
    address: String,
    connected: bool,
}

#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
struct InfoDto {
    model: Option<String>,
    serial: Option<String>,
    firmware: Option<String>,
    battery: Option<u8>,
}

#[tauri::command]
async fn list_devices(state: State<'_, AppState>, scan_secs: u64) -> CmdResult<Vec<DeviceDto>> {
    let adapter = state.adapter().await?;
    let found = clicks_ble::discover(adapter, Duration::from_secs(scan_secs)).await.map_err(err)?;
    let mut seen = state.seen.lock().await;
    Ok(found
        .into_iter()
        .map(|(p, c)| {
            let id = c.id.to_string();
            seen.insert(id.clone(), p);
            DeviceDto { id, name: c.name, address: c.address, connected: c.connected }
        })
        .collect())
}

#[tauri::command]
async fn connect(app: AppHandle, state: State<'_, AppState>, id: String) -> CmdResult<Snapshot> {
    if let Some(old) = state.keyboard.lock().await.take() {
        old.close().await;
    }
    let peripheral = state.seen.lock().await.get(&id).cloned().ok_or("unknown device; refresh the list")?;
    let adapter = state.adapter().await?;
    let kb = Keyboard::open(adapter, peripheral).await.map_err(err)?;

    let mut events = kb.events();
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        while let Ok(e) = events.recv().await {
            let done = e == Event::Disconnected;
            let payload = match e {
                Event::Battery(level) => KeyboardEvent { kind: "battery", value: Some(level as u32) },
                Event::Disconnected => KeyboardEvent { kind: "disconnected", value: None },
                Event::Unsolicited(_) => continue,
            };
            let _ = handle.emit("keyboard", payload);
            if done {
                break;
            }
        }
    });

    *state.keyboard.lock().await = Some(kb.clone());
    let s = kb
        .sync(|done, total| {
            let _ = app.emit("sync-progress", (done, total));
        })
        .await
        .map_err(err)?;
    Ok(view::snapshot(&s))
}

#[derive(Serialize, Clone)]
struct KeyboardEvent {
    kind: &'static str,
    value: Option<u32>,
}

#[tauri::command]
async fn disconnect(state: State<'_, AppState>) -> CmdResult<()> {
    if let Some(kb) = state.keyboard.lock().await.take() {
        kb.close().await;
    }
    Ok(())
}

#[tauri::command]
async fn device_info(state: State<'_, AppState>) -> CmdResult<InfoDto> {
    let kb = state.keyboard().await?;
    let info = kb.device_info().await.map_err(err)?;
    Ok(InfoDto {
        model: info.model,
        serial: info.serial,
        firmware: info.firmware.or(info.software),
        battery: kb.battery().await.ok().flatten(),
    })
}

#[tauri::command]
async fn resync(app: AppHandle, state: State<'_, AppState>) -> CmdResult<Snapshot> {
    let kb = state.keyboard().await?;
    let s = kb
        .sync(|done, total| {
            let _ = app.emit("sync-progress", (done, total));
        })
        .await
        .map_err(err)?;
    Ok(view::snapshot(&s))
}

#[tauri::command]
async fn set_register(state: State<'_, AppState>, id: String, value: u32) -> CmdResult<Snapshot> {
    let kb = state.keyboard().await?;
    let s = kb.update(|s| s.set_register(&id, value)).await.map_err(err)?;
    Ok(view::snapshot(&s))
}

#[tauri::command]
async fn set_flag(state: State<'_, AppState>, bit: u8, on: bool) -> CmdResult<Snapshot> {
    let kb = state.keyboard().await?;
    let s = kb.update(|s| s.set_flag(bit, on)).await.map_err(err)?;
    Ok(view::snapshot(&s))
}

/// `action` is parsed with `KeyAction::from_str`; `None` disables the slot.
#[tauri::command]
async fn set_key_slot(state: State<'_, AppState>, index: u8, action: Option<String>) -> CmdResult<Snapshot> {
    let action = action.map(|a| a.parse::<KeyAction>()).transpose().map_err(err)?;
    let kb = state.keyboard().await?;
    let s = kb.update(|s| s.set_key_slot(index, action)).await.map_err(err)?;
    Ok(view::snapshot(&s))
}

#[tauri::command]
async fn set_wireless_charging(state: State<'_, AppState>, on: bool) -> CmdResult<Snapshot> {
    let kb = state.keyboard().await?;
    let s = kb.update(|s| Ok(s.set_wireless_charging(on))).await.map_err(err)?;
    Ok(view::snapshot(&s))
}

#[tauri::command]
async fn set_host_name(state: State<'_, AppState>, slot: u8, name: String) -> CmdResult<Snapshot> {
    let kb = state.keyboard().await?;
    let s = kb.update(|s| s.set_host_name(slot, &name)).await.map_err(err)?;
    Ok(view::snapshot(&s))
}

#[tauri::command]
async fn remove_host(state: State<'_, AppState>, slot: u8) -> CmdResult<Snapshot> {
    let kb = state.keyboard().await?;
    let s = kb.update(|s| s.remove_host(slot)).await.map_err(err)?;
    Ok(view::snapshot(&s))
}

#[tauri::command]
async fn reset_key_slots(state: State<'_, AppState>) -> CmdResult<Snapshot> {
    let kb = state.keyboard().await?;
    let s = kb.update(|s| Ok(s.reset_key_slots())).await.map_err(err)?;
    Ok(view::snapshot(&s))
}

#[derive(Serialize)]
struct RawReply {
    status: u8,
    data: Vec<u8>,
}

#[tauri::command]
async fn send_raw(state: State<'_, AppState>, payload: Vec<u8>) -> CmdResult<RawReply> {
    let (&opcode, args) = payload.split_first().ok_or("empty payload")?;
    let kb = state.keyboard().await?;
    let r = kb.request(&Request::Raw { opcode, args: args.to_vec() }).await.map_err(err)?;
    Ok(RawReply { status: r.status, data: r.data })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FirmwareStatus {
    model: String,
    installed: Option<String>,
    latest: Option<FirmwareInfo>,
    update: Option<FirmwareInfo>,
}

async fn fetch_manifest() -> CmdResult<Vec<FirmwareInfo>> {
    let json = reqwest::get(firmware::MANIFEST_URL)
        .await
        .and_then(|r| r.error_for_status())
        .map_err(err)?
        .text()
        .await
        .map_err(err)?;
    firmware::parse_manifest(&json).map_err(|e| format!("unexpected manifest format: {e}"))
}

/// Looks up the newest published firmware for the connected keyboard (or the
/// Power Keyboard model when none is connected).
#[tauri::command]
async fn check_firmware(state: State<'_, AppState>) -> CmdResult<FirmwareStatus> {
    let (model, installed) = match state.keyboard().await {
        Ok(kb) => {
            let info = kb.device_info().await.map_err(err)?;
            (info.model.unwrap_or_else(|| "CP-1000".into()), info.firmware.or(info.software))
        }
        Err(_) => ("CP-1000".into(), None),
    };
    let manifest = fetch_manifest().await?;
    Ok(FirmwareStatus {
        latest: firmware::latest(&manifest, &model).cloned(),
        update: installed.as_deref().and_then(|v| firmware::select_update(&manifest, &model, v)).cloned(),
        model,
        installed,
    })
}

/// Downloads `info`, checks its SHA-256 and saves it to the Downloads folder.
#[tauri::command]
async fn download_firmware(app: AppHandle, info: FirmwareInfo) -> CmdResult<String> {
    if !info.url.starts_with("https://xinyi1.clicks.tech/") {
        return Err("refusing to download from an unexpected host".into());
    }
    let image = reqwest::get(&info.url)
        .await
        .and_then(|r| r.error_for_status())
        .map_err(err)?
        .bytes()
        .await
        .map_err(err)?;
    if !firmware::verify(&image, &info.sha256) {
        return Err("downloaded image failed the SHA-256 check".into());
    }
    let dir = app.path().download_dir().or_else(|_| app.path().app_data_dir()).map_err(err)?;
    std::fs::create_dir_all(&dir).map_err(err)?;
    let path = dir.join(firmware::file_name(&info));
    std::fs::write(&path, &image).map_err(err)?;
    Ok(path.display().to_string())
}

/// Names the key-slot editor offers as suggestions.
#[tauri::command]
fn key_names() -> Vec<String> {
    view::key_names()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    env_logger::init();
    tauri::Builder::default()
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            list_devices,
            connect,
            disconnect,
            device_info,
            resync,
            set_register,
            set_flag,
            set_key_slot,
            set_wireless_charging,
            set_host_name,
            remove_host,
            reset_key_slots,
            send_raw,
            key_names,
            check_firmware,
            download_firmware,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
