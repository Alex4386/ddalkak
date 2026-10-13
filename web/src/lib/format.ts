import type { Register, RegisterKind } from "@/backend";

export const hex = (n: number) => "0x" + n.toString(16).padStart(2, "0");
export const hexBytes = (bytes: number[]) => bytes.map((b) => b.toString(16).padStart(2, "0")).join(" ");

export function formatDuration(value: number, unitMs: number) {
  const ms = value * unitMs;
  if (ms < 1000) return `${ms} ms`;
  if (ms < 60_000) return `${ms / 1000} s`;
  if (ms < 3_600_000) return `${+(ms / 60_000).toFixed(1)} min`;
  return `${+(ms / 3_600_000).toFixed(2)} h`;
}

export function levelText(r: Register, k: Extract<RegisterKind, { type: "level" }>, v: number) {
  // Brightness is stored 0–255 but shown as a percentage, like the official app.
  if (r.id === "brightness") return `${Math.round((v / 255) * 100)}%`;
  if (r.id === "chargeReserve") return v === 0 ? "None" : `${v}%`;
  return `${v}${k.unit ? " " + k.unit : ""}`;
}

/** Value of a FEATURES flag by id, from a snapshot's settings. */
export function flag(settings: Register[], id: string): boolean | null {
  for (const r of settings) {
    if (r.kind.type !== "flags") continue;
    const bit = r.kind.bits.find((b) => b.id === id);
    if (bit) return r.value == null ? null : (r.value & (1 << bit.bit)) !== 0;
  }
  return null;
}

export const register = (settings: Register[], id: string) => settings.find((r) => r.id === id);
