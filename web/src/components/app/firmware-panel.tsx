import { CircleCheckIcon, DownloadIcon, RefreshCwIcon, SparklesIcon } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { toast } from "sonner";

import type { FirmwareStatus } from "@/backend";
import { Alert, AlertDescription, AlertTitle } from "@/components/reui/alert";
import { Badge } from "@/components/reui/badge";
import { Frame, FrameDescription, FrameFooter, FrameHeader, FramePanel, FrameTitle } from "@/components/reui/frame";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import { Spinner } from "@/components/ui/spinner";
import { useKeyboard } from "@/hooks/use-keyboard";

export function FirmwarePanel() {
  const { call, backend } = useKeyboard();
  const [status, setStatus] = useState<FirmwareStatus | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [checking, setChecking] = useState(false);
  const [downloading, setDownloading] = useState(false);

  const check = useCallback(async () => {
    setChecking(true);
    setError(null);
    try {
      setStatus(await call("check_firmware"));
    } catch (e) {
      setError(String(e));
    } finally {
      setChecking(false);
    }
  }, [call]);

  useEffect(() => {
    if (backend) check();
  }, [backend, check]);

  const target = status?.update ?? status?.latest ?? null;

  return (
    <Frame>
      <FrameHeader className="flex-row items-start justify-between gap-3">
        <div>
          <FrameTitle>Firmware</FrameTitle>
          <FrameDescription>Checked against the vendor's update list; downloads are verified with SHA-256.</FrameDescription>
        </div>
        <Button variant="ghost" size="sm" onClick={check} disabled={checking}>
          {checking ? <Spinner /> : <RefreshCwIcon />}
          <span className="hidden sm:inline">Check</span>
        </Button>
      </FrameHeader>
      <FramePanel>
        {error ? (
          <Alert variant="warning">
            <AlertTitle>Couldn't check for updates</AlertTitle>
            <AlertDescription>{error}</AlertDescription>
          </Alert>
        ) : !status ? (
          <div className="space-y-2">
            <Skeleton className="h-6 w-40" />
            <Skeleton className="h-4 w-64" />
          </div>
        ) : (
          <div className="flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between">
            <div className="flex items-center gap-3">
              <div
                className={
                  status.update
                    ? "flex size-11 items-center justify-center rounded-xl bg-info/10 text-info"
                    : "flex size-11 items-center justify-center rounded-xl bg-success/10 text-success"
                }
              >
                {status.update ? <SparklesIcon className="size-5" /> : <CircleCheckIcon className="size-5" />}
              </div>
              <div>
                <div className="text-base font-semibold">
                  {!status.latest
                    ? `Nothing published for ${status.model}`
                    : status.update
                      ? `Version ${status.update.version} is available`
                      : status.installed
                        ? "Up to date"
                        : `Latest is ${status.latest.version}`}
                </div>
                <div className="mt-0.5 flex flex-wrap items-center gap-1.5 text-xs text-muted-foreground">
                  <Badge variant="outline" size="sm">
                    {status.model}
                  </Badge>
                  {status.installed && <span>Installed {status.installed}</span>}
                </div>
              </div>
            </div>
            {target && (
              <Button
                variant={status.update ? "default" : "outline"}
                disabled={downloading}
                onClick={async () => {
                  setDownloading(true);
                  try {
                    const where = await call("download_firmware", { info: target });
                    toast.success(`Saved ${where}`);
                  } catch (e) {
                    toast.error(String(e));
                  } finally {
                    setDownloading(false);
                  }
                }}
              >
                {downloading ? <Spinner /> : <DownloadIcon />}
                Download {target.version}
              </Button>
            )}
          </div>
        )}
      </FramePanel>
      <FrameFooter className="text-xs text-muted-foreground">
        Installing isn't supported here yet; use the official app to flash an update.
      </FrameFooter>
    </Frame>
  );
}
