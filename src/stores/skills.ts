import { create } from "zustand";
import { saveState, loadState } from "../api";

// a reusable prompt/command you can drag into any terminal
export interface Snippet {
  id: string;
  name: string;
  body: string;
}

interface SkillsState {
  snippets: Snippet[];
  loaded: boolean;
  load: () => Promise<void>;
  addSnippet: (name: string, body: string) => void;
  updateSnippet: (id: string, name: string, body: string) => void;
  removeSnippet: (id: string) => void;
}

const uid = () => crypto.randomUUID();
const persist = (get: () => SkillsState) =>
  void saveState("skills", JSON.stringify({ snippets: get().snippets })).catch(() => {});

export const useSkills = create<SkillsState>()((set, get) => ({
  snippets: [],
  loaded: false,
  load: async () => {
    if (get().loaded) return;
    const raw = await loadState("skills").catch(() => null);
    if (raw) {
      try {
        const p = JSON.parse(raw);
        if (Array.isArray(p?.snippets)) set({ snippets: p.snippets });
      } catch {
        /* ignore a bad blob */
      }
    }
    set({ loaded: true });
  },
  addSnippet: (name, body) => {
    set((s) => ({ snippets: [...s.snippets, { id: uid(), name: name.trim(), body }] }));
    persist(get);
  },
  updateSnippet: (id, name, body) => {
    set((s) => ({
      snippets: s.snippets.map((x) => (x.id === id ? { ...x, name: name.trim(), body } : x)),
    }));
    persist(get);
  },
  removeSnippet: (id) => {
    set((s) => ({ snippets: s.snippets.filter((x) => x.id !== id) }));
    persist(get);
  },
}));
