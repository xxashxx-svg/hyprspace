// Mint a HyprSpace license key.
//   node scripts/license-keygen.mjs "Customer Name"
// Signs with the private key at ~/.hyprspace-signing/hyprspace-license.pem
// (override with HSPACE_LICENSE_PEM). Prints the key to stdout.
import { createPrivateKey, sign } from "node:crypto";
import { readFileSync } from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";

const name = process.argv.slice(2).join(" ").trim();
if (!name) {
  console.error('usage: node scripts/license-keygen.mjs "Customer Name"');
  process.exit(1);
}

const pemPath = process.env.HSPACE_LICENSE_PEM || join(homedir(), ".hyprspace-signing", "hyprspace-license.pem");
let key;
try {
  key = createPrivateKey(readFileSync(pemPath));
} catch (e) {
  console.error(`couldn't read signing key at ${pemPath}: ${e.message}`);
  console.error("run: node scripts/gen-license-keypair.mjs  (first time only)");
  process.exit(1);
}

const b64url = (buf) => Buffer.from(buf).toString("base64url");
const issued = new Date().toISOString().slice(0, 10); // YYYY-MM-DD
const payload = b64url(JSON.stringify({ n: name, iss: issued }));
const signature = b64url(sign(null, Buffer.from(payload, "utf8"), key));

console.log(`HSPACE-${payload}.${signature}`);
