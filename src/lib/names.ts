// short friendly names for agent panes, so a grid of identical agents is distinguishable
// (e.g. "Gus" running Claude vs "Wynn" running Claude) instead of eight panes all titled "Claude".
const POOL = [
  "Gus", "Wynn", "Theo", "Remy", "Enzo", "Dara", "Zoe", "Faye", "Otto", "Ivy",
  "Cleo", "Cy", "Nico", "Juno", "Knox", "Nell", "Vera", "Milo", "Lena", "Rex",
  "Iris", "Hugo", "Maya", "Finn", "Ada", "Leo", "Nina", "Kai", "Ruby", "Sage",
];

// first pool name not already taken; if the pool's exhausted, start suffixing (" 2", " 3", …)
export function pickAgentName(used: Set<string>): string {
  for (const n of POOL) if (!used.has(n)) return n;
  for (let i = 2; ; i++) for (const n of POOL) if (!used.has(`${n} ${i}`)) return `${n} ${i}`;
}
