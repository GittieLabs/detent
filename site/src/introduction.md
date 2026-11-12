# What Detent is

<div class="detent-hero">
<svg viewBox="0 0 32 32" aria-hidden="true"><rect width="32" height="32" rx="8" fill="#0f172a"/><circle cx="16" cy="17" r="8.5" fill="none" stroke="#f8fafc" stroke-width="2.5"/><path d="M16 5.5v6" stroke="#fbbf24" stroke-width="2.5" stroke-linecap="round"/></svg>
<p class="detent-lede">Detent turns a USB controller you build into a control surface for your Mac. Press a button or turn a knob to open an app, send a shortcut, run an AppleScript, show a dashboard or change the volume.</p>
<div class="detent-actions">
<a class="detent-button primary" href="https://github.com/gittielabs/detent/releases/latest">Download for macOS</a>
<a class="detent-button secondary" href="install.html">Install guide</a>
<a class="detent-button secondary" href="build-your-own.html">Build your own device</a>
</div>
</div>

Detent is a macOS menu bar app. Each button press or encoder turn on your device runs an action on your Mac: open an app, show a web dashboard full screen, send a shortcut to an app, run an AppleScript, or change the volume.

Detent only listens to devices you've paired. The device doesn't pretend to be a keyboard, so it can't type into whatever has focus, and your real keyboard can never trigger an action.

## How it fits together

- **The device** is a small board (an ESP32-S3 or a Raspberry Pi Pico) running Detent's firmware, with buttons, an optional rotary encoder and an optional OLED. It reports presses and turns over its own USB channel. See [Boards and firmware](boards.md).
- **The app** lives in the menu bar. It pairs devices, keeps your mappings, and runs the actions.
- **A mapping** is one control, one action and one scope: *button 3* runs *Extrude* only while *Plasticity* is in front, and *Mute* everywhere else.

## What it isn't

Detent v0.1 runs on macOS 13 or later only, talks to one connected device at a time over USB (no Bluetooth), and has no profiles, layers or macro recorder. The app has three screens, Devices, Mappings and Test, and the action list is short on purpose: AppleScript covers the rest.

The project is open source under the Apache 2.0 license: [github.com/gittielabs/detent](https://github.com/gittielabs/detent). The wire protocol is documented there too, so you can write your own firmware.
