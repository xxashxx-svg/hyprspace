// ── SUBSCRIPTION GATING — currently DORMANT (everyone is free) ───────────────────────────────
// On launch the app asks a backend "is this account entitled?". While we're free, that backend
// (a Supabase Edge Function named `entitlement`) either doesn't exist yet or returns mode:"free",
// so every user is entitled and the paywall NEVER shows. Going paid is a SERVER change, not an app
// update: deploy/flip the function to return mode:"paid" + per-account entitlement, and every app
// already installed in the field starts enforcing it on its next check — no new build required.
//
// The function should return JSON:
//   { mode: "free"|"paid", entitled: boolean, tier?: string, reason?: string, token?: string }
// `token` is optional but recommended — it's Ed25519-verified offline (same key as licenses) so a
// user can't fake entitlement by editing the cached blob. Shape:
//   "HSENT-<base64url payload {uid,tier,mode,exp}>.<base64url ed25519 sig>"
// signed with ~/.hyprspace-signing/hyprspace-license.pem (exp = unix seconds, 0 = no expiry).
//
// SAFETY: this is fail-OPEN. No backend / no session / network error / unverifiable token →
// the app treats you as entitled and never locks. Only an explicit paid+not-entitled answer locks.
import { create } from "zustand";
import { entitlementVerify, saveState, loadState } from "../api";
import { supabase } from "../lib/supabase";
import { useAuth } from "./auth";

type Mode = "free" | "paid";

const ENTITLEMENT_FN = "entitlement"; // the Supabase Edge Function to deploy when going paid
const OFFLINE_GRACE_MS = 7 * 24 * 60 * 60 * 1000; // trust a cached signed token this long offline

interface EntResult {
  mode?: Mode;
  entitled?: boolean;
  tier?: string;
  reason?: string;
  token?: string;
}
type Resolved = { mode: Mode; entitled: boolean; tier: string };

// turn a backend result into a resolved verdict; verify + cache a signed token if present
async function resolveResult(res: EntResult): Promise<Resolved> {
  let mode: Mode = res.mode === "paid" ? "paid" : "free";
  let tier = res.tier ?? "";
  let entitled = res.entitled ?? true;
  if (res.token) {
    const claims = await entitlementVerify(res.token).catch(() => null);
    if (!claims) return { mode, entitled: false, tier }; // a token that won't verify isn't trusted
    mode = claims.mode === "paid" ? "paid" : "free";
    tier = claims.tier || tier;
    entitled = claims.exp === 0 || claims.exp * 1000 > Date.now();
    void saveState("entitlement", JSON.stringify({ token: res.token, at: Date.now() })).catch(() => {});
  }
  return { mode, entitled, tier };
}

// offline fallback: a cached signed token still inside the grace window
async function fromCache(): Promise<Resolved | null> {
  const raw = await loadState("entitlement").catch(() => null);
  if (!raw) return null;
  try {
    const { token, at } = JSON.parse(raw);
    if (typeof at === "number" && Date.now() - at > OFFLINE_GRACE_MS) return null;
    const claims = await entitlementVerify(token).catch(() => null);
    if (!claims) return null;
    return {
      mode: claims.mode === "paid" ? "paid" : "free",
      entitled: claims.exp === 0 || claims.exp * 1000 > Date.now(),
      tier: claims.tier || "",
    };
  } catch {
    return null;
  }
}

interface EntState {
  status: "checking" | "ok" | "locked";
  mode: Mode;
  tier: string;
  reason: string;
  checkedAt: number;
  check: () => Promise<void>;
}

export const useEntitlement = create<EntState>((set) => ({
  status: "checking",
  mode: "free",
  tier: "",
  reason: "",
  checkedAt: 0,

  check: async () => {
    const session = useAuth.getState().session;
    // no backend wired / not signed in → dormant: entitled. (the app stays free until a real
    // `entitlement` function is deployed and the server flips to paid.)
    if (!supabase || !session) {
      set({ status: "ok", mode: "free", reason: "", checkedAt: Date.now() });
      return;
    }
    try {
      const { data, error } = await supabase.functions.invoke(ENTITLEMENT_FN);
      if (error || !data) throw error ?? new Error("no entitlement");
      const r = await resolveResult(data as EntResult);
      set({
        status: r.mode === "paid" && !r.entitled ? "locked" : "ok",
        mode: r.mode,
        tier: r.tier,
        reason: (data as EntResult).reason ?? "",
        checkedAt: Date.now(),
      });
    } catch {
      // backend absent/unreachable → use a cached signed token within grace, else FAIL OPEN
      const cached = await fromCache();
      if (cached) {
        set({
          status: cached.mode === "paid" && !cached.entitled ? "locked" : "ok",
          mode: cached.mode,
          tier: cached.tier,
          checkedAt: Date.now(),
        });
      } else {
        set({ status: "ok", mode: "free", checkedAt: Date.now() });
      }
    }
  },
}));
