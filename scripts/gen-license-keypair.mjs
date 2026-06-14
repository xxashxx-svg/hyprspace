// One-time: generate the HyprSpace license signing keypair.
// - writes the PRIVATE key (PKCS8 PEM) to ~/.hyprspace-signing/hyprspace-license.pem  (keep secret!)
// - prints the PUBLIC key (base64url) to embed in src-tauri/src/license.rs
// Refuses to clobber an existing private key unless you pass --force.
import { generateKeyPairSync } from "node:crypto";
import { mkdirSync, writeFileSync, existsSync } from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";

const dir = join(homedir(), ".hyprspace-signing");
const pemPath = join(dir, "hyprspace-license.pem");

if (existsSync(pemPath) && !process.argv.includes("--force")) {
  console.error(`refusing to overwrite ${pemPath} (pass --force to regenerate — this invalidates every key already issued)`);
  process.exit(1);
}

const { publicKey, privateKey } = generateKeyPairSync("ed25519");
const pem = privateKey.export({ format: "pem", type: "pkcs8" });
const pubX = publicKey.export({ format: "jwk" }).x; // raw 32-byte public key, base64url

mkdirSync(dir, { recursive: true });
writeFileSync(pemPath, pem, { mode: 0o600 });

console.log("private key written to:", pemPath);
console.log("");
console.log("embed this in src-tauri/src/license.rs as LICENSE_PUBKEY:");
console.log(pubX);
