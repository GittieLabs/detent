// The Mappings screen (architecture.md, UX): the unit's drawing, and a side
// panel that edits the clicked button: when (press or long press), where
// (everywhere, or only while one app is in front; product.md's one concept),
// and what it does. App mappings win over Everywhere for that app. The Rust side
// checks every edit (`app_state::set_mapping`); the panel checks ranges in
// percent first so its messages match what the user typed.

import type { Action, AppEntry, AppState, ControlView, Mapping, MenuItem, PairedUnit, Scope, Trigger } from "./api";
import { DEVICE_MENU_ITEMS, MAX_MENU_ITEMS } from "./api";
import * as api from "./api";
import { type AppName, actionText, idName, namesFrom, shortcutText, triggerText } from "./describe";
import { MAX_COMBOS, comboOf } from "./keys";
import { accessibilityBanner } from "./banners";
import { drawUnit, pickUnit, unitPicker } from "./drawing";
import { button, el, keepingFocus, keepingScroll } from "./dom";
import type { Screen } from "./devices";

type Kind = "none" | Action["type"];

const KINDS: [Kind, string][] = [
  ["none", "Does nothing"],
  ["open_app", "Open an app"],
  ["volume_up", "Turn volume up by…"],
  ["volume_down", "Turn volume down by…"],
  ["volume_set", "Set volume to…"],
  ["mute_toggle", "Mute or unmute"],
  ["send_shortcut", "Send a shortcut"],
  ["applescript", "Run an AppleScript"],
];

/** The form, as typed: numbers stay text until saved. */
interface Draft {
  control: number;
  trigger: Trigger;
  scope: Scope; // { app: "" } while the app is still to be chosen
  kind: Kind;
  bundleId: string;
  stepPct: string;
  levelPct: string;
  source: string;
  timeoutS: string;
  keys: string; // canonical shortcut, recorded in the shortcut field
}

const trim = (n: number) => String(Math.round(n * 100) / 100);

function draftFrom(control: number, trigger: Trigger, scope: Scope, m: Mapping | undefined): Draft {
  const d: Draft = {
    control,
    trigger,
    scope,
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

/** Same scope; app bundle IDs match ignoring case, as the router does. */
function sameScope(a: Scope, b: Scope): boolean {
  if (a === "everywhere" || b === "everywhere") return a === b;
  return a.app.toLowerCase() === b.app.toLowerCase();
}

const findMapping = (u: PairedUnit, control: number, trigger: Trigger, scope: Scope) =>
  u.mappings.find((m) => m.control === control && m.trigger === trigger && sameScope(m.scope, scope));

/** The device menu group goes at the end of the control list. */
function withMenu(list: HTMLElement, menu: HTMLElement): HTMLElement {
  if (list.classList.contains("control-list")) {
    list.append(menu);
    return list;
  }
  // No controls yet (not connected): the menu can still be edited.
  return el("div", { class: "control-list" }, list, menu);
}

export function mappingsScreen(root: HTMLElement, chosen: { serial: string | null }): Screen {
  let current: AppState | null = null;
  let selected: number | null = null;
  let trigger: Trigger = "press"; // outlives the draft, so a save stays on it
  let where: Scope = "everywhere"; // likewise for the scope being edited
  /** The device menu item being edited: its position, or "new" (D-086). */
  let menuSel: number | "new" | null = null;
  let menuLabel = ""; // the label being typed; the action is in `draft`
  /** Which item `draft` and `menuLabel` hold, so a stale form is never shown. */
  let menuDraftFor: number | "new" | null = null;
  let draft: Draft | null = null;
  let note: { text: string; error: boolean } | null = null;
  let busy = false;
  let recordFresh = true; // the shortcut field's next key starts a new shortcut
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
  /**
   * Runs a command; its answer is the new state. `onSaved` runs before the
   * redraw, so selection follows the edit (a moved item's new position)
   * before the form reloads from the new state.
   */
  const run = async (call: () => Promise<AppState>, done: string, onSaved?: () => void) => {
    busy = true;
    try {
      current = await call();
      onSaved?.();
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

  /** Records the keys pressed while it has focus: one combo, or a sequence. */
  function shortcutField(d: Draft): HTMLElement {
    const input = el("input", {
      class: "field-input shortcut",
      value: d.keys ? shortcutText(d.keys) : "",
      ariaLabel: "Shortcut",
      attrs: { readonly: "", placeholder: "Click here, then press the keys" },
      onClick: () => {
        recordFresh = true;
      },
      onKey: (e) => {
        // Plain Tab still moves focus; with a modifier it's recorded.
        if (e.code === "Tab" && !(e.metaKey || e.altKey || e.ctrlKey)) return;
        e.preventDefault();
        const combo = comboOf(e);
        if (!combo) return; // a modifier on its own: wait for the key
        const seq = recordFresh || !d.keys ? [] : d.keys.split(" ");
        recordFresh = false;
        const next = seq.length >= MAX_COMBOS ? [combo] : [...seq, combo];
        edit({ keys: next.join(" ") });
      },
    });
    return el(
      "div",
      { class: "fields" },
      el(
        "label",
        { class: "field" },
        el("span", {}, "Shortcut"),
        el(
          "span",
          { class: "with-unit" },
          input,
          button("Clear", () => {
            recordFresh = true;
            edit({ keys: "" });
          }),
        ),
      ),
      el(
        "p",
        { class: "hint" },
        `Press up to ${MAX_COMBOS} combos in a row for a sequence. Shortcuts go to the app in front; password fields and some games block them.`,
      ),
    );
  }

  function fields(d: Draft): HTMLElement | null {
    switch (d.kind) {
      case "send_shortcut":
        return shortcutField(d);
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

  function perApp(unit: PairedUnit, control: number, editing: Draft): HTMLElement | null {
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
            {
              class:
                m.trigger === editing.trigger && sameScope(m.scope, editing.scope) ? "row editing" : "row",
            },
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
            button("Edit", () => {
              trigger = m.trigger;
              where = m.scope;
              draft = null;
              note = null;
              recordFresh = true;
              rerender();
            }),
            button(
              "Remove",
              () => void run(() => api.removeMapping(unit.serial, control, m.trigger, m.scope), "Removed."),
              { disabled: busy },
            ),
          ),
        ),
      ),
      el("p", { class: "hint" }, "These win over Everywhere while that app is in front."),
    );
  }

  /** Everywhere, or only while one app is in front. */
  function whereField(unit: PairedUnit, c: ControlView, d: Draft): HTMLElement {
    const choose = (scope: Scope) => {
      where = scope;
      draft = null;
      note = null;
      recordFresh = true;
      rerender();
    };
    const inApp = d.scope !== "everywhere";
    const appId = d.scope === "everywhere" ? "" : d.scope.app;
    const option = (a: AppEntry) => el("option", { value: a.bundle_id }, a.name);
    const known = apps.some((a) => a.bundle_id.toLowerCase() === appId.toLowerCase());
    return el(
      "div",
      { class: "fields" },
      el(
        "div",
        { class: "segmented", ariaLabel: "Where" },
        el(
          "button",
          {
            type: "button",
            class: inApp ? "seg" : "seg active",
            onClick: () => choose("everywhere"),
          },
          "Everywhere",
          findMapping(unit, c.id, d.trigger, "everywhere")
            ? el("span", { class: "mapped-dot", ariaLabel: "mapped" })
            : null,
        ),
        el(
          "button",
          {
            type: "button",
            class: inApp ? "seg active" : "seg",
            onClick: () => {
              if (!inApp) choose({ app: "" });
            },
          },
          "Only in an app",
        ),
      ),
      inApp
        ? el(
            "label",
            { class: "field" },
            el("span", {}, "When this app is in front"),
            el(
              "select",
              {
                class: "field-input",
                ariaLabel: "Scope app",
                value: appId,
                onInput: (v) => choose({ app: v }),
              },
              el("option", { value: "" }, apps.length ? "Choose an app…" : "Loading apps…"),
              appId && !known ? el("option", { value: appId }, `${appId} (not found)`) : null,
              el("optgroup", { attrs: { label: "Running" } }, ...apps.filter((a) => a.running).map(option)),
              el(
                "optgroup",
                { attrs: { label: "Other apps" } },
                ...apps.filter((a) => !a.running).map(option),
              ),
            ),
          )
        : null,
    );
  }

  function panel(unit: PairedUnit, c: ControlView): HTMLElement {
    if (!draft || draft.control !== c.id || draft.trigger !== trigger || !sameScope(draft.scope, where)) {
      draft = draftFrom(c.id, trigger, where, findMapping(unit, c.id, trigger, where));
    }
    const d = draft;
    const pendingApp = d.scope !== "everywhere" && d.scope.app === "";
    const saved = pendingApp ? undefined : findMapping(unit, c.id, d.trigger, d.scope);
    const triggers: Trigger[] = c.triggers;

    const save = () => {
      if (pendingApp) {
        note = { text: "Choose the app this applies in.", error: true };
        rerender();
        return;
      }
      const result = actionFrom(d);
      if (result === null) {
        if (saved) void run(() => api.removeMapping(unit.serial, c.id, d.trigger, d.scope), "Saved.");
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
        scope: d.scope,
        action: result.action,
      };
      void run(() => api.setMapping(mapping), "Saved.");
    };

    return el(
      "aside",
      { class: "panel", attrs: { "data-scroll": "panel" } },
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
            pendingApp || !findMapping(unit, c.id, t, d.scope)
              ? null
              : el("span", { class: "mapped-dot", ariaLabel: "mapped" }),
          ),
        ),
      ),
      whereField(unit, c, d),
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
              () => void run(() => api.removeMapping(unit.serial, c.id, d.trigger, d.scope), "Removed."),
              { class: "danger", disabled: busy },
            )
          : null,
        button("Save", save, { class: "primary", disabled: busy }),
      ),
      perApp(unit, c.id, d),
    );
  }

  /** Device menu labels: 1 to 16 printable ASCII (the OLED's font). */
  const labelProblem = (label: string): string | null => {
    if (label.length === 0) return "Give the item a label.";
    if (label.length > 16) return "Labels are at most 16 characters.";
    if (!/^[\x20-\x7E]+$/.test(label)) return "Labels use plain characters only (no accents or emoji).";
    return null;
  };

  /** The board's menu: its own items greyed, then the app's, then Add. */
  function menuGroup(unit: PairedUnit): HTMLElement {
    const pickItem = (i: number | "new") => {
      selected = null;
      menuSel = i;
      const item = i === "new" ? null : (unit.menu[i] ?? null);
      menuLabel = item?.label ?? "";
      draft = draftFrom(-1, "press", "everywhere", item ? ({ action: item.action } as Mapping) : undefined);
      menuDraftFor = i;
      note = null;
      recordFresh = true;
      rerender();
    };
    const rows = unit.menu.map((m, i) =>
      el(
        "div",
        {
          class: menuSel === i ? "control-row selected" : "control-row",
          onClick: () => pickItem(i),
        },
        el("span", { class: "row-name" }, m.label),
        el(
          "span",
          { class: "scope-chip everywhere", title: actionText(m.action, appName) },
          actionText(m.action, appName),
        ),
      ),
    );
    const full = unit.menu.length >= MAX_MENU_ITEMS;
    return el(
      "section",
      // Shares the height with Buttons and scrolls in its own box.
      { class: "control-group" },
      el("h3", { class: "group-title" }, "Device menu"),
      el(
        "div",
        { class: "group-rows", attrs: { "data-scroll": "group:menu" } },
        el(
          "div",
          {
            class: "control-row device-owned",
            title: "The device's own items; Detent's follow them on the OLED menu",
          },
          el("span", { class: "row-name" }, "On the device"),
          el("span", { class: "on-device" }, DEVICE_MENU_ITEMS.join(" · ")),
        ),
        ...rows,
        el(
          "div",
          { class: "control-row add-row" },
          full
            ? el("span", { class: "none" }, `Full: the device shows ${MAX_MENU_ITEMS} items`)
            : button("Add item", () => pickItem("new"), { class: menuSel === "new" ? "primary" : "" }),
        ),
      ),
    );
  }

  /** Edits one device menu item: label, action, order. */
  function menuPanel(unit: PairedUnit, index: number | "new"): HTMLElement {
    const existing = index === "new" ? null : index;
    if (!draft || menuDraftFor !== index) {
      menuDraftFor = index;
      // After a save `run` clears the draft: reload it from the saved item.
      const saved = existing === null ? null : (unit.menu[existing] ?? null);
      draft = draftFrom(-1, "press", "everywhere", saved ? ({ action: saved.action } as Mapping) : undefined);
      if (saved) menuLabel = saved.label;
    }
    const d = draft;
    const count = unit.menu.length;
    const save = () => {
      const problem = labelProblem(menuLabel);
      const result = actionFrom(d);
      if (problem || result === null || "error" in result) {
        note = {
          text: problem ?? (result === null ? "Choose what it does." : "error" in result ? result.error : ""),
          error: true,
        };
        rerender();
        return;
      }
      const item: MenuItem = { label: menuLabel, action: result.action };
      void run(() => api.setMenuItem(unit.serial, existing, item), "Saved.", () => {
        if (existing === null) menuSel = Math.min(count, MAX_MENU_ITEMS - 1);
      });
    };
    const move = (to: number) => {
      if (existing === null) return;
      void run(() => api.moveMenuItem(unit.serial, existing, to), "Moved.", () => {
        menuSel = to;
      });
    };
    return el(
      "aside",
      { class: "panel", attrs: { "data-scroll": "panel" } },
      el("h2", { class: "panel-title" }, existing === null ? "New device menu item" : "Device menu item"),
      el(
        "label",
        { class: "field" },
        el("span", {}, `Label (${menuLabel.length}/16)`),
        el("input", {
          class: "field-input",
          value: menuLabel,
          ariaLabel: "Menu label",
          attrs: { maxlength: "16", placeholder: "Mail", spellcheck: "false" },
          onInput: (v) => {
            menuLabel = v;
            note = null;
            rerender();
          },
          onKey: (e) => {
            if (e.key === "Enter") save();
          },
        }),
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
            value: d.kind === "none" ? "" : d.kind,
            onInput: (v) => edit({ kind: v as Kind }),
          },
          el("option", { value: "" }, "Choose…"),
          ...KINDS.filter(([k]) => k !== "none").map(([k, label]) => el("option", { value: k }, label)),
        ),
      ),
      fields(d),
      note ? el("p", { class: note.error ? "note error" : "note" }, note.text) : null,
      el(
        "footer",
        { class: "actions" },
        existing !== null
          ? button("Move up", () => move(existing - 1), { disabled: busy || existing === 0 })
          : null,
        existing !== null
          ? button("Move down", () => move(existing + 1), { disabled: busy || existing >= count - 1 })
          : null,
      ),
      el(
        "footer",
        { class: "actions" },
        existing !== null
          ? button(
              "Remove",
              () =>
                void run(() => api.removeMenuItem(unit.serial, existing), "Removed.", () => {
                  menuSel = null;
                }),
              { class: "danger", disabled: busy },
            )
          : null,
        button("Save", save, { class: "primary", disabled: busy }),
      ),
      el("p", { class: "hint" }, "Items show on the device's menu after its own, in this order."),
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
      menuSel = null;
      draft = null;
    }
    chosen.serial = unit.serial;
    const control = unit.controls.find((c) => c.id === selected && !c.handled_by_device) ?? null;
    if (typeof menuSel === "number" && menuSel >= unit.menu.length) menuSel = null;
    if (!control) {
      selected = null;
      if (menuSel === null) draft = null;
    }
    /** A row opens its first trigger, Everywhere; a chip opens its own mapping. */
    const pick = (c: ControlView, t?: Trigger, scope?: Scope) => {
      const nextTrigger = t ?? c.triggers[0] ?? "press";
      const nextWhere = scope ?? "everywhere";
      if (selected !== c.id || trigger !== nextTrigger || where !== nextWhere || menuSel !== null) {
        menuSel = null;
        recordFresh = true;
        selected = c.id;
        trigger = nextTrigger;
        where = nextWhere;
        draft = null;
        note = null;
      }
      rerender();
    };
    keepingScroll(root, () =>
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
            withMenu(drawUnit(unit, { selected, onPick: pick, appName }), menuGroup(unit)),
            control
              ? panel(unit, control)
              : menuSel !== null
                ? menuPanel(unit, menuSel)
                : el(
                    "aside",
                    { class: "panel empty-panel" },
                    "Click a button or a device menu item to choose what it does.",
                  ),
          ),
        ].filter((p): p is HTMLElement => p !== null),
      ),
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
