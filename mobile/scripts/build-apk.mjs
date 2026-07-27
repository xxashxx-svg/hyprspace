// Build an installable APK locally — no Expo account, no EAS, no cloud queue.
//
//   npm run apk              release build, signed with your keystore if you have one
//   npm run apk -- debug     debug build (faster, no minification)
//   npm run apk -- emu       for the Android emulator, which is x86_64 (a phone build won't install)
//   npm run apk -- all       every CPU architecture at once (slow; see withGradleProps.js)
//
// It finds a JDK (Android Studio ships one), regenerates android/ when it's missing or stale, runs
// Gradle, and copies the result to build/ with a sensible filename.

import { execFileSync } from "node:child_process";
import { existsSync, mkdirSync, copyFileSync, readFileSync, statSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const app = join(here, "..");
const androidDir = join(app, "android");

const args = process.argv.slice(2);
const variant = args.includes("debug") ? "debug" : "release";
// the default (arm64-v8a, set in withGradleProps.js) is every real phone; the emulator is x86_64
const abis = args.includes("all")
  ? "armeabi-v7a,arm64-v8a,x86,x86_64"
  : args.includes("emu")
    ? "x86_64"
    : null;
const { expo } = JSON.parse(readFileSync(join(app, "app.json"), "utf8"));

// --- JDK: honour JAVA_HOME, else fall back to the one Android Studio installs ---
const STUDIO_JBRS = [
  "C:\\Program Files\\Android\\Android Studio\\jbr",
  "C:\\Program Files\\Android\\Android Studio Preview\\jbr",
  "/Applications/Android Studio.app/Contents/jbr/Contents/Home",
  join(process.env.HOME ?? "", ".jdks"),
];
function findJdk() {
  const exe = process.platform === "win32" ? "java.exe" : "java";
  if (process.env.JAVA_HOME && existsSync(join(process.env.JAVA_HOME, "bin", exe))) {
    return process.env.JAVA_HOME;
  }
  return STUDIO_JBRS.find((p) => existsSync(join(p, "bin", exe)));
}

const javaHome = findJdk();
if (!javaHome) {
  console.error(
    "No JDK found. Install Android Studio (it bundles one) or set JAVA_HOME to a JDK 17+.",
  );
  process.exit(1);
}

const sdk = process.env.ANDROID_HOME ?? process.env.ANDROID_SDK_ROOT;
if (!sdk || !existsSync(sdk)) {
  console.error(
    "No Android SDK. Install it via Android Studio, then set ANDROID_HOME to it\n" +
      "(usually %LOCALAPPDATA%\\Android\\Sdk on Windows, ~/Library/Android/sdk on macOS).",
  );
  process.exit(1);
}

const env = { ...process.env, JAVA_HOME: javaHome, ANDROID_HOME: sdk };
const run = (cmd, args, cwd = app) =>
  execFileSync(cmd, args, { cwd, env, stdio: "inherit", shell: process.platform === "win32" });

// --- android/ is generated; regenerate it when it's missing or older than the config ---
const configFiles = [join(app, "app.json"), join(app, "package.json")];
const stale =
  !existsSync(androidDir) ||
  configFiles.some((f) => statSync(f).mtimeMs > statSync(androidDir).mtimeMs);

if (stale) {
  console.log("› regenerating the native project (expo prebuild)");
  run("npx", ["expo", "prebuild", "-p", "android", "--no-install"]);
}

// absolute path: cmd.exe won't resolve a bare `gradlew.bat` out of the working directory
const gradlew = join(androidDir, process.platform === "win32" ? "gradlew.bat" : "gradlew");
const task = `assemble${variant[0].toUpperCase()}${variant.slice(1)}`;
const gradleArgs = [task, "--console=plain"];
// -P beats gradle.properties, so switching ABIs doesn't need a prebuild (which would wipe the
// compiled native cache and cost another cold build)
if (abis) gradleArgs.push(`-PreactNativeArchitectures=${abis}`);

console.log(`› gradle ${task}  (${abis ?? "arm64-v8a"})  JDK: ${javaHome}`);
run(gradlew, gradleArgs, androidDir);

const built = join(androidDir, "app", "build", "outputs", "apk", variant, `app-${variant}.apk`);
if (!existsSync(built)) {
  console.error(`Gradle finished but ${built} isn't there.`);
  process.exit(1);
}

const outDir = join(app, "build");
mkdirSync(outDir, { recursive: true });
const tag = [variant === "debug" ? "debug" : null, abis === "x86_64" ? "emu" : null]
  .filter(Boolean)
  .join("-");
const out = join(outDir, `HyprSpace-${expo.version}${tag ? `-${tag}` : ""}.apk`);
copyFileSync(built, out);

const mb = (statSync(out).size / 1024 / 1024).toFixed(1);
console.log(`\n✔ ${out}  (${mb} MB)`);
console.log("  install it with:  adb install -r " + JSON.stringify(out));
