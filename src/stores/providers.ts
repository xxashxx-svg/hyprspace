// Which agent CLIs are installed on this machine, what they report about themselves, and the
// model catalog Codex keeps locally. Checked once at startup and again on demand from Settings.
import { create } from "zustand";
import { providerStatus, refreshPath, getHomeDir, readFile, type ProviderStatus } from "../api";
import { AGENT_IDS, CATALOG, type AgentCatalog, type ModelOption, type ProviderId } from "../lib/models";

interface ProvidersState {
  status: Partial<Record<ProviderId, ProviderStatus>>;
  checking: boolean;
  checkedAt: number | null;
  /** Codex's model list from ~/.codex/models_cache.json, when that file exists */
  codexModels: ModelOption[] | null;
  refresh: () => Promise<void>;
}

// The CLI writes this file itself, so it is display-only data we never fetch. Only listed models,
// in the CLI's own order.
async function loadCodexCatalog(): Promise<ModelOption[] | null> {
  try {
    const home = await getHomeDir();
    const raw = await readFile(`${home.replace(/[\\/]+$/, "")}/.codex/models_cache.json`);
    const data = JSON.parse(raw) as {
      models?: {
        slug: string;
        display_name?: string;
        description?: string;
        visibility?: string;
        priority?: number;
        default_reasoning_level?: string;
        supported_reasoning_levels?: { effort: string; description?: string }[];
      }[];
    };
    const models = (data.models ?? [])
      .filter((m) => m.slug && m.visibility !== "hide")
      .sort((a, b) => (a.priority ?? 999) - (b.priority ?? 999))
      .map<ModelOption>((m) => ({
        id: m.slug,
        label: m.display_name ?? m.slug,
        note: m.description,
        efforts: (m.supported_reasoning_levels ?? []).map((l) => l.effort),
        effortNotes: Object.fromEntries((m.supported_reasoning_levels ?? []).map((l) => [l.effort, l.description ?? ""])),
        defaultEffort: m.default_reasoning_level,
      }));
    return models.length ? models : null;
  } catch {
    return null;
  }
}

export const useProviders = create<ProvidersState>()((set, get) => ({
  status: {},
  checking: false,
  checkedAt: null,
  codexModels: null,

  refresh: async () => {
    if (get().checking) return;
    set({ checking: true });
    await refreshPath().catch(() => {});
    await Promise.all([
      ...AGENT_IDS.map(async (id) => {
        const st = await providerStatus(id).catch<ProviderStatus>(() => ({
          id,
          installed: false,
          version: null,
          account: null,
          plan: null,
          detail: "Could not check",
        }));
        set((s) => ({ status: { ...s.status, [id]: st } }));
      }),
      loadCodexCatalog().then((codexModels) => set({ codexModels })),
    ]);
    set({ checking: false, checkedAt: Date.now() });
  },
}));

/** True when the CLI is known to be installed, or when it has not been checked yet. */
export function isInstalled(status: ProvidersState["status"], id: ProviderId): boolean {
  if (id === "terminal" || id === "wsl") return true;
  const st = status[id];
  return st ? st.installed : true;
}

/** The catalog for a provider, with Codex's live model list swapped in when we have it. */
export function catalogFor(id: ProviderId, codexModels: ModelOption[] | null): AgentCatalog {
  const base = CATALOG[id];
  if (id !== "codex" || !codexModels) return base;
  return { ...base, models: [base.models[0], ...codexModels] };
}
