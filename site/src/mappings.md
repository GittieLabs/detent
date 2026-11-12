# Mappings

A mapping is a **control**, a **trigger**, an **action** and a **scope**.

The Mappings screen lists the device's controls, grouped as Buttons and Encoders, with a cell per trigger. Each cell holds a chip per scope: **Everywhere** and one per app. Hover a chip to see what it runs; click it, or the row, to edit it in the side panel.

## Triggers

| Control | Triggers |
| --- | --- |
| Button | **Press** (fires as it goes down) and **Long press** (held about 0.6 s) |
| Encoder (Mapped mode) | **Clockwise** and **Counterclockwise**, once per detent |
| Encoder push switch (Mapped mode) | Press and Long press, like a button |

In Volume mode the knob and its push are **handled by the device**: they show greyed and can't be mapped.

## Scope

**Everywhere**, or **only while an app is in front**. When both exist for the same control and trigger, the app's wins. For example: button 3 is Extrude in Plasticity and Mute everywhere else.

## Actions

| Action | What it does |
| --- | --- |
| Open app | Opens the app, or brings it to the front. |
| Open dashboard | Shows a web page on a chosen display, full screen or in a window, without taking focus. Press again to hide it. By default it also hides when no paired device is connected. |
| Send shortcut | Sends a key combo, or up to four in a row (`cmd+k cmd+s`), to the app in front. Needs Accessibility. |
| Volume up, down, set, mute | Changes the Mac's output volume and shows a small level bar. |
| Run AppleScript | Runs your script, with a timeout. The escape hatch for everything else: see [AppleScript recipes](applescript.md). |

**Shortcuts go to the app in front**, because that's how macOS delivers keys: Detent can't press keys inside a background app. Password fields turn on Secure Input, which blocks synthetic keys; Detent then sends nothing and says so. A few apps ignore synthetic keys; use Run AppleScript for those.

The first time a script tells an app to do something, macOS asks whether Detent may control it. The time you take to answer doesn't count against the script's timeout.

The result of each action shows briefly on the device's OLED, such as `Vol 38%` or `Opened Mail`.
