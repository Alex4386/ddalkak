// Shapes shared by both backends. Snapshot/Register/Slot/Host mirror
// `clicks_core::view`; the rest mirror the Tauri commands in app/src-tauri.

export type RegisterKind =
  | { type: "level"; min: number; max: number; step: number; unit: string }
  | { type: "duration"; unitMs: number; presets: number[] }
  | { type: "flags"; bits: FlagBit[] }
  | { type: "readOnly" };

export interface FlagBit {
  bit: number;
  id: string;
  label: string;
  note: string;
}

export interface Register {
  id: string;
  addr: number;
  len: number;
  kind: RegisterKind;
  label: string;
  note: string;
  value: number | null;
}

/** The keyboard keeps one key table per host OS and uses the active host's. */
export type Os = "ios" | "android";

export interface Slot {
  os: Os;
  index: number;
  addr: number;
  kind: "remap" | "shortcut";
  label: string;
  native: string;
  enabled: boolean;
  description: string | null;
  factory: string | null;
  /** Suggested choices as names; `null` = disabled. */
  presets: (string | null)[];
}

export type HostKind = "empty" | "unknown" | "ios" | "android";

export interface Host {
  slot: number;
  kind: HostKind;
  name: string | null;
  displayName: string;
  active: boolean;
}

export interface Snapshot {
  synced: boolean;
  settings: Register[];
  info: Register[];
  keySlots: Record<Os, Slot[]>;
  /** The table the keyboard is using: the active host's. */
  activeOs: Os | null;
  hosts: Host[];
  wirelessCharging: boolean | null;
  status: number[] | null;
}

export interface Device {
  id: string;
  name: string | null;
  /** Empty in the browser, which hides addresses. */
  address: string;
  connected: boolean;
}

export interface Info {
  model: string | null;
  serial: string | null;
  firmware: string | null;
  battery: number | null;
}

export interface FirmwareInfo {
  model: string;
  version: string;
  url: string;
  sha256: string;
}

export interface FirmwareStatus {
  model: string;
  installed: string | null;
  latest: FirmwareInfo | null;
  update: FirmwareInfo | null;
}

export interface RawReply {
  status: number;
  data: number[];
}

export type KeyboardEvent = { kind: "battery"; value: number } | { kind: "disconnected"; value: null };

/** Everything the UI can ask of a transport. */
export interface Commands {
  /** `scanSecs > 0` means the user asked to look (the browser opens its picker). */
  list_devices(args: { scanSecs: number }): Device[];
  connect(args: { id: string }): Snapshot;
  disconnect(): void;
  device_info(): Info;
  resync(): Snapshot;
  set_register(args: { id: string; value: number }): Snapshot;
  set_flag(args: { bit: number; on: boolean }): Snapshot;
  set_key_slot(args: { os: Os; index: number; action: string | null }): Snapshot;
  set_wireless_charging(args: { on: boolean }): Snapshot;
  set_host_name(args: { slot: number; name: string }): Snapshot;
  set_host_os(args: { slot: number; os: Os }): Snapshot;
  remove_host(args: { slot: number }): Snapshot;
  reset_key_slots(args: { os: Os }): Snapshot;
  send_raw(args: { payload: number[] }): RawReply;
  key_names(): string[];
  check_firmware(): FirmwareStatus;
  /** Returns where the image went (a path, or a file name in the browser). */
  download_firmware(args: { info: FirmwareInfo }): string;
}

export type Command = keyof Commands;
export type ArgsOf<C extends Command> = Parameters<Commands[C]> extends [infer A] ? A : undefined;
export type ResultOf<C extends Command> = ReturnType<Commands[C]>;

export interface Events {
  "sync-progress": [done: number, total: number];
  keyboard: KeyboardEvent;
}

export interface Support {
  ok: boolean;
  /** Why the transport can't work here, when `ok` is false. */
  reason?: string;
}

export interface Backend {
  kind: "tauri" | "web";
  support(): Promise<Support>;
  /** Rejects with a message string, like Tauri commands do. */
  invoke<C extends Command>(cmd: C, ...args: ArgsOf<C> extends undefined ? [] : [ArgsOf<C>]): Promise<ResultOf<C>>;
  listen<E extends keyof Events>(event: E, handler: (payload: Events[E]) => void): Promise<() => void>;
}
