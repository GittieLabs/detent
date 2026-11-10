// Typed calls into Detent's Rust side. Mirrors `detent_lib::app_state` and
// `src-tauri/src/commands.rs`; the logic lives there (D-065).

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export interface PairedUnit {
  name: string;
  vid: number;
  pid: number;
  serial: string;
  connected: boolean;
  model: string | null;
  firmware: string | null;
  buttons: number;
  encoders: number;
}

export interface Candidate {
  vid: number;
  pid: number;
  serial: string;
  model: string | null;
  firmware: string | null;
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
  problem: string | null;
}

/** The event the Rust side sends with the whole state after any change. */
const STATE_EVENT = "state";
/** The event carrying each resolved input, for the Test screen. */
const INPUT_EVENT = "input";

export type Trigger = "press" | "long_press" | "clockwise" | "counterclockwise";

/** A config action, as `store::Action` serializes it. */
export type Action =
  | { type: "open_app"; bundle_id: string }
  | { type: "volume_up"; step: number }
  | { type: "volume_down"; step: number }
  | { type: "volume_set"; level: number }
  | { type: "mute_toggle" }
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
export const listApps = () => invoke<AppEntry[]>("list_apps");

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
