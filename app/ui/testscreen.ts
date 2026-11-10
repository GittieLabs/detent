// The Test screen (architecture.md, UX): the same drawing, live. Presses
// light up their control; Dry run arrives in a later task (D-079).

import type { AppState } from "./api";
import { drawUnit, pickUnit, unitPicker } from "./drawing";
import { el } from "./dom";
import type { Screen } from "./devices";

export function testScreen(root: HTMLElement, chosen: { serial: string | null }): Screen {
  function render(state: AppState) {
    const unit = pickUnit(state, chosen.serial);
    if (!unit) {
      root.replaceChildren(
        el("header", { class: "bar" }, el("h1", {}, "Test")),
        el("p", { class: "empty" }, "Pair a device on Devices first."),
      );
      return;
    }
    chosen.serial = unit.serial;
    root.replaceChildren(
      ...[
        el(
          "header",
          { class: "bar" },
          el("h1", {}, "Test"),
          unitPicker(state, unit, (serial) => {
            chosen.serial = serial;
            render(state);
          }),
        ),
        drawUnit(unit),
      ].filter((p): p is HTMLElement => p !== null),
    );
  }
  return {
    show: render,
    error() {},
  };
}
