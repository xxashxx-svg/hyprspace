import { create } from "zustand";
import type { Action } from "./projectConfig";

// drives the Add/Edit Action modal. `editing` null = a new action for `folder`.
interface ActionEditorState {
  open: boolean;
  folder: string;
  wsId?: string; // the open workspace at that folder, if any (enables run-on-save)
  editing: Action | null;
  openEditor: (folder: string, opts?: { wsId?: string; action?: Action }) => void;
  close: () => void;
}

export const useActionEditor = create<ActionEditorState>((set) => ({
  open: false,
  folder: "",
  wsId: undefined,
  editing: null,
  openEditor: (folder, opts) =>
    set({ open: true, folder, wsId: opts?.wsId, editing: opts?.action ?? null }),
  close: () => set({ open: false, editing: null }),
}));
