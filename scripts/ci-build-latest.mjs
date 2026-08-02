// Merge this runner's platform entry into the release's Tauri updater manifest.
//   node scripts/ci-build-latest.mjs <tag> <artifactsDir> <releasesRepo> [existingManifest]
// Starts from <existingManifest> if present (so entries other jobs already published —
// windows-x86_64 from deploy.ps1, darwin-aarch64 from the macos job — are preserved), adds
// whatever updater artifacts it finds in <artifactsDir>, and writes <artifactsDir>/latest.json.
//
// Platform-agnostic on purpose: the mac and linux jobs run the same command, and adding a target
// (say darwin-x86_64) is a row in PLATFORMS rather than a new script.
import { readFileSync, readdirSync, writeFileSync, existsSync } from "node:fs";
import { join } from "node:path";

const [tag, dir, repo, existingPath] = process.argv.slice(2);
if (!tag || !dir || !repo) {
  console.error("usage: ci-build-latest.mjs <tag> <artifactsDir> <releasesRepo> [existingManifest]");
  process.exit(1);
}

const version = tag.replace(/^v/, "");
const files = readdirSync(dir);
const base = `https://github.com/${repo}/releases/download/${tag}`;

// Tauri's updater key → the bundle suffixes that carry it, best first. Linux is listed twice
// because the AppImage updater artifact is a .AppImage.tar.gz on some Tauri versions and the bare
// .AppImage on others; whichever this build produced is the one we use.
const PLATFORMS = [
  { key: "darwin-aarch64", suffixes: [".app.tar.gz"] },
  { key: "linux-x86_64", suffixes: [".AppImage.tar.gz", ".AppImage"] },
];

// start from the existing manifest if we have one, so we only ever ADD our own platform
let manifest = {
  version,
  notes: `HyprSpace ${version}`,
  pub_date: new Date().toISOString(),
  platforms: {},
};
if (existingPath && existsSync(existingPath)) {
  try {
    const prev = JSON.parse(readFileSync(existingPath, "utf8"));
    manifest = {
      version,
      notes: prev.notes || manifest.notes,
      pub_date: manifest.pub_date,
      platforms: { ...(prev.platforms || {}) },
    };
  } catch {
    console.warn("existing manifest was unreadable; starting fresh");
  }
}

const added = [];
for (const { key, suffixes } of PLATFORMS) {
  for (const suffix of suffixes) {
    // Pick the first bundle that actually HAS a signature, not merely the first that matches the
    // suffix: the jobs also drop an unsigned stable-named copy alongside (HyprSpace-linux-x86_64
    // .AppImage next to HyprSpace_1.2.3_amd64.AppImage) for the website's download link, and that
    // one sorts first. An unsigned bundle can't be served to the updater — the client would reject
    // it — so a candidate without a .sig is skipped rather than published.
    const bundle = files
      .filter((f) => f.endsWith(suffix))
      .find((f) => files.includes(`${f}.sig`));
    if (!bundle) continue;
    manifest.platforms[key] = {
      signature: readFileSync(join(dir, `${bundle}.sig`), "utf8").trim(),
      url: `${base}/${bundle}`,
    };
    added.push(`${key} ← ${bundle}`);
    break;
  }
}

if (!added.length) {
  console.error("no signed updater artifact found in", dir, "→", files);
  console.error("expected one of:", PLATFORMS.flatMap((p) => p.suffixes).join(", "), "plus its .sig");
  process.exit(1);
}

writeFileSync(join(dir, "latest.json"), JSON.stringify(manifest, null, 2));
console.log("added:", added.join(", "));
console.log(JSON.stringify(manifest, null, 2));
