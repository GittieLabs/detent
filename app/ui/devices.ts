// The Devices screen (product.md): paired units with a live connected dot,
// rename and unpair; units plugged in but not paired; the Ignored list; and
// Pair new device, which opens the wizard.

import * as api from "./api";
import type { AppState, PairedUnit, UnknownUnit } from "./api";
import { button, el, keepingFocus } from "./dom";

/** What the screen is in the middle of; lives only in the window. */
interface Local {
  renaming: string | null; // serial
  draft: string; // the name being typed, kept across re-renders
  confirmUnpair: { serial: string; mappings: number } | null;
  error: string | null;
}

const local: Local = { renaming: null, draft: "", confirmUnpair: null, error: null };

export interface Screen {
  /** Re-renders from the latest state. */
  show(state: AppState): void;
  /** Shows an error above the lists, e.g. when the wizard couldn't start. */
  error(message: string): void;
}

export function devicesScreen(root: HTMLElement, openWizard: (() => void) | null): Screen {
  let current: AppState | null = null;

  const rerender = () => {
    if (current) render(current);
  };

  /** Runs a command; its answer is the new state, its error shown inline. */
  const run = async (call: () => Promise<AppState>) => {
    try {
      current = await call();
      local.error = null;
    } catch (e) {
      local.error = api.message(e);
    }
    rerender();
  };

  /** Opens the pairing wizard, or shows disabled until it exists. */
  function wizardButton(label: string, props: { class?: string } = {}): HTMLElement {
    return openWizard
      ? button(label, openWizard, props)
      : button(label, () => {}, { ...props, disabled: true, title: "Arrives with the pairing wizard" });
  }

  function details(u: PairedUnit): string {
    const parts: string[] = [];
    if (u.outdated) parts.push("Firmware too old for this Detent: update it to use this unit");
    if (u.newer) parts.push("Firmware newer than this Detent: update Detent to use this unit");
    if (u.model) parts.push(u.model);
    if (u.firmware) parts.push(`firmware ${u.firmware}`);
    if (u.buttons || u.encoders) {
      const b = `${u.buttons} button${u.buttons === 1 ? "" : "s"}`;
      const e = `${u.encoders} encoder${u.encoders === 1 ? "" : "s"}`;
      parts.push(`${b}, ${e}`);
    }
    if (parts.length === 0) parts.push(u.connected ? "Connected" : "Not seen since Detent started");
    return parts.join(" · ");
  }

  function pairedRow(u: PairedUnit): HTMLElement {
    const dot = el("span", {
      class: `dot ${u.connected ? "on" : "off"}`,
      title: u.connected ? "Connected" : "Not connected",
      ariaLabel: u.connected ? "Connected" : "Not connected",
    });

    if (local.renaming === u.serial) {
      const input = el("input", {
        class: "name-input",
        value: local.draft,
        ariaLabel: "Device name",
        onInput: (value) => {
          local.draft = value;
        },
        onKey: (e) => {
          if (e.key === "Enter") save();
          if (e.key === "Escape") cancel();
        },
      }) as HTMLInputElement;
      const save = () => {
        local.renaming = null;
        void run(() => api.rename(u.serial, input.value));
      };
      const cancel = () => {
        local.renaming = null;
        rerender();
      };
      queueMicrotask(() => input.focus());
      return el(
        "li",
        { class: "row" },
        dot,
        el("div", { class: "grow" }, input),
        button("Save", save, { class: "primary" }),
        button("Cancel", cancel),
      );
    }

    if (local.confirmUnpair?.serial === u.serial) {
      const n = local.confirmUnpair.mappings;
      const what =
        n === 0 ? "It has no mappings." : `This also removes its ${n} mapping${n === 1 ? "" : "s"}.`;
      return el(
        "li",
        { class: "row confirm" },
        el("div", { class: "grow" }, el("strong", {}, `Unpair ${u.name}?`), " ", what),
        button(
          "Unpair",
          () => {
            local.confirmUnpair = null;
            void run(() => api.unpair(u.serial));
          },
          { class: "danger" },
        ),
        button("Cancel", () => {
          local.confirmUnpair = null;
          rerender();
        }),
      );
    }

    return el(
      "li",
      { class: "row" },
      dot,
      el(
        "div",
        { class: "grow" },
        el("div", { class: "name" }, u.name),
        el("div", { class: "sub" }, details(u)),
        el("div", { class: "serial" }, `Serial ${u.serial}`),
      ),
      button("Rename", () => {
        local.renaming = u.serial;
        local.draft = u.name;
        local.confirmUnpair = null;
        rerender();
      }),
      button("Unpair", async () => {
        try {
          const mappings = await api.mappingsOf(u.serial);
          local.confirmUnpair = { serial: u.serial, mappings };
          local.renaming = null;
        } catch (e) {
          local.error = api.message(e);
        }
        rerender();
      }),
    );
  }

  function unknownRow(u: UnknownUnit): HTMLElement {
    const serial = u.serial;
    return el(
      "li",
      { class: "row" },
      el("span", { class: "dot new", ariaLabel: "New" }),
      el(
        "div",
        { class: "grow" },
        el("div", { class: "name" }, "Detent device, not paired"),
        el("div", { class: "serial" }, serial ? `Serial ${serial}` : "No serial: it can't be paired"),
      ),
      serial ? wizardButton("Pair…") : null,
      serial ? button("Ignore", () => void run(() => api.ignore(u.vid, u.pid, serial))) : null,
    );
  }

  function render(state: AppState) {
    const paired = state.paired.length
      ? el("ul", { class: "list" }, ...state.paired.map(pairedRow))
      : el(
          "p",
          { class: "empty" },
          "No paired device yet. Plug in your Detent and choose Pair new device.",
        );

    const unknown = state.unknown;
    const ignored = state.ignored;

    const parts: (HTMLElement | null)[] = [
      el(
        "header",
        { class: "bar" },
        el("h1", {}, "Devices"),
        wizardButton("Pair new device", { class: "primary" }),
      ),
      state.problem
        ? el(
            "div",
            { class: "banner" },
            el("strong", {}, "Detent can't run normally. "),
            state.problem,
            state.config_broken
              ? el(
                  "p",
                  {},
                  "Fix the file and relaunch Detent, or start fresh: the broken file is kept beside the new one. ",
                  button("Start fresh", () => run(api.startFresh)),
                )
              : null,
          )
        : null,
      local.error ? el("div", { class: "banner" }, local.error) : null,
      state.paused
        ? el(
            "div",
            { class: "banner" },
            el("strong", {}, "Paused. "),
            "Presses are logged but run nothing. Turn Pause off in the menu bar.",
          )
        : null,
      el("section", {}, paired),
      unknown.length
        ? el(
            "section",
            {},
            el("h2", {}, "Plugged in, not paired"),
            el("ul", { class: "list" }, ...unknown.map(unknownRow)),
          )
        : null,
      ignored.length
        ? el(
            "section",
            {},
            el("h2", {}, "Ignored"),
            el(
              "p",
              { class: "hint" },
              "Detent never opens these or offers them for pairing.",
            ),
            el(
              "ul",
              { class: "list" },
              ...ignored.map((u) =>
                el(
                  "li",
                  { class: "row" },
                  el("div", { class: "grow" }, el("div", { class: "serial" }, `Serial ${u.serial}`)),
                  button("Un-ignore", () => void run(() => api.unignore(u.serial))),
                ),
              ),
            ),
          )
        : null,
    ];
    keepingFocus(root, () =>
      root.replaceChildren(...parts.filter((p): p is HTMLElement => p !== null)),
    );
  }

  return {
    show(state: AppState) {
      current = state;
      // A unit that left can't still be mid rename or unpair.
      const serials = new Set(state.paired.map((u) => u.serial));
      if (local.renaming && !serials.has(local.renaming)) local.renaming = null;
      if (local.confirmUnpair && !serials.has(local.confirmUnpair.serial)) local.confirmUnpair = null;
      render(state);
    },
    error(message: string) {
      local.error = message;
      rerender();
    },
  };
}
