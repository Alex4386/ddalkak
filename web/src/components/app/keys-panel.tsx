import { CheckIcon, RotateCcwIcon } from "lucide-react";
import { Fragment, useState } from "react";

import type { Slot } from "@/backend";
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
  AlertDialogTrigger,
} from "@/components/ui/alert-dialog";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Kbd } from "@/components/ui/kbd";
import { Select, SelectContent, SelectItem, SelectSeparator, SelectTrigger, SelectValue } from "@/components/ui/select";
import { useKeyboard } from "@/hooks/use-keyboard";
import { setHighlight } from "@/hooks/use-highlight";

const OFF = "\u0000off";
const CUSTOM = "\u0000custom";

function SlotEditor({ slot }: { slot: Slot }) {
  const { apply, busy } = useKeyboard();
  const current = slot.enabled ? slot.description : null;
  // The official choices, plus the factory and current values when they aren't among them.
  const presets = [...new Set([...slot.presets, slot.factory, current])].filter((p): p is string => p != null);
  const known = current == null || presets.includes(current);
  // Remounted (via `key`) whenever the slot's value changes.
  const [custom, setCustom] = useState(!known);
  const [text, setText] = useState(known ? "" : (current ?? ""));

  const items = [
    { value: OFF, label: `Off · ${slot.native}` },
    ...presets.map((p) => ({ value: p, label: p })),
    { value: CUSTOM, label: "Custom…" },
  ];
  const selected = custom ? CUSTOM : (current ?? OFF);

  const commit = () => {
    const action = text.trim();
    if (action && action !== current) apply("set_key_slot", { index: slot.index, action });
  };

  return (
    <div className="flex w-full flex-col gap-2">
      <Select
        items={items}
        value={selected}
        disabled={busy}
        onValueChange={(v) => {
          if (v === CUSTOM) return setCustom(true);
          setCustom(false);
          apply("set_key_slot", { index: slot.index, action: v === OFF ? null : v });
        }}
      >
        <SelectTrigger className="w-full" aria-label={`Action for ${slot.label}`}>
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          {items.map((i, n) => (
            <Fragment key={i.value}>
              {n === items.length - 1 && <SelectSeparator />}
              <SelectItem value={i.value}>{i.label}</SelectItem>
            </Fragment>
          ))}
        </SelectContent>
      </Select>
      {custom && (
        <form
          className="flex gap-2"
          onSubmit={(e) => {
            e.preventDefault();
            commit();
          }}
        >
          <Input
            value={text}
            onChange={(e) => setText(e.target.value)}
            list="key-names"
            spellCheck={false}
            autoFocus={!current}
            placeholder={slot.kind === "remap" ? "e.g. Ctrl+Space" : "e.g. Play/Pause"}
          />
          <Button type="submit" size="icon" variant="outline" disabled={busy || !text.trim()} aria-label="Apply">
            <CheckIcon />
          </Button>
        </form>
      )}
    </div>
  );
}

function SlotHeader({ slot }: { slot: Slot }) {
  const changed = (slot.enabled ? slot.description : null) !== slot.factory;
  return (
    <div className="flex min-w-0 items-center gap-2">
      {slot.kind === "shortcut" ? (
        <span className="flex items-center gap-1 text-sm">
          <Kbd>Sym</Kbd>+<Kbd>{slot.index - 3}</Kbd>
        </span>
      ) : (
        <span className="text-sm font-medium">{slot.label}</span>
      )}
      {changed && (
        <Badge variant="info-light" size="sm" radius="full">
          Changed
        </Badge>
      )}
    </div>
  );
}

const hoverProps = (index: number) => ({
  onPointerEnter: () => setHighlight(index),
  onPointerLeave: () => setHighlight(null),
  onFocus: () => setHighlight(index),
  onBlur: () => setHighlight(null),
});

function ResetKeys() {
  const { apply, busy } = useKeyboard();
  const [open, setOpen] = useState(false);
  return (
    <AlertDialog open={open} onOpenChange={setOpen}>
      <AlertDialogTrigger render={<Button variant="ghost" size="sm" disabled={busy} />}>
        <RotateCcwIcon />
        <span className="hidden sm:inline">Restore defaults</span>
      </AlertDialogTrigger>
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogTitle>Restore factory keys?</AlertDialogTitle>
          <AlertDialogDescription>
            Every remapped key and Sym shortcut goes back to how the keyboard shipped.
          </AlertDialogDescription>
        </AlertDialogHeader>
        <AlertDialogFooter>
          <AlertDialogCancel>Cancel</AlertDialogCancel>
          <AlertDialogAction
            onClick={() => {
              setOpen(false);
              apply("reset_key_slots", undefined, "Keys restored");
            }}
          >
            Restore
          </AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  );
}

export function KeysPanel() {
  const { snapshot, keyNames } = useKeyboard();
  if (!snapshot) return null;
  const remap = snapshot.keySlots.filter((s) => s.kind === "remap");
  const shortcuts = snapshot.keySlots.filter((s) => s.kind === "shortcut");

  return (
    <div className="flex flex-col gap-4">
      <datalist id="key-names">
        {keyNames.map((n) => (
          <option key={n} value={n} />
        ))}
      </datalist>

      <Frame>
        <FrameHeader className="flex-row items-start justify-between gap-3">
          <div>
            <FrameTitle className="flex items-center gap-2">
              Key remapping
              <Badge variant="warning-light" size="sm" radius="full">
                Android app only
              </Badge>
            </FrameTitle>
            <FrameDescription>
              A key, or a modifier with a key (<code>Ctrl+Space</code>). Custom accepts <code>key:0x29</code> too.
            </FrameDescription>
          </div>
          <ResetKeys />
        </FrameHeader>
        <FramePanel className="divide-y p-0">
          {remap.map((s) => (
            <div
              key={s.index}
              {...hoverProps(s.index)}
              className="grid gap-2 px-(--frame-panel-px) py-3 transition-colors hover:bg-muted/40 sm:grid-cols-[1fr_16rem] sm:items-center"
            >
              <div className="space-y-0.5">
                <SlotHeader slot={s} />
                <p className="text-xs text-muted-foreground">Factory: {s.factory ?? `Off (${s.native})`}</p>
              </div>
              <SlotEditor key={`${s.enabled}:${s.description}`} slot={s} />
            </div>
          ))}
        </FramePanel>
      </Frame>

      <Frame>
        <FrameHeader>
          <FrameTitle>Sym shortcuts</FrameTitle>
          <FrameDescription>
            Hold Sym and press a number. A key (<code>Escape</code>) or a media key (<code>Play/Pause</code>).
          </FrameDescription>
        </FrameHeader>
        <div className="grid gap-(--frame-gap) sm:grid-cols-2 2xl:grid-cols-3">
          {shortcuts.map((s) => (
            <FramePanel key={s.index} {...hoverProps(s.index)} className="flex flex-col gap-3 transition-shadow hover:shadow-md">
              <div className="flex items-center justify-between gap-2">
                <SlotHeader slot={s} />
                <span className="truncate text-xs text-muted-foreground">{s.factory ?? "Off"}</span>
              </div>
              <SlotEditor key={`${s.enabled}:${s.description}`} slot={s} />
            </FramePanel>
          ))}
        </div>
      </Frame>
    </div>
  );
}
