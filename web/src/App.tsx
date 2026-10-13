import { ThemeProvider } from "next-themes";

import { ConnectScreen } from "@/components/app/connect-screen";
import { Dashboard } from "@/components/app/dashboard";
import { Header } from "@/components/app/header";
import { Toaster } from "@/components/ui/sonner";
import { Spinner } from "@/components/ui/spinner";
import { TooltipProvider } from "@/components/ui/tooltip";
import { KeyboardProvider, useKeyboard } from "@/hooks/use-keyboard";

function Screen() {
  const { phase } = useKeyboard();
  if (phase === "loading") {
    return (
      <div className="flex flex-1 items-center justify-center">
        <Spinner className="size-6 text-muted-foreground" />
      </div>
    );
  }
  return phase === "connected" ? <Dashboard /> : <ConnectScreen />;
}

export default function App() {
  return (
    <ThemeProvider attribute="class" defaultTheme="system" enableSystem disableTransitionOnChange>
      <TooltipProvider>
        <KeyboardProvider>
          <div className="relative flex min-h-svh flex-col overflow-x-clip">
            <div className="pointer-events-none absolute inset-x-0 top-0 -z-10 h-96 bg-[radial-gradient(60%_100%_at_50%_0%,oklch(0.7_0.12_250/0.12),transparent)]" />
            <Header />
            <main className="flex flex-1 flex-col">
              <Screen />
            </main>
          </div>
          <Toaster position="bottom-center" richColors={false} />
        </KeyboardProvider>
      </TooltipProvider>
    </ThemeProvider>
  );
}
