// The settings window. M3 has the Devices screen and its pairing wizard;
// Mappings and Test arrive in M4 (three screens at most, product.md). State
// comes from the Rust side: fetched once, then pushed on every change (D-071).

import * as api from "./api";
import type { AppState } from "./api";
import { devicesScreen } from "./devices";
import { pairingWizard } from "./wizard";
import "./style.css";

async function start(root: HTMLElement) {
  let latest: AppState | null = null;
  const wizard = pairingWizard(root, () => {
    if (latest) devices.show(latest);
  });
  const devices = devicesScreen(root, () => {
    wizard.open().catch((e) => devices.error(`Couldn't start pairing: ${api.message(e)}`));
  });
  const show = (state: AppState) => {
    latest = state;
    // Pairing started from the tray's "New device found" opens the wizard.
    if (!wizard.active() && state.identifying) wizard.adopt(state);
    else if (wizard.active()) wizard.show(state);
    else devices.show(state);
  };
  await api.onState(show);
  show(await api.getState());
}

const root = document.querySelector<HTMLElement>("#app");
if (root) {
  start(root).catch((e) => {
    root.textContent = `Detent couldn't load its settings: ${api.message(e)}`;
  });
}
