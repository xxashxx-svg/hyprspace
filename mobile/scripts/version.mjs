// Bump the mobile app's version. The desktop has deploy.ps1 for this; the phone app ships through a
// different channel, so it gets its own.
//
//   node scripts/version.mjs patch|minor|major     (or: npm run version -- minor)
//
// Two numbers move together:
//   version      "0.2.0"  — what people see, adapted SemVer exactly like the desktop (docs/VERSIONING.md)
//   versionCode  200      — Android's own counter, derived from the version
//
// versionCode is the one that bites: Android REFUSES to install an update whose code isn't higher
// than what's on the device, and Play rejects a reused one outright. Deriving it from the version
// (rather than a hand-kept counter) means the two can never drift apart.

import { readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const app = join(dirname(fileURLToPath(import.meta.url)), "..");
const appJsonPath = join(app, "app.json");
const pkgPath = join(app, "package.json");

const kind = process.argv[2];
if (!["patch", "minor", "major"].includes(kind)) {
  console.error("usage: node scripts/version.mjs patch|minor|major");
  process.exit(1);
}

const appJson = JSON.parse(readFileSync(appJsonPath, "utf8"));
const pkg = JSON.parse(readFileSync(pkgPath, "utf8"));

const current = String(appJson.expo.version ?? "0.0.0");
const m = /^(\d+)\.(\d+)\.(\d+)$/.exec(current);
if (!m) {
  console.error(`app.json has a version I can't parse: ${current}`);
  process.exit(1);
}
let [major, minor, patch] = m.slice(1).map(Number);

if (kind === "major") [major, minor, patch] = [major + 1, 0, 0];
else if (kind === "minor") [minor, patch] = [minor + 1, 0];
else patch += 1;

// 0.2.0 → 200, 1.0.0 → 10000. Monotonic as long as minor and patch stay under 100, which the
// check below enforces rather than silently producing a code that goes backwards.
if (minor > 99 || patch > 99) {
  console.error(`${major}.${minor}.${patch} would break the versionCode scheme (minor/patch cap at 99).`);
  process.exit(1);
}
const version = `${major}.${minor}.${patch}`;
const versionCode = major * 10000 + minor * 100 + patch;

const prevCode = Number(appJson.expo.android?.versionCode ?? 0);
if (versionCode <= prevCode) {
  console.error(`versionCode would not increase (${prevCode} → ${versionCode}). Android would refuse the update.`);
  process.exit(1);
}

appJson.expo.version = version;
appJson.expo.android = { ...appJson.expo.android, versionCode };
pkg.version = version;

writeFileSync(appJsonPath, JSON.stringify(appJson, null, 2) + "\n");
writeFileSync(pkgPath, JSON.stringify(pkg, null, 2) + "\n");

console.log(`${current} → ${version}  (versionCode ${prevCode} → ${versionCode})`);
console.log("next: npm run apk, then attach build/HyprSpace-" + version + ".apk to the release");
