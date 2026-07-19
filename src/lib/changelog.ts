// The changelog is bundled into the app (Vite ?raw), so after an update we can show exactly what
// changed in the running version — no network needed. deploy.ps1 finalizes the version's section
// before the build, so the shipped bundle always carries the right notes.
import raw from "../../docs/CHANGELOG.md?raw";

// the bullet lines under "## <version>" (e.g. "## 0.5.2 — 2026-06-26"), until the next "## " heading
export function changelogFor(version: string): string[] {
  const v = version.replace(/^v/, "").trim();
  const esc = v.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const head = new RegExp(`^##\\s+v?${esc}(?:\\s|—|-|$)`);
  const lines = raw.split(/\r?\n/);
  let i = lines.findIndex((l) => head.test(l.trim()));
  if (i < 0) return [];
  const out: string[] = [];
  for (i += 1; i < lines.length; i++) {
    if (/^##\s/.test(lines[i])) break;
    const item = lines[i].trim().replace(/^[-*]\s+/, "");
    if (item) out.push(item);
  }
  return out;
}
