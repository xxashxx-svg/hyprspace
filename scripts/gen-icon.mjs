// Rasterize scripts/logo.svg -> a 1024px PNG that `tauri icon` can consume.
//   node scripts/gen-icon.mjs && npx tauri icon scripts/logo-1024.png
import sharp from "sharp";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const here = dirname(fileURLToPath(import.meta.url));
const out = join(here, "logo-1024.png");

await sharp(join(here, "logo.svg"), { density: 384 })
  .resize(1024, 1024, { fit: "contain", background: { r: 0, g: 0, b: 0, alpha: 0 } })
  .png()
  .toFile(out);

console.log("wrote", out);
