import { create } from "zustand";

export interface ConfirmReq {
  title: string;
  message: string;
  confirmLabel?: string;
  cancelLabel?: string;
  danger?: boolean;
}

interface ConfirmState {
  req: ConfirmReq | null;
  resolve: ((ok: boolean) => void) | null;
  open: (req: ConfirmReq) => Promise<boolean>;
  answer: (ok: boolean) => void;
}

export const useConfirm = create<ConfirmState>((set, get) => ({
  req: null,
  resolve: null,
  open: (req) =>
    new Promise<boolean>((resolve) => {
      // if a confirm is somehow already open, dismiss it as "no" first
      get().resolve?.(false);
      set({ req, resolve });
    }),
  answer: (ok) => {
    const r = get().resolve;
    set({ req: null, resolve: null });
    r?.(ok);
  },
}));

// in-app replacement for the native ask() dialog — callable from anywhere (actions, etc.)
export function confirmDialog(req: ConfirmReq): Promise<boolean> {
  return useConfirm.getState().open(req);
}
