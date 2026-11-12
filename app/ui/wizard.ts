// The pairing wizard (architecture.md, UX): a view inside the Devices screen,
// not a screen of its own (D-072). It runs while the Rust side is
// identifying: unpaired units are open read only and a button press picks one
// (D-066). Product.md's four steps show as three panels: plug in and press a
// button share the first.

import * as api from "./api";
import type { AppState, Candidate } from "./api";
import { type AppName, idName, mappingText, namesFrom } from "./describe";
import { button, el, keepingFocus } from "./dom";

type Step = "find" | "name" | "done";

export interface Wizard {
  /** Starts identification and shows the first panel. */
  open(): Promise<void>;
  /** Shows the first panel for identification started elsewhere (the tray). */
  adopt(state: AppState): void;
  /** The wizard is showing, so state goes to it rather than to Devices. */
  active(): boolean;
  show(state: AppState): void;
}

function title(model: string | null): string {
  return model ? `Detent ${model}` : "Detent";
}

function describe(c: Candidate): string {
  const parts: string[] = [];
  if (c.outdated) return "Firmware too old for this Detent: update it before pairing";
  if (c.newer) return "Firmware newer than this Detent: update Detent before pairing";
  if (c.firmware) parts.push(`firmware ${c.firmware}`);
  if (c.buttons || c.encoders) {
    parts.push(
      `${c.buttons} button${c.buttons === 1 ? "" : "s"}, ` +
        `${c.encoders} encoder${c.encoders === 1 ? "" : "s"}`,
    );
  }
  return parts.join(" · ") || "Reading its details…";
}

/** `onClose` runs after the wizard ends, so Devices can show again. */
export function pairingWizard(root: HTMLElement, onClose: () => void): Wizard {
  let step: Step | null = null;
  let current: AppState | null = null;
  let picked: Candidate | null = null; // the unit being named
  let draft = "";
  let paired: { serial: string; name: string } | null = null;
  let error: string | null = null;
  let busy = false; // a command is running; its own answer settles the step
  let fresh = false; // the name panel just opened: focus and select the name
  let useTemplate = true; // the starter template box (D-077), ticked by default
  let appName: AppName = idName;

  const reset = () => {
    step = null;
    picked = null;
    draft = "";
    paired = null;
    error = null;
    busy = false;
    fresh = false;
    useTemplate = true;
  };

  /** App names for the template list; bundle IDs until they arrive. */
  const loadAppNames = () => {
    api
      .listApps()
      .then((apps) => {
        appName = namesFrom(apps);
        render();
      })
      .catch(() => {});
  };

  const close = async () => {
    try {
      await api.endPairing();
    } catch {
      // Closing the window ends pairing too; nothing to show here.
    }
    reset();
    onClose();
  };

  /** Moves between steps from the latest state. */
  function advance(state: AppState) {
    if (step === "find") {
      const c = state.candidates.find((c) => c.picked);
      if (c) {
        picked = c;
        draft = c.suggested_name;
        error = null;
        fresh = true;
        step = "name";
      }
    } else if (step === "name" && picked && !busy) {
      const serial = picked.serial;
      const still = state.candidates.find((c) => c.serial === serial);
      if (still) {
        picked = still;
      } else {
        picked = null;
        error = "That device was unplugged or ignored. Plug it in and press a button on it.";
        step = "find";
      }
    }
  }

  function header(text: string, n: number): HTMLElement {
    return el(
      "header",
      { class: "bar" },
      el("h1", {}, text),
      el("span", { class: "steps" }, `Step ${n} of 3`),
    );
  }

  function candidateRow(c: Candidate): HTMLElement {
    return el(
      "li",
      { class: "row" },
      el("span", { class: "dot new", ariaLabel: "Not paired" }),
      el(
        "div",
        { class: "grow" },
        el("div", { class: "name" }, title(c.model)),
        el("div", { class: "sub" }, describe(c)),
        el("div", { class: "serial" }, `Serial ${c.serial}`),
      ),
      button("Ignore", () => {
        void (async () => {
          try {
            await api.ignore(c.vid, c.pid, c.serial);
          } catch (e) {
            error = api.message(e);
            render();
          }
        })();
      }),
    );
  }

  function findPanel(state: AppState): HTMLElement[] {
    const serialless = state.unknown.some((u) => u.serial === null);
    return [
      header("Pair new device", 1),
      el(
        "p",
        { class: "lead" },
        "Plug in your Detent, then press any button on the one you want to pair.",
      ),
      error ? el("div", { class: "banner" }, error) : null,
      state.candidates.length
        ? el("ul", { class: "list" }, ...state.candidates.map(candidateRow))
        : el(
            "p",
            { class: "empty" },
            "Looking for an unpaired Detent… If yours is plugged in, try a USB cable that carries data.",
          ),
      serialless
        ? el("p", { class: "hint" }, "A Detent without a USB serial number is plugged in. It can't be paired.")
        : null,
      el("footer", { class: "actions" }, button("Cancel", () => void close())),
    ].filter((p): p is HTMLElement => p !== null);
  }

  function namePanel(c: Candidate): HTMLElement[] {
    const save = async () => {
      busy = true;
      try {
        const name = draft.trim();
        await api.pair(c.vid, c.pid, c.serial, name);
        paired = { serial: c.serial, name };
        picked = null;
        error = null;
        step = "done";
      } catch (e) {
        error = api.message(e);
      }
      busy = false;
      render();
    };
    const input = el("input", {
      class: "name-input",
      value: draft,
      ariaLabel: "Device name",
      onInput: (value) => {
        draft = value;
      },
      onKey: (e) => {
        if (e.key === "Enter" && !busy) void save();
      },
    });
    return [
      header("Name it", 2),
      el(
        "div",
        { class: "picked" },
        el("span", { class: "dot new", ariaLabel: "Picked" }),
        el(
          "div",
          { class: "grow" },
          el("div", { class: "name" }, `${title(c.model)} picked`),
          el("div", { class: "sub" }, describe(c)),
          el("div", { class: "serial" }, `Serial ${c.serial}`),
        ),
      ),
      error ? el("div", { class: "banner" }, error) : null,
      el("label", { class: "field" }, el("span", {}, "Name"), input),
      el(
        "footer",
        { class: "actions" },
        button("Cancel", () => void close()),
        button("Pair", () => void save(), { class: "primary", disabled: busy }),
      ),
    ].filter((p): p is HTMLElement => p !== null);
  }

  function donePanel(state: AppState, done: { serial: string; name: string }): HTMLElement[] {
    const unit = state.paired.find((u) => u.serial === done.serial);
    const status = unit?.connected
      ? ["Connected", unit.firmware ? `firmware ${unit.firmware}` : null]
          .filter((s): s is string => s !== null)
          .join(" · ")
      : "Connecting…";
    const model = unit?.model ?? null;
    const template = unit?.template ?? [];
    const finish = async () => {
      if (useTemplate && template.length) {
        busy = true;
        try {
          await api.applyTemplate(done.serial);
        } catch (e) {
          busy = false;
          error = api.message(e);
          render();
          return;
        }
      }
      await close();
    };
    const box = template.length
      ? el(
          "label",
          { class: "check" },
          el("input", {
            type: "checkbox",
            checked: useTemplate,
            ariaLabel: "Add starter mappings",
            onClick: (e) => {
              useTemplate = (e.target as HTMLInputElement).checked;
            },
          }),
          el(
            "span",
            {},
            `Start with the ${title(model)} mappings`,
            el(
              "ul",
              { class: "template" },
              ...template.map((m) => el("li", {}, mappingText(m, appName))),
            ),
            el("span", { class: "hint-inline" }, "You can change them on Mappings."),
          ),
        )
      : el(
          "p",
          { class: "hint" },
          `${title(model)} has no starter mappings. Add your own on Mappings.`,
        );
    return [
      header("Done", 3),
      el(
        "div",
        { class: "picked" },
        el("span", {
          class: `dot ${unit?.connected ? "on" : "off"}`,
          ariaLabel: unit?.connected ? "Connected" : "Connecting",
        }),
        el(
          "div",
          { class: "grow" },
          el("div", { class: "name" }, `${done.name} is paired`),
          el("div", { class: "sub" }, status),
        ),
      ),
      error ? el("div", { class: "banner" }, error) : null,
      box,
      el(
        "footer",
        { class: "actions" },
        button("Finish", () => void finish(), { class: "primary", disabled: busy }),
      ),
    ].filter((p): p is HTMLElement => p !== null);
  }

  function render() {
    const state = current;
    if (!state || step === null) return;
    keepingFocus(root, () => {
      let parts: HTMLElement[] = [];
      if (step === "find") parts = findPanel(state);
      else if (step === "name" && picked) parts = namePanel(picked);
      else if (step === "done" && paired) parts = donePanel(state, paired);
      root.replaceChildren(el("div", { class: "wizard" }, ...parts));
    });
    if (step === "name" && fresh) {
      fresh = false;
      const input = root.querySelector<HTMLInputElement>(".name-input");
      input?.focus();
      input?.select();
    }
  }

  return {
    async open() {
      reset();
      loadAppNames();
      const state = await api.startPairing();
      step = "find";
      current = state;
      render();
    },
    adopt(state: AppState) {
      reset();
      loadAppNames();
      step = "find";
      current = state;
      advance(state);
      render();
    },
    active: () => step !== null,
    show(state: AppState) {
      current = state;
      if (step === null) return;
      if (!state.identifying) {
        // Identification ended elsewhere (the window closed, or another
        // window's call): the wizard has nothing left to show.
        reset();
        onClose();
        return;
      }
      advance(state);
      render();
    },
  };
}
