// Starter recipes for the Loops page. Each one prefills a loop with a real-world automation
// pattern (the kind people actually run agents for) so a new user starts from something useful
// instead of a blank box. The folder + API key are still the user's to fill in.
import { newLoop, type LoopDef } from "../stores/loops";

export interface LoopTemplate {
  id: string;
  title: string;
  blurb: string;
  icon: string; // lucide icon name, mapped to a component in LoopsManager
  build: (folder: string) => LoopDef;
}

// apply overrides on top of a sane default loop
function tpl(folder: string, name: string, patch: Partial<LoopDef>): LoopDef {
  const base = newLoop(folder);
  return { ...base, ...patch, name, stop: { ...base.stop, ...(patch.stop ?? {}) } };
}

export const LOOP_TEMPLATES: LoopTemplate[] = [
  {
    id: "fix-tests",
    title: "Fix until tests pass",
    blurb: "Runs your suite, hunts the root cause of any failure, and keeps fixing until it's green.",
    icon: "FlaskConical",
    build: (f) =>
      tpl(f, "Fix until tests pass", {
        prompt:
          "Run the test suite. If anything fails, find the real root cause and fix the code (never weaken or delete a test to make it pass). Re-run and keep going until everything passes.",
        mode: "until-done",
        session: "continue",
        permissionMode: "acceptEdits",
        worktree: true,
        stop: { maxIterations: 8, noProgress: true, untilCheck: "npm test" },
      }),
  },
  {
    id: "build-green",
    title: "Build until it compiles",
    blurb: "Keeps fixing type/compile errors until the project builds cleanly.",
    icon: "Hammer",
    build: (f) =>
      tpl(f, "Build until it compiles", {
        prompt:
          "Build the project. If the build fails, fix the errors it reports — one cause at a time. Repeat until it builds with no errors.",
        mode: "until-done",
        session: "continue",
        permissionMode: "acceptEdits",
        worktree: true,
        stop: { maxIterations: 6, noProgress: true, untilCheck: "npm run build" },
      }),
  },
  {
    id: "task-runner",
    title: "Work through a task list",
    blurb: "Implements the next unchecked item in TASKS.md, one per pass, until they're all done.",
    icon: "ListChecks",
    build: (f) =>
      tpl(f, "Work through TASKS.md", {
        prompt:
          "Open TASKS.md. Implement the next unchecked item, then check it off. Keep the change small and focused. When every item is checked, print LOOP_DONE and stop.",
        mode: "until-done",
        session: "continue",
        permissionMode: "acceptEdits",
        worktree: true,
        stop: { maxIterations: 20, noProgress: true, sentinel: "LOOP_DONE" },
      }),
  },
  {
    id: "lint-fix",
    title: "Lint & format autofix",
    blurb: "Cleans up every lint error and warning it safely can, until the linter passes.",
    icon: "Sparkles",
    build: (f) =>
      tpl(f, "Lint & format autofix", {
        prompt:
          "Run the linter and formatter. Fix every error and warning you can do safely without changing behavior. Repeat until the linter reports no problems.",
        mode: "until-done",
        session: "fresh",
        permissionMode: "acceptEdits",
        worktree: true,
        stop: { maxIterations: 6, noProgress: true, untilCheck: "npm run lint" },
      }),
  },
  {
    id: "dep-updates",
    title: "Nightly dependency updates",
    blurb: "Each night: bump outdated deps, run tests, revert anything that breaks, summarize.",
    icon: "RefreshCw",
    build: (f) =>
      tpl(f, "Nightly dependency updates", {
        prompt:
          "Update outdated dependencies to safe, non-breaking versions. Run the tests. If an update breaks something, revert just that one. Finish with a short summary of what changed and why.",
        mode: "cron",
        schedule: { dailyAt: "03:00" },
        session: "fresh",
        permissionMode: "acceptEdits",
        worktree: true,
        stop: { maxIterations: 30, noProgress: false },
      }),
  },
  {
    id: "docs-sync",
    title: "Keep docs in sync",
    blurb: "Each night, updates the README and docs to match recent code changes.",
    icon: "BookText",
    build: (f) =>
      tpl(f, "Keep docs in sync", {
        prompt:
          "Look at what changed in the code recently and update the README and any docs so they match — commands, options, examples, file paths. Only touch documentation, not code.",
        mode: "cron",
        schedule: { dailyAt: "04:00" },
        session: "fresh",
        permissionMode: "acceptEdits",
        worktree: true,
        stop: { maxIterations: 30, noProgress: false },
      }),
  },
  {
    id: "code-review",
    title: "Continuous code review",
    blurb: "Re-reviews your uncommitted changes on a timer and lists concrete findings. Read-only.",
    icon: "ScanEye",
    build: (f) =>
      tpl(f, "Continuous code review", {
        prompt:
          "Review the current uncommitted changes for bugs, security issues, and style problems. List concrete findings as file:line with a one-line fix suggestion each. Do not edit anything.",
        mode: "interval",
        intervalSec: 900,
        session: "fresh",
        permissionMode: "plan",
        worktree: false,
        stop: { maxIterations: 20, noProgress: false },
      }),
  },
];
