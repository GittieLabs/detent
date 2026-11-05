---
description: Pick up the next open task in the current milestone and build it
argument-hint: "[optional task text to pick a specific task]"
---

Start a Detent task. $ARGUMENTS

1. Read `CLAUDE.md`, `docs/product.md`, `docs/architecture.md`, and the current milestone in `docs/roadmap.md`. Read `docs/protocol.md` if the task touches firmware, protocol, transport, or devices.
2. Pick the first unticked task in the current milestone (or the one named above, if it's in the current milestone). Never pick a task from another milestone.
3. Before writing code, restate in a few lines: what the task delivers, which files change, and exactly which tests will prove it. If something is ambiguous or touches an open question, ask Keith now.
4. Write the tests first where practical, then the code.
5. Run the definition of done from `CLAUDE.md`: tests, fmt, clippy, tsc if UI changed, `pio run` if firmware changed. Fix until clean.
6. If the task needs a physical button press, a permission dialog, or a signed build check, write that step into `docs/acceptance.md` and tell Keith what to run. Don't mark it passed.
7. Tick the task in `docs/roadmap.md`. Add a `docs/decisions.md` row for any choice you made between reasonable options.
8. Commit with a message that names the task.
