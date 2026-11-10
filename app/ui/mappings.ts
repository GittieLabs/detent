// The Mappings screen (architecture.md, UX): the unit's drawing; clicking a
// control edits it in a side panel (the editor lands in the next task).

import type { AppState, ControlView } from "./api";
import * as api from "./api";
import { type AppName, idName, namesFrom } from "./describe";
import { drawUnit, pickUnit, unitPicker } from "./drawing";
import { el } from "./dom";
import type { Screen } from "./devices";

export function mappingsScreen(root: HTMLElement, chosen: { serial: string | null }): Screen {
  let current: AppState | null = null;
  let selected: number | null = null;
  let appName: AppName = idName;

  api
    .listApps()
    .then((apps) => {
      appName = namesFrom(apps);
      if (current) render(current);
    })
    .catch(() => {});

  function render(state: AppState) {
    const unit = pickUnit(state, chosen.serial);
    if (!unit) {
      root.replaceChildren(
        el("header", { class: "bar" }, el("h1", {}, "Mappings")),
        el("p", { class: "empty" }, "Pair a device on Devices first."),
      );
      return;
    }
    chosen.serial = unit.serial;
    const pick = (c: ControlView) => {
      selected = c.id;
      render(state);
    };
    root.replaceChildren(
      ...[
        el(
          "header",
          { class: "bar" },
          el("h1", {}, "Mappings"),
          unitPicker(state, unit, (serial) => {
            chosen.serial = serial;
            selected = null;
            render(state);
          }),
        ),
        el("p", { class: "hint" }, "Click a button to choose what it does."),
        drawUnit(unit, { selected, onPick: pick, appName }),
      ].filter((p): p is HTMLElement => p !== null),
    );
  }

  return {
    show(state: AppState) {
      current = state;
      render(state);
    },
    error() {},
  };
}
