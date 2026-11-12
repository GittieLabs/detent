# AppleScript recipes

AppleScript is the Mac's built-in language for telling apps what to do. It has been part of macOS for over 30 years, and most Apple apps and many others understand it. Paired with Detent's **Run AppleScript** action, one press can do things no shortcut can: open a folder, start a playlist, file a reminder, flip Dark Mode, or run a Shortcut.

This page points you to the best ways to learn it, explains how scripts behave inside Detent, and gives you recipes to copy.

## Learn AppleScript

- **Script Editor** is already on your Mac, in Applications › Utilities. Write a script, press Run, and see the result at the bottom. **File › Open Dictionary** shows every command an app understands; it's the most useful page in AppleScript. See the [Script Editor User Guide](https://support.apple.com/guide/script-editor/welcome/mac).
- **[Mac Automation Scripting Guide](https://developer.apple.com/library/archive/documentation/LanguagesUtilities/Conceptual/MacAutomationScriptingGuide/index.html)** is Apple's friendliest introduction, with many task-based examples. Start here.
- **[AppleScript Language Guide](https://developer.apple.com/library/archive/documentation/AppleScript/Conceptual/AppleScriptLangGuide/introduction/ASLR_intro.html)** is the full reference for the language itself.
- **[Technical Note TN2065](https://developer.apple.com/library/archive/technotes/tn2065/_index.html)** covers `do shell script`, which runs any Terminal command from a script.
- **[Run shortcuts from the command line](https://support.apple.com/guide/shortcuts-mac/run-shortcuts-from-the-command-line-apd455c82f02/mac)** shows how Shortcuts and scripts work together. A Shortcut you've already built can be one button press away (see [Run a Shortcut](#run-a-shortcut)).

## Using a script in Detent

1. **Try it in Script Editor first.** If it works there, it works in Detent.
2. **Paste it into a Run AppleScript mapping.** The timeout defaults to 5 seconds and can be set from 0.1 to 60. A script that runs longer is stopped.
3. **Allow the prompt.** The first time a script tells an app to do something, macOS asks whether Detent may control that app. Time spent answering doesn't count against the timeout. If you declined, see [Troubleshooting](troubleshooting.md).
4. **Check the result.** On the Test screen, turn Dry run off and press the button: Test shows what the script returned, or its error. The device's OLED shows `Script done` or `Failed`.

Scripts that press keys through **System Events** (`keystroke`, `key code`) also need Accessibility, the same permission as the Send shortcut action.

Every script on this page compiles on macOS against the apps it names. Change the names in quotes (folders, lists, shortcuts, URLs) to yours.

## Apps and windows

### Open a folder in Finder

Opens your Downloads folder in a new window. Swap in `desktop folder`, `documents folder` or `home folder`, or a path such as `POSIX file "/Users/you/Projects"`.

```applescript
tell application "Finder"
	activate
	make new Finder window to (path to downloads folder)
end tell
```

### Run a command in Terminal

Opens a new Terminal window and runs a command in it.

```applescript
tell application "Terminal"
	activate
	do script "cd ~/repos && git status"
end tell
```

## Web

### Open a page in Safari

Opens a URL in a new tab, making a window first if none is open.

```applescript
tell application "Safari"
	activate
	if (count windows) is 0 then make new document
	tell front window to set current tab to (make new tab with properties {URL:"https://gittielabs.github.io/detent/"})
end tell
```

### Open a page in Chrome

```applescript
tell application "Google Chrome"
	activate
	if (count windows) is 0 then make new window
	tell front window to make new tab with properties {URL:"https://calendar.google.com"}
end tell
```

## Music

### Play or pause, next track

Two one-line scripts, good on two buttons. Spotify understands the same commands: change `"Music"` to `"Spotify"`.

```applescript
tell application "Music" to playpause
```

```applescript
tell application "Music" to next track
```

## Communication

### Start an email

Opens a new message in Mail, addressed and titled, for you to finish.

```applescript
tell application "Mail"
	activate
	set m to make new outgoing message with properties {subject:"Status update", visible:true}
	tell m to make new to recipient with properties {address:"team@example.com"}
end tell
```

### Mute in Zoom from any app

Brings Zoom forward and presses its mute shortcut (⇧⌘A). Needs Accessibility. If you only mute while Zoom is already in front, a **Send shortcut** mapping scoped to Zoom does the same with no script.

```applescript
tell application "zoom.us" to activate
tell application "System Events" to keystroke "a" using {command down, shift down}
```

## Notes, reminders, calendar

### Start a note

Makes a note titled with today's date in the Notes folder. If your notes live in another folder, change `"Notes"`.

```applescript
tell application "Notes"
	activate
	make new note at folder "Notes" with properties {name:"Log " & (short date string of (current date))}
end tell
```

### Add a reminder

Adds "Follow up" to your default list, due in an hour.

```applescript
tell application "Reminders"
	tell default list to make new reminder with properties {name:"Follow up", remind me date:(current date) + 60 * 60}
end tell
```

### Show today in Calendar

```applescript
tell application "Calendar"
	activate
	switch view to day view
	view calendar at (current date)
end tell
```

## System

### Run a Shortcut

Runs a Shortcut from the Shortcuts app by its name. This opens up everything Shortcuts can do: Focus modes, Home devices, smart lights, and other people's shortcuts.

```applescript
tell application "Shortcuts Events" to run the shortcut named "Start Focus"
```

### Toggle Dark Mode

```applescript
tell application "System Events"
	tell appearance preferences to set dark mode to not dark mode
end tell
```

### Show a notification

```applescript
display notification "Build started" with title "Detent" sound name "Glass"
```

### Turn off the display

Sleeps the display at once; touch a key to wake it.

```applescript
do shell script "pmset displaysleepnow"
```

### Copy today's date

Puts today's date (`2026-11-12`) on the clipboard, ready to paste.

```applescript
set the clipboard to (do shell script "date '+%Y-%m-%d'")
```

## Writing your own

- **Find the commands.** In Script Editor, **File › Open Dictionary** lists what an app understands. Apps without a dictionary (Zoom, Slack and many others) can still be driven with keystrokes through System Events.
- **Keep it fast.** A press feels instant when the script finishes in well under a second. For slow work, start it and let it run on its own, for example with `do shell script "long-task > /dev/null 2>&1 &"`.
- **Return something useful.** A script's last value shows on the Test screen, which helps while you build it.
