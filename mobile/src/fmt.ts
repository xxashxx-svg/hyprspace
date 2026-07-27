/** Same relative-time shorthand the desktop uses (src/lib/time.ts). */
export function relTime(ts: number): string {
  const s = Math.max(0, Math.floor((Date.now() - ts) / 1000));
  if (s < 60) return "now";
  if (s < 3600) return `${Math.floor(s / 60)}m`;
  if (s < 86400) return `${Math.floor(s / 3600)}h`;
  return `${Math.floor(s / 86400)}d`;
}

/** "in 4m" / "in 2h" — for an automation's next run */
export function untilTime(ts: number): string {
  const s = Math.floor((ts - Date.now()) / 1000);
  if (s <= 0) return "due";
  if (s < 60) return `in ${s}s`;
  if (s < 3600) return `in ${Math.floor(s / 60)}m`;
  if (s < 86400) return `in ${Math.floor(s / 3600)}h`;
  return `in ${Math.floor(s / 86400)}d`;
}

/** last one or two path segments — a full desktop path is far too wide for a phone row */
export function shortPath(p: string): string {
  const parts = p.split(/[\\/]/).filter(Boolean);
  if (parts.length <= 2) return parts.join("/");
  return parts.slice(-2).join("/");
}

export function folderName(p: string): string {
  return p.split(/[\\/]/).filter(Boolean).pop() ?? p;
}
