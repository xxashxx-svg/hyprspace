// prep an app screenshot for the website: trims the window-frame border and recompresses.
//   node scripts/site-shot.mjs <capture.png> <website/assets/shots/name.png> [borderPx]
import sharp from "sharp";

const [src, dst, borderArg] = process.argv.slice(2);
if (!src || !dst) {
  console.error("usage: node scripts/site-shot.mjs <src.png> <dst.png> [borderPx]");
  process.exit(1);
}
const border = borderArg ? parseInt(borderArg, 10) : 8;
const meta = await sharp(src).metadata();
const out = await sharp(src)
  .extract({ left: border, top: border, width: meta.width - border * 2, height: meta.height - border * 2 })
  .png({ compressionLevel: 9 })
  .toFile(dst);
console.log(`${dst}: ${out.width}x${out.height}, ${(out.size / 1024).toFixed(0)} KB`);
