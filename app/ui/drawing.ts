// A unit drawn from its HELLO (architecture.md, UX): buttons as keys, the
// encoder as a knob. Shared by Mappings (click to edit) and Test (lights up
// on a press). Which controls exist and which the device handles comes from
// the Rust side (`app_state::ControlView`, D-081).

import type { AppState, ControlView, Mapping, PairedUnit } from "./api";
import { type AppName, actionText, idName } from "./describe";
import { el } from "./dom";

export interface DrawOptions {
  /** The control being edited. */
  selected?: number | null;
  /** The control just pressed (Test). */
  lit?: number | null;
  /** Makes mappable controls clickable. */
  onPick?: (control: ControlView) => void;
  appName?: AppName;
}

/** The encoder and its push switch are drawn as the knob. */
const onKnob = (c: ControlView) => c.type === "encoder" || c.label === "Encoder push";

/** What a control does everywhere on a press, in a few words. */
function summary(c: ControlView, mappings: Mapping[], appName: AppName): string {
  if (c.handled_by_device) return c.type === "encoder" ? "Volume (on the device)" : "Mute (on the device)";
  const mine = mappings.filter((m) => m.control === c.id && m.scope === "everywhere");
  const press = mine.find((m) => m.trigger === "press");
  const long = mine.find((m) => m.trigger === "long_press");
  const parts = [press ? actionText(press.action, appName) : "Not mapped"];
  if (long) parts.push(`hold: ${actionText(long.action, appName)}`);
  return parts.join(" · ");
}

function appBadge(c: ControlView, mappings: Mapping[], appName: AppName): HTMLElement | null {
  const apps = new Set(
    mappings
      .filter((m) => m.control === c.id && m.scope !== "everywhere")
      .map((m) => (m.scope === "everywhere" ? "" : appName(m.scope.app))),
  );
  if (apps.size === 0) return null;
  const names = [...apps];
  return el(
    "span",
    { class: "badge", title: `Does something else in ${names.join(", ")}` },
    names.length === 1 ? `in ${names[0]}` : `in ${names.length} apps`,
  );
}

function control(c: ControlView, unit: PairedUnit, opts: DrawOptions): HTMLElement {
  const appName = opts.appName ?? idName;
  const classes = ["control", onKnob(c) ? (c.type === "encoder" ? "knob" : "push") : "key"];
  if (c.handled_by_device) classes.push("device-owned");
  if (opts.selected === c.id) classes.push("selected");
  if (opts.lit === c.id) classes.push("lit");
  const pickable = !!opts.onPick && !c.handled_by_device;
  const onPick = opts.onPick;
  return el(
    pickable ? "button" : "div",
    {
      class: classes.join(" "),
      type: pickable ? "button" : undefined,
      title: c.handled_by_device
        ? "Handled by the device in Volume mode, so Detent doesn't map it"
        : undefined,
      onClick: pickable && onPick ? () => onPick(c) : undefined,
    },
    el("span", { class: "control-name" }, c.label),
    el("span", { class: "control-does" }, summary(c, unit.mappings, appName)),
    appBadge(c, unit.mappings, appName),
  );
}

/** The drawing, or a note when the unit hasn't connected this session. */
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
  const keys = unit.controls.filter((c) => !onKnob(c));
  const knob = unit.controls.filter(onKnob);
  return el(
    "div",
    { class: "device" },
    el("div", { class: "keys" }, ...keys.map((c) => control(c, unit, opts))),
    knob.length ? el("div", { class: "knob-group" }, ...knob.map((c) => control(c, unit, opts))) : null,
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
