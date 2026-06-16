import { create } from "zustand";

export type NotifKind = "update" | "info" | "account";

export interface Notif {
  id: string;
  title: string;
  body?: string;
  kind: NotifKind;
  ts: number;
  read: boolean;
}

interface NotifState {
  items: Notif[];
  add: (n: { id?: string; title: string; body?: string; kind?: NotifKind }) => void;
  markAllRead: () => void;
  clear: () => void;
}

let seq = 0;

export const useNotifications = create<NotifState>()((set) => ({
  items: [
    {
      id: "welcome",
      title: "Welcome to HyprSpace",
      body: "Tile Claude Code & terminals across projects. Press Ctrl+K to run a command.",
      kind: "info",
      ts: Date.now(),
      read: false,
    },
    {
      id: "account",
      title: "Accounts are here",
      body: "Sign in with Google or email — you stay signed in across launches.",
      kind: "account",
      ts: Date.now(),
      read: false,
    },
  ],
  add: (n) =>
    set((s) => {
      // de-dupe by explicit id so repeated update checks don't stack
      if (n.id && s.items.some((it) => it.id === n.id)) return {};
      const item: Notif = {
        id: n.id ?? `n${seq++}`,
        title: n.title,
        body: n.body,
        kind: n.kind ?? "info",
        ts: Date.now(),
        read: false,
      };
      return { items: [item, ...s.items] };
    }),
  markAllRead: () => set((s) => ({ items: s.items.map((it) => ({ ...it, read: true })) })),
  clear: () => set({ items: [] }),
}));
