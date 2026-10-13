import { SendIcon, TriangleAlertIcon } from "lucide-react";
import { useState } from "react";

import { Alert, AlertDescription, AlertTitle } from "@/components/reui/alert";
import { Frame, FrameDescription, FrameHeader, FramePanel, FrameTitle } from "@/components/reui/frame";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { ScrollArea } from "@/components/ui/scroll-area";
import { useKeyboard } from "@/hooks/use-keyboard";
import { hex, hexBytes } from "@/lib/format";

type LogLine = { request: number[]; reply: string; ok: boolean };

export function AdvancedPanel() {
  const { snapshot, call, busy } = useKeyboard();
  const [input, setInput] = useState("");
  const [log, setLog] = useState<LogLine[]>([]);
  const [error, setError] = useState<string | null>(null);
  if (!snapshot) return null;

  const send = async () => {
    const clean = input.replace(/[^0-9a-f]/gi, "");
    if (!clean || clean.length % 2) return setError("Enter an even number of hex digits.");
    setError(null);
    const payload = clean.match(/../g)!.map((b) => parseInt(b, 16));
    let line: LogLine;
    try {
      const r = await call("send_raw", { payload });
      line = { request: payload, reply: `status ${hex(r.status)}  ${hexBytes(r.data)}`, ok: r.status === 0 };
    } catch (e) {
      line = { request: payload, reply: String(e), ok: false };
    }
    setLog((l) => [line, ...l].slice(0, 100));
  };

  const rows: [string, string, string][] = [
    ...snapshot.info.map((r): [string, string, string] => [hex(r.addr), r.value == null ? "?" : hex(r.value), r.label]),
    ["—", snapshot.status ? hexBytes(snapshot.status) : "?", "Status (opcode 27)"],
  ];

  return (
    <div className="grid gap-4 xl:grid-cols-2">
      <Alert variant="destructive" className="xl:col-span-2">
        <TriangleAlertIcon />
        <AlertTitle>Use at your own risk</AlertTitle>
        <AlertDescription>
          These tools talk to the keyboard directly, with no safety checks. A wrong write can change settings you can't
          change back, unpair hosts or leave the keyboard misbehaving until a factory reset. Nothing here is supported
          by Clicks.
        </AlertDescription>
      </Alert>

      <Frame>
        <FrameHeader>
          <FrameTitle>Registers</FrameTitle>
          <FrameDescription>Read-only values from the last sync.</FrameDescription>
        </FrameHeader>
        <FramePanel className="p-0">
          <table className="w-full text-sm">
            <tbody className="divide-y">
              {rows.map(([addr, value, label]) => (
                <tr key={label}>
                  <td className="px-(--frame-panel-px) py-2 font-mono text-xs text-muted-foreground">{addr}</td>
                  <td className="py-2 font-mono text-xs">{value}</td>
                  <td className="px-(--frame-panel-px) py-2 text-right text-muted-foreground">{label}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </FramePanel>
      </Frame>

      <Frame>
        <FrameHeader>
          <FrameTitle>Raw console</FrameTitle>
          <FrameDescription>Sends a request payload (opcode + args); framing and CRC are added.</FrameDescription>
        </FrameHeader>
        <FramePanel className="flex flex-col gap-3">
          <Alert variant="warning">
            <AlertDescription>Writes go straight to the keyboard. Opcodes 03, 08, 09 and A0–A3 are best left alone.</AlertDescription>
          </Alert>
          <form
            className="flex gap-2"
            onSubmit={(e) => {
              e.preventDefault();
              send();
            }}
          >
            <Input
              value={input}
              onChange={(e) => setInput(e.target.value)}
              placeholder="25 08 01"
              spellCheck={false}
              className="font-mono"
              aria-invalid={error != null}
            />
            <Button type="submit" disabled={busy}>
              <SendIcon />
              Send
            </Button>
          </form>
          {error && <p className="text-xs text-destructive">{error}</p>}
          <ScrollArea className="h-48 rounded-lg border bg-muted/30">
            <div className="space-y-2 p-3 font-mono text-xs">
              {log.length === 0 && <div className="text-muted-foreground">Replies show up here.</div>}
              {log.map((l, i) => (
                <div key={log.length - i}>
                  <div>
                    <span className="text-muted-foreground">› </span>
                    {hexBytes(l.request)}
                  </div>
                  <div className={l.ok ? "text-success" : "text-destructive"}>
                    <span className="text-muted-foreground">‹ </span>
                    {l.reply}
                  </div>
                </div>
              ))}
            </div>
          </ScrollArea>
        </FramePanel>
      </Frame>
    </div>
  );
}
