// The Test screen (architecture.md, UX): the drawing, live. A press lights
// its control and lists what fired for the frontmost app and what happened.
// Dry run is on while this screen shows and executes nothing (D-079); main.ts
// turns it on and off as the screen is entered and left. Debug (a toggle
// here, or `--debug`) adds the raw event log, the last HELLO and the
// permissions panel (architecture.md, UX; M9).

import type { AppState, DebugEntry, InputView, PairedUnit, ResultView } from "./api";
import * as api from "./api";
import { type AppName, actionText, idName, namesFrom, triggerText } from "./describe";
import { accessibilityBanner } from "./banners";
import { drawUnit, pickUnit, unitPicker } from "./drawing";
import { button, el, keepingScroll } from "./dom";

export interface TestScreen {
  show(state: AppState): void;
  input(view: InputView): void;
  result(view: ResultView): void;
  error(message: string): void;
  /** A Debug log entry, while Debug is on; drawn only if `visible`. */
  debug(entry: DebugEntry, visible: boolean): void;
  /** The Debug log so far (the window opened with Debug already on). */
  debugLoad(entries: DebugEntry[]): void;
}

interface Entry {
  input: InputView;
  result: ResultView | null;
}

const KEEP = 8; // presses listed
const LIT_MS = 500;

export function testScreen(root: HTMLElement, chosen: { serial: string | null }): TestScreen {
  let current: AppState | null = null;
  let entries: Entry[] = [];
  let lit: { serial: string; control: number } | null = null;
  let litTimer: number | undefined;
  let note: string | null = null;
  let appName: AppName = idName;
  // Newest last, at most DEBUG_KEEP; drawn at most once a frame, so a fast
  // encoder can't stall the window.
  let debugLog: DebugEntry[] = [];
  let debugFrame: number | null = null;

  api
    .listApps()
    .then((apps) => {
      appName = namesFrom(apps);
      rerender();
    })
    .catch(() => {});

  const rerender = () => {
    if (current) render(current);
  };

  function controlName(state: AppState, serial: string, control: number): string {
    const unit = state.paired.find((u) => u.serial === serial);
    return unit?.controls.find((c) => c.id === control)?.label ?? `Control ${control}`;
  }

  function entryRow(state: AppState, e: Entry): HTMLElement {
    const i = e.input;
    const what = i.mapping ? actionText(i.mapping, appName) : "No mapping";
    const where = i.frontmost ? ` in ${appName(i.frontmost)}` : "";
    let outcome: string;
    let tone = "";
    if (!i.mapping) {
      outcome = "nothing to run";
      tone = "muted";
    } else if (i.paused) {
      outcome = "paused: not run";
      tone = "muted";
    } else if (e.result) {
      outcome = e.result.outcome;
      tone = e.result.ok ? (e.result.ran ? "ran" : "dry") : "failed";
    } else {
      outcome = i.dry_run ? "dry run…" : "running…";
    }
    const unit = state.paired.find((u) => u.serial === i.serial);
    const who = state.paired.length > 1 && unit ? `${unit.name}: ` : "";
    return el(
      "li",
      { class: "row feed-row" },
      el(
        "div",
        { class: "grow" },
        el(
          "div",
          {},
          el("strong", {}, `${who}${controlName(state, i.serial, i.control)}`),
          ` · ${triggerText(i.trigger)}${i.count > 1 ? ` ×${i.count}` : ""}${where}`,
        ),
        el("div", { class: "sub" }, what),
      ),
      el("span", { class: `outcome ${tone}` }, outcome),
    );
  }

  function time(ms: number): string {
    const d = new Date(ms);
    const pad = (n: number, w = 2) => String(n).padStart(w, "0");
    return `${pad(d.getHours())}:${pad(d.getMinutes())}:${pad(d.getSeconds())}.${pad(d.getMilliseconds(), 3)}`;
  }

  function debugRow(state: AppState, e: DebugEntry): HTMLElement {
    const unit = e.serial ? state.paired.find((u) => u.serial.toLowerCase() === e.serial?.toLowerCase()) : undefined;
    const who = state.paired.length > 1 && unit ? `${unit.name} · ` : "";
    return el(
      "li",
      { class: `debug-row ${e.kind}` },
      el("span", { class: "debug-time" }, time(e.at_ms)),
      el("span", { class: "debug-kind" }, e.kind.replace("_", " ")),
      el(
        "div",
        { class: "grow" },
        el("div", { class: "debug-text" }, `${who}${e.text}`),
        e.raw ? el("code", { class: "debug-raw" }, e.raw) : null,
      ),
    );
  }

  function helloPanel(unit: PairedUnit): HTMLElement {
    const controls = unit.controls.map((c) => c.label).join(", ");
    return el(
      "div",
      { class: "debug-facts" },
      el("h3", {}, "Last HELLO"),
      unit.controls.length || unit.firmware
        ? el(
            "dl",
            {},
            el("dt", {}, "Firmware"),
            el("dd", {}, `${unit.firmware ?? "unknown"}${unit.outdated ? " (older protocol: update it)" : ""}`),
            el("dt", {}, "Model"),
            el("dd", {}, unit.model ?? "unknown"),
            el("dt", {}, "Controls"),
            el("dd", {}, controls || "none reported"),
          )
        : el("p", { class: "empty" }, unit.connected ? "Waiting for its HELLO." : "Not connected."),
    );
  }

  function permissionsPanel(state: AppState): HTMLElement {
    const a = state.accessibility;
    return el(
      "div",
      { class: "debug-facts" },
      el("h3", {}, "Permissions"),
      el(
        "dl",
        {},
        el("dt", {}, "Accessibility"),
        el("dd", {}, a.granted ? "granted" : a.needed ? "not granted (Send shortcut needs it)" : "not granted (not needed yet)"),
        el("dt", {}, "Automation"),
        el("dd", {}, "asked per app the first time a Run AppleScript targets it"),
        el("dt", {}, "Input Monitoring"),
        el("dd", {}, "not needed"),
      ),
    );
  }

  function debugSection(state: AppState, unit: PairedUnit): HTMLElement {
    return el(
      "section",
      { class: "debug" },
      el(
        "header",
        { class: "bar debug-bar" },
        el("h2", {}, `Debug · ${debugLog.length} of the last ${api.DEBUG_KEEP} events`),
        button("Export logs", () => {
          note = "Exporting logs…";
          rerender();
          api
            .exportLogs()
            .then((path) => {
              note = `Logs exported to ${path}, with names, scripts and URLs taken out.`;
              rerender();
            })
            .catch((err) => {
              note = `Couldn't export logs: ${api.message(err)}`;
              rerender();
            });
        }),
      ),
      el("div", { class: "debug-panels" }, helloPanel(unit), permissionsPanel(state)),
      debugLog.length
        ? el(
            "ul",
            { class: "list debug-log", attrs: { "data-scroll": "debug" } },
            ...[...debugLog].reverse().map((e) => debugRow(state, e)),
          )
        : el("p", { class: "empty" }, "Every report, mapping and result shows here as it happens."),
    );
  }

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
    const dry = el("input", {
      type: "checkbox",
      checked: state.dry_run,
      ariaLabel: "Dry run",
      onClick: (e) => {
        const on = (e.target as HTMLInputElement).checked;
        api.setDryRun(on).catch((err) => {
          note = api.message(err);
          rerender();
        });
      },
    });
    keepingScroll(root, () => root.replaceChildren(
      ...[
        el(
          "header",
          { class: "bar" },
          el("h1", {}, "Test"),
          unitPicker(state, unit, (serial) => {
            chosen.serial = serial;
            rerender();
          }),
        ),
        el(
          "label",
          { class: state.dry_run ? "dry-run on" : "dry-run off" },
          dry,
          el(
            "span",
            {},
            el("strong", {}, "Dry run"),
            state.dry_run
              ? " is on: presses show what would happen, and nothing runs."
              : " is off: presses run for real.",
          ),
        ),
        el(
          "label",
          { class: "debug-toggle" },
          el("input", {
            type: "checkbox",
            checked: state.debug,
            ariaLabel: "Debug",
            onClick: (e) => {
              const on = (e.target as HTMLInputElement).checked;
              if (on) debugLog = [];
              api
                .setDebug(on)
                .then((s) => {
                  current = s;
                  rerender();
                })
                .catch((err) => {
                  note = api.message(err);
                  rerender();
                });
            },
          }),
          el("span", {}, el("strong", {}, "Debug"), " shows raw reports, every mapping and result, the last HELLO and permissions."),
        ),
        state.paused
          ? el("div", { class: "banner" }, el("strong", {}, "Paused. "), "Nothing runs until Pause is off in the menu bar.")
          : null,
        accessibilityBanner(state, (m) => {
          note = m;
          rerender();
        }),
        note ? el("div", { class: "banner" }, note) : null,
        el(
          "div",
          { class: "test-body" },
          drawUnit(unit, {
            appName,
            lit: lit && lit.serial === unit.serial ? lit.control : null,
          }),
          el(
            "section",
            { class: "presses" },
            el("h2", {}, "Presses"),
            entries.length
              ? el(
                  "ul",
                  { class: "list feed", attrs: { "data-scroll": "feed" } },
                  ...entries.map((e) => entryRow(state, e)),
                )
              : el("p", { class: "empty" }, "Press a button on the device."),
          ),
          state.debug ? debugSection(state, unit) : null,
        ),
      ].filter((p): p is HTMLElement => p !== null),
    ));
  }

  return {
    show(state: AppState) {
      current = state;
      render(state);
    },
    input(view: InputView) {
      entries = [{ input: view, result: null }, ...entries].slice(0, KEEP);
      // A press on another unit switches the drawing to it.
      chosen.serial = view.serial;
      lit = { serial: view.serial, control: view.control };
      window.clearTimeout(litTimer);
      litTimer = window.setTimeout(() => {
        lit = null;
        rerender();
      }, LIT_MS);
      rerender();
    },
    result(view: ResultView) {
      // Results come in order: the oldest press still waiting for this one.
      const waiting = entries
        .filter((e) => !e.result && e.input.serial === view.serial && e.input.action === view.action)
        .pop();
      if (waiting) waiting.result = view;
      rerender();
    },
    error(message: string) {
      note = message;
      rerender();
    },
    debug(entry: DebugEntry, visible: boolean) {
      debugLog.push(entry);
      if (debugLog.length > api.DEBUG_KEEP) debugLog = debugLog.slice(-api.DEBUG_KEEP);
      // The screens share one root: drawing Test from here while another
      // screen shows would replace it.
      if (visible && debugFrame === null) {
        debugFrame = window.requestAnimationFrame(() => {
          debugFrame = null;
          if (visible) rerender();
        });
      }
    },
    debugLoad(entries: DebugEntry[]) {
      debugLog = entries.slice(-api.DEBUG_KEEP);
      rerender();
    },
  };
}
