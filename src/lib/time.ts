/** How long something has been going, counting up: "45s" / "7m 13s" / "1h 4m". */
export function elapsed(since: number, now = Date.now()): string {
  const s = Math.max(0, Math.floor((now - since) / 1000));
  if (s < 60) return `${s}s`;
  if (s < 3600) return `${Math.floor(s / 60)}m ${s % 60}s`;
  return `${Math.floor(s / 3600)}h ${Math.floor((s % 3600) / 60)}m`;
}

// Compact relative time from a ms timestamp: "now" / "2m" / "3h" / "5d".
export function relTime(ts: number): string {
  const s = Math.max(0, Math.floor((Date.now() - ts) / 1000));
  if (s < 60) return "now";
  if (s < 3600) return `${Math.floor(s / 60)}m`;
  if (s < 86400) return `${Math.floor(s / 3600)}h`;
  return `${Math.floor(s / 86400)}d`;
}
