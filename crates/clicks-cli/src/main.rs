use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use clicks_ble::{discover, Keyboard};
use clicks_core::registers::{self, RegisterKind};
use clicks_core::{firmware, frame, hosts, keymap, KeyAction, KeyboardState, Request};

#[derive(Parser)]
#[command(name = "clicks", about = "Configure a Clicks Power Keyboard over BLE")]
struct Cli {
    /// Name, address or id substring of the keyboard to use (default: first found).
    #[arg(short, long, global = true)]
    device: Option<String>,
    /// Seconds to scan if the keyboard isn't already connected to this computer.
    #[arg(long, global = true, default_value_t = 4)]
    scan: u64,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// List keyboards.
    List,
    /// Show device information, settings, key slots and hosts.
    Dump,
    /// Set a register: `clicks set idleTimeout 3600`. Run `dump` for ids.
    Set { id: String, value: u32 },
    /// Turn a keyboard feature on or off: `clicks flag capsLock off`.
    Flag { name: String, state: OnOff },
    /// Enable or disable wireless charging pass-through.
    Charging { state: OnOff },
    /// Assign a key slot (`clicks key 0 ctrl+space`) or disable it (`clicks key 0 off`).
    Key { slot: u8, action: String },
    /// Name a paired host slot (1–9); an empty name restores the default.
    HostName { slot: u8, name: String },
    /// Forget a paired host slot (1–9).
    HostRemove { slot: u8 },
    /// Check for a firmware update and optionally download it (flashing is not supported).
    Firmware {
        /// Download and verify the image.
        #[arg(long)]
        download: bool,
        /// Directory to save the image in.
        #[arg(long, default_value = ".")]
        out: PathBuf,
        /// Don't connect to a keyboard; just show the newest image.
        #[arg(long)]
        offline: bool,
    },
    /// Read every register (0x00–0x47) and print them; read-only. Use `--save`
    /// before changing a setting in the official app and `--diff` after it to
    /// see which bytes it touched.
    Regs {
        #[arg(long)]
        save: Option<PathBuf>,
        #[arg(long)]
        diff: Option<PathBuf>,
    },
    /// Restore all key slots to factory defaults.
    ResetKeys,
    /// Send a raw request payload (hex, without framing): `clicks raw 2502 01`.
    Raw { hex: Vec<String> },
}

#[derive(Clone, Copy, clap::ValueEnum)]
enum OnOff {
    On,
    Off,
}

async fn open(cli: &Cli) -> Result<Arc<Keyboard>> {
    let adapter = clicks_ble::default_adapter().await?;
    let mut found = discover(&adapter, Duration::ZERO).await?;
    if found.is_empty() && cli.scan > 0 {
        eprintln!("scanning for {}s…", cli.scan);
        found = discover(&adapter, Duration::from_secs(cli.scan)).await?;
    }
    let (peripheral, cand) = found
        .into_iter()
        .find(|(_, c)| match &cli.device {
            None => true,
            Some(q) => {
                let q = q.to_lowercase();
                [c.name.clone().unwrap_or_default(), c.address.clone(), c.id.to_string()]
                    .iter()
                    .any(|s| s.to_lowercase().contains(&q))
            }
        })
        .context("no Power Keyboard found (is it paired and powered on?)")?;
    eprintln!("using {} ({})", cand.name.as_deref().unwrap_or("?"), cand.address);
    Ok(Keyboard::open(&adapter, peripheral).await?)
}

fn print_state(state: &KeyboardState) {
    println!("\nSettings");
    for r in registers::SETTINGS {
        let v = state.register(r).map(|v| v.to_string()).unwrap_or_else(|| "?".into());
        let extra = match r.kind {
            RegisterKind::Flags { .. } => state.register(r).map(|v| format!(" ({v:#010b})")).unwrap_or_default(),
            RegisterKind::Duration { unit_ms: 1, .. } => " ms".into(),
            RegisterKind::Duration { .. } => " s".into(),
            _ => String::new(),
        };
        println!("  {:<14} [{:#04x}] {:<28} {v}{extra}", r.id, r.addr, r.label);
        if let (RegisterKind::Flags { bits }, Some(v)) = (r.kind, state.register(r)) {
            for b in bits {
                let on = v & (1 << b.bit) != 0;
                println!("    {:<20} bit {}  {}", b.id, b.bit, if on { "on" } else { "off" });
            }
        }
    }
    if let Some(on) = state.wireless_charging() {
        println!("  {:<14} [0x26]  {:<28} {}", "charging", "Wireless charging", if on { "on" } else { "off" });
    }
    if let Some(s) = state.status() {
        println!("  {:<14}        {:<28} {:02x?}", "status", "", s);
    }
    println!("\nKey slots");
    for s in state.key_slots() {
        let native = keymap::SLOTS[s.index as usize].native;
        let action = if s.enabled { s.description.clone().unwrap_or_default() } else { format!("({native})") };
        println!("  {:>2} [{:#04x}] {:<14} {action}", s.index, keymap::slot_addr(s.index), s.label);
    }
    println!("\nPaired hosts");
    for h in state.hosts() {
        println!("  {} {:<7} {}{}", h.slot, h.kind.label(), h.name.as_deref().unwrap_or(""), if h.active { "  (active)" } else { "" });
    }
}

fn parse_hex(parts: &[String]) -> Result<Vec<u8>> {
    let s: String = parts.concat().chars().filter(|c| !c.is_whitespace()).collect();
    if !s.len().is_multiple_of(2) {
        bail!("odd number of hex digits");
    }
    (0..s.len()).step_by(2).map(|i| Ok(u8::from_str_radix(&s[i..i + 2], 16)?)).collect()
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    let cli = Cli::parse();

    if let Command::List = cli.command {
        let adapter = clicks_ble::default_adapter().await?;
        for (_, c) in discover(&adapter, Duration::from_secs(cli.scan)).await? {
            println!(
                "{}\t{}\t{}",
                c.address,
                c.name.as_deref().unwrap_or("?"),
                if c.connected { "connected" } else { "" }
            );
        }
        return Ok(());
    }

    if let Command::Firmware { download, out, offline } = &cli.command {
        let installed = if *offline {
            None
        } else {
            let kb = open(&cli).await?;
            let info = kb.device_info().await;
            kb.close().await;
            let info = info?;
            Some((info.model.unwrap_or_default(), info.firmware.or(info.software).unwrap_or_default()))
        };
        return firmware_cmd(installed, *download, out).await;
    }

    let kb = open(&cli).await?;
    let result = run(&cli, &kb).await;
    kb.close().await;
    result
}

async fn run(cli: &Cli, kb: &Keyboard) -> Result<()> {
    match &cli.command {
        Command::List | Command::Firmware { .. } => unreachable!(),
        Command::Dump => {
            let info = kb.device_info().await?;
            println!("Model     {}", info.model.as_deref().unwrap_or("?"));
            println!("Serial    {}", info.serial.as_deref().unwrap_or("?"));
            println!("Firmware  {}", info.firmware.or(info.software).as_deref().unwrap_or("?"));
            if let Some(b) = kb.battery().await? {
                println!("Battery   {b}%");
            }
            let state = kb.sync(|i, n| eprint!("\rreading {i}/{n}")).await?;
            eprintln!();
            print_state(&state);
        }
        Command::Set { id, value } => {
            kb.update(|s| s.set_register(id, *value)).await?;
            println!("{id} = {value}");
        }
        Command::Flag { name, state } => {
            let bit = registers::feature_bit(name).with_context(|| format!("unknown feature {name:?}"))?;
            sync_one(kb, &[registers::FEATURES.addr]).await?;
            let s = kb.update(|s| s.set_flag(bit.bit, matches!(state, OnOff::On))).await?;
            println!("features = {:#010b}", s.register(&registers::FEATURES).unwrap_or(0));
        }
        Command::Charging { state } => {
            let s = kb.update(|s| Ok(s.set_wireless_charging(matches!(state, OnOff::On)))).await?;
            kb.request(&Request::ReadStatus).await?;
            println!("wireless charging: {:?}", kb.state().wireless_charging().or(s.wireless_charging()));
        }
        Command::HostName { slot, name } => {
            kb.update(|s| s.set_host_name(*slot, name)).await?;
            println!("host {slot} renamed");
        }
        Command::HostRemove { slot } => {
            sync_one(kb, &[hosts::ACTIVE_ADDR, hosts::kind_location((*slot).clamp(1, hosts::SLOT_COUNT)).0]).await?;
            kb.update(|s| s.remove_host(*slot)).await?;
            println!("host {slot} removed");
        }
        Command::Key { slot, action } => {
            let action = match action.as_str() {
                "off" | "none" => None,
                a => Some(a.parse::<KeyAction>()?),
            };
            sync_one(kb, &[keymap::ENABLE_MASK_ADDR, keymap::ENABLE_MASK_ADDR + 1]).await?;
            let s = kb.update(|s| s.set_key_slot(*slot, action)).await?;
            if let Some(k) = s.key_slot(*slot) {
                println!("slot {slot}: {}", if k.enabled { k.description.unwrap_or_default() } else { "off".into() });
            }
        }
        Command::Regs { save, diff } => {
            let mut regs = Vec::with_capacity(REG_SPACE as usize);
            for addr in (0..REG_SPACE).step_by(16) {
                let len = 16.min(REG_SPACE - addr);
                let r = kb.request(&Request::ReadRegister { addr, len }).await?;
                if !r.is_ok() || r.data.len() != len as usize {
                    bail!("reading {addr:#04x}+{len} failed (status {:#04x})", r.status);
                }
                regs.extend(r.data);
            }
            for (row, chunk) in regs.chunks(16).enumerate() {
                println!("{:02x}: {}", row * 16, chunk.iter().map(|b| format!("{b:02x}")).collect::<Vec<_>>().join(" "));
            }
            if let Some(path) = diff {
                let old = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
                println!("\nChanged since {}:", path.display());
                let mut any = false;
                for (addr, (a, b)) in old.iter().zip(&regs).enumerate() {
                    if a != b {
                        any = true;
                        let name = registers::by_addr(addr as u8).map(|r| r.id).unwrap_or("");
                        println!("  {addr:#04x}  {a:02x} -> {b:02x}  {name}");
                    }
                }
                if !any {
                    println!("  nothing");
                }
            }
            if let Some(path) = save {
                std::fs::write(path, &regs)?;
                eprintln!("saved to {}", path.display());
            }
        }
        Command::ResetKeys => {
            sync_one(kb, &[keymap::ENABLE_MASK_ADDR + 1]).await?;
            kb.update(|s| Ok(s.reset_key_slots())).await?;
            println!("key slots restored");
        }
        Command::Raw { hex } => {
            let payload = parse_hex(hex)?;
            let (&opcode, args) = payload.split_first().context("empty payload")?;
            let req = Request::Raw { opcode, args: args.to_vec() };
            println!("> {:02x?}", frame::encode(&payload)?);
            let r = kb.request(&req).await?;
            println!("< status {:#04x} data {:02x?}", r.status, r.data);
        }
    }
    Ok(())
}

async fn firmware_cmd(installed: Option<(String, String)>, download: bool, out: &std::path::Path) -> Result<()> {
    let http = reqwest::Client::new();
    let json = http.get(firmware::MANIFEST_URL).send().await?.error_for_status()?.text().await?;
    let manifest = firmware::parse_manifest(&json).context("unexpected manifest format")?;
    let (model, current) = installed.clone().unwrap_or_else(|| ("CP-1000".into(), String::new()));
    let latest = firmware::latest(&manifest, &model).with_context(|| format!("no firmware published for {model}"))?;
    println!("Model      {model}");
    if installed.is_some() {
        println!("Installed  {current}");
    }
    println!("Latest     {}", latest.version);
    let target = match &installed {
        Some(_) => match firmware::select_update(&manifest, &model, &current) {
            Some(f) => {
                println!("Update available: {}", f.version);
                f
            }
            None => {
                println!("Up to date.");
                if !download {
                    return Ok(());
                }
                latest
            }
        },
        None => latest,
    };
    if !download {
        println!("Run with --download to fetch {}.", target.url);
        return Ok(());
    }
    let image = http.get(&target.url).send().await?.error_for_status()?.bytes().await?;
    if !firmware::verify(&image, &target.sha256) {
        bail!("SHA-256 mismatch for {}; not saved", target.url);
    }
    std::fs::create_dir_all(out)?;
    let path = out.join(firmware::file_name(target));
    std::fs::write(&path, &image)?;
    println!("Saved {} ({} bytes, SHA-256 verified)", path.display(), image.len());
    println!("Flashing is not supported by this tool; use the official app to install it.");
    Ok(())
}

/// Size of the register file the official app reads from (`0x00`–`0x47`).
const REG_SPACE: u8 = 0x48;

/// Reads single-byte registers needed to compute a read-modify-write.
async fn sync_one(kb: &Keyboard, addrs: &[u8]) -> Result<()> {
    for &addr in addrs {
        kb.request(&Request::ReadRegister { addr, len: 1 }).await?;
    }
    Ok(())
}
