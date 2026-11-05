# Product

Changes rarely. If something here needs to change, log it in `decisions.md` first.

## What Detent is

Detent is a macOS menu bar app that turns a paired ESP32 USB device into a programmable control surface. Each button press or encoder turn maps to an action: launch an app, show a web dashboard full screen, send a shortcut to a specific app, or change the volume. The app listens only to devices the user has paired, so the built in keyboard can never trigger an action.

## Problem

Stream Decks and macro pads exist, but they lock you into their hardware and their app. Building your own physical controller today means faking a keyboard, which leaks keystrokes into whatever app has focus and can't tell your device apart from your real keyboard.

## Users

1. **Keith**, driving a Mac mini dashboard (Homepage on a secondary display) and Plasticity shortcuts.
2. **Makers** who build their own ESP32 controller and want a polished host app instead of writing one. The open protocol is what makes this possible.

## Goals for v1

1. Pair one ESP32 device through a short wizard and remember it across restarts.
2. Map each control on that device to an action from a small, fixed list.
3. Let a mapping apply everywhere or only when a chosen app is frontmost.
4. Show a web dashboard full screen on a chosen display.
5. Run in the background from login, recovering from unplug, sleep and wake.
6. Give the user a Test mode to confirm mappings and a Debug mode to see raw input.

## Non goals for v1

Windows support, Bluetooth, more than one connected device at a time, cloud sync, a macro recorder, a plugin system, Mac App Store distribution. The data model and transport layer are designed so multiple devices and Bluetooth can be added later without a rewrite.

## Success measures

| Measure | Target |
| --- | --- |
| Pair a new device from first launch | Under 2 minutes, no docs |
| Button press to action dispatched | Under 100 ms |
| Actions triggered by the built in keyboard | Zero |
| Recovery after unplug, replug, sleep, wake | Automatic, no app restart |
| New mapping created by a first time user | Without opening a help page |

## The one concept: a mapping

A mapping is a **control**, an **action**, and a **scope**. Everything in the UI is a detail of one of those three.

**Scope has two choices.** Everywhere, or Only when a chosen app is frontmost. If both exist for the same control, the app scoped one wins. No profiles, layers, or modes in v1. Example: button 3 is Extrude in Plasticity, Mute everywhere else.

## Actions in v1 (fixed list)

| Action | Needs permission | Notes |
| --- | --- | --- |
| Open app | None | By bundle ID via NSWorkspace |
| Open dashboard | None | A URL shown full screen on a chosen display, inside the app. URL is per action, no default. |
| Send shortcut | Accessibility | A key combo or short sequence to the frontmost app |
| Volume up, down, set, mute | None | Through CoreAudio, so any control can be remapped to it |
| Run AppleScript | Automation, per target app | The escape hatch for anything not on this list |
| Run shell command | None | Second escape hatch, off by default behind a toggle |

## Pass through controls

The encoder in Volume mode sends standard media keys (USB Consumer Control) straight from the device. It works with the app closed and shows the native macOS volume HUD. The app shows pass through controls as "handled by device" and doesn't remap them.

## Expectations about shortcuts

A sent shortcut always goes to the frontmost app, because that's how macOS delivers keystrokes. Detent can't press keys inside a background app. Password fields and some games block synthetic keystrokes (Secure Input). A few apps ignore synthetic events; those get an AppleScript action instead. The UI says this in one line where you pick Send shortcut.

## Complexity guardrails

- Three screens maximum: Devices, Mappings, Test. Debug lives inside Test as a toggle.
- No nested menus, no settings page longer than one screen.
- A new action type is added only when the AppleScript escape hatch has been used for the same thing at least twice.
- Every feature request is checked against the non goals before it gets a roadmap slot.
