import { AppleIcon, EllipsisVerticalIcon, LaptopIcon, PencilIcon, SmartphoneIcon, Trash2Icon } from "lucide-react";
import { useState } from "react";

import type { Host, Os } from "@/backend";
import { Badge } from "@/components/reui/badge";
import { Frame, FrameDescription, FrameHeader, FramePanel, FrameTitle } from "@/components/reui/frame";
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuGroup,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Input } from "@/components/ui/input";
import { Kbd } from "@/components/ui/kbd";
import { useKeyboard } from "@/hooks/use-keyboard";
import { cn } from "cn";

// "unknown" (type 1) is what the keyboard leaves after re-pairing an Android
// slot: Android keys, but nothing bonded.
const KIND_LABEL: Record<Host["kind"], string> = { empty: "Empty", unknown: "Not paired", ios: "iOS", android: "Android" };
const KIND_ICON = { empty: LaptopIcon, unknown: LaptopIcon, ios: AppleIcon, android: SmartphoneIcon };

function HostCard({ host, onRename, onRemove }: { host: Host; onRename(): void; onRemove(): void }) {
  const { busy, apply } = useKeyboard();
  const empty = host.kind === "empty";
  // Only bonded hosts can switch: the keyboard won't let the bond bit be set
  // over Bluetooth, so only the iOS/Android bit is ever written.
  const paired = host.kind === "ios" || host.kind === "android";
  const Icon = KIND_ICON[host.kind];
  return (
    <FramePanel
      className={cn(
        "flex items-center gap-3 transition-all",
        empty && "border-dashed bg-transparent shadow-none",
        host.active && "ring-2 ring-success/50",
      )}
    >
      <div
        className={cn(
          "relative flex size-10 shrink-0 items-center justify-center rounded-xl",
          empty ? "bg-muted/60 text-muted-foreground/60" : "bg-primary/5 text-foreground",
          host.active && "bg-success/10 text-success",
        )}
      >
        <Icon className="size-5" />
        <span className="absolute -right-1.5 -bottom-1.5 flex size-5 items-center justify-center rounded-full border bg-background text-[0.65rem] font-semibold tabular-nums">
          {host.slot}
        </span>
      </div>
      <div className="min-w-0 flex-1">
        <div className={cn("truncate text-sm font-medium", empty && "text-muted-foreground")}>
          {empty ? "Empty slot" : host.displayName}
        </div>
        <div className="mt-1 flex flex-wrap items-center gap-1">
          {!empty && host.name && (
            <Badge variant="outline" size="sm">
              {KIND_LABEL[host.kind]}
            </Badge>
          )}
          {host.active && (
            <Badge variant="success-light" size="sm" radius="full">
              <span className="size-1.5 animate-pulse rounded-full bg-success" />
              Active
            </Badge>
          )}
          {empty && <span className="text-xs text-muted-foreground/70">Pair a new device here</span>}
        </div>
      </div>
      {!empty && (
        <DropdownMenu>
          <DropdownMenuTrigger render={<Button variant="ghost" size="icon-sm" disabled={busy} aria-label="Host actions" />}>
            <EllipsisVerticalIcon />
          </DropdownMenuTrigger>
          <DropdownMenuContent align="end" className="w-48">
            {paired && (
              <>
                <DropdownMenuGroup>
                  <DropdownMenuLabel>Device type</DropdownMenuLabel>
                  <DropdownMenuRadioGroup
                    value={host.kind}
                    onValueChange={(v) => {
                      const os = v as Os;
                      if (os !== host.kind) apply("set_host_os", { slot: host.slot, os }, `Slot ${host.slot} now uses ${KIND_LABEL[os]} keys`);
                    }}
                  >
                    <DropdownMenuRadioItem value="ios" closeOnClick>
                      <AppleIcon />
                      iOS
                    </DropdownMenuRadioItem>
                    <DropdownMenuRadioItem value="android" closeOnClick>
                      <SmartphoneIcon />
                      Android
                    </DropdownMenuRadioItem>
                  </DropdownMenuRadioGroup>
                </DropdownMenuGroup>
                <DropdownMenuSeparator />
              </>
            )}
            <DropdownMenuItem onClick={onRename}>
              <PencilIcon />
              Rename
            </DropdownMenuItem>
            <DropdownMenuItem variant="destructive" disabled={host.active} onClick={onRemove}>
              <Trash2Icon />
              {host.active ? "In use" : "Remove"}
            </DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>
      )}
    </FramePanel>
  );
}

export function HostsPanel() {
  const { snapshot, apply } = useKeyboard();
  const [renaming, setRenaming] = useState<Host | null>(null);
  const [name, setName] = useState("");
  const [removing, setRemoving] = useState<Host | null>(null);
  if (!snapshot) return null;

  return (
    <Frame>
      <FrameHeader>
        <FrameTitle>Paired devices</FrameTitle>
        <FrameDescription className="flex flex-wrap items-center gap-1">
          Switch on the keyboard with <Kbd>Power</Kbd> + <Kbd>1</Kbd>…<Kbd>9</Kbd>. A device's type picks which key
          table it uses; new devices start as iOS.
        </FrameDescription>
      </FrameHeader>
      {snapshot.hosts.length ? (
        <div className="grid gap-(--frame-gap) sm:grid-cols-2 2xl:grid-cols-3">
          {snapshot.hosts.map((h) => (
            <HostCard
              key={h.slot}
              host={h}
              onRename={() => {
                setName(h.name ?? "");
                setRenaming(h);
              }}
              onRemove={() => setRemoving(h)}
            />
          ))}
        </div>
      ) : (
        <FramePanel className="text-sm text-muted-foreground">Not read from the keyboard yet.</FramePanel>
      )}

      <Dialog open={renaming != null} onOpenChange={(o) => !o && setRenaming(null)}>
        <DialogContent>
          <form
            className="grid gap-4"
            onSubmit={(e) => {
              e.preventDefault();
              if (!renaming) return;
              const slot = renaming.slot;
              setRenaming(null);
              apply("set_host_name", { slot, name }, "Renamed");
            }}
          >
            <DialogHeader>
              <DialogTitle>Rename slot {renaming?.slot}</DialogTitle>
              <DialogDescription>
                Up to 30 bytes. Leave empty to show “{renaming ? KIND_LABEL[renaming.kind] : ""}” again.
              </DialogDescription>
            </DialogHeader>
            <Input value={name} onChange={(e) => setName(e.target.value)} autoFocus maxLength={30} placeholder="e.g. Work phone" />
            <DialogFooter>
              <Button type="button" variant="outline" onClick={() => setRenaming(null)}>
                Cancel
              </Button>
              <Button type="submit">Save</Button>
            </DialogFooter>
          </form>
        </DialogContent>
      </Dialog>

      <AlertDialog open={removing != null} onOpenChange={(o) => !o && setRemoving(null)}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>Remove “{removing?.displayName}”?</AlertDialogTitle>
            <AlertDialogDescription>
              Clears slot {removing?.slot}'s name and type, like the official app's Remove Device.
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>Cancel</AlertDialogCancel>
            <AlertDialogAction
              variant="destructive"
              onClick={() => {
                if (!removing) return;
                const slot = removing.slot;
                setRemoving(null);
                apply("remove_host", { slot }, "Removed");
              }}
            >
              Remove
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </Frame>
  );
}
