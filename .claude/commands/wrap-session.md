---
description: Close out a session: tick tasks, log decisions, note what's next
---

Wrap up this Detent session.

1. Review what changed this session (`git log` since the last session log entry and `git status`).
2. In `docs/roadmap.md`, make sure every finished task is ticked and nothing unfinished is ticked. If a task turned out bigger than planned, split the remainder into a new unticked task.
3. Add any decisions made this session to `docs/decisions.md` (append only).
4. If a module's job or boundary changed, update `docs/architecture.md`. If the wire format changed, confirm the version bump, crate, and header all landed together.
5. Add one or two lines to the top of the Session log in `docs/roadmap.md`: what landed, what's next, and any manual steps waiting on Keith in `docs/acceptance.md`.
6. Commit the doc updates.
7. Tell Keith in three lines or fewer: what landed, what's next, what needs his hands.
