import { BatteryChargingIcon, KeyboardIcon, SunIcon, type LucideIcon, SlidersHorizontalIcon } from "lucide-react";
import { useState, type ReactNode } from "react";

import type { Register, RegisterKind } from "@/backend";
import { Frame, FrameHeader, FramePanel, FrameTitle } from "@/components/reui/frame";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Slider } from "@/components/ui/slider";
import { Switch } from "@/components/ui/switch";
import { useKeyboard } from "@/hooks/use-keyboard";
import { formatDuration, levelText } from "@/lib/format";

// Display order; ids are register ids, feature bit ids, or "wirelessCharging".
const GROUPS: { title: string; icon: LucideIcon; ids: string[] }[] = [
  { title: "Backlight", icon: SunIcon, ids: ["backlight", "brightness", "dimTimeout", "backlightShortcut"] },
  { title: "Battery", icon: BatteryChargingIcon, ids: ["idleTimeout", "wirelessCharging", "chargeReserve"] },
  { title: "Typing", icon: KeyboardIcon, ids: ["capsLock", "symLock", "cursorMode", "softReturn"] },
];

function Row({ label, note, children, wide }: { label: string; note?: string; children: ReactNode; wide?: boolean }) {
  return (
    <div className={wide ? "flex flex-col gap-3 py-3.5 first:pt-0 last:pb-0" : "flex items-center justify-between gap-4 py-3.5 first:pt-0 last:pb-0"}>
      <div className="min-w-0 space-y-0.5">
        <div className="text-sm font-medium">{label}</div>
        {note && <p className="text-xs leading-relaxed text-muted-foreground">{note}</p>}
      </div>
      <div className={wide ? "flex w-full items-center gap-3" : "shrink-0"}>{children}</div>
    </div>
  );
}

function LevelControl({ r, k }: { r: Register; k: Extract<RegisterKind, { type: "level" }> }) {
  const { apply, busy } = useKeyboard();
  // Remounted (via `key`) whenever the keyboard reports a new value.
  const [value, setValue] = useState(r.value ?? k.min);
  return (
    <>
      <Slider
        min={k.min}
        max={k.max}
        step={k.step}
        value={value}
        disabled={busy || r.value == null}
        onValueChange={(v) => setValue(v as number)}
        onValueCommitted={(v) => {
          if (v !== r.value) apply("set_register", { id: r.id, value: v as number });
        }}
        aria-label={r.label}
      />
      <span className="w-12 shrink-0 text-right text-sm font-medium tabular-nums">
        {r.value == null ? "?" : levelText(r, k, value)}
      </span>
    </>
  );
}

function DurationControl({ r, k }: { r: Register; k: Extract<RegisterKind, { type: "duration" }> }) {
  const { apply, busy } = useKeyboard();
  const values = [...new Set([...k.presets, ...(r.value == null ? [] : [r.value])])].sort((a, b) => a - b);
  const items = values.map((v) => ({ value: String(v), label: formatDuration(v, k.unitMs) }));
  return (
    <Select
      items={items}
      value={r.value == null ? null : String(r.value)}
      disabled={busy}
      onValueChange={(v) => v != null && apply("set_register", { id: r.id, value: Number(v) })}
    >
      <SelectTrigger className="w-full sm:w-56" aria-label={r.label}>
        <SelectValue placeholder="Unknown" />
      </SelectTrigger>
      <SelectContent>
        {items.map((i) => (
          <SelectItem key={i.value} value={i.value}>
            {i.label}
          </SelectItem>
        ))}
      </SelectContent>
    </Select>
  );
}

export function SettingsPanel() {
  const { snapshot, apply, busy } = useKeyboard();
  if (!snapshot) return null;

  const rows = new Map<string, ReactNode>();
  for (const r of snapshot.settings) {
    const k = r.kind;
    if (k.type === "flags") {
      for (const b of k.bits) {
        const on = r.value != null && (r.value & (1 << b.bit)) !== 0;
        rows.set(
          b.id,
          <Row key={b.id} label={b.label} note={b.note}>
            <Switch
              checked={on}
              disabled={busy || r.value == null}
              onCheckedChange={(v) => apply("set_flag", { bit: b.bit, on: v })}
              aria-label={b.label}
            />
          </Row>,
        );
      }
    } else if (k.type === "level") {
      rows.set(
        r.id,
        <Row key={r.id} label={r.label} note={r.note} wide>
          <LevelControl key={r.value ?? "?"} r={r} k={k} />
        </Row>,
      );
    } else if (k.type === "duration") {
      rows.set(
        r.id,
        <Row key={r.id} label={r.label} note={r.note} wide>
          <DurationControl r={r} k={k} />
        </Row>,
      );
    }
  }
  if (snapshot.wirelessCharging != null) {
    rows.set(
      "wirelessCharging",
      <Row key="wirelessCharging" label="Charge the phone" note="share the keyboard battery with the phone, down to the reserve below">
        <Switch
          checked={snapshot.wirelessCharging}
          disabled={busy}
          onCheckedChange={(on) => apply("set_wireless_charging", { on })}
          aria-label="Charge the phone"
        />
      </Row>,
    );
  }

  const groups = GROUPS.map((g) => ({ ...g, rows: g.ids.flatMap((id) => (rows.has(id) ? [rows.get(id)] : [])) }));
  const used = new Set(GROUPS.flatMap((g) => g.ids));
  const other = [...rows].filter(([id]) => !used.has(id)).map(([, row]) => row);
  if (other.length) groups.push({ title: "Other", icon: SlidersHorizontalIcon, ids: [], rows: other });

  return (
    <div className="grid items-start gap-4 xl:grid-cols-2">
      {groups
        .filter((g) => g.rows.length)
        .map((g) => (
          <Frame key={g.title} className={g.title === "Typing" ? "xl:col-span-2" : undefined}>
            <FrameHeader>
              <FrameTitle className="flex items-center gap-2">
                <g.icon className="size-4 text-muted-foreground" />
                {g.title}
              </FrameTitle>
            </FrameHeader>
            <FramePanel className="divide-y">{g.rows}</FramePanel>
          </Frame>
        ))}
    </div>
  );
}
