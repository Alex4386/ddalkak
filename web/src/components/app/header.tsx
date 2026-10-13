import { BatteryFullIcon, BatteryLowIcon, BatteryMediumIcon, LogOutIcon, MonitorIcon, MoonIcon, RefreshCwIcon, SunIcon } from "lucide-react";
import { useTheme } from "next-themes";
import { cn } from "cn";

import { Badge } from "@/components/reui/badge";
import { Button } from "@/components/ui/button";
import { DropdownMenu, DropdownMenuContent, DropdownMenuRadioGroup, DropdownMenuRadioItem, DropdownMenuTrigger } from "@/components/ui/dropdown-menu";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { useKeyboard } from "@/hooks/use-keyboard";

export function BatteryBadge({ level, className }: { level: number; className?: string }) {
  const Icon = level > 60 ? BatteryFullIcon : level > 20 ? BatteryMediumIcon : BatteryLowIcon;
  return (
    <Badge variant={level > 20 ? "success-light" : "destructive-light"} size="lg" radius="full" className={cn("tabular-nums", className)}>
      <Icon />
      {level}%
    </Badge>
  );
}

function ThemeMenu() {
  const { theme = "system", setTheme } = useTheme();
  return (
    <DropdownMenu>
      <DropdownMenuTrigger render={<Button variant="ghost" size="icon" aria-label="Theme" />}>
        <SunIcon className="dark:hidden" />
        <MoonIcon className="hidden dark:block" />
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end" className="w-36">
        <DropdownMenuRadioGroup value={theme} onValueChange={setTheme}>
          <DropdownMenuRadioItem value="light">
            <SunIcon /> Light
          </DropdownMenuRadioItem>
          <DropdownMenuRadioItem value="dark">
            <MoonIcon /> Dark
          </DropdownMenuRadioItem>
          <DropdownMenuRadioItem value="system">
            <MonitorIcon /> System
          </DropdownMenuRadioItem>
        </DropdownMenuRadioGroup>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}

export function Header() {
  const { phase, battery, busy, resync, disconnect } = useKeyboard();
  const connected = phase === "connected";
  return (
    <header className="sticky top-0 z-30 border-b bg-background/70 backdrop-blur-xl supports-[backdrop-filter]:bg-background/60">
      <div className="mx-auto flex h-14 max-w-7xl items-center gap-3 px-4">
        <div className="flex items-center gap-2.5">
          <img src={`${import.meta.env.BASE_URL}favicon.svg`} alt="" className="size-8 rounded-lg shadow-sm ring-1 ring-white/10" />
          <span className="font-semibold tracking-tight">ddalkak</span>
        </div>
        <div className="flex-1" />
        {connected && battery != null && <BatteryBadge level={battery} className="lg:hidden" />}
        {connected && (
          <>
            <Tooltip>
              <TooltipTrigger render={<Button variant="ghost" size="icon" onClick={resync} disabled={busy} aria-label="Read again" />}>
                <RefreshCwIcon className={busy ? "animate-spin" : undefined} />
              </TooltipTrigger>
              <TooltipContent>Read again</TooltipContent>
            </Tooltip>
            <Tooltip>
              <TooltipTrigger render={<Button variant="ghost" size="icon" onClick={disconnect} aria-label="Disconnect" />}>
                <LogOutIcon />
              </TooltipTrigger>
              <TooltipContent>Stop managing this keyboard (typing keeps working)</TooltipContent>
            </Tooltip>
          </>
        )}
        <ThemeMenu />
      </div>
    </header>
  );
}
