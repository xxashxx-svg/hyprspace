// Assemble the cross-platform Tauri updater manifest from the built artifacts.
//   node scripts/ci-build-latest.mjs <tag> <artifactsDir> <releasesRepo>
// Scans <artifactsDir> for the Windows NSIS installer and the macOS .app.tar.gz
// (each with its .sig), and writes <artifactsDir>/latest.json pointing at the
// release assets on the public releases repo.
import { readFileSync, readdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const [tag, dir, repo] = process.argv.slice(2);
if (!tag || !dir || !repo) {
  console.error("usage: ci-build-latest.mjs <tag> <artifactsDir> <releasesRepo>");
  process.exit(1);
}

const version = tag.replace(/^v/, "");
const files = readdirSync(dir);
const base = `https://github.com/${repo}/releases/download/${tag}`;
const find = (suffix) => files.find((f) => f.endsWith(suffix));
const sig = (name) => readFileSync(join(dir, name), "utf8").trim();

const platforms = {};

const win = find("-setup.exe");
const winSig = find("-setup.exe.sig");
if (win && winSig) {
  platforms["windows-x86_64"] = { signature: sig(winSig), url: `${base}/${win}` };
}

// the macOS updater artifact is the .app.tar.gz (the .dmg is only for first install)
const mac = find(".app.tar.gz");
const macSig = find(".app.tar.gz.sig");
if (mac && macSig) {
  platforms["darwin-aarch64"] = { signature: sig(macSig), url: `${base}/${mac}` };
}

if (Object.keys(platforms).length === 0) {
  console.error("no signed artifacts found in", dir, "→", files);
  process.exit(1);
}

const manifest = {
  version,
  notes: `HyprSpace ${version}`,
  pub_date: new Date().toISOString(),
  platforms,
};
writeFileSync(join(dir, "latest.json"), JSON.stringify(manifest, null, 2));
console.log(JSON.stringify(manifest, null, 2));
