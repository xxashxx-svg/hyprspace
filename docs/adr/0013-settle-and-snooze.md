# ADR 0013: Settling and snoozing threads instead of archiving them

- Status: Accepted
- Date: 2026-10-03

## Context

Ash runs many threads at once and clears finished ones out of the way all the time. Archiving a
thread only hid it. Its terminal and agent kept running, and a Claude process holds a few hundred
MB, so a day of work left dozens of idle agents behind. T3 Code handles the same problem with
settle, auto-settle and snooze, and Ash asked for that.

## Decision

- **Settle replaces archiving a thread.** A settled thread leaves its space for one Settled
  shelf at the bottom of the sidebar, newest first, each row naming its space. A space whose
  threads all settled shows just its header. Saved `archived` threads load as settled.
- **Settling frees the session.** An idle settled thread's terminal and agent close, and the
  conversation resumes when the thread is opened again (ADR 0006). A busy one finishes its turn
  first, and one waiting on an approval keeps its question. A plain shell has nothing to resume
  and may be running a dev server, so it keeps its terminal. This is what keeps a large sidebar
  cheap.
- **Untouched threads settle by themselves** after three days by default (Settings, General:
  never, 1 day, 3 days, 1 week). "Touched" means a turn started or ended, or the thread came
  back from Settled or Snoozed. Only opening it doesn't count, so the age in its row stays the
  time of its last activity. A thread on screen or at work never settles by itself.
- **Snooze** hides a thread until a time (1 hour, 3 hours, this evening, tomorrow morning, next
  Monday morning) or until its agent finishes. A snoozed thread keeps its session, wakes into the
  active list marked new, and waits on a shelf near the bottom of the sidebar meanwhile.
- **Opening** a settled or snoozed thread brings it back. Settling and snoozing show a toast with
  Undo for five seconds.
- Spaces are still archived, as before.
