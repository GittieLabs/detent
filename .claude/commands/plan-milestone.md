---
description: Break the next milestone into concrete tasks in docs/roadmap.md
---

Plan the next milestone for Detent.

1. Read `CLAUDE.md`, `docs/product.md`, `docs/architecture.md`, `docs/decisions.md`, and `docs/roadmap.md`. Read `docs/protocol.md` if the milestone touches firmware, protocol, transport, or devices.
2. Confirm the current milestone's exit criteria are all ticked, including manual ones recorded in `docs/acceptance.md`. If any aren't, stop and list what's missing. Don't plan ahead of an unfinished milestone.
3. Move the "Current milestone" marker to the next milestone.
4. Break it into tasks small enough to finish and test in one session each. Each task is one checkbox line that names the files or modules it touches and how it will be tested (unit, integration, HIL, or manual step in `acceptance.md`).
5. Order tasks so each builds on the last and the riskiest unknown comes first.
6. Add any manual steps the exit criteria need to `docs/acceptance.md` under a new heading for this milestone.
7. If planning surfaced a choice between two reasonable options, add a row to `docs/decisions.md`. If it surfaced a question only Keith can answer, add it to Open questions in the roadmap.
8. Show Keith the task list and wait for approval before starting any task.
