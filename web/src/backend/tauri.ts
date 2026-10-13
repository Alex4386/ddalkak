import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

import type { Backend } from "./types";

/** Desktop app: commands run in app/src-tauri over btleplug. */
export const backend: Backend = {
  kind: "tauri",
  support: async () => ({ ok: true }),
  invoke: (cmd, ...args) => invoke(cmd, args[0] as Record<string, unknown> | undefined),
  listen: (event, handler) => listen(event, (e) => handler(e.payload as never)),
};
