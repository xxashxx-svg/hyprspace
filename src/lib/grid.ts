// Balanced tiling math, shared by the real pane grid and the launcher's live preview so the two
// always agree. Split N panes into ~√N rows, biggest rows first, and let each row fill the full
// width evenly — so 7 → 4+3, 10 → 4+3+3, never a lonely full-width pane (the old "3+3+1"). Rows can
// have different counts, so we render on a grid whose column count is the LCM of the row sizes and
// span each pane to fill its row (row of 4 → span 3 of 12, row of 3 → span 4, etc.).
export type GridLayout = { cols: string; span: (i: number) => string | undefined };

const gcd = (a: number, b: number): number => (b ? gcd(b, a % b) : a);
const lcm = (a: number, b: number) => (a / gcd(a, b)) * b;

export function getLayout(n: number): GridLayout {
  if (n <= 1) return { cols: "1fr", span: () => undefined };
  const rows = Math.max(1, Math.floor(Math.sqrt(n)));
  const base = Math.floor(n / rows);
  const extra = n % rows; // the first `extra` rows get one more pane (bigger rows on top)
  const rowSizes = Array.from({ length: rows }, (_, r) => base + (r < extra ? 1 : 0));
  const cols = rowSizes.reduce((acc, s) => lcm(acc, s), 1);
  const rowOf: number[] = []; // pane index → how many panes share its row
  for (const s of rowSizes) for (let k = 0; k < s; k++) rowOf.push(s);
  return { cols: `repeat(${cols}, 1fr)`, span: (i) => `span ${cols / rowOf[i]}` };
}

// ---- pickable layouts ----
// A user-choosable arrangement for N panes. `cells[i]` places pane i with explicit grid lines, so we
// can express things the auto tiling can't (a pane spanning two rows, etc). The first preset for each
// count is the balanced default (matches getLayout); the rest are alternatives shown in the picker.
export type CellPlace = { gridColumn?: string; gridRow?: string };
export type LayoutPreset = {
  id: string;
  label: string;
  cols: string; // grid-template-columns
  rows: string; // grid-template-rows
  cells: CellPlace[]; // per-pane placement
};

const C = (gridColumn: string, gridRow: string): CellPlace => ({ gridColumn, gridRow });

// curated presets for the common pane counts. 7+ panes fall back to the auto tiling (too many
// permutations to be useful). label order = display order; cells order = pane order.
export const LAYOUTS: Record<number, LayoutPreset[]> = {
  2: [
    { id: "cols", label: "Side by side", cols: "1fr 1fr", rows: "1fr", cells: [C("1", "1"), C("2", "1")] },
    { id: "rows", label: "Stacked", cols: "1fr", rows: "1fr 1fr", cells: [C("1", "1"), C("1", "2")] },
  ],
  3: [
    { id: "cols", label: "Columns", cols: "1fr 1fr 1fr", rows: "1fr", cells: [C("1", "1"), C("2", "1"), C("3", "1")] },
    { id: "2top", label: "2 top · 1 bottom", cols: "1fr 1fr", rows: "1fr 1fr", cells: [C("1", "1"), C("2", "1"), C("1 / 3", "2")] },
    { id: "1top", label: "1 top · 2 bottom", cols: "1fr 1fr", rows: "1fr 1fr", cells: [C("1 / 3", "1"), C("1", "2"), C("2", "2")] },
    { id: "1left", label: "1 left · 2 right", cols: "1fr 1fr", rows: "1fr 1fr", cells: [C("1", "1 / 3"), C("2", "1"), C("2", "2")] },
    { id: "1right", label: "2 left · 1 right", cols: "1fr 1fr", rows: "1fr 1fr", cells: [C("1", "1"), C("1", "2"), C("2", "1 / 3")] },
    { id: "rows", label: "Rows", cols: "1fr", rows: "1fr 1fr 1fr", cells: [C("1", "1"), C("1", "2"), C("1", "3")] },
  ],
  4: [
    { id: "grid", label: "2 × 2", cols: "1fr 1fr", rows: "1fr 1fr", cells: [C("1", "1"), C("2", "1"), C("1", "2"), C("2", "2")] },
    { id: "cols", label: "Columns", cols: "repeat(4, 1fr)", rows: "1fr", cells: [C("1", "1"), C("2", "1"), C("3", "1"), C("4", "1")] },
    { id: "rows", label: "Rows", cols: "1fr", rows: "repeat(4, 1fr)", cells: [C("1", "1"), C("1", "2"), C("1", "3"), C("1", "4")] },
    { id: "1left", label: "1 left · 3 right", cols: "1fr 1fr", rows: "repeat(3, 1fr)", cells: [C("1", "1 / 4"), C("2", "1"), C("2", "2"), C("2", "3")] },
    { id: "1top", label: "1 top · 3 bottom", cols: "repeat(3, 1fr)", rows: "1fr 1fr", cells: [C("1 / 4", "1"), C("1", "2"), C("2", "2"), C("3", "2")] },
  ],
  5: [
    { id: "auto", label: "3 top · 2 bottom", cols: "repeat(6, 1fr)", rows: "1fr 1fr", cells: [C("1 / 3", "1"), C("3 / 5", "1"), C("5 / 7", "1"), C("1 / 4", "2"), C("4 / 7", "2")] },
    { id: "2top", label: "2 top · 3 bottom", cols: "repeat(6, 1fr)", rows: "1fr 1fr", cells: [C("1 / 4", "1"), C("4 / 7", "1"), C("1 / 3", "2"), C("3 / 5", "2"), C("5 / 7", "2")] },
    { id: "1left", label: "1 left · 4 right", cols: "1fr 1fr 1fr", rows: "1fr 1fr", cells: [C("1", "1 / 3"), C("2", "1"), C("3", "1"), C("2", "2"), C("3", "2")] },
    { id: "cols", label: "Columns", cols: "repeat(5, 1fr)", rows: "1fr", cells: [C("1", "1"), C("2", "1"), C("3", "1"), C("4", "1"), C("5", "1")] },
  ],
  6: [
    { id: "grid", label: "3 × 2", cols: "repeat(3, 1fr)", rows: "1fr 1fr", cells: [C("1", "1"), C("2", "1"), C("3", "1"), C("1", "2"), C("2", "2"), C("3", "2")] },
    { id: "grid2", label: "2 × 3", cols: "1fr 1fr", rows: "repeat(3, 1fr)", cells: [C("1", "1"), C("2", "1"), C("1", "2"), C("2", "2"), C("1", "3"), C("2", "3")] },
    { id: "cols", label: "Columns", cols: "repeat(6, 1fr)", rows: "1fr", cells: [C("1", "1"), C("2", "1"), C("3", "1"), C("4", "1"), C("5", "1"), C("6", "1")] },
  ],
};

export type ResolvedLayout = { cols: string; rows?: string; place: (i: number) => CellPlace };

// the layouts available for a count (for the picker). empty = no choices (use auto tiling).
export function layoutsFor(n: number): LayoutPreset[] {
  return LAYOUTS[n] ?? [];
}

// the active layout for a count + chosen preset id. unknown id / count → the balanced default.
export function resolveLayout(n: number, id?: string): ResolvedLayout {
  const presets = LAYOUTS[n];
  if (presets && presets.length) {
    const p = (id && presets.find((x) => x.id === id)) || presets[0];
    return { cols: p.cols, rows: p.rows, place: (i) => p.cells[i] ?? {} };
  }
  const g = getLayout(n); // 7+ panes (or 1): the auto tiling, rows auto-flow
  return { cols: g.cols, place: (i) => ({ gridColumn: g.span(i) }) };
}

// the effective preset id for a count (chosen, else the default's id) — for highlighting the picker
export function activeLayoutId(n: number, id?: string): string {
  const presets = LAYOUTS[n];
  if (!presets || !presets.length) return "";
  return (id && presets.some((x) => x.id === id) && id) || presets[0].id;
}
