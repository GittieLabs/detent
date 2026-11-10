// Banners shared by Mappings and Test.

import type { AppState } from "./api";
import * as api from "./api";
import { button, el } from "./dom";

/**
 * Shown while a mapping sends a shortcut and Detent lacks Accessibility
 * (architecture.md, Permissions). Fix asks macOS and opens the Settings
 * pane; the banner goes once the grant is seen, within about 2 s.
 */
export function accessibilityBanner(state: AppState, onError: (m: string) => void): HTMLElement | null {
  if (!state.accessibility.banner) return null;
  return el(
    "div",
    { class: "banner permission" },
    el(
      "div",
      { class: "grow" },
      el("strong", {}, "Shortcuts need Accessibility. "),
      "Turn on Detent in System Settings > Privacy & Security > Accessibility. Until then, shortcut buttons do nothing.",
    ),
    button(
      "Fix",
      () => {
        api.fixAccessibility().catch((e) => onError(api.message(e)));
      },
      { class: "primary" },
    ),
  );
}
