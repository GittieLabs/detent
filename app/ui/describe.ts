// Mappings in plain words, for the wizard, the editor and the Test screen.
// Display only: what a mapping does is decided on the Rust side.

import type { Action, Mapping, Scope, Trigger } from "./api";

/** Looks up an app's name by bundle ID; falls back to the ID. */
export type AppName = (bundleId: string) => string;

export const idName: AppName = (id) => id;

const pct = (v: number) => `${Math.round(v * 100)}%`;

const SYMBOLS: Record<string, string> = { cmd: "⌘", option: "⌥", control: "⌃", shift: "⇧" };
const KEY_TEXT: Record<string, string> = {
  space: "Space",
  return: "↩",
  tab: "⇥",
  escape: "⎋",
  delete: "⌫",
  forwarddelete: "⌦",
  left: "←",
  right: "→",
  up: "↑",
  down: "↓",
  pageup: "⇞",
  pagedown: "⇟",
  home: "↖",
  end: "↘",
  minus: "-",
  equal: "=",
  leftbracket: "[",
  rightbracket: "]",
  backslash: "\\",
  semicolon: ";",
  quote: "'",
  comma: ",",
  period: ".",
  slash: "/",
  grave: "`",
};

/** `cmd+shift+e` as the Mac shows it: ⌘⇧E (modifiers in Apple's order). */
export function shortcutText(keys: string): string {
  return keys
    .trim()
    .split(/\s+/)
    .map((combo) => {
      const parts = combo.toLowerCase().split("+");
      const key = parts.pop() ?? "";
      const mods = ["control", "option", "shift", "cmd"]
        .filter((m) => parts.includes(m))
        .map((m) => SYMBOLS[m])
        .join("");
      const shown = KEY_TEXT[key] ?? (key.length === 1 ? key.toUpperCase() : key.toUpperCase());
      return mods + shown;
    })
    .join(" ");
}

export function actionText(action: Action, appName: AppName = idName): string {
  switch (action.type) {
    case "open_app":
      return `Open ${appName(action.bundle_id)}`;
    case "volume_up":
      return `Volume up by ${pct(action.step)}`;
    case "volume_down":
      return `Volume down by ${pct(action.step)}`;
    case "volume_set":
      return `Volume set to ${pct(action.level)}`;
    case "mute_toggle":
      return "Mute or unmute";
    case "send_shortcut":
      return `Send ${shortcutText(action.keys)}`;
    case "open_dashboard": {
      const on = action.display ? ` on ${action.display}` : "";
      return `Show dashboard${on}${action.mode === "window" ? " in a window" : ""}`;
    }
    case "applescript":
      return "Run an AppleScript";
  }
}

export function triggerText(trigger: Trigger): string {
  switch (trigger) {
    case "press":
      return "Press";
    case "long_press":
      return "Long press";
    case "clockwise":
      return "Turn right";
    case "counterclockwise":
      return "Turn left";
  }
}

export function scopeText(scope: Scope, appName: AppName = idName): string {
  return scope === "everywhere" ? "everywhere" : `in ${appName(scope.app)}`;
}

/** `Button 1, press, in Safari: Mute or unmute`. */
export function mappingText(m: Mapping, appName: AppName = idName): string {
  const where = m.scope === "everywhere" ? "" : `, ${scopeText(m.scope, appName)}`;
  const how = m.trigger === "press" ? "" : `, ${triggerText(m.trigger).toLowerCase()}`;
  return `Button ${m.control}${how}${where}: ${actionText(m.action, appName)}`;
}

/** Builds an `AppName` from the app picker's list. */
export function namesFrom(apps: { bundle_id: string; name: string }[]): AppName {
  const byId = new Map(apps.map((a) => [a.bundle_id.toLowerCase(), a.name]));
  return (id) => byId.get(id.toLowerCase()) ?? id;
}
