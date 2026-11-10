// The settings window. M3 has the Devices screen; Mappings and Test arrive in
// M4 (three screens at most, product.md). State comes from the Rust side:
// fetched once, then pushed on every change (D-071).

import * as api from "./api";
import { devicesScreen } from "./devices";
import "./style.css";

async function start(root: HTMLElement) {
  // The pairing wizard arrives in the next task; until then its buttons show
  // disabled.
  const devices = devicesScreen(root, null);
  await api.onState((state) => devices.show(state));
  devices.show(await api.getState());
}

const root = document.querySelector<HTMLElement>("#app");
if (root) {
  start(root).catch((e) => {
    root.textContent = `Detent couldn't load its settings: ${api.message(e)}`;
  });
}
