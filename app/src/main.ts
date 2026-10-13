import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

type RegisterKind =
  | { type: "level"; min: number; max: number; step: number; unit: string }
  | { type: "duration"; unitMs: number; presets: number[] }
  | { type: "flags"; bits: { bit: number; id: string; label: string; note: string }[] }
  | { type: "readOnly" };

interface Register {
  id: string;
  addr: number;
  len: number;
  kind: RegisterKind;
  label: string;
  note: string;
  value: number | null;
}

interface Slot {
  index: number;
  addr: number;
  kind: "remap" | "shortcut";
  label: string;
  native: string;
  enabled: boolean;
  description: string | null;
  factory: string | null;
  presets: (string | null)[];
}

interface Host {
  slot: number;
  kind: "empty" | "unknown" | "ios" | "android";
  name: string | null;
  displayName: string;
  active: boolean;
}

interface Snapshot {
  synced: boolean;
  settings: Register[];
  info: Register[];
  keySlots: Slot[];
  hosts: Host[];
  wirelessCharging: boolean | null;
  status: number[] | null;
}

interface Device {
  id: string;
  name: string | null;
  address: string;
  connected: boolean;
}

interface FirmwareInfo {
  model: string;
  version: string;
  url: string;
  sha256: string;
}

interface FirmwareStatus {
  model: string;
  installed: string | null;
  latest: FirmwareInfo | null;
  update: FirmwareInfo | null;
}

interface Info {
  model: string | null;
  serial: string | null;
  firmware: string | null;
  battery: number | null;
}

const $ = <T extends HTMLElement = HTMLElement>(id: string) => document.getElementById(id) as T;
const hex = (n: number) => "0x" + n.toString(16).padStart(2, "0");

let devices: Device[] = [];
let busy = false;

function toast(message: string, error = false) {
  const el = $("toast");
  el.textContent = message;
  el.classList.toggle("error", error);
  el.classList.remove("hidden");
  clearTimeout((el as any)._t);
  (el as any)._t = setTimeout(() => el.classList.add("hidden"), error ? 5000 : 2000);
}

async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T | undefined> {
  try {
    return await invoke<T>(cmd, args);
  } catch (e) {
    toast(String(e), true);
    return undefined;
  }
}

function setBusy(on: boolean) {
  busy = on;
  for (const id of ["refresh", "connect", "resync", "disconnect", "reset-keys"]) {
    ($(id) as HTMLButtonElement).disabled = on;
  }
}

// ---------- device list ----------

async function refresh(scanSecs = 4) {
  setBusy(true);
  const sel = $<HTMLSelectElement>("devices");
  sel.innerHTML = `<option>Searching…</option>`;
  devices = (await call<Device[]>("list_devices", { scanSecs })) ?? [];
  sel.innerHTML = devices.length
    ? devices
        .map((d) => `<option value="${d.id}">${d.name ?? "Unknown"} · ${d.address}${d.connected ? " ●" : ""}</option>`)
        .join("")
    : `<option value="">No keyboards found</option>`;
  setBusy(false);
}

async function connect() {
  const id = $<HTMLSelectElement>("devices").value;
  const dev = devices.find((d) => d.id === id);
  if (!dev) return;
  setBusy(true);
  showProgress(0, 1);
  const snap = await call<Snapshot>("connect", { id });
  hideProgress();
  setBusy(false);
  if (!snap) return;
  $("empty").classList.add("hidden");
  $("device").classList.remove("hidden");
  $("dev-name").textContent = dev.name ?? "Power Keyboard";
  render(snap);
  const info = await call<Info>("device_info");
  if (info) {
    $("dev-meta").textContent = [info.model, info.firmware && `firmware ${info.firmware}`, info.serial && `S/N ${info.serial}`]
      .filter(Boolean)
      .join(" · ");
    setBattery(info.battery);
  }
  checkFirmware();
}

async function disconnect() {
  await call("disconnect");
  showDisconnected();
}

function showDisconnected() {
  $("device").classList.add("hidden");
  $("empty").classList.remove("hidden");
}

function setBattery(level: number | null | undefined) {
  $("battery").textContent = level == null ? "" : `🔋 ${level}%`;
}

function showProgress(done: number, total: number) {
  const p = $("progress");
  p.classList.remove("hidden");
  (p.firstElementChild as HTMLElement).style.width = `${Math.round((done / Math.max(total, 1)) * 100)}%`;
}

function hideProgress() {
  $("progress").classList.add("hidden");
}

// ---------- rendering ----------

function render(s: Snapshot) {
  renderSettings(s.settings, s.wirelessCharging);
  renderSlots(s.keySlots);
  renderHosts(s.hosts);
  $("readonly").innerHTML =
    s.info.map((r) => `<span>${hex(r.addr)}</span><span>${r.value == null ? "?" : hex(r.value)} <span class="muted">${r.label}</span></span>`).join("") +
    `<span>status</span><span>${s.status ? s.status.map((b) => b.toString(16).padStart(2, "0")).join(" ") : "?"}</span>`;
}

const KIND_LABEL: Record<Host["kind"], string> = { empty: "Empty", unknown: "Paired", ios: "iOS", android: "Android" };

function renderHosts(hosts: Host[]) {
  const root = $("hosts");
  root.innerHTML = "";
  for (const h of hosts) {
    const li = document.createElement("li");
    const empty = h.kind === "empty";
    li.innerHTML = `
      <span class="slot-no">${h.slot}</span>
      <span class="host-name ${empty ? "muted" : ""}">${escapeHtml(empty ? "Empty" : h.displayName)}</span>
      ${empty || !h.name ? "" : `<span class="badge">${KIND_LABEL[h.kind]}</span>`}
      ${h.active ? `<span class="badge active">Active</span>` : ""}
      <span class="spacer"></span>`;
    if (!empty) {
      const rename = document.createElement("button");
      rename.className = "ghost small";
      rename.textContent = "Rename";
      rename.onclick = () => {
        const name = prompt(`Name for slot ${h.slot} (empty restores "${KIND_LABEL[h.kind]}")`, h.name ?? "");
        if (name != null) apply("set_host_name", { slot: h.slot, name });
      };
      const remove = document.createElement("button");
      remove.className = "ghost small danger";
      remove.textContent = "Remove";
      remove.disabled = h.active;
      remove.title = h.active ? "Switch the keyboard to another host first" : "Unpair and clear this slot";
      remove.onclick = () => {
        if (confirm(`Remove "${h.displayName}" from slot ${h.slot}?`)) apply("remove_host", { slot: h.slot });
      };
      li.append(rename, remove);
    }
    root.append(li);
  }
  if (!hosts.length) root.innerHTML = `<li class="muted">Unknown</li>`;
}

function formatDuration(value: number, unitMs: number) {
  const ms = value * unitMs;
  if (ms < 1000) return `${ms} ms`;
  if (ms < 60_000) return `${ms / 1000} s`;
  if (ms < 3_600_000) return `${+(ms / 60_000).toFixed(1)} min`;
  return `${+(ms / 3_600_000).toFixed(2)} h`;
}

function settingRow(label: string, note: string, inline = false): [HTMLElement, HTMLElement] {
  const row = document.createElement("div");
  row.className = inline ? "setting inline" : "setting";
  row.innerHTML = `<div class="label">${label}${note ? `<small>${note}</small>` : ""}</div><div class="control"></div>`;
  return [row, row.querySelector(".control") as HTMLElement];
}

function toggle(on: boolean, onChange: (on: boolean) => void): HTMLElement {
  const label = document.createElement("label");
  label.className = "toggle";
  label.innerHTML = `<input type="checkbox" ${on ? "checked" : ""}/><span></span>`;
  const box = label.querySelector("input") as HTMLInputElement;
  box.onchange = () => onChange(box.checked);
  return label;
}

function levelText(r: Register, k: Extract<RegisterKind, { type: "level" }>, v: number) {
  // Brightness is stored 0–255 but shown as a percentage, like the official app.
  if (r.id === "brightness") return `${Math.round((v / 255) * 100)} %`;
  if (r.id === "chargeReserve") return v === 0 ? "None" : `${v} %`;
  return `${v}${k.unit ? " " + k.unit : ""}`;
}

// Display order; ids are register ids, feature bit ids, or "wirelessCharging".
const GROUPS: [string, string[]][] = [
  ["Backlight", ["backlight", "brightness", "dimTimeout", "backlightShortcut"]],
  ["Battery", ["idleTimeout", "wirelessCharging", "chargeReserve"]],
  ["Typing", ["capsLock", "symLock", "cursorMode", "softReturn"]],
];

function renderSettings(settings: Register[], wirelessCharging: boolean | null) {
  const rows = new Map<string, HTMLElement>();
  for (const r of settings) {
    const k = r.kind;
    if (k.type === "flags") {
      for (const b of k.bits) {
        const [row, control] = settingRow(b.label, b.note, true);
        const on = r.value != null && (r.value & (1 << b.bit)) !== 0;
        control.append(toggle(on, (v) => apply("set_flag", { bit: b.bit, on: v })));
        rows.set(b.id, row);
      }
      continue;
    }
    const [row, control] = settingRow(r.label, r.note);
    if (k.type === "level") {
      const range = document.createElement("input");
      range.type = "range";
      range.min = String(k.min);
      range.max = String(k.max);
      range.step = String(k.step);
      range.value = String(r.value ?? k.min);
      const out = document.createElement("span");
      out.className = "value";
      out.textContent = r.value == null ? "?" : levelText(r, k, r.value);
      range.oninput = () => (out.textContent = levelText(r, k, Number(range.value)));
      range.onchange = () => apply("set_register", { id: r.id, value: Number(range.value) });
      control.append(range, out);
    } else if (k.type === "duration") {
      const select = document.createElement("select");
      const values = [...new Set([...k.presets, ...(r.value == null ? [] : [r.value])])].sort((a, b) => a - b);
      select.innerHTML = values
        .map((v) => `<option value="${v}" ${v === r.value ? "selected" : ""}>${formatDuration(v, k.unitMs)}</option>`)
        .join("");
      select.onchange = () => apply("set_register", { id: r.id, value: Number(select.value) });
      control.append(select);
    }
    rows.set(r.id, row);
  }
  if (wirelessCharging != null) {
    const [row, control] = settingRow("Wireless charging", "pass power through to the phone until the reserve is reached", true);
    control.append(toggle(wirelessCharging, (on) => apply("set_wireless_charging", { on })));
    rows.set("wirelessCharging", row);
  }

  const root = $("settings");
  root.innerHTML = "";
  const place = (title: string, ids: string[]) => {
    const present = ids.map((id) => rows.get(id)).filter((r): r is HTMLElement => !!r);
    if (!present.length) return;
    const group = document.createElement("div");
    group.className = "setting-group";
    group.innerHTML = `<h3>${title}</h3>`;
    group.append(...present);
    root.append(group);
    ids.forEach((id) => rows.delete(id));
  };
  for (const [title, ids] of GROUPS) place(title, ids);
  place("Other", [...rows.keys()]);
}

const CUSTOM = "\u0000custom";

function renderSlots(slots: Slot[]) {
  const body = $("slots");
  body.innerHTML = "";
  let group = "";
  for (const s of slots) {
    if (s.kind !== group) {
      group = s.kind;
      const head = document.createElement("tr");
      head.className = "group";
      head.innerHTML = `<td colspan="3">${s.kind === "remap" ? "Key remapping <span class=\"badge\">Android app only</span>" : "Number row shortcuts"}</td>`;
      body.append(head);
    }
    const tr = document.createElement("tr");
    tr.innerHTML = `
      <td class="slot-label">${s.label}</td>
      <td>
        <div class="slot-edit">
          <select></select>
          <input type="text" list="key-names" spellcheck="false" class="hidden"
            placeholder="${s.kind === "remap" ? "e.g. Ctrl+Space" : "e.g. Play/Pause"}"/>
          <button class="ghost small hidden">Apply</button>
        </div>
      </td>
      <td class="factory" title="Factory default">${s.factory ?? `Off`}</td>`;
    const select = tr.querySelector("select") as HTMLSelectElement;
    const text = tr.querySelector("input") as HTMLInputElement;
    const button = tr.querySelector("button") as HTMLButtonElement;

    const choices = [null, ...s.presets.filter((p): p is string => p != null)];
    const current = s.enabled ? s.description : null;
    const known = choices.includes(current);
    select.innerHTML =
      choices
        .map((c) => {
          const label = c == null ? `Off (${s.native})` : escapeHtml(c);
          return `<option value="${c == null ? "" : escapeHtml(c)}" ${c === current ? "selected" : ""}>${label}</option>`;
        })
        .join("") + `<option value="${CUSTOM}" ${known ? "" : "selected"}>Custom…</option>`;
    const showCustom = (on: boolean) => {
      text.classList.toggle("hidden", !on);
      button.classList.toggle("hidden", !on);
    };
    text.value = known ? "" : current ?? "";
    showCustom(!known);

    const commitCustom = () => {
      const action = text.value.trim();
      if (!action) return toast("Enter a key, e.g. Escape or Ctrl+Space", true);
      apply("set_key_slot", { index: s.index, action });
    };
    select.onchange = () => {
      if (select.value === CUSTOM) {
        showCustom(true);
        text.focus();
        return;
      }
      showCustom(false);
      apply("set_key_slot", { index: s.index, action: select.value || null });
    };
    text.onkeydown = (e) => e.key === "Enter" && commitCustom();
    button.onclick = commitCustom;
    body.append(tr);
  }
}

async function apply(cmd: string, args: Record<string, unknown>) {
  if (busy) return;
  setBusy(true);
  const snap = await call<Snapshot>(cmd, args);
  setBusy(false);
  if (snap) {
    render(snap);
    toast("Saved");
  }
}

function escapeHtml(s: string) {
  return s.replace(/[&<>"']/g, (c) => `&#${c.charCodeAt(0)};`);
}

// ---------- firmware ----------

async function checkFirmware() {
  const root = $("firmware");
  const button = $<HTMLButtonElement>("fw-check");
  button.disabled = true;
  root.textContent = "Checking…";
  const st = await call<FirmwareStatus>("check_firmware");
  button.disabled = false;
  if (!st) {
    root.textContent = "Couldn't reach the update server.";
    return;
  }
  const target = st.update ?? st.latest;
  const installed = st.installed ? `Installed <b>${escapeHtml(st.installed)}</b> · ` : "";
  const status = !st.latest
    ? `No firmware published for ${escapeHtml(st.model)}.`
    : st.update
      ? `Update available: <b>${escapeHtml(st.update.version)}</b>`
      : st.installed
        ? "Up to date."
        : `Latest is <b>${escapeHtml(st.latest.version)}</b>.`;
  root.classList.remove("muted");
  root.innerHTML = `<div>${installed}${status}</div>
    <p class="hint">Downloads are checked against the vendor's SHA-256. Installing isn't supported here yet; use the official app.</p>`;
  if (target) {
    const dl = document.createElement("button");
    dl.className = "ghost small";
    dl.textContent = `Download ${target.version}`;
    dl.onclick = async () => {
      dl.disabled = true;
      dl.textContent = "Downloading…";
      const path = await call<string>("download_firmware", { info: target });
      dl.disabled = false;
      dl.textContent = `Download ${target.version}`;
      if (path) toast(`Saved to ${path}`);
    };
    root.append(dl);
  }
}

// ---------- raw console ----------

async function sendRaw(e: Event) {
  e.preventDefault();
  const input = $<HTMLInputElement>("raw-input");
  const clean = input.value.replace(/[^0-9a-f]/gi, "");
  if (!clean || clean.length % 2) return toast("Enter an even number of hex digits", true);
  const payload = clean.match(/../g)!.map((b) => parseInt(b, 16));
  const reply = await call<{ status: number; data: number[] }>("send_raw", { payload });
  const log = $("raw-log");
  const fmt = (a: number[]) => a.map((b) => b.toString(16).padStart(2, "0")).join(" ");
  log.textContent =
    `> ${fmt(payload)}\n` + (reply ? `< status ${hex(reply.status)}  ${fmt(reply.data)}\n` : "< error\n") + log.textContent;
}

// ---------- wiring ----------

window.addEventListener("DOMContentLoaded", async () => {
  $("refresh").onclick = () => refresh();
  $("connect").onclick = connect;
  $("disconnect").onclick = disconnect;
  $("resync").onclick = async () => {
    setBusy(true);
    const snap = await call<Snapshot>("resync");
    hideProgress();
    setBusy(false);
    if (snap) render(snap);
  };
  $("reset-keys").onclick = () => {
    if (confirm("Restore all key slots to their factory assignments?")) apply("reset_key_slots", {});
  };
  $("raw").onsubmit = sendRaw;
  $("fw-check").onclick = checkFirmware;

  await listen<[number, number]>("sync-progress", (e) => showProgress(...e.payload));
  await listen<{ kind: string; value: number | null }>("keyboard", (e) => {
    if (e.payload.kind === "battery") setBattery(e.payload.value);
    if (e.payload.kind === "disconnected") {
      toast("Keyboard disconnected", true);
      showDisconnected();
    }
  });

  const names = (await call<string[]>("key_names")) ?? [];
  $("key-names").innerHTML = names.map((n) => `<option value="${escapeHtml(n)}"></option>`).join("");

  await refresh(0);
  if (devices.length === 0) await refresh();
});
