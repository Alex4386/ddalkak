import { BluetoothIcon, BluetoothSearchingIcon, ChevronRightIcon, KeyboardIcon, RefreshCwIcon, TriangleAlertIcon } from "lucide-react";

import { Alert, AlertDescription, AlertTitle } from "@/components/reui/alert";
import { Badge } from "@/components/reui/badge";
import { Button } from "@/components/ui/button";
import { Item, ItemActions, ItemContent, ItemDescription, ItemGroup, ItemMedia, ItemTitle } from "@/components/ui/item";
import { Progress } from "@/components/ui/progress";
import { Spinner } from "@/components/ui/spinner";
import { useKeyboard } from "@/hooks/use-keyboard";

import { KeyboardPreview } from "./keyboard-preview";

export function ConnectScreen() {
  const { backend, support, devices, phase, busy, progress, refresh, connect } = useKeyboard();
  const web = backend?.kind === "web";
  const connecting = phase === "connecting";
  const unsupported = support && !support.ok;

  return (
    <div className="mx-auto flex w-full max-w-5xl flex-1 flex-col items-center justify-center gap-10 px-4 py-10 lg:flex-row lg:gap-16 lg:py-16">
      <div className="relative w-full max-w-md lg:order-2">
        <div className="absolute -inset-10 -z-10 rounded-full bg-[radial-gradient(closest-side,oklch(0.7_0.14_250/0.25),transparent)] blur-2xl" />
        <KeyboardPreview glow={connecting ? 1 : 0.35} className="animate-in fade-in-0 zoom-in-95 duration-700" />
      </div>

      <div className="flex w-full max-w-md flex-col gap-6 lg:order-1">
        <div className="space-y-3">
          <Badge variant="primary-light" radius="full" size="lg">
            <BluetoothIcon />
            {web ? "Web Bluetooth" : "Desktop"}
          </Badge>
          <h1 className="text-3xl font-semibold tracking-tight text-balance sm:text-4xl">Set up your Power Keyboard</h1>
          <p className="text-muted-foreground text-pretty">
            Backlight, battery sharing, key remaps and paired devices, without the phone app. Pair the keyboard with
            this {web ? "device" : "computer"} first.
          </p>
        </div>

        {unsupported ? (
          <Alert variant="warning">
            <TriangleAlertIcon />
            <AlertTitle>Bluetooth isn't available here</AlertTitle>
            <AlertDescription>{support.reason}</AlertDescription>
          </Alert>
        ) : (
          <>
            {connecting && (
              <div className="space-y-2">
                <div className="flex items-center justify-between text-sm">
                  <span className="flex items-center gap-2">
                    <Spinner /> Reading settings…
                  </span>
                  <span className="text-muted-foreground tabular-nums">{Math.round((progress ?? 0) * 100)}%</span>
                </div>
                <Progress value={(progress ?? 0) * 100} />
              </div>
            )}

            {devices.length > 0 && (
              <ItemGroup className="gap-2">
                {devices.map((d) => (
                  <Item
                    key={d.id}
                    variant="outline"
                    render={<button type="button" disabled={busy} onClick={() => connect(d.id)} />}
                    className="cursor-pointer text-left hover:bg-muted/50 disabled:cursor-default disabled:opacity-60"
                  >
                    <ItemMedia variant="icon" className="size-9 rounded-lg bg-muted">
                      <KeyboardIcon />
                    </ItemMedia>
                    <ItemContent>
                      <ItemTitle>{d.name ?? "Unknown keyboard"}</ItemTitle>
                      <ItemDescription>{[d.address, d.connected ? "Connected" : "Not connected"].filter(Boolean).join(" · ")}</ItemDescription>
                    </ItemContent>
                    <ItemActions>
                      <ChevronRightIcon className="size-4 text-muted-foreground" />
                    </ItemActions>
                  </Item>
                ))}
              </ItemGroup>
            )}

            <div className="flex flex-wrap gap-2">
              <Button size="lg" onClick={() => refresh(true)} disabled={busy || !backend}>
                {busy && !connecting ? <Spinner /> : web ? <BluetoothSearchingIcon /> : <RefreshCwIcon />}
                {web ? (devices.length ? "Choose another keyboard" : "Choose keyboard") : "Search again"}
              </Button>
            </div>

            {web && (
              <p className="text-xs leading-relaxed text-muted-foreground">
                Works in Chrome and Edge. If the keyboard doesn't show up in the picker, make sure it's on and paired;
                on Linux, enable <code className="rounded bg-muted px-1">chrome://flags/#enable-web-bluetooth</code>.
              </p>
            )}
          </>
        )}
      </div>
    </div>
  );
}
