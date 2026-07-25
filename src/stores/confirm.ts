import { create } from "zustand";
import { useSettings } from "./settings";

export interface ConfirmReq {
  title: string;
  message: string;
  confirmLabel?: string;
  cancelLabel?: string;
  danger?: boolean;
  /** optional third choice (e.g. "Discard") — resolves "alt" instead of true/false */
  altLabel?: string;
  dontAskId?: string; // set to offer a "don't ask again" checkbox; dismissed ids auto-confirm
}

/** true = confirmed, false = cancelled, "alt" = the optional third button */
export type ConfirmAnswer = boolean | "alt";

interface ConfirmState {
  req: ConfirmReq | null;
  resolve: ((ok: ConfirmAnswer) => void) | null;
  open: (req: ConfirmReq) => Promise<ConfirmAnswer>;
  answer: (ok: ConfirmAnswer) => void;
}

export const useConfirm = create<ConfirmState>((set, get) => ({
  req: null,
  resolve: null,
  open: (req) =>
    new Promise<ConfirmAnswer>((resolve) => {
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
export function confirmDialog(req: ConfirmReq): Promise<ConfirmAnswer> {
  if (req.dontAskId && useSettings.getState().dismissedConfirms.includes(req.dontAskId)) {
    return Promise.resolve(true);
  }
  return useConfirm.getState().open(req);
}
