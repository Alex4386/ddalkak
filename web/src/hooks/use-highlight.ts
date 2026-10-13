import { useSyncExternalStore } from "react";

/** Key slot the pointer or focus is on, so the keyboard preview can light it up. */
let current: number | null = null;
const subscribers = new Set<() => void>();

export function setHighlight(slot: number | null) {
  if (slot === current) return;
  current = slot;
  subscribers.forEach((s) => s());
}

export const useHighlight = () =>
  useSyncExternalStore(
    (cb) => {
      subscribers.add(cb);
      return () => subscribers.delete(cb);
    },
    () => current,
  );
