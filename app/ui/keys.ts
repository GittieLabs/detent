// The shortcut field's key mapping: a KeyboardEvent to Detent's key names
// (D-082). KeyboardEvent.code is the physical key, matching the US ANSI key
// codes Detent posts. `keycodes.json` is checked against `src/keys.rs` by a
// Rust test.

import table from "./keycodes.json";

const KEYS: Record<string, string> = table;

export interface KeyLike {
  code: string;
  metaKey: boolean;
  altKey: boolean;
  ctrlKey: boolean;
  shiftKey: boolean;
}

/**
 * The combo a key press means, in canonical form (`cmd+shift+e`), or null
 * for a modifier on its own or a key Detent can't send.
 */
export function comboOf(e: KeyLike): string | null {
  const key = KEYS[e.code];
  if (!key) return null;
  const mods = [
    e.metaKey ? "cmd" : null,
    e.altKey ? "option" : null,
    e.ctrlKey ? "control" : null,
    e.shiftKey ? "shift" : null,
  ].filter((m): m is string => m !== null);
  return [...mods, key].join("+");
}

/** The most combos one shortcut holds (`keys::MAX_COMBOS`). */
export const MAX_COMBOS = 4;
