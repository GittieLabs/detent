// The settings window: three screens, Devices, Mappings and Test (product.md),
// under a tab bar. The pairing wizard is a view of Devices (D-072). State
// comes from the Rust side: fetched once, then pushed on every change (D-071).

import * as api from "./api";
import type { AppState } from "./api";
import { type Screen, devicesScreen } from "./devices";
import { el } from "./dom";
import { mappingsScreen } from "./mappings";
import { testScreen } from "./testscreen";
import { pairingWizard } from "./wizard";
import "./style.css";

type Tab = "devices" | "mappings" | "test";
const TABS: [Tab, string][] = [
  ["devices", "Devices"],
  ["mappings", "Mappings"],
  ["test", "Test"],
];

async function start(app: HTMLElement) {
  const nav = el("nav", { class: "tabs", ariaLabel: "Screens" });
  const root = el("div", { class: "screen" });
  app.replaceChildren(nav, root);

  let latest: AppState | null = null;
  let tab: Tab = "devices";
  // The unit Mappings and Test show, shared so switching keeps it.
  const chosen = { serial: null as string | null };

  const wizard = pairingWizard(root, () => {
    if (latest) show(latest);
  });
  const devices = devicesScreen(root, () => {
    wizard.open().catch((e) => devices.error(`Couldn't start pairing: ${api.message(e)}`));
  });
  const test = testScreen(root, chosen);
  const screens: Record<Tab, Screen> = {
    devices,
    mappings: mappingsScreen(root, chosen),
    test,
  };

  /** Switches screens. Dry run is on exactly while Test shows (D-079). */
  function go(next: Tab) {
    if (next === tab) return;
    const was = tab;
    tab = next;
    if (next === "test") api.setDryRun(true).catch((e) => test.error(api.message(e)));
    else if (was === "test") api.setDryRun(false).catch(() => {});
    if (latest) show(latest);
  }

  function renderNav() {
    // The wizard has its own Cancel and Finish; no switching mid pairing.
    nav.hidden = wizard.active();
    nav.replaceChildren(
      ...TABS.map(([id, label]) =>
        el(
          "button",
          {
            type: "button",
            class: id === tab ? "tab active" : "tab",
            ariaLabel: label,
            onClick: () => go(id),
          },
          label,
        ),
      ),
    );
  }

  function show(state: AppState) {
    latest = state;
    // Pairing started from the tray's "New device found" opens the wizard.
    if (!wizard.active() && state.identifying) {
      if (tab === "test") api.setDryRun(false).catch(() => {});
      tab = "devices";
      wizard.adopt(state);
    } else if (wizard.active()) {
      wizard.show(state);
    } else {
      screens[tab].show(state);
    }
    // Mappings and Test fill the window and scroll inside (their list, panel
    // and feed); Devices and the wizard scroll as a page.
    root.classList.toggle("fill", !wizard.active() && tab !== "devices");
    renderNav();
  }

  await api.onState(show);
  await api.onInput((view) => {
    if (tab === "test" && !wizard.active()) test.input(view);
  });
  await api.onResult((view) => {
    if (tab === "test" && !wizard.active()) test.result(view);
  });
  // Debug entries are kept even off the Test screen, so switching back
  // shows what happened meanwhile.
  await api.onDebug((entry) => test.debug(entry, tab === "test" && !wizard.active()));
  const first = await api.getState();
  // Launched with --debug: open on Test with what's been logged so far.
  if (first.debug) {
    test.debugLoad(await api.getDebugLog());
    go("test");
  }
  show(first);
}

const app = document.querySelector<HTMLElement>("#app");
if (app) {
  start(app).catch((e) => {
    app.textContent = `Detent couldn't load its settings: ${api.message(e)}`;
  });
}
