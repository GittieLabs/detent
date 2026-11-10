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
  const screens: Record<Tab, Screen> = {
    devices,
    mappings: mappingsScreen(root, chosen),
    test: testScreen(root, chosen),
  };

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
            onClick: () => {
              tab = id;
              if (latest) show(latest);
            },
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
      tab = "devices";
      wizard.adopt(state);
    } else if (wizard.active()) {
      wizard.show(state);
    } else {
      screens[tab].show(state);
    }
    renderNav();
  }

  await api.onState(show);
  show(await api.getState());
}

const app = document.querySelector<HTMLElement>("#app");
if (app) {
  start(app).catch((e) => {
    app.textContent = `Detent couldn't load its settings: ${api.message(e)}`;
  });
}
