// Account auth via Supabase — email/password (with a one-time email code to verify the address)
// plus Google. Desktop Google OAuth: signInWithOAuth (PKCE) → system browser → loopback (:8765)
// catches ?code= → exchangeCodeForSession.
import { create } from "zustand";
import { openUrl } from "@tauri-apps/plugin-opener";
import type { Session, User } from "@supabase/supabase-js";
import { supabase } from "../lib/supabase";
import { oauthListen } from "../api";

interface AuthState {
  ready: boolean; // finished the initial session check
  session: Session | null;
  user: User | null;
  signingIn: boolean;
  error: string;
  notice: string; // non-error message (e.g. "we emailed a code")
  pendingEmail: string | null; // email awaiting its verification code
  init: () => Promise<void>;
  signInWithGoogle: () => Promise<void>;
  signInWithEmail: (email: string, password: string) => Promise<void>;
  signUpWithEmail: (email: string, password: string) => Promise<void>;
  verifyCode: (token: string) => Promise<void>;
  resendCode: () => Promise<void>;
  cancelVerify: () => void;
  signOut: () => Promise<void>;
  clearMessages: () => void;
}

let subscribed = false;
const msg = (e: unknown) => (e as Error)?.message ?? String(e);

export const useAuth = create<AuthState>()((set, get) => ({
  ready: false,
  session: null,
  user: null,
  signingIn: false,
  error: "",
  notice: "",
  pendingEmail: null,

  clearMessages: () => set({ error: "", notice: "" }),
  cancelVerify: () => set({ pendingEmail: null, error: "", notice: "" }),

  init: async () => {
    if (!supabase) {
      set({ ready: true });
      return;
    }
    if (!subscribed) {
      subscribed = true;
      supabase.auth.onAuthStateChange((_event, session) => {
        set({ session, user: session?.user ?? null });
      });
    }
    try {
      // never let a slow/hung/throwing session check (a wedged storage lock, a network blip) brick
      // the app at the blank boot screen — time it out and fall through to the sign-in form.
      const timeout = new Promise<never>((_, rej) =>
        setTimeout(() => rej(new Error("session check timed out")), 8000),
      );
      const { data } = await Promise.race([supabase.auth.getSession(), timeout]);
      set({ session: data.session, user: data.session?.user ?? null });
    } catch (e) {
      console.error("auth init failed:", e);
    } finally {
      set({ ready: true }); // always become ready so the gate renders instead of a blank screen
    }
  },

  signInWithGoogle: async () => {
    if (!supabase) {
      set({ error: "Sign-in isn't configured." });
      return;
    }
    if (get().signingIn) return;
    set({ signingIn: true, error: "", notice: "" });
    try {
      const { data, error } = await supabase.auth.signInWithOAuth({
        provider: "google",
        options: { redirectTo: "http://localhost:8765", skipBrowserRedirect: true },
      });
      if (error) throw error;
      if (!data?.url) throw new Error("Couldn't start Google sign-in.");

      const capture = oauthListen(); // binds :8765, resolves when the browser redirects back
      await openUrl(data.url);
      const redirect = await capture; // e.g. "/?code=...&state=..."

      const qs = redirect.includes("?") ? redirect.slice(redirect.indexOf("?") + 1) : "";
      const params = new URLSearchParams(qs);
      const oauthErr = params.get("error_description") || params.get("error");
      if (oauthErr) throw new Error(oauthErr);

      // CSRF defense-in-depth: if the provider echoed a `state`, it must match the one we kicked
      // off with. The primary protection is PKCE — the code-verifier Supabase stashed on
      // signInWithOAuth — so the loopback can only redeem a code bound to *our* challenge.
      let expectedState: string | null = null;
      try {
        expectedState = new URL(data.url).searchParams.get("state");
      } catch {
        /* data.url should always parse; ignore if not */
      }
      const returnedState = params.get("state");
      if (expectedState && returnedState && expectedState !== returnedState) {
        throw new Error("Sign-in rejected: OAuth state mismatch.");
      }

      const code = params.get("code");
      if (!code) throw new Error("No authorization code returned.");

      const { error: exErr } = await supabase.auth.exchangeCodeForSession(code);
      if (exErr) throw exErr;
      // onAuthStateChange fires and sets the session
    } catch (e) {
      set({ error: msg(e) });
    } finally {
      set({ signingIn: false });
    }
  },

  signInWithEmail: async (email, password) => {
    if (!supabase) {
      set({ error: "Sign-in isn't configured." });
      return;
    }
    if (get().signingIn) return;
    set({ signingIn: true, error: "", notice: "" });
    try {
      const { error } = await supabase.auth.signInWithPassword({ email, password });
      if (error) {
        // signed up before but never verified → send a fresh code and switch to the verify step
        if (/not confirmed/i.test(error.message)) {
          await supabase.auth.resend({ type: "signup", email }).catch(() => {});
          set({ pendingEmail: email, notice: `Verify your email — we sent a code to ${email}.` });
          return;
        }
        throw error;
      }
      // onAuthStateChange sets the session
    } catch (e) {
      set({ error: msg(e) });
    } finally {
      set({ signingIn: false });
    }
  },

  signUpWithEmail: async (email, password) => {
    if (!supabase) {
      set({ error: "Sign-in isn't configured." });
      return;
    }
    if (get().signingIn) return;
    set({ signingIn: true, error: "", notice: "" });
    try {
      const { data, error } = await supabase.auth.signUp({ email, password });
      if (error) throw error;
      if (data.session) return; // confirmation disabled → already signed in
      // confirmation required → collect the emailed code
      set({ pendingEmail: email, notice: `We emailed a 6-digit code to ${email}.` });
    } catch (e) {
      set({ error: msg(e) });
    } finally {
      set({ signingIn: false });
    }
  },

  verifyCode: async (token) => {
    const email = get().pendingEmail;
    if (!email || !supabase || get().signingIn) return;
    set({ signingIn: true, error: "", notice: "" });
    try {
      const { error } = await supabase.auth.verifyOtp({ email, token, type: "signup" });
      if (error) throw error;
      set({ pendingEmail: null, notice: "" });
      // onAuthStateChange sets the session
    } catch (e) {
      set({ error: msg(e) });
    } finally {
      set({ signingIn: false });
    }
  },

  resendCode: async () => {
    const email = get().pendingEmail;
    if (!email || !supabase) return;
    set({ error: "", notice: "" });
    const { error } = await supabase.auth.resend({ type: "signup", email });
    if (error) set({ error: msg(error) });
    else set({ notice: `New code sent to ${email}.` });
  },

  signOut: async () => {
    if (supabase) await supabase.auth.signOut().catch(() => {});
    set({ session: null, user: null });
  },
}));
