import type { Backend } from "./types";

export type * from "./types";

/** The same UI runs inside the Tauri app and in a plain browser. */
export const isTauri = "__TAURI_INTERNALS__" in window;

export async function loadBackend(): Promise<Backend> {
  if (isTauri) return (await import("./tauri")).backend;
  return (await import("./bluetooth")).backend;
}
