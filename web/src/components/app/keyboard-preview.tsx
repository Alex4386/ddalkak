import { cn } from "cn";

/** Keys the configurable slots live on, for highlighting. */
export type KeyId = "tab" | "gemini" | "alt" | "backspace" | "sym";

const SLOT_KEYS: KeyId[] = ["tab", "gemini", "alt", "backspace"];
/** Slots 4–12 are Sym + 1…9. */
export const slotKey = (index: number): KeyId => SLOT_KEYS[index] ?? "sym";

type Cap = { label: string; id?: KeyId; grow?: number };

// A stylised thumb keyboard: it shows roughly where the configurable keys
// are, it isn't a drawing of the real layout.
const ROWS: Cap[][] = [
  ["Q", "W", "E", "R", "T", "Y", "U", "I", "O", "P"].map((label) => ({ label })),
  [
    ...["A", "S", "D", "F", "G", "H", "J", "K", "L"].map((label) => ({ label })),
    { label: "⌫", id: "backspace" },
  ],
  [
    { label: "⇧" },
    ...["Z", "X", "C", "V", "B", "N", "M"].map((label) => ({ label })),
    { label: "$" },
    { label: "⏎" },
  ],
  [
    { label: "Tab", id: "tab", grow: 1.4 },
    { label: "Alt", id: "alt", grow: 1.4 },
    { label: "Sym", id: "sym", grow: 1.4 },
    { label: "", grow: 4 },
    { label: "✦", id: "gemini", grow: 1.4 },
    { label: "🎤", grow: 1.4 },
  ],
];

export function KeyboardPreview({
  glow,
  highlight,
  chord,
  className,
}: {
  /** Backlight level 0–1; 0 is off. */
  glow: number;
  highlight?: KeyId;
  /** Shown on the highlighted key, e.g. "+3" for Sym + 3. */
  chord?: string;
  className?: string;
}) {
  return (
    <div
      className={cn(
        "relative select-none rounded-[1.4rem] border bg-gradient-to-b from-zinc-800 to-zinc-950 p-2.5 shadow-2xl shadow-black/30 ring-1 ring-white/5 sm:p-3",
        className,
      )}
      style={{ "--glow": glow } as React.CSSProperties}
      aria-hidden
    >
      <div
        className="pointer-events-none absolute inset-0 rounded-[inherit] opacity-(--glow) transition-opacity duration-500"
        style={{ background: "radial-gradient(120% 90% at 50% 100%, oklch(0.75 0.12 250 / 0.35), transparent 70%)" }}
      />
      <div className="relative flex flex-col gap-1 sm:gap-1.5">
        {ROWS.map((row, r) => (
          <div key={r} className="flex gap-1 sm:gap-1.5">
            {row.map((cap, c) => {
              const on = cap.id != null && cap.id === highlight;
              return (
                <div
                  key={c}
                  style={{ flexGrow: cap.grow ?? 1 }}
                  className={cn(
                    "relative flex h-7 min-w-0 basis-0 items-center justify-center rounded-md border border-white/5 bg-gradient-to-b from-zinc-700 to-zinc-800 text-[0.6rem] font-medium shadow-[inset_0_-2px_0_rgb(0_0_0/0.35)] transition-all duration-300 sm:h-9 sm:text-xs",
                    on && "z-10 -translate-y-0.5 border-sky-300/60 from-sky-500 to-sky-600 text-white shadow-[0_0_18px_oklch(0.75_0.15_240/0.7)]",
                  )}
                >
                  <span
                    className={cn("transition-colors duration-500", !on && "text-zinc-500")}
                    style={on ? undefined : { color: `color-mix(in oklch, oklch(0.93 0.05 240) calc(var(--glow) * 100%), oklch(0.55 0 0))` }}
                  >
                    {cap.label}
                  </span>
                  {on && chord && (
                    <span className="absolute -top-2.5 -right-1.5 rounded-full bg-amber-400 px-1 text-[0.55rem] leading-4 font-semibold text-zinc-900 shadow">
                      {chord}
                    </span>
                  )}
                </div>
              );
            })}
          </div>
        ))}
      </div>
    </div>
  );
}
