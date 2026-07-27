// Create the release signing key, once, and record it where Gradle will find it.
//
//   npm run keystore
//
// The keystore and its passwords go in your home directory, NEVER the repo:
//   ~/.hyprspace-signing/hyprspace-mobile.jks     the key itself
//   ~/.gradle/gradle.properties                   the path + passwords Gradle reads
//
// Back the keystore up. Android identifies an app by its signature, so losing it means you can never
// ship an update that installs over what people already have.

import { execFileSync } from "node:child_process";
import { chmodSync, existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";
import { randomBytes } from "node:crypto";

// Both files below hold signing material, so they're owner-only. POSIX modes are a no-op on Windows
// (where the user profile's ACL already restricts them) — harmless there, and it matters on
// macOS/Linux, where ~/.gradle/gradle.properties would otherwise be world-readable.
const ownerOnly = (path, mode) => {
  try {
    chmodSync(path, mode);
  } catch {
    // a filesystem that doesn't do modes; the ACL still applies
  }
};

const dir = join(homedir(), ".hyprspace-signing");
const keystore = join(dir, "hyprspace-mobile.jks");
const gradleProps = join(homedir(), ".gradle", "gradle.properties");

if (existsSync(keystore)) {
  console.log(`Already there: ${keystore}\nDelete it first if you really mean to start over.`);
  process.exit(0);
}

const exe = process.platform === "win32" ? "keytool.exe" : "keytool";
const candidates = [
  process.env.JAVA_HOME,
  "C:\\Program Files\\Android\\Android Studio\\jbr",
  "/Applications/Android Studio.app/Contents/jbr/Contents/Home",
].filter(Boolean);
const javaHome = candidates.find((p) => existsSync(join(p, "bin", exe)));
if (!javaHome) {
  console.error("No JDK found — install Android Studio or set JAVA_HOME.");
  process.exit(1);
}

mkdirSync(dir, { recursive: true, mode: 0o700 });
ownerOnly(dir, 0o700);
const password = randomBytes(18).toString("base64url");

execFileSync(
  join(javaHome, "bin", exe),
  [
    "-genkeypair", "-v", "-storetype", "JKS",
    "-keystore", keystore,
    "-alias", "hyprspace",
    "-keyalg", "RSA", "-keysize", "2048", "-validity", "10000",
    "-storepass", password, "-keypass", password,
    "-dname", "CN=HyprSpace, OU=HyprSpace, O=HyprSpace, C=IN",
  ],
  { stdio: "inherit" },
);
ownerOnly(keystore, 0o600);

const gradleDir = join(homedir(), ".gradle");
mkdirSync(gradleDir, { recursive: true, mode: 0o700 });
const existing = existsSync(gradleProps) ? readFileSync(gradleProps, "utf8") : "";
const kept = existing
  .split(/\r?\n/)
  .filter((l) => !l.startsWith("HYPRSPACE_"))
  .join("\n")
  .trimEnd();

writeFileSync(
  gradleProps,
  `${kept ? kept + "\n" : ""}HYPRSPACE_STORE_FILE=${keystore.replace(/\\/g, "/")}
HYPRSPACE_KEY_ALIAS=hyprspace
HYPRSPACE_STORE_PASSWORD=${password}
HYPRSPACE_KEY_PASSWORD=${password}
`,
  { encoding: "utf8", mode: 0o600 },
);
// mode only applies when writeFileSync CREATES the file — an existing one keeps its old bits
ownerOnly(gradleProps, 0o600);

console.log(`\n✔ ${keystore}`);
console.log(`✔ credentials written to ${gradleProps}`);
console.log("\nBack that keystore up somewhere safe — it can't be regenerated.");
