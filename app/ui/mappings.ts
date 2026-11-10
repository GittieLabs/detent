// The Mappings screen (architecture.md, UX): the unit's drawing, and a side
// panel that edits the clicked button. Scope is Everywhere here: app scoped
// mappings show read only with Remove (creating them is M5). The Rust side
// checks every edit (`app_state::set_mapping`); the panel checks ranges in
// percent first so its messages match what the user typed.

import type { Action, AppEntry, AppState, ControlView, Mapping, PairedUnit, Trigger } from "./api";
import * as api from "./api";
import { type AppName, actionText, idName, namesFrom, triggerText } from "./describe";
import { accessibilityBanner } from "./banners";
import { drawUnit, pickUnit, unitPicker } from "./drawing";
import { button, el, keepingFocus } from "./dom";
import type { Screen } from "./devices";

type Kind = "none" | Action["type"];

const KINDS: [Kind, string][] = [
  ["none", "Does nothing"],
  ["open_app", "Open an app"],
  ["volume_up", "Turn volume up by…"],
  ["volume_down", "Turn volume down by…"],
  ["volume_set", "Set volume to…"],
  ["mute_toggle", "Mute or unmute"],
  ["applescript", "Run an AppleScript"],
];

/** The form, as typed: numbers stay text until saved. */
interface Draft {
  control: number;
  trigger: Trigger;
  kind: Kind;
  bundleId: string;
  stepPct: string;
  levelPct: string;
  source: string;
  timeoutS: string;
  keys: string; // canonical shortcut, recorded in the shortcut field
}

const trim = (n: number) => String(Math.round(n * 100) / 100);

function draftFrom(control: number, trigger: Trigger, m: Mapping | undefined): Draft {
  const d: Draft = {
    control,
    trigger,
    kind: "none",
    bundleId: "",
    stepPct: "6.25",
    levelPct: "50",
    source: "",
    timeoutS: "5",
    keys: "",
  };
  const a = m?.action;
  if (!a) return d;
  d.kind = a.type;
  if (a.type === "open_app") d.bundleId = a.bundle_id;
  if (a.type === "volume_up" || a.type === "volume_down") d.stepPct = trim(a.step * 100);
  if (a.type === "volume_set") d.levelPct = trim(a.level * 100);
  if (a.type === "send_shortcut") d.keys = a.keys;
  if (a.type === "applescript") {
    d.source = a.source;
    d.timeoutS = trim(a.timeout_ms / 1000);
  }
  return d;
}

/** The action the form describes, or why it can't be saved yet. */
function actionFrom(d: Draft): { action: Action } | { error: string } | null {
  const num = (s: string) => (s.trim() === "" ? NaN : Number(s));
  switch (d.kind) {
    case "none":
      return null;
    case "open_app":
      return d.bundleId ? { action: { type: "open_app", bundle_id: d.bundleId } } : { error: "Choose an app." };
    case "volume_up":
    case "volume_down": {
      const pct = num(d.stepPct);
      if (!(pct > 0 && pct <= 100)) return { error: "The step must be more than 0% and at most 100%." };
      return { action: { type: d.kind, step: pct / 100 } };
    }
    case "volume_set": {
      const pct = num(d.levelPct);
      if (!(pct >= 0 && pct <= 100)) return { error: "The volume must be from 0% to 100%." };
      return { action: { type: "volume_set", level: pct / 100 } };
    }
    case "mute_toggle":
      return { action: { type: "mute_toggle" } };
    case "send_shortcut":
      return d.keys ? { action: { type: "send_shortcut", keys: d.keys } } : { error: "Record a shortcut." };
    case "applescript": {
      if (d.source.trim() === "") return { error: "Write the script to run." };
      const s = num(d.timeoutS);
      if (!(s >= 0.1 && s <= 60)) return { error: "The time limit must be from 0.1 to 60 seconds." };
      return { action: { type: "applescript", source: d.source, timeout_ms: Math.round(s * 1000) } };
    }
  }
}

const everywhere = (u: PairedUnit, control: number, trigger: Trigger) =>
  u.mappings.find((m) => m.control === control && m.trigger === trigger && m.scope === "everywhere");

export function mappingsScreen(root: HTMLElement, chosen: { serial: string | null }): Screen {
  let current: AppState | null = null;
  let selected: number | null = null;
  let trigger: Trigger = "press"; // outlives the draft, so a save stays on it
  let draft: Draft | null = null;
  let note: { text: string; error: boolean } | null = null;
  let busy = false;
  let apps: AppEntry[] = [];
  let appName: AppName = idName;

  api
    .listApps()
    .then((list) => {
      apps = list;
      appName = namesFrom(list);
      rerender();
    })
    .catch(() => {});

  const rerender = () => {
    if (current) render(current);
  };

  const edit = (change: Partial<Draft>) => {
    if (!draft) return;
    draft = { ...draft, ...change };
    note = null;
    rerender();
  };

  /** Runs a command; its answer is the new state. */
  const run = async (call: () => Promise<AppState>, done: string) => {
    busy = true;
    try {
      current = await call();
      draft = null; // re-read from the saved mappings
      note = { text: done, error: false };
    } catch (e) {
      note = { text: api.message(e), error: true };
    }
    busy = false;
    rerender();
  };

  function appPicker(d: Draft): HTMLElement {
    const known = apps.some((a) => a.bundle_id.toLowerCase() === d.bundleId.toLowerCase());
    const option = (a: AppEntry) => el("option", { value: a.bundle_id }, a.name);
    return el(
      "select",
      {
        class: "field-input",
        ariaLabel: "App",
        value: d.bundleId,
        onInput: (v) => edit({ bundleId: v }),
      },
      el("option", { value: "" }, apps.length ? "Choose an app…" : "Loading apps…"),
      d.bundleId && !known ? el("option", { value: d.bundleId }, `${d.bundleId} (not found)`) : null,
      el("optgroup", { attrs: { label: "Running" } }, ...apps.filter((a) => a.running).map(option)),
      el("optgroup", { attrs: { label: "Other apps" } }, ...apps.filter((a) => !a.running).map(option)),
    );
  }

  function percent(label: string, value: string, change: (v: string) => void, min: string): HTMLElement {
    return el(
      "label",
      { class: "field" },
      el("span", {}, label),
      el(
        "span",
        { class: "with-unit" },
        el("input", {
          class: "field-input number",
          type: "number",
          value,
          ariaLabel: label,
          attrs: { min, max: "100", step: "any" },
          onInput: change,
        }),
        "%",
      ),
    );
  }

  function fields(d: Draft): HTMLElement | null {
    switch (d.kind) {
      case "open_app":
        return el("label", { class: "field" }, el("span", {}, "App"), appPicker(d));
      case "volume_up":
      case "volume_down":
        return percent(
          d.kind === "volume_up" ? "Up by" : "Down by",
          d.stepPct,
          (v) => edit({ stepPct: v }),
          "0",
        );
      case "volume_set":
        return percent("Set to", d.levelPct, (v) => edit({ levelPct: v }), "0");
      case "applescript":
        return el(
          "div",
          { class: "fields" },
          el(
            "label",
            { class: "field" },
            el("span", {}, "Script"),
            el("textarea", {
              class: "field-input script",
              value: d.source,
              ariaLabel: "Script",
              attrs: { rows: "6", spellcheck: "false", placeholder: 'display notification "Hello"' },
              onInput: (v) => edit({ source: v }),
            }),
          ),
          el(
            "label",
            { class: "field" },
            el("span", {}, "Time limit"),
            el(
              "span",
              { class: "with-unit" },
              el("input", {
                class: "field-input number",
                type: "number",
                value: d.timeoutS,
                ariaLabel: "Time limit",
                attrs: { min: "0.1", max: "60", step: "any" },
                onInput: (v) => edit({ timeoutS: v }),
              }),
              "seconds",
            ),
          ),
          el(
            "p",
            { class: "hint" },
            "The first run against an app asks for macOS's permission to control it.",
          ),
        );
      default:
        return null;
    }
  }

  function perApp(unit: PairedUnit, control: number): HTMLElement | null {
    const scoped = unit.mappings.filter((m) => m.control === control && m.scope !== "everywhere");
    if (scoped.length === 0) return null;
    return el(
      "div",
      { class: "per-app" },
      el("h3", {}, "In particular apps"),
      el(
        "ul",
        { class: "list" },
        ...scoped.map((m) =>
          el(
            "li",
            { class: "row" },
            el(
              "div",
              { class: "grow" },
              el(
                "div",
                {},
                `${triggerText(m.trigger)} in ${m.scope === "everywhere" ? "" : appName(m.scope.app)}`,
              ),
              el("div", { class: "sub" }, actionText(m.action, appName)),
            ),
            button(
              "Remove",
              () => void run(() => api.removeMapping(unit.serial, control, m.trigger, m.scope), "Removed."),
              { disabled: busy },
            ),
          ),
        ),
      ),
      el("p", { class: "hint" }, "These win over the setting above while that app is in front."),
    );
  }

  function panel(unit: PairedUnit, c: ControlView): HTMLElement {
    if (!draft || draft.control !== c.id || draft.trigger !== trigger) {
      draft = draftFrom(c.id, trigger, everywhere(unit, c.id, trigger));
    }
    const d = draft;
    const saved = everywhere(unit, c.id, d.trigger);
    const triggers: Trigger[] = c.type === "encoder" ? ["clockwise", "counterclockwise"] : ["press", "long_press"];

    const save = () => {
      const result = actionFrom(d);
      if (result === null) {
        if (saved) void run(() => api.removeMapping(unit.serial, c.id, d.trigger, "everywhere"), "Saved.");
        else note = { text: "Nothing to save.", error: false };
        rerender();
        return;
      }
      if ("error" in result) {
        note = { text: result.error, error: true };
        rerender();
        return;
      }
      const mapping: Mapping = {
        device: unit.serial,
        control: c.id,
        trigger: d.trigger,
        scope: "everywhere",
        action: result.action,
      };
      void run(() => api.setMapping(mapping), "Saved.");
    };

    return el(
      "aside",
      { class: "panel" },
      el("h2", { class: "panel-title" }, c.label),
      el(
        "div",
        { class: "segmented", ariaLabel: "When" },
        ...triggers.map((t) =>
          el(
            "button",
            {
              type: "button",
              class: t === d.trigger ? "seg active" : "seg",
              onClick: () => {
                trigger = t;
                draft = null;
                note = null;
                rerender();
              },
            },
            triggerText(t),
            everywhere(unit, c.id, t) ? el("span", { class: "mapped-dot", ariaLabel: "mapped" }) : null,
          ),
        ),
      ),
      el(
        "label",
        { class: "field" },
        el("span", {}, "Does"),
        el(
          "select",
          {
            class: "field-input",
            ariaLabel: "Action",
            value: d.kind,
            onInput: (v) => edit({ kind: v as Kind }),
          },
          ...KINDS.map(([k, label]) => el("option", { value: k }, label)),
        ),
      ),
      fields(d),
      note ? el("p", { class: note.error ? "note error" : "note" }, note.text) : null,
      el(
        "footer",
        { class: "actions" },
        saved
          ? button(
              "Remove",
              () => void run(() => api.removeMapping(unit.serial, c.id, d.trigger, "everywhere"), "Removed."),
              { class: "danger", disabled: busy },
            )
          : null,
        button("Save", save, { class: "primary", disabled: busy }),
      ),
      perApp(unit, c.id),
    );
  }

  function render(state: AppState) {
    const unit = pickUnit(state, chosen.serial);
    if (!unit) {
      root.replaceChildren(
        el("header", { class: "bar" }, el("h1", {}, "Mappings")),
        el("p", { class: "empty" }, "Pair a device on Devices first."),
      );
      return;
    }
    if (chosen.serial !== unit.serial) {
      selected = null;
      draft = null;
    }
    chosen.serial = unit.serial;
    const control = unit.controls.find((c) => c.id === selected && !c.handled_by_device) ?? null;
    if (!control) {
      selected = null;
      draft = null;
    }
    const pick = (c: ControlView) => {
      if (selected !== c.id) {
        selected = c.id;
        trigger = c.type === "encoder" ? "clockwise" : "press";
        draft = null;
        note = null;
      }
      rerender();
    };
    keepingFocus(root, () =>
      root.replaceChildren(
        ...[
          el(
            "header",
            { class: "bar" },
            el("h1", {}, "Mappings"),
            unitPicker(state, unit, (serial) => {
              chosen.serial = serial;
              rerender();
            }),
          ),
          accessibilityBanner(state, (m) => {
            note = { text: m, error: true };
            rerender();
          }),
          el(
            "div",
            { class: "editor" },
            drawUnit(unit, { selected, onPick: pick, appName }),
            control
              ? panel(unit, control)
              : el("aside", { class: "panel empty-panel" }, "Click a button to choose what it does."),
          ),
        ].filter((p): p is HTMLElement => p !== null),
      ),
    );
  }

  return {
    show(state: AppState) {
      current = state;
      render(state);
    },
    error(message: string) {
      note = { text: message, error: true };
      rerender();
    },
  };
}
