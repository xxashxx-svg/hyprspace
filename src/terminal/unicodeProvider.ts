import { Unicode11Addon } from "@xterm/addon-unicode11";
import type { IUnicodeVersionProvider, Terminal } from "@xterm/xterm";

// xterm packs char info into one int: (charKind << 3) | (width << 1) | shouldJoin.
const ZWJ = 0x200d;
const VERSION = "hs-11-zwj";
const widthOf = (p: number) => ((p >> 1) & 3) as 0 | 1 | 2;
const kindOf = (p: number) => p >> 3;
const pack = (kind: number, w: 0 | 1 | 2, join: boolean) =>
  ((kind & 0xffffff) << 3) | ((w & 3) << 1) | (join ? 1 : 0);

// ZWJ emoji (👨‍💻, 🏳️‍🌈, family emoji) render as ONE glyph and CLIs budget them as a single wide
// cell — but plain Unicode 11 advances a cell for each joined part, so the whole line mis-widths.
// wrap the Unicode 11 provider to keep a ZWJ run glued to one cell pair. (approach borrowed from Orca)
class ZwjProvider implements IUnicodeVersionProvider {
  readonly version = VERSION;
  constructor(private base: IUnicodeVersionProvider) {}
  wcwidth(cp: number): 0 | 1 | 2 {
    return this.base.wcwidth(cp);
  }
  charProperties(cp: number, prev: number): number {
    const pw = widthOf(prev);
    if (cp === ZWJ && pw > 0) return pack(ZWJ, pw, true);
    if (kindOf(prev) === ZWJ && pw > 0 && this.wcwidth(cp) > 0) return pack(cp, pw, true);
    return this.base.charProperties(cp, prev);
  }
}

type CoreUnicode = { _core?: { unicodeService?: { _providers?: Record<string, IUnicodeVersionProvider> } } };

// load Unicode 11 widths + the ZWJ refinement onto a terminal. Call once, after term.open().
export function applyUnicode(term: Terminal) {
  try {
    if (term.unicode.activeVersion === VERSION) return;
    term.loadAddon(new Unicode11Addon()); // registers the "11" provider synchronously
    const base = (term as unknown as CoreUnicode)._core?.unicodeService?._providers?.["11"];
    if (base && !term.unicode.versions.includes(VERSION)) {
      term.unicode.register(new ZwjProvider(base));
      term.unicode.activeVersion = VERSION;
    } else {
      term.unicode.activeVersion = "11"; // at least get Unicode 11 widths
    }
  } catch {
    /* keep xterm's default widths if the internal shape changes */
  }
}
