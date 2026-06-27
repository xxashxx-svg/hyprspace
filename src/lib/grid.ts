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
