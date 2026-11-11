// Typed calls into Detent's Rust side. Mirrors `detent_lib::app_state` and
// `src-tauri/src/commands.rs`; the logic lives there (D-065).

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

/** One control from the unit's HELLO (`app_state::ControlView`). */
export interface ControlView {
  id: number;
  type: "button" | "encoder";
  label: string; // "Button 1", "Encoder", "Encoder push"; with 2+ encoders "Encoder 5", "Encoder 5 push"
  push_of: number | null; // for an encoder's push switch, the encoder's ID (from HELLO, D-094)
  handled_by_device: boolean;
  triggers: Trigger[]; // what it can fire, in the editor's order
}

/** Everywhere, or only while this bundle ID is frontmost. */
export type Scope = "everywhere" | { app: string };

/** A config mapping (`store::Mapping`). */
export interface Mapping {
  device: string; // serial
  control: number;
  trigger: Trigger;
  scope: Scope;
  action: Action;
}

export interface PairedUnit {
  name: string;
  vid: number;
  pid: number;
  serial: string;
  connected: boolean;
  model: string | null;
  firmware: string | null;
  outdated: boolean; // its firmware speaks an older protocol; update it (D-094)
  buttons: number;
  encoders: number;
  controls: ControlView[]; // empty until its HELLO this session
  mappings: Mapping[];
  template: Mapping[]; // what its starter template would still add (D-077)
  menu: MenuItem[]; // its OLED menu items (D-086)
}

/** One app owned item on a device's OLED menu (`store::MenuItem`). */
export interface MenuItem {
  label: string; // 1 to 16 printable ASCII
  action: Action;
}

/** The device's own menu items, shown before the app's (firmware lib/menu). */
export const DEVICE_MENU_ITEMS = ["Volume", "Mapped", "Setup", "Device info"];
export const MAX_MENU_ITEMS = 8;

export interface Candidate {
  vid: number;
  pid: number;
  serial: string;
  model: string | null;
  firmware: string | null;
  outdated: boolean; // see PairedUnit.outdated
  buttons: number;
  encoders: number;
  picked: boolean;
  suggested_name: string; // the wizard's prefilled name
}

export interface UnknownUnit {
  vid: number;
  pid: number;
  serial: string | null;
}

export interface UnitRef {
  vid: string; // "0x303A": the config's own form
  pid: string;
  serial: string;
}

export interface AppState {
  paired: PairedUnit[];
  candidates: Candidate[];
  unknown: UnknownUnit[];
  ignored: UnitRef[];
  identifying: boolean;
  paused: boolean;
  dry_run: boolean; // only while the Test screen shows (D-079)
  accessibility: { granted: boolean; needed: boolean; banner: boolean };
  problem: string | null;
  /** The config on disk didn't load; Start fresh is offered (M9). */
  config_broken: boolean;
  /** Debug is on in the Test screen (M9). */
  debug: boolean;
  /** Errors the engine recovered from since launch (M9). */
  recovered: number;
}

/** The event the Rust side sends with the whole state after any change. */
const STATE_EVENT = "state";
/** The event carrying each resolved input, for the Test screen. */
const INPUT_EVENT = "input";
/** The event carrying each action's result, for the Test screen. */
const RESULT_EVENT = "result";

/** An action's result (`app_state::ResultView`). */
export interface ResultView {
  serial: string;
  action: string;
  mapping: Action;
  ran: boolean; // something actually happened
  ok: boolean;
  outcome: string; // "was 13%, now 38%", "not run (dry run)", "failed: …"
  took_ms: number;
}

export type Trigger = "press" | "long_press" | "clockwise" | "counterclockwise";

/** A config action, as `store::Action` serializes it. */
export type Action =
  | { type: "open_app"; bundle_id: string }
  | { type: "volume_up"; step: number }
  | { type: "volume_down"; step: number }
  | { type: "volume_set"; level: number }
  | { type: "mute_toggle" }
  | { type: "send_shortcut"; keys: string }
  | {
      type: "open_dashboard";
      url: string;
      display: string | null;
      mode: "fullscreen" | "window";
      close_without_device: boolean; // hide it when no paired device is connected (D-090)
    }
  | { type: "applescript"; source: string; timeout_ms: number };

/** One resolved input (`app_state::InputView`). */
export interface InputView {
  serial: string;
  control: number;
  trigger: Trigger;
  frontmost: string | null;
  action: string | null; // the log's short form
  mapping: Action | null;
  count: number;
  paused: boolean;
  dry_run: boolean;
}

export interface AppEntry {
  bundle_id: string;
  name: string;
  running: boolean;
}

export const getState = () => invoke<AppState>("get_state");
export const startPairing = () => invoke<AppState>("start_pairing");
export const endPairing = () => invoke<AppState>("end_pairing");
export const pair = (vid: number, pid: number, serial: string, name: string) =>
  invoke<AppState>("pair", { vid, pid, serial, name });
export const rename = (serial: string, name: string) =>
  invoke<AppState>("rename", { serial, name });
export const mappingsOf = (serial: string) => invoke<number>("mappings_of", { serial });
export const unpair = (serial: string) => invoke<AppState>("unpair", { serial });
export const ignore = (vid: number, pid: number, serial: string) =>
  invoke<AppState>("ignore", { vid, pid, serial });
export const unignore = (serial: string) => invoke<AppState>("unignore", { serial });
export const setPaused = (on: boolean) => invoke<AppState>("set_paused", { on });
export const setDryRun = (on: boolean) => invoke<void>("set_dry_run", { on });
export const fixAccessibility = () => invoke<AppState>("fix_accessibility");
export const setMapping = (mapping: Mapping) => invoke<AppState>("set_mapping", { mapping });
export const setMenuItem = (serial: string, index: number | null, item: MenuItem) =>
  invoke<AppState>("set_menu_item", { serial, index, item });
export const removeMenuItem = (serial: string, index: number) =>
  invoke<AppState>("remove_menu_item", { serial, index });
export const moveMenuItem = (serial: string, from: number, to: number) =>
  invoke<AppState>("move_menu_item", { serial, from, to });
export const applyTemplate = (serial: string) =>
  invoke<AppState>("apply_template", { serial });
export const removeMapping = (serial: string, control: number, trigger: Trigger, scope: Scope) =>
  invoke<AppState>("remove_mapping", { serial, control, trigger, scope });
export const listApps = () => invoke<AppEntry[]>("list_apps");
/** Connected displays' names, main first (Open dashboard's picker). */
export const listDisplays = () => invoke<string[]>("list_displays");
/** Moves a config that didn't load aside and starts empty (M9). */
export const startFresh = () => invoke<AppState>("start_fresh");
/** Open dashboard's default page (D-088; `store::DEFAULT_DASHBOARD_URL`). */
export const DEFAULT_DASHBOARD_URL = "https://gittielabs.github.io/detent/";

/** One Debug log line (`app_state::DebugEntry`, M9). */
export interface DebugEntry {
  at_ms: number;
  kind: "report" | "device" | "resolved" | "ran" | "menu" | "reloaded" | "dry_run" | "recovered" | "wake";
  serial: string | null;
  text: string;
  raw: string | null; // hex, trailing zeros collapsed
}

/** Entries the Debug log keeps (`app_state::DEBUG_KEEP`). */
export const DEBUG_KEEP = 500;
const DEBUG_EVENT = "debug";

export const setDebug = (on: boolean) => invoke<AppState>("set_debug", { on });
export const getDebugLog = () => invoke<DebugEntry[]>("get_debug_log");
/** Zips the logs, redacted config and app facts to the Desktop and shows it
 * in Finder (D-100); answers the zip's path. */
export const exportLogs = () => invoke<string>("export_logs");

export function onDebug(handler: (entry: DebugEntry) => void): Promise<UnlistenFn> {
  return listen<DebugEntry>(DEBUG_EVENT, (event) => handler(event.payload));
}

export function onResult(handler: (result: ResultView) => void): Promise<UnlistenFn> {
  return listen<ResultView>(RESULT_EVENT, (event) => handler(event.payload));
}

export function onInput(handler: (input: InputView) => void): Promise<UnlistenFn> {
  return listen<InputView>(INPUT_EVENT, (event) => handler(event.payload));
}

export function onState(handler: (state: AppState) => void): Promise<UnlistenFn> {
  return listen<AppState>(STATE_EVENT, (event) => handler(event.payload));
}

/** A command's error as text; the Rust side sends plain strings. */
export function message(error: unknown): string {
  return typeof error === "string" ? error : String(error);
}
