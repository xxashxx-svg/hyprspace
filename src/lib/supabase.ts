// Supabase client for account sign-in (Google). Both values come from the environment — copy
// .env.example to .env to point at your own project. The anon key is a PUBLIC (publishable) key:
// it's meant to ship in the client and row-level security guards the data.
//
// Leave them unset and `supabase` is null — AuthGate then falls open and the app runs without
// sign-in, so a fresh clone works out of the box.
import type { SupabaseClient } from "@supabase/supabase-js";

const SUPABASE_URL = import.meta.env.VITE_SUPABASE_URL ?? "";
const SUPABASE_ANON_KEY = import.meta.env.VITE_SUPABASE_ANON_KEY ?? "";

export const supabaseReady = Boolean(SUPABASE_URL && SUPABASE_ANON_KEY);

// supabase-js serializes auth calls through the navigator LockManager; some webview origins
// (notably the http://localhost dev origin) can wedge on it, hanging getSession() forever. In dev
// we're a single window with no concurrency, so just run the operation directly. Prod keeps the
// real lock (it works there) so multi-call refreshes stay serialized.
const passthroughLock = async <R>(_name: string, _acquireTimeout: number, fn: () => Promise<R>): Promise<R> =>
  fn();

let client: Promise<SupabaseClient | null> | undefined;

/**
 * The client, built on first use rather than at import.
 *
 * supabase-js is ~210KB and nothing on the launch path needs it: HyprSpace runs your own CLIs on
 * your own machine and an account is never required, so signing in is something you go and do from
 * Settings. Importing it dynamically keeps it, and the auth refresh timer `createClient` starts,
 * off cold start entirely. Memoised, so there is still exactly one client per run.
 *
 * A failed import isn't cached: the memo is cleared so the next attempt can try again.
 */
export function getSupabase(): Promise<SupabaseClient | null> {
  if (!supabaseReady) return Promise.resolve(null);
  client ??= import("@supabase/supabase-js")
    .then(({ createClient }) =>
      createClient(SUPABASE_URL, SUPABASE_ANON_KEY, {
        auth: {
          // desktop OAuth: the browser sends the code back to our localhost loopback, and we
          // exchange it ourselves — so don't try to read it from the window URL.
          flowType: "pkce",
          persistSession: true,
          autoRefreshToken: true,
          detectSessionInUrl: false,
          ...(import.meta.env.DEV ? { lock: passthroughLock } : {}),
        },
      }),
    )
    .catch((e) => {
      client = undefined;
      console.error("supabase failed to load:", e);
      return null;
    });
  return client;
}
