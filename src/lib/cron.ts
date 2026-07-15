// tiny 5-field cron: "min hour day-of-month month day-of-week". Supports * , - / and
// combinations ("*/15", "9-17", "1,15", "9-17/2"). Standard vixie rule: when BOTH dom and
// dow are restricted, a date matches if EITHER does.

interface CronSpec {
  min: Set<number>;
  hour: Set<number>;
  dom: Set<number>;
  mon: Set<number>;
  dow: Set<number>;
  domAny: boolean; // field was "*" (unrestricted)
  dowAny: boolean;
}

const BOUNDS: [number, number][] = [
  [0, 59], // minute
  [0, 23], // hour
  [1, 31], // day of month
  [1, 12], // month
  [0, 6], // day of week (0 = sunday; 7 accepted as sunday too)
];

function parseField(field: string, lo: number, hi: number): Set<number> | null {
  const out = new Set<number>();
  for (const part of field.split(",")) {
    const m = part.match(/^(\*|\d+(?:-\d+)?)(?:\/(\d+))?$/);
    if (!m) return null;
    const step = m[2] ? parseInt(m[2], 10) : 1;
    if (step < 1) return null;
    let a = lo;
    let b = hi;
    if (m[1] !== "*") {
      const r = m[1].split("-").map(Number);
      a = r[0];
      b = r.length > 1 ? r[1] : m[2] ? hi : r[0]; // "5/10" = every 10 starting at 5
    }
    if (a > b) return null;
    for (let v = a; v <= b; v += step) {
      let n = v;
      if (hi === 6 && n === 7) n = 0; // dow: 7 → sunday
      if (n < lo || n > hi) return null;
      out.add(n);
    }
  }
  return out.size ? out : null;
}

export function parseCron(expr: string): CronSpec | null {
  const fields = expr.trim().split(/\s+/);
  if (fields.length !== 5) return null;
  const sets: Set<number>[] = [];
  for (let i = 0; i < 5; i++) {
    const s = parseField(fields[i], BOUNDS[i][0], BOUNDS[i][1]);
    if (!s) return null;
    sets.push(s);
  }
  return {
    min: sets[0],
    hour: sets[1],
    dom: sets[2],
    mon: sets[3],
    dow: sets[4],
    // vixie treats "*/n" like "*" for the dom/dow OR rule, so match on the star prefix
    domAny: fields[2].startsWith("*"),
    dowAny: fields[4].startsWith("*"),
  };
}

export const cronValid = (expr: string) => parseCron(expr) !== null;

// next fire time strictly after `from` (ms), scanning minute-by-minute up to a year out.
// null = bad expression or nothing matches (e.g. feb 30).
// known limit: a job scheduled inside the spring-forward DST gap (e.g. 02:30 on the jump night)
// slides to the next day instead of running right after the jump — rare enough to live with.
const MAX_DOM = [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]; // per-month max day-of-month

export function nextCron(expr: string, from = Date.now()): number | null {
  const spec = parseCron(expr);
  if (!spec) return null;
  // a dom/mon combo that can never exist (e.g. "0 0 30 2 *") would otherwise scan the whole
  // horizon minute by minute — bail out up front. only decisive on the AND path; the OR path
  // (both dom and dow restricted) always has a weekday escape hatch.
  if (spec.domAny || spec.dowAny) {
    let possible = false;
    for (const mo of spec.mon) for (const dd of spec.dom) if (dd <= MAX_DOM[mo - 1]) possible = true;
    if (!possible) return null;
  }
  const d = new Date(from);
  d.setSeconds(0, 0);
  d.setMinutes(d.getMinutes() + 1);
  for (let i = 0; i < 366 * 24 * 60; i++) {
    const monOk = spec.mon.has(d.getMonth() + 1);
    const domOk = spec.dom.has(d.getDate());
    const dowOk = spec.dow.has(d.getDay());
    // vixie: the star flags only pick OR vs AND — the sets themselves always apply
    // (a plain "*" set contains every value, so the AND path is naturally satisfied)
    const dayOk = !spec.domAny && !spec.dowAny ? domOk || dowOk : domOk && dowOk;
    if (monOk && dayOk) {
      if (spec.min.has(d.getMinutes()) && spec.hour.has(d.getHours())) return d.getTime();
      // skip fast when the hour can't match
      if (!spec.hour.has(d.getHours())) {
        d.setMinutes(0);
        d.setHours(d.getHours() + 1);
      } else {
        d.setMinutes(d.getMinutes() + 1);
      }
    } else {
      // the date alone rules this whole day out — jump to the next midnight
      d.setHours(0, 0, 0, 0);
      d.setDate(d.getDate() + 1);
    }
  }
  return null;
}
