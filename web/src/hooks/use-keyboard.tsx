import { createContext, useCallback, useContext, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { toast } from "sonner";

import { loadBackend, type ArgsOf, type Backend, type Command, type Device, type Info, type ResultOf, type Snapshot, type Support } from "@/backend";

type Phase = "loading" | "idle" | "connecting" | "connected";

interface KeyboardContext {
  backend: Backend | null;
  support: Support | null;
  phase: Phase;
  devices: Device[];
  device: Device | null;
  snapshot: Snapshot | null;
  info: Info | null;
  battery: number | null;
  /** Sync progress, 0–1, while reading the keyboard. */
  progress: number | null;
  busy: boolean;
  keyNames: string[];
  refresh(scan?: boolean): Promise<void>;
  connect(id: string): Promise<void>;
  disconnect(): Promise<void>;
  resync(): Promise<void>;
  /** Runs a command that returns a fresh snapshot, then shows it. */
  apply<C extends Command>(cmd: C, args: ArgsOf<C>, success?: string): Promise<boolean>;
  /** Raw command access; rejects with a message string. */
  call<C extends Command>(cmd: C, args?: ArgsOf<C>): Promise<ResultOf<C>>;
}

const Ctx = createContext<KeyboardContext | null>(null);

export function useKeyboard() {
  const ctx = useContext(Ctx);
  if (!ctx) throw new Error("useKeyboard outside KeyboardProvider");
  return ctx;
}

const message = (e: unknown) => (typeof e === "string" ? e : e instanceof Error ? e.message : String(e));

export function KeyboardProvider({ children }: { children: ReactNode }) {
  const [backend, setBackend] = useState<Backend | null>(null);
  const [support, setSupport] = useState<Support | null>(null);
  const [phase, setPhase] = useState<Phase>("loading");
  const [devices, setDevices] = useState<Device[]>([]);
  const [device, setDevice] = useState<Device | null>(null);
  const [snapshot, setSnapshot] = useState<Snapshot | null>(null);
  const [info, setInfo] = useState<Info | null>(null);
  const [battery, setBattery] = useState<number | null>(null);
  const [progress, setProgress] = useState<number | null>(null);
  const [busy, setBusy] = useState(false);
  const [keyNames, setKeyNames] = useState<string[]>([]);
  const busyRef = useRef(false);

  const call = useCallback(
    <C extends Command>(cmd: C, args?: ArgsOf<C>) => {
      if (!backend) return Promise.reject("still loading");
      const invoke = backend.invoke as (cmd: C, args?: ArgsOf<C>) => Promise<ResultOf<C>>;
      return invoke(cmd, args);
    },
    [backend],
  );

  /** Serialises user actions and reports failures as toasts. */
  const guarded = useCallback(async <T,>(fn: () => Promise<T>): Promise<T | undefined> => {
    if (busyRef.current) return undefined;
    busyRef.current = true;
    setBusy(true);
    try {
      return await fn();
    } catch (e) {
      toast.error(message(e));
      return undefined;
    } finally {
      busyRef.current = false;
      setBusy(false);
      setProgress(null);
    }
  }, []);

  const showDisconnected = useCallback(() => {
    setPhase("idle");
    setSnapshot(null);
    setInfo(null);
    setBattery(null);
    setDevice(null);
  }, []);

  // Load the backend for this platform, then look for keyboards without a prompt.
  useEffect(() => {
    let cancelled = false;
    const unlisten: (() => void)[] = [];
    (async () => {
      const b = await loadBackend();
      const s = await b.support();
      if (cancelled) return;
      setBackend(b);
      setSupport(s);
      unlisten.push(await b.listen("sync-progress", ([done, total]) => setProgress(done / Math.max(total, 1))));
      unlisten.push(
        await b.listen("keyboard", (e) => {
          if (e.kind === "battery") setBattery(e.value);
          if (e.kind === "disconnected") {
            toast.error("Keyboard disconnected");
            showDisconnected();
          }
        }),
      );
      setPhase("idle");
      if (!s.ok) return;
      b.invoke("key_names").then(setKeyNames, () => {});
      try {
        let found = await b.invoke("list_devices", { scanSecs: 0 });
        // The desktop can scan on its own; the browser needs a click first.
        if (!found.length && b.kind === "tauri") found = await b.invoke("list_devices", { scanSecs: 4 });
        if (!cancelled) setDevices(found);
      } catch (e) {
        toast.error(message(e));
      }
    })();
    return () => {
      cancelled = true;
      unlisten.forEach((u) => u());
    };
  }, [showDisconnected]);

  const refresh = useCallback(
    async (scan = true) => {
      await guarded(async () => {
        const found = await call("list_devices", { scanSecs: scan ? 4 : 0 });
        setDevices(found);
      });
    },
    [call, guarded],
  );

  const connect = useCallback(
    async (id: string) => {
      const target = devices.find((d) => d.id === id) ?? null;
      setPhase("connecting");
      setProgress(0);
      const snap = await guarded(() => call("connect", { id }));
      if (!snap) {
        setPhase("idle");
        return;
      }
      setDevice(target);
      setSnapshot(snap);
      setPhase("connected");
      const i = await call("device_info").catch(() => null);
      if (i) {
        setInfo(i);
        setBattery((b) => i.battery ?? b);
      }
    },
    [call, devices, guarded],
  );

  const disconnect = useCallback(async () => {
    await call("disconnect").catch(() => {});
    showDisconnected();
  }, [call, showDisconnected]);

  const resync = useCallback(async () => {
    setProgress(0);
    const snap = await guarded(() => call("resync"));
    if (snap) setSnapshot(snap);
  }, [call, guarded]);

  const apply = useCallback(
    async <C extends Command>(cmd: C, args: ArgsOf<C>, success = "Saved") => {
      const snap = await guarded(() => call(cmd, args));
      if (!snap) return false;
      setSnapshot(snap as Snapshot);
      toast.success(success);
      return true;
    },
    [call, guarded],
  );

  const value = useMemo<KeyboardContext>(
    () => ({
      backend,
      support,
      phase,
      devices,
      device,
      snapshot,
      info,
      battery,
      progress,
      busy,
      keyNames,
      refresh,
      connect,
      disconnect,
      resync,
      apply,
      call,
    }),
    [backend, support, phase, devices, device, snapshot, info, battery, progress, busy, keyNames, refresh, connect, disconnect, resync, apply, call],
  );

  return <Ctx.Provider value={value}>{children}</Ctx.Provider>;
}
