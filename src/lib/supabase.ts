// Supabase client for account sign-in (Google). The anon key is a PUBLIC key — it's meant to ship
// in the client, and row-level security guards the data — so it's fine to keep here and commit it
// (Prince's build then talks to the same project). Fill these in after creating the project.
//
// Until both are set, `supabase` is null and the app falls back to the license-key gate, so the
// dev build keeps working while we get the project set up.
import { createClient, type SupabaseClient } from "@supabase/supabase-js";

const SUPABASE_URL = "https://cibfmxazhbcpazsdpipm.supabase.co";
const SUPABASE_ANON_KEY = "sb_publishable_N3_d6v34fPbNPm3BVaBxNA_OjreiDNH"; // publishable (public) key — safe to ship

export const supabaseReady = Boolean(SUPABASE_URL && SUPABASE_ANON_KEY);

export const supabase: SupabaseClient | null = supabaseReady
  ? createClient(SUPABASE_URL, SUPABASE_ANON_KEY, {
      auth: {
        // desktop OAuth: the browser sends the code back to our localhost loopback, and we
        // exchange it ourselves — so don't try to read it from the window URL.
        flowType: "pkce",
        persistSession: true,
        autoRefreshToken: true,
        detectSessionInUrl: false,
      },
    })
  : null;
