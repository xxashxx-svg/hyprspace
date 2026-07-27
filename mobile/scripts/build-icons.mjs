// Generate every app icon from the HyprSpace mark, so the phone icon is the same drawing as the
// desktop's (src/components/Logo.tsx — an isometric cube, indigo top, neutral sides) rather than a
// resized screenshot of it.
//
//   node scripts/build-icons.mjs      (also runs via `npm run icons`)
//
// Uses the desktop app's `sharp` — this only ever runs on a maintainer's machine, so it's not a
// dependency of the mobile app itself.

import { writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { createRequire } from "node:module";

const here = dirname(fileURLToPath(import.meta.url));
const root = join(here, "..", "..");
const assets = join(here, "..", "assets");
const sharp = createRequire(join(root, "package.json"))("sharp");

const TILE = "#161616"; // --bg-base
const TOP = "#3d54e8"; // the accent, as it renders in the shipped desktop icon
const LEFT = "#f5f5f5"; // --text-1
const RIGHT = "#767676"; // --text-3

// the mark's three faces, in the Logo component's own 24-unit box
const TOP_FACE = "M12 3 L20.5 8 L12 13 L3.5 8 Z";
const LEFT_FACE = "M3.5 8 L12 13 L12 21 L3.5 16 Z";
const RIGHT_FACE = "M20.5 8 L20.5 16 L12 21 L12 13 Z";

/** place the 24-unit mark centred on a `size` canvas, occupying `frac` of its width */
function cube(size, frac, colors = { top: TOP, left: LEFT, right: RIGHT }) {
  const scale = (size * frac) / 17; // the cube is 17 units wide (x 3.5 → 20.5)
  const off = size / 2 - 12 * scale; // its centre sits at (12, 12)
  return `<g transform="translate(${off} ${off}) scale(${scale})">
    <path d="${TOP_FACE}" fill="${colors.top}" />
    <path d="${LEFT_FACE}" fill="${colors.left}" />
    <path d="${RIGHT_FACE}" fill="${colors.right}" />
  </g>`;
}

function svg(size, body) {
  return `<svg xmlns="http://www.w3.org/2000/svg" width="${size}" height="${size}" viewBox="0 0 ${size} ${size}">${body}</svg>`;
}

function tile(size, frac = 0.55) {
  const rx = size * 0.225;
  return svg(
    size,
    `<rect width="${size}" height="${size}" rx="${rx}" fill="${TILE}" />${cube(size, frac)}`,
  );
}

/** Android's monochrome layer is an alpha mask, so the mark becomes a one-color wireframe cube */
function wireframe(size) {
  const scale = (size * 0.52) / 17;
  const off = size / 2 - 12 * scale;
  const w = 1.4; // in mark units, so it scales with everything else
  return svg(
    size,
    `<g transform="translate(${off} ${off}) scale(${scale})" fill="none" stroke="#ffffff"
        stroke-width="${w}" stroke-linejoin="round" stroke-linecap="round">
      <path d="M12 3 L20.5 8 L20.5 16 L12 21 L3.5 16 L3.5 8 Z" />
      <path d="M3.5 8 L12 13 L20.5 8" />
      <path d="M12 13 L12 21" />
    </g>`,
  );
}

const jobs = [
  // the main icon: Expo masks it per platform, so it ships as the full tile
  ["icon.png", tile(1024)],
  ["favicon.png", tile(96, 0.6)],
  // adaptive icon: Android crops to a circle/squircle, so the foreground gets a generous margin
  ["android-icon-foreground.png", svg(512, cube(512, 0.42))],
  ["android-icon-background.png", svg(512, `<rect width="512" height="512" fill="${TILE}" />`)],
  ["android-icon-monochrome.png", wireframe(432)],
  // splash: the mark alone; the surrounding color comes from app.json
  ["splash-icon.png", svg(1024, cube(1024, 0.34))],
];

for (const [name, markup] of jobs) {
  const out = join(assets, name);
  const png = await sharp(Buffer.from(markup)).png().toBuffer();
  writeFileSync(out, png);
  console.log(`wrote assets/${name}`);
}
