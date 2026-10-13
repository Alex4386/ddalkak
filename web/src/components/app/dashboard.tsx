import { CpuIcon, KeyboardIcon, LaptopMinimalIcon, SlidersHorizontalIcon, TerminalIcon } from "lucide-react";
import { useState } from "react";

import { Badge } from "@/components/reui/badge";
import { Frame, FramePanel } from "@/components/reui/frame";
import { Progress } from "@/components/ui/progress";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { useHighlight } from "@/hooks/use-highlight";
import { useKeyboard } from "@/hooks/use-keyboard";
import { flag, register } from "@/lib/format";

import { AdvancedPanel } from "./advanced-panel";
import { FirmwarePanel } from "./firmware-panel";
import { BatteryBadge } from "./header";
import { HostsPanel } from "./hosts-panel";
import { KeyboardPreview, slotKey } from "./keyboard-preview";
import { KeysPanel } from "./keys-panel";
import { SettingsPanel } from "./settings-panel";

const TABS = [
  { id: "settings", label: "Settings", icon: SlidersHorizontalIcon, panel: SettingsPanel },
  { id: "keys", label: "Keys", icon: KeyboardIcon, panel: KeysPanel },
  { id: "devices", label: "Devices", icon: LaptopMinimalIcon, panel: HostsPanel },
  { id: "firmware", label: "Firmware", icon: CpuIcon, panel: FirmwarePanel },
  { id: "advanced", label: "Advanced", icon: TerminalIcon, panel: AdvancedPanel },
];

function Overview() {
  const { snapshot, device, info, battery, progress } = useKeyboard();
  const highlight = useHighlight();
  if (!snapshot) return null;

  const backlight = flag(snapshot.settings, "backlight");
  const brightness = register(snapshot.settings, "brightness")?.value ?? 0;
  const glow = backlight === false ? 0 : Math.max(0.08, brightness / 255);
  const active = snapshot.hosts.find((h) => h.active);

  return (
    <Frame className="lg:sticky lg:top-20">
      <FramePanel className="flex flex-col gap-5 overflow-visible">
        <div className="flex items-start justify-between gap-3">
          <div className="min-w-0">
            <div className="truncate text-lg font-semibold tracking-tight">{device?.name ?? "Power Keyboard"}</div>
            <div className="mt-1 flex flex-wrap gap-1.5">
              {info?.model && (
                <Badge variant="outline" size="sm">
                  {info.model}
                </Badge>
              )}
              {info?.firmware && (
                <Badge variant="outline" size="sm">
                  v{info.firmware}
                </Badge>
              )}
            </div>
          </div>
          {battery != null && <BatteryBadge level={battery} />}
        </div>

        <KeyboardPreview
          glow={glow}
          highlight={highlight == null ? undefined : slotKey(highlight)}
          chord={highlight != null && highlight >= 4 ? `+${highlight - 3}` : undefined}
        />

        <dl className="grid grid-cols-2 gap-3 text-sm">
          <div className="rounded-lg bg-muted/50 p-3">
            <dt className="text-xs text-muted-foreground">Backlight</dt>
            <dd className="mt-0.5 font-medium tabular-nums">
              {backlight === false ? "Off" : `${Math.round((brightness / 255) * 100)}%`}
            </dd>
          </div>
          <div className="rounded-lg bg-muted/50 p-3">
            <dt className="text-xs text-muted-foreground">Typing on</dt>
            <dd className="mt-0.5 truncate font-medium">{active ? `${active.slot} · ${active.displayName}` : "—"}</dd>
          </div>
          <div className="rounded-lg bg-muted/50 p-3">
            <dt className="text-xs text-muted-foreground">Phone charging</dt>
            <dd className="mt-0.5 font-medium">
              {snapshot.wirelessCharging == null ? "—" : snapshot.wirelessCharging ? "On" : "Off"}
            </dd>
          </div>
          <div className="rounded-lg bg-muted/50 p-3">
            <dt className="text-xs text-muted-foreground">Remapped keys</dt>
            <dd className="mt-0.5 font-medium tabular-nums">
              {snapshot.keySlots.filter((s) => (s.enabled ? s.description : null) !== s.factory).length} changed
            </dd>
          </div>
        </dl>

        {progress != null && <Progress value={progress * 100} />}
      </FramePanel>
    </Frame>
  );
}

export function Dashboard() {
  const [tab, setTab] = useState("settings");
  return (
    <div className="mx-auto grid w-full max-w-7xl gap-6 px-4 py-6 lg:grid-cols-[22rem_1fr] lg:py-8 xl:grid-cols-[26rem_1fr]">
      <aside className="animate-in fade-in-0 slide-in-from-bottom-2 duration-500">
        <Overview />
      </aside>
      <Tabs value={tab} onValueChange={setTab} className="min-w-0 gap-4">
        <div className="-mx-4 overflow-x-auto px-4 [scrollbar-width:none]">
          <TabsList className="h-9">
            {TABS.map((t) => (
              <TabsTrigger key={t.id} value={t.id} className="px-3">
                <t.icon />
                {t.label}
              </TabsTrigger>
            ))}
          </TabsList>
        </div>
        {TABS.map((t) => (
          <TabsContent key={t.id} value={t.id} className="animate-in fade-in-0 slide-in-from-bottom-1 duration-300">
            <t.panel />
          </TabsContent>
        ))}
      </Tabs>
    </div>
  );
}
