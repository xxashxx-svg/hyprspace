// Where new projects get created. Used by both the chat orchestrator and the New Project dialog
// so they always agree: the user's "Projects folder" setting, else ~/Documents/HyprSpace.
import { getHomeDir } from "../api";
import { useSettings } from "../stores/settings";

let homeCache = "";
async function home(): Promise<string> {
  if (!homeCache) homeCache = await getHomeDir().catch(() => "");
  return homeCache;
}

export function joinPath(base: string, name: string): string {
  if (!base) return name;
  const sep = base.includes("\\") ? "\\" : "/";
  return `${base}${sep}${name}`;
}

// one level up, or the path itself at a root. Handles both separators so it works on whichever OS
// this desktop is — the phone never does path math, it asks us (see mobileBridge.ts).
export function parentOf(path: string): string {
  return path.replace(/[\\/][^\\/]+[\\/]?$/, "") || path;
}

// seeded into a brand-new project when "add .gitignore" is on. Shared by the desktop's New Project
// dialog and the phone's, so the two can't drift apart.
export const DEFAULT_GITIGNORE = `node_modules/
dist/
build/
target/
__pycache__/
.venv/
*.log
.env
.env.local
.DS_Store
`;

// the base folder under which new projects live (no trailing separator)
export async function projectsBaseDir(): Promise<string> {
  const set = useSettings.getState().projectsDir.trim();
  if (set) return set;
  const h = await home();
  if (!h) return "";
  const sep = h.includes("\\") ? "\\" : "/";
  return [h, "Documents", "HyprSpace"].join(sep);
}
