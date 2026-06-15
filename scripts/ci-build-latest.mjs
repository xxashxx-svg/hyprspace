// Merge the macOS (darwin-aarch64) entry into the release's Tauri updater manifest.
//   node scripts/ci-build-latest.mjs <tag> <artifactsDir> <releasesRepo> [existingManifest]
// Starts from <existingManifest> if present (so the windows-x86_64 entry deploy.ps1
// published is preserved), adds darwin-aarch64 from the .app.tar.gz in <artifactsDir>,
// and writes <artifactsDir>/latest.json.
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
const find = (suffix) => files.find((f) => f.endsWith(suffix));

// start from the existing manifest if we have one (keeps windows-x86_64 etc.)
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

const mac = find(".app.tar.gz");
const macSig = find(".app.tar.gz.sig");
if (!mac || !macSig) {
  console.error("no macOS updater artifact (.app.tar.gz[.sig]) found in", dir, "→", files);
  process.exit(1);
}
manifest.platforms["darwin-aarch64"] = {
  signature: readFileSync(join(dir, macSig), "utf8").trim(),
  url: `${base}/${mac}`,
};

writeFileSync(join(dir, "latest.json"), JSON.stringify(manifest, null, 2));
console.log(JSON.stringify(manifest, null, 2));
