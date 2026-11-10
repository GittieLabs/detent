// A unit's controls from its HELLO as a grouped list (D-085). Shared by
// Mappings (click to edit) and Test (lights up on a press). Which controls
// exist and which the device handles comes from the Rust side
// (`app_state::ControlView`, D-081).

import type { AppState, ControlView, Mapping, PairedUnit, Scope, Trigger } from "./api";
import { type AppName, actionText, idName } from "./describe";
import { el } from "./dom";

export interface DrawOptions {
  /** The control being edited. */
  selected?: number | null;
  /** The control just pressed (Test). */
  lit?: number | null;
  /**
   * Makes mappable controls clickable. A chip passes its trigger and scope,
   * so the panel opens on that exact mapping.
   */
  onPick?: (control: ControlView, trigger?: Trigger, scope?: Scope) => void;
  appName?: AppName;
}

/** One trigger of one control: a cell in a control's row. */
interface Cell {
  label: string;
  control: ControlView;
  trigger: Trigger;
}

/** One row: a control (or, for an encoder, the knob and its push switch). */
interface Row {
  name: string;
  controls: ControlView[];
  cells: Cell[];
  /** Set when the device handles it all itself: shown instead of cells. */
  onDevice: string | null;
}

/** The encoder's push switch: a button the drawing shows on the knob's row. */
const isPush = (c: ControlView) => c.type === "button" && c.label === "Encoder push";

function rowsOf(unit: PairedUnit): { title: string; rows: Row[] }[] {
  const buttons = unit.controls.filter((c) => c.type === "button" && !isPush(c));
  const encoders = unit.controls.filter((c) => c.type === "encoder");
  const push = unit.controls.find(isPush) ?? null;
  const groups: { title: string; rows: Row[] }[] = [];
  if (buttons.length) {
    groups.push({
      title: buttons.length === 1 ? "Button" : "Buttons",
      rows: buttons.map((c) => ({
        name: c.label,
        controls: [c],
        cells: [
          { label: "Press", control: c, trigger: "press" },
          { label: "Long press", control: c, trigger: "long_press" },
        ],
        onDevice: null,
      })),
    });
  }
  if (encoders.length) {
    groups.push({
      title: encoders.length === 1 ? "Encoder" : "Encoders",
      rows: encoders.map((c, i) => {
        // One push switch per device today; it belongs to the first knob.
        const sw = i === 0 ? push : null;
        const controls = sw ? [c, sw] : [c];
        const onDevice = controls.every((x) => x.handled_by_device)
          ? `Turn: volume${sw ? " · Push: mute" : ""}, on the device`
          : null;
        const cells: Cell[] = [];
        if (!c.handled_by_device) {
          cells.push({ label: "Turn left", control: c, trigger: "counterclockwise" });
          cells.push({ label: "Turn right", control: c, trigger: "clockwise" });
        }
        if (sw && !sw.handled_by_device) cells.push({ label: "Push", control: sw, trigger: "press" });
        return { name: c.label, controls, cells, onDevice };
      }),
    });
  } else if (push) {
    // A push switch with no knob reported: list it with the buttons.
    groups.push({
      title: "Switch",
      rows: [
        {
          name: push.label,
          controls: [push],
          cells: [{ label: "Press", control: push, trigger: "press" }],
          onDevice: push.handled_by_device ? "Handled on the device" : null,
        },
      ],
    });
  }
  return groups;
}

const scopeName = (scope: Scope, appName: AppName) =>
  scope === "everywhere" ? "Everywhere" : appName(scope.app);

/** Everywhere first, then apps by name. */
function ordered(mappings: Mapping[], appName: AppName): Mapping[] {
  return [...mappings].sort((a, b) => {
    if (a.scope === "everywhere") return -1;
    if (b.scope === "everywhere") return 1;
    return scopeName(a.scope, appName).localeCompare(scopeName(b.scope, appName));
  });
}

function cellView(cell: Cell, unit: PairedUnit, opts: DrawOptions): HTMLElement {
  const appName = opts.appName ?? idName;
  const mine = ordered(
    unit.mappings.filter((m) => m.control === cell.control.id && m.trigger === cell.trigger),
    appName,
  );
  const onPick = opts.onPick;
  const chips = mine.map((m) => {
    const where = m.scope === "everywhere" ? "everywhere" : `in ${appName(m.scope.app)}`;
    const tip = `${cell.label} ${where}: ${actionText(m.action, appName)}`;
    return el(
      onPick ? "button" : "span",
      {
        type: onPick ? "button" : undefined,
        class: m.scope === "everywhere" ? "scope-chip everywhere" : "scope-chip app",
        title: tip,
        ariaLabel: tip,
        onClick: onPick
          ? (e) => {
              e.stopPropagation();
              onPick(cell.control, cell.trigger, m.scope);
            }
          : undefined,
      },
      scopeName(m.scope, appName),
    );
  });
  return el(
    "div",
    { class: "cell", title: mine.length ? undefined : `${cell.label}: nothing mapped` },
    el("span", { class: "cell-label" }, cell.label),
    el("span", { class: "chips" }, ...(chips.length ? chips : [el("span", { class: "none" }, "—")])),
  );
}

function rowView(row: Row, unit: PairedUnit, opts: DrawOptions): HTMLElement {
  const classes = ["control-row"];
  if (row.onDevice) classes.push("device-owned");
  if (row.controls.some((c) => c.id === opts.selected)) classes.push("selected");
  if (row.controls.some((c) => c.id === opts.lit)) classes.push("lit");
  const first = row.cells[0];
  const onPick = opts.onPick;
  const pickable = !!onPick && !!first;
  return el(
    "div",
    {
      class: classes.join(" "),
      title: row.onDevice ? "The device handles this in Volume mode, so Detent doesn't map it" : undefined,
      onClick: pickable && onPick ? () => onPick(first.control, first.trigger, "everywhere") : undefined,
    },
    el("span", { class: "row-name" }, row.name),
    row.onDevice
      ? el("span", { class: "on-device" }, row.onDevice)
      : el("div", { class: "cells" }, ...row.cells.map((c) => cellView(c, unit, opts))),
  );
}

/**
 * The unit as a list (D-085): one row per control, grouped by type, each
 * trigger a cell of scope chips (Everywhere, then each app) whose hover text
 * says what fires. Works for any count and mix of controls HELLO reports.
 */
export function drawUnit(unit: PairedUnit, opts: DrawOptions = {}): HTMLElement {
  if (unit.controls.length === 0) {
    return el(
      "p",
      { class: "empty" },
      unit.connected
        ? `Waiting for ${unit.name} to describe its controls…`
        : `Connect ${unit.name} to see its controls.`,
    );
  }
  return el(
    "div",
    { class: "control-list" },
    ...rowsOf(unit).map((g) =>
      el(
        "section",
        // A group of one or two rows keeps its height; longer ones scroll.
        { class: g.rows.length <= 2 ? "control-group fixed" : "control-group" },
        el("h3", { class: "group-title" }, g.title),
        // Each group scrolls on its own, so a long one never hides the rest.
        el(
          "div",
          { class: "group-rows", attrs: { "data-scroll": `group:${g.title}` } },
          ...g.rows.map((r) => rowView(r, unit, opts)),
        ),
      ),
    ),
  );
}

/** The unit to show: the chosen one if still paired, else a connected one. */
export function pickUnit(state: AppState, serial: string | null): PairedUnit | null {
  return (
    state.paired.find((u) => u.serial === serial) ??
    state.paired.find((u) => u.connected) ??
    state.paired[0] ??
    null
  );
}

/** A unit picker, shown only with more than one paired unit. */
export function unitPicker(
  state: AppState,
  current: PairedUnit,
  onChoose: (serial: string) => void,
): HTMLElement | null {
  if (state.paired.length < 2) return null;
  return el(
    "select",
    { class: "unit-picker", ariaLabel: "Device", value: current.serial, onInput: onChoose },
    ...state.paired.map((u) =>
      el("option", { value: u.serial }, `${u.name}${u.connected ? "" : " (not connected)"}`),
    ),
  );
}
