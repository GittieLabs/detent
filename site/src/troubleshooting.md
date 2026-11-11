# Troubleshooting

**The menu bar dot is white and the device is plugged in.** Is it paired? An unpaired device shows **New device found** instead. Check the cable carries data, and that the device isn't on the Ignored list in Devices.

**A Send shortcut does nothing.** Detent needs Accessibility: the settings window shows a banner with **Fix** if it's missing. A password field in front turns on Secure Input, which blocks every synthetic key; Detent sends nothing and Test says so.

**An AppleScript fails with "not allowed".** You declined the Automation prompt for that app. Turn it back on in System Settings, Privacy & Security, Automation, under Detent.

**The volume knob doesn't reach the app.** In Volume mode the device handles it itself. Switch the device to Mapped in its menu.

**Devices says the firmware is too old.** Flash the current firmware ([Boards and firmware](boards.md)).

**The menu bar line says "recovered from an error".** Something inside Detent failed and it carried on. It's worth a bug report: open Test, tick Debug, **Export logs**, and attach the zip to a [new issue](https://github.com/gittielabs/detent/issues).

**The config won't load.** Devices shows why, naming the field. Fix the file and relaunch, or choose **Start fresh** to set it aside and begin again.
