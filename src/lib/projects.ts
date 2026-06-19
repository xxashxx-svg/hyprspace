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

// the base folder under which new projects live (no trailing separator)
export async function projectsBaseDir(): Promise<string> {
  const set = useSettings.getState().projectsDir.trim();
  if (set) return set;
  const h = await home();
  if (!h) return "";
  const sep = h.includes("\\") ? "\\" : "/";
  return [h, "Documents", "HyprSpace"].join(sep);
}
