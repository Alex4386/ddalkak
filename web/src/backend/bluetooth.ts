// Browser backend: Web Bluetooth carries the bytes, clicks-core (compiled to
// WebAssembly) does the protocol. Mirrors app/src-tauri command for command.

import init, {
  ClicksSession,
  firmwareFileName,
  firmwareManifestUrl,
  firmwareStatus,
  gattUuids,
  keyNames,
  verifyFirmware,
} from "@/wasm/clicks_wasm.js";

import type { Backend, Commands, Device, Events, FirmwareInfo, FirmwareStatus, Info, RawReply, Snapshot } from "./types";

/** Matches the official app's per-request timeout. */
const RESPONSE_TIMEOUT_MS = 3000;
const FIRMWARE_HOST = "https://xinyi1.clicks.tech/";
const FALLBACK_MODEL = "CP-1000";

type Reply =
  | { type: "applied"; opcode: number; status: number; data: number[] }
  | { type: "rejected"; opcode: number; status: number };
type SessionEvent =
  | Reply
  | { type: "stray"; opcode: number }
  | { type: "unsolicited"; payload: number[] }
  | { type: "frameError"; message: string };

interface Uuids {
  service: string;
  notify: string;
  write: string;
  batteryService: string;
  batteryLevel: string;
  deviceInfoService: string;
  modelNumber: string;
  serialNumber: string;
  firmwareRevision: string;
  softwareRevision: string;
  namePrefix: string;
}

const ready = init();
let uuidCache: Uuids | undefined;
const uuids = () => (uuidCache ??= gattUuids() as Uuids);

const hex = (n: number) => "0x" + n.toString(16).padStart(2, "0");

// ---------- events ----------

const listeners = new Map<keyof Events, Set<(payload: never) => void>>();

function emit<E extends keyof Events>(event: E, payload: Events[E]) {
  listeners.get(event)?.forEach((h) => (h as (p: Events[E]) => void)(payload));
}

// ---------- connection ----------

class Link {
  readonly session = new ClicksSession();
  private waiter: ((r: Reply | Error) => void) | null = null;
  private ops: Promise<unknown> = Promise.resolve();
  private closed = false;

  readonly device: BluetoothDevice;
  private readonly server: BluetoothRemoteGATTServer;
  private readonly write: BluetoothRemoteGATTCharacteristic;
  private readonly notify: BluetoothRemoteGATTCharacteristic;
  private readonly battery: BluetoothRemoteGATTCharacteristic | undefined;

  private constructor(
    device: BluetoothDevice,
    server: BluetoothRemoteGATTServer,
    write: BluetoothRemoteGATTCharacteristic,
    notify: BluetoothRemoteGATTCharacteristic,
    battery: BluetoothRemoteGATTCharacteristic | undefined,
  ) {
    this.device = device;
    this.server = server;
    this.write = write;
    this.notify = notify;
    this.battery = battery;
  }

  static async open(device: BluetoothDevice): Promise<Link> {
    const u = uuids();
    const server = await device.gatt!.connect().catch((e) => {
      // Chrome on macOS lists paired keyboards from the system's paired-device
      // list as Classic devices, which can't do GATT. The real (LE) keyboard
      // only shows up while it's advertising, not while it's typing for the Mac.
      if (e instanceof DOMException && /unsupported device/i.test(e.message)) {
        throw new Error(
          "Chrome only sees this keyboard through the system's paired-device list, which can't be used for settings. Use the desktop app instead.",
        );
      }
      throw e;
    });
    const service = await server.getPrimaryService(u.service).catch(() => {
      throw new Error("device does not expose the configuration service (fff0)");
    });
    const [write, notify] = await Promise.all([service.getCharacteristic(u.write), service.getCharacteristic(u.notify)]);
    const battery = await server
      .getPrimaryService(u.batteryService)
      .then((s) => s.getCharacteristic(u.batteryLevel))
      .catch(() => undefined);

    const link = new Link(device, server, write, notify, battery);
    notify.addEventListener("characteristicvaluechanged", link.onNotify);
    await notify.startNotifications();
    if (battery?.properties.notify) {
      battery.addEventListener("characteristicvaluechanged", link.onBattery);
      await battery.startNotifications().catch(() => {});
    }
    device.addEventListener("gattserverdisconnected", link.onDisconnected);
    return link;
  }

  private onNotify = (e: Event) => {
    const v = (e.target as BluetoothRemoteGATTCharacteristic).value;
    if (!v) return;
    const events = this.session.handleNotification(new Uint8Array(v.buffer, v.byteOffset, v.byteLength)) as SessionEvent[];
    for (const ev of events) {
      if (ev.type === "applied" || ev.type === "rejected") this.waiter?.(ev);
      else console.warn("clicks:", ev);
    }
  };

  private onBattery = (e: Event) => {
    const v = (e.target as BluetoothRemoteGATTCharacteristic).value;
    if (v?.byteLength) emit("keyboard", { kind: "battery", value: v.getUint8(0) });
  };

  private onDisconnected = () => {
    this.waiter?.(new Error("connection closed"));
    this.detach();
    if (link === this) link = null;
    emit("keyboard", { kind: "disconnected", value: null });
  };

  private detach() {
    this.closed = true;
    this.notify.removeEventListener("characteristicvaluechanged", this.onNotify);
    this.battery?.removeEventListener("characteristicvaluechanged", this.onBattery);
    this.device.removeEventListener("gattserverdisconnected", this.onDisconnected);
    this.session.reset();
  }

  /**
   * Stops listening without calling `gatt.disconnect()`: the keyboard's HID
   * link must survive. Chrome drops its own GATT reference when the tab goes
   * away, and leaves paired devices connected.
   */
  async close() {
    if (this.closed) return;
    await this.exclusive(async () => {
      await this.notify.stopNotifications().catch(() => {});
      await this.battery?.stopNotifications().catch(() => {});
    }).catch(() => {});
    this.detach();
  }

  /** One operation at a time, so read-modify-writes don't interleave. */
  exclusive<T>(fn: () => Promise<T>): Promise<T> {
    const run = this.ops.then(fn, fn);
    this.ops = run.catch(() => {});
    return run;
  }

  /** Sends one frame and waits for the response it gets. */
  private send(frame: Uint8Array): Promise<Reply> {
    return new Promise((resolve, reject) => {
      let settled = false;
      const finish = (r: Reply | Error) => {
        if (settled) return;
        settled = true;
        clearTimeout(timer);
        if (this.waiter === finish) this.waiter = null;
        if (r instanceof Error) reject(r);
        else resolve(r);
      };
      const timer = setTimeout(
        () => finish(new Error(`timed out waiting for a response to opcode ${hex(frame[2])}`)),
        RESPONSE_TIMEOUT_MS,
      );
      this.waiter = finish;
      const data = frame as Uint8Array<ArrayBuffer>;
      const write = this.write.properties.write
        ? this.write.writeValueWithResponse(data)
        : this.write.writeValueWithoutResponse(data);
      write.catch((e) => finish(e instanceof Error ? e : new Error(String(e))));
    });
  }

  /** Writes every queued frame in order; stops at the first failure. */
  private async drain(opts: { progress?: (done: number, total: number) => void; allowReject?: boolean } = {}) {
    if (this.closed) throw new Error("not connected");
    const total = this.session.pending();
    const replies: Reply[] = [];
    for (let frame = this.session.pollTransmit(); frame; frame = this.session.pollTransmit()) {
      let reply: Reply;
      try {
        reply = await this.send(frame);
      } catch (e) {
        this.session.handleTimeout();
        this.session.clearQueue();
        throw e;
      }
      if (reply.type === "rejected" && !opts.allowReject) {
        this.session.clearQueue();
        throw new Error(`keyboard rejected opcode ${hex(reply.opcode)} with status ${hex(reply.status)}`);
      }
      replies.push(reply);
      opts.progress?.(replies.length, total);
    }
    return replies;
  }

  sync(): Promise<Snapshot> {
    return this.exclusive(async () => {
      this.session.enqueueSync();
      await this.drain({ progress: (done, total) => emit("sync-progress", [done, total]) });
      return this.session.snapshot() as Snapshot;
    });
  }

  /** `queue` adds requests to the session (and throws on invalid input). */
  update(queue: (s: ClicksSession) => void): Promise<Snapshot> {
    return this.exclusive(async () => {
      queue(this.session);
      await this.drain();
      return this.session.snapshot() as Snapshot;
    });
  }

  raw(payload: number[]): Promise<RawReply> {
    return this.exclusive(async () => {
      const [opcode, ...args] = payload;
      if (opcode == null) throw new Error("empty payload");
      this.session.enqueueRaw(opcode, Uint8Array.from(args));
      const [reply] = await this.drain({ allowReject: true });
      return { status: reply.status, data: reply.type === "applied" ? reply.data : [] };
    });
  }

  deviceInfo(): Promise<Info> {
    return this.exclusive(async () => {
      const u = uuids();
      const service = await this.server.getPrimaryService(u.deviceInfoService).catch(() => undefined);
      const text = async (uuid: string) => {
        try {
          const v = await (await service!.getCharacteristic(uuid)).readValue();
          return new TextDecoder().decode(v).replace(/\0+$/, "").trim() || null;
        } catch {
          // Missing, or on the Web Bluetooth blocklist (the serial number is).
          return null;
        }
      };
      const battery = await this.battery
        ?.readValue()
        .then((v) => (v.byteLength ? v.getUint8(0) : null))
        .catch(() => null);
      return {
        model: await text(u.modelNumber),
        serial: await text(u.serialNumber),
        firmware: (await text(u.firmwareRevision)) ?? (await text(u.softwareRevision)),
        battery: battery ?? null,
      };
    });
  }
}

let link: Link | null = null;
const known = new Map<string, BluetoothDevice>();

function current(): Link {
  if (!link) throw new Error("not connected");
  return link;
}

// ---------- commands ----------

async function listDevices(scanSecs: number): Promise<Device[]> {
  const bt = navigator.bluetooth;
  // Remembered devices; only exposed behind a Chrome flag for now.
  if (typeof bt.getDevices === "function") {
    for (const d of await bt.getDevices().catch(() => [])) known.set(d.id, d);
  }
  let picked: string | undefined;
  if (scanSecs > 0) {
    const u = uuids();
    try {
      const d = await bt.requestDevice({
        filters: [{ namePrefix: u.namePrefix }, { services: [u.service] }],
        optionalServices: [u.service, u.batteryService, u.deviceInfoService],
      });
      known.set(d.id, d);
      picked = d.id;
    } catch (e) {
      // Closing the picker isn't an error worth reporting.
      if (!(e instanceof DOMException && e.name === "NotFoundError" && /cancel/i.test(e.message))) throw e;
    }
  }
  return [...known.values()]
    .map((d) => ({ id: d.id, name: d.name ?? null, address: "", connected: d.gatt?.connected ?? false }))
    .sort((a, b) => Number(b.id === picked) - Number(a.id === picked));
}

async function connect(id: string): Promise<Snapshot> {
  await link?.close();
  link = null;
  const device = known.get(id);
  if (!device) throw new Error("unknown device; choose it again");
  link = await Link.open(device);
  return link.sync();
}

async function checkFirmware(): Promise<FirmwareStatus> {
  let model = FALLBACK_MODEL;
  let installed: string | null = null;
  if (link) {
    const info = await link.deviceInfo();
    model = info.model ?? model;
    installed = info.firmware;
  }
  let json: string;
  try {
    const r = await fetch(firmwareManifestUrl());
    if (!r.ok) throw new Error(`HTTP ${r.status}`);
    json = await r.text();
  } catch {
    throw new Error(
      "The update server doesn't accept requests from web pages. Use the desktop app or `clicks firmware` to check.",
    );
  }
  return firmwareStatus(json, model, installed) as FirmwareStatus;
}

async function downloadFirmware(info: FirmwareInfo): Promise<string> {
  if (!info.url.startsWith(FIRMWARE_HOST)) throw new Error("refusing to download from an unexpected host");
  const image = new Uint8Array(await (await fetch(info.url)).arrayBuffer());
  if (!verifyFirmware(image, info.sha256)) throw new Error("downloaded image failed the SHA-256 check");
  const name = firmwareFileName(info.model, info.version);
  const url = URL.createObjectURL(new Blob([image], { type: "application/octet-stream" }));
  const a = Object.assign(document.createElement("a"), { href: url, download: name });
  a.click();
  setTimeout(() => URL.revokeObjectURL(url), 10_000);
  return name;
}

type Handlers = { [C in keyof Commands]: (args: Parameters<Commands[C]>[0]) => Promise<ReturnType<Commands[C]>> };

const handlers: Handlers = {
  list_devices: ({ scanSecs }) => listDevices(scanSecs),
  connect: ({ id }) => connect(id),
  disconnect: async () => {
    await link?.close();
    link = null;
  },
  device_info: () => current().deviceInfo(),
  resync: () => current().sync(),
  set_register: ({ id, value }) => current().update((s) => s.setRegister(id, value)),
  set_flag: ({ bit, on }) => current().update((s) => s.setFlag(bit, on)),
  set_key_slot: ({ os, index, action }) => current().update((s) => s.setKeySlot(os, index, action)),
  set_wireless_charging: ({ on }) => current().update((s) => s.setWirelessCharging(on)),
  set_host_name: ({ slot, name }) => current().update((s) => s.setHostName(slot, name)),
  set_host_os: ({ slot, os }) => current().update((s) => s.setHostOs(slot, os)),
  remove_host: ({ slot }) => current().update((s) => s.removeHost(slot)),
  reset_key_slots: ({ os }) => current().update((s) => s.resetKeySlots(os)),
  send_raw: ({ payload }) => current().raw(payload),
  key_names: async () => keyNames(),
  check_firmware: () => checkFirmware(),
  download_firmware: ({ info }) => downloadFirmware(info),
};

export const backend: Backend = {
  kind: "web",

  async support() {
    if (!window.isSecureContext) return { ok: false, reason: "Web Bluetooth needs HTTPS (or localhost)." };
    if (!navigator.bluetooth) {
      return {
        ok: false,
        reason:
          "This browser has no Web Bluetooth. Use Chrome or Edge on Windows, macOS, ChromeOS or Android; on Linux enable chrome://flags/#enable-web-bluetooth first.",
      };
    }
    if (!(await navigator.bluetooth.getAvailability().catch(() => true))) {
      return { ok: false, reason: "No Bluetooth adapter is available, or Bluetooth is turned off." };
    }
    return { ok: true };
  },

  async invoke(cmd, ...args) {
    try {
      await ready;
      const handler = handlers[cmd] as (a: unknown) => Promise<never>;
      return await handler(args[0]);
    } catch (e) {
      throw e instanceof Error ? e.message : String(e);
    }
  },

  async listen(event, handler) {
    let set = listeners.get(event);
    if (!set) listeners.set(event, (set = new Set()));
    const h = handler as (payload: never) => void;
    set.add(h);
    return () => void set.delete(h);
  },
};
