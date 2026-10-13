import { cn } from "cn";
import type { ReactNode } from "react";

/** Keys the configurable slots live on, for highlighting. */
export type KeyId = "tab" | "gemini" | "alt" | "backspace" | "sym" | `${1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9}`;

// Physical keys per slot, from the official app's key icons (ic_key_tab is
// the red Clicks key, ic_key_alt_ble the keyboard key next to it).
const SLOT_KEYS: KeyId[] = ["tab", "gemini", "alt", "backspace"];
/** Slots 4–12 are Sym + 1…9 on the number row. */
export const slotKeys = (index: number): KeyId[] =>
  index < 4 ? [SLOT_KEYS[index]] : ["sym", String(index - 3) as KeyId];

type Glyph = "shift" | "backspace" | "enter" | "meta" | "clicks" | "keyboard" | "voice";
type Cap = { x: number; w: number; label?: string; alt?: string; glyph?: Glyph; text?: string; id?: KeyId; red?: boolean };
type Row = { y: number; h: number; caps: Cap[] };

// Traced from the official app's Power Keyboard render (1920×1080): the rows
// fan out towards the bottom, so each row has its own pitch.
const row = (y: number, h: number, x0: number, pitch: number, w: number, caps: Omit<Cap, "x" | "w">[]): Row => ({
  y,
  h,
  caps: caps.map((c, i) => ({ ...c, x: x0 + i * pitch, w })),
});

const ROWS: Row[] = [
  row(
    293,
    50,
    248,
    158.2,
    128,
    "1234567890".split("").map((d) => ({ label: d, id: d === "0" ? undefined : (d as KeyId) })),
  ),
  row(380, 96, 228, 162.4, 132, [
    { label: "Q", alt: "#" },
    { label: "W", alt: "▲" },
    { label: "E", alt: "~" },
    { label: "R", alt: "=" },
    { label: "T", alt: "(" },
    { label: "Y", alt: ")" },
    { label: "U", alt: "_" },
    { label: "I", alt: "-" },
    { label: "O", alt: "+" },
    { label: "P", alt: "@" },
  ]),
  row(490, 100, 207, 166.6, 136, [
    { label: "A", alt: "◀" },
    { label: "S", alt: "▼" },
    { label: "D", alt: "▶" },
    { label: "F", alt: "*" },
    { label: "G", alt: "/" },
    { label: "H", alt: ":" },
    { label: "J", alt: ";" },
    { label: "K", alt: "'" },
    { label: "L", alt: '"' },
    { glyph: "backspace", id: "backspace" },
  ]),
  row(603, 104, 187, 172.2, 140, [
    { glyph: "shift" },
    { label: "Z", alt: "%" },
    { label: "X", alt: "&" },
    { label: "C", alt: "^" },
    { label: "V", alt: "\\" },
    { label: "B", alt: "?" },
    { label: "N", alt: "!" },
    { label: "M", alt: "," },
    { label: ".", alt: "$" },
    { glyph: "enter" },
  ]),
  {
    y: 720,
    h: 104,
    caps: [
      { x: 250, w: 144, text: "sym", id: "sym" },
      { x: 428, w: 144, text: "ctrl" },
      { x: 606, w: 144, glyph: "meta" },
      { x: 958, w: 496 },
      { x: 1312, w: 144, glyph: "clicks", id: "tab", red: true },
      { x: 1490, w: 144, glyph: "keyboard", id: "alt" },
      { x: 1666, w: 144, glyph: "voice", id: "gemini" },
    ],
  },
];

/** Line-art legends, drawn around (0, 0) in `currentColor`. */
function GlyphIcon({ glyph }: { glyph: Glyph }): ReactNode {
  const stroke = { fill: "none", stroke: "currentColor", strokeWidth: 7, strokeLinecap: "round", strokeLinejoin: "round" } as const;
  switch (glyph) {
    case "shift":
      return (
        <g {...stroke}>
          <path d="M0,-30 L-26,-2 H-12 V20 H12 V-2 H26 Z" />
          <path d="M-14,32 H14" />
        </g>
      );
    case "backspace":
      return (
        <g {...stroke}>
          <path d="M-12,-18 H30 V18 H-12 L-32,0 Z" />
          <path d="M0,-8 L16,8 M16,-8 L0,8" />
        </g>
      );
    case "enter":
      return (
        <g {...stroke}>
          <path d="M26,-24 V8 H-24" />
          <path d="M-12,-4 L-26,8 L-12,20" />
        </g>
      );
    case "meta":
      return (
        <g {...stroke} strokeWidth={5}>
          {[-1, 1].flatMap((sx) => [-1, 1].map((sy) => <circle key={`${sx}${sy}`} cx={sx * 17} cy={sy * 13} r={8} />))}
        </g>
      );
    case "clicks":
      return (
        <g {...stroke} strokeWidth={11}>
          <path d="M22,-16 A27,27 0 1 0 22,16" />
          <path d="M2,0 H27" />
        </g>
      );
    case "keyboard":
      return (
        <g fill="currentColor">
          {[-1, 0, 1].flatMap((c) => [0, 1].map((r) => <circle key={`${c}${r}`} cx={c * 16} cy={-24 + r * 15} r={6} />))}
          <path d="M0,6 L28,30 H-28 Z" fill="none" stroke="currentColor" strokeWidth={6} strokeLinejoin="round" />
        </g>
      );
    case "voice":
      return (
        <g {...stroke} strokeWidth={6}>
          <rect x={-18} y={-20} width={16} height={30} rx={8} />
          <path d="M-28,2 A18,18 0 0 0 8,2 M-10,20 V30 M-20,30 H0" />
          <path d="M8,-26 L12,-32 M20,-22 L27,-27 M24,-12 L32,-13" />
        </g>
      );
  }
}

function Key({ cap, y, h, on, legend }: { cap: Cap; y: number; h: number; on: boolean; legend: string }) {
  const fill = on ? "url(#kp-key-on)" : cap.red ? "url(#kp-key-red)" : "url(#kp-key)";
  const color = on || cap.red ? "#fff" : legend;
  const small = h < 60;
  return (
    <g transform={`translate(${cap.x} ${y})`} className="transition-transform duration-300" style={on ? { transform: `translate(${cap.x}px, ${y - 6}px)` } : undefined}>
      <rect x={-cap.w / 2} y={-h / 2 + 8} width={cap.w} height={h} rx={h / 2} fill="#000" opacity={0.7} />
      <rect
        x={-cap.w / 2}
        y={-h / 2}
        width={cap.w}
        height={h}
        rx={h / 2}
        fill={fill}
        stroke={on ? "oklch(0.85 0.1 235)" : "rgb(255 255 255 / 0.07)"}
        strokeWidth={2}
        filter={on ? "url(#kp-glow)" : undefined}
      />
      <g fill={color} color={color} className="transition-[fill,color] duration-500">
        {cap.glyph && <GlyphIcon glyph={cap.glyph} />}
        {cap.text && (
          <text textAnchor="middle" y={14} fontSize={44} fontWeight={500}>
            {cap.text}
          </text>
        )}
        {cap.label &&
          (small ? (
            <text textAnchor="middle" y={13} fontSize={36} fontWeight={500}>
              {cap.label}
            </text>
          ) : (
            <>
              <text textAnchor="middle" x={-12} y={cap.label === "." ? 14 : 22} fontSize={cap.label === "." ? 64 : 56} fontWeight={600}>
                {cap.label}
              </text>
              {cap.alt && (
                <text textAnchor="middle" x={30} y={-8} fontSize={38} fontWeight={600}>
                  {cap.alt}
                </text>
              )}
            </>
          ))}
      </g>
    </g>
  );
}

export function KeyboardPreview({
  glow,
  highlight = [],
  className,
}: {
  /** Backlight level 0–1; 0 is off. */
  glow: number;
  highlight?: KeyId[];
  className?: string;
}) {
  // Legends are lit from behind: grey when the backlight is off, near white
  // at full brightness.
  const legend = `color-mix(in oklch, oklch(0.97 0.02 240) ${Math.round(glow * 100)}%, oklch(0.42 0 0))`;
  return (
    <svg viewBox="30 110 1860 975" className={cn("w-full select-none drop-shadow-2xl", className)} aria-hidden>
      <defs>
        <linearGradient id="kp-body" x1="0" y1="0" x2="0" y2="1">
          <stop offset="0" stopColor="#1b1b1d" />
          <stop offset="1" stopColor="#09090a" />
        </linearGradient>
        <linearGradient id="kp-key" x1="0" y1="0" x2="0" y2="1">
          <stop offset="0" stopColor="#262628" />
          <stop offset="1" stopColor="#0d0d0e" />
        </linearGradient>
        <linearGradient id="kp-key-red" x1="0" y1="0" x2="0" y2="1">
          <stop offset="0" stopColor="#b3141c" />
          <stop offset="1" stopColor="#6b0a10" />
        </linearGradient>
        <linearGradient id="kp-key-on" x1="0" y1="0" x2="0" y2="1">
          <stop offset="0" stopColor="oklch(0.72 0.15 240)" />
          <stop offset="1" stopColor="oklch(0.55 0.17 250)" />
        </linearGradient>
        <radialGradient id="kp-backlight" cx="0.5" cy="0.55" r="0.6">
          <stop offset="0" stopColor="oklch(0.8 0.08 240)" stopOpacity={0.16} />
          <stop offset="1" stopColor="oklch(0.8 0.08 240)" stopOpacity={0} />
        </radialGradient>
        <filter id="kp-glow" x="-50%" y="-50%" width="200%" height="200%">
          <feGaussianBlur stdDeviation="10" result="b" />
          <feFlood floodColor="oklch(0.75 0.15 240)" floodOpacity="0.8" />
          <feComposite in2="b" operator="in" />
          <feMerge>
            <feMergeNode />
            <feMergeNode in="SourceGraphic" />
          </feMerge>
        </filter>
      </defs>

      {/* Phone mount with the magnetic ring, then the keyboard body. */}
      <path d="M150,110 H1770 V190 H150 Z" fill="#0c0c0d" />
      <ellipse cx={960} cy={25} rx={480} ry={78} fill="#050505" stroke="#1d1d1f" strokeWidth={3} />
      <path
        d="M150,185 H1770 C1800,185 1816,200 1820,230 L1868,640 C1880,730 1880,800 1870,862 C1854,1000 1760,1075 1620,1075 H300 C160,1075 66,1000 50,862 C40,800 40,730 52,640 L100,230 C104,200 120,185 150,185 Z"
        fill="url(#kp-body)"
      />
      <path d="M152,188 H1768" stroke="rgb(255 255 255 / 0.08)" strokeWidth={3} />
      <path
        d="M220,225 H1700 C1745,225 1765,245 1770,280 L1810,640 C1822,740 1790,830 1690,830 H230 C130,830 98,740 110,640 L150,280 C155,245 175,225 220,225 Z"
        fill="#0e0e0f"
        stroke="rgb(255 255 255 / 0.04)"
        strokeWidth={2}
      />
      <rect x={130} y={215} width={1660} height={640} fill="url(#kp-backlight)" opacity={glow} className="transition-opacity duration-500" />

      {ROWS.map((r) =>
        r.caps.map((cap, i) => <Key key={`${r.y}-${i}`} cap={cap} y={r.y} h={r.h} on={cap.id != null && highlight.includes(cap.id)} legend={legend} />),
      )}

      {/* Status LED, USB-C port and power key along the bottom edge. */}
      <circle cx={538} cy={1022} r={13} fill="oklch(0.75 0.15 240)" opacity={0.4 + glow * 0.6} />
      <rect x={845} y={1000} width={230} height={52} rx={26} fill="#030303" stroke="#1f1f21" strokeWidth={3} />
      <rect x={1240} y={985} width={245} height={68} rx={22} fill="#0c0c0d" stroke="#232325" strokeWidth={3} />
    </svg>
  );
}
