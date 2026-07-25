// cwd → git branch, for the sidebar's branch tier. A tiny module-level cache with a subscription:
// the rail needs this while rendering (so it can't await), and several panes usually share a cwd.
import { gitBranchInfo } from "../api";

const cache = new Map<string, string>(); // normalized cwd → branch ("" = not a repo / unknown)
const inflight = new Set<string>();
const subs = new Set<() => void>();

const key = (p: string) => p.replace(/[\\/]+$/, "").toLowerCase();

/** the branch for a cwd if we already know it; kicks off a lookup (once) when we don't */
export function branchOf(cwd: string): string | undefined {
  if (!cwd) return undefined;
  const k = key(cwd);
  const hit = cache.get(k);
  if (hit !== undefined) return hit || undefined;
  if (!inflight.has(k)) {
    inflight.add(k);
    void gitBranchInfo(cwd)
      .then((b) => cache.set(k, b.is_repo ? b.branch : ""))
      .catch(() => cache.set(k, ""))
      .finally(() => {
        inflight.delete(k);
        for (const fn of subs) fn();
      });
  }
  return undefined;
}

/** re-render when a lookup lands */
export function onBranchResolved(fn: () => void): () => void {
  subs.add(fn);
  return () => subs.delete(fn);
}

/** branches move (checkout, new worktree) — drop what we know so it's re-read */
export function invalidateBranches() {
  cache.clear();
}
