# Subscription gating (free → paid)

How to flip HyprSpace from free to subscription-only **without shipping an app update** — including
for users who already have it installed.

## The idea
The app already ships (since this was added) with a **dormant** entitlement check: on launch and
periodically, it asks a backend "is this account entitled?". While free, there's no backend (or it
says `free`), so the check **fails open** — everyone is entitled, no paywall. Going paid is a
**server change**: stand up the backend + flip a mode, and every installed app enforces it on its
next check. No new build, no forced update.

## Client pieces (already in the build)
| File | Role |
|---|---|
| `src/stores/entitlement.ts` | the check: calls the backend, verifies + caches a signed token, computes locked/ok. **Fail-open.** |
| `src/components/EntitlementGate.tsx` | wraps the app (inside `AuthGate`); shows the paywall only on an explicit `locked` |
| `src/components/Paywall.tsx` | the "subscription required" screen (Subscribe / re-check / sign out) |
| `src-tauri/src/license.rs` → `entitlement_verify` | Ed25519-verifies a signed entitlement token offline (same key as licenses) |

Knobs to set when going paid:
- `ENTITLEMENT_FN` (entitlement.ts) — the Supabase Edge Function name (default `entitlement`).
- `SUBSCRIBE_URL` (Paywall.tsx) — your Polar checkout link / storefront.
- `OFFLINE_GRACE_MS` (entitlement.ts) — how long a cached signed token is trusted offline (7d).
- `RECHECK_MS` (EntitlementGate.tsx) — re-check cadence while open (6h).

## Backend contract
The app calls a **Supabase Edge Function** (it already has the Supabase client + the user's session,
so the function gets the authed user for free). Return JSON:
```json
{ "mode": "free|paid", "entitled": true, "tier": "pro", "reason": "…", "token": "HSENT-…" }
```
- `mode` — global switch. `free` = nobody is gated. `paid` = gate non-entitled accounts.
- `entitled` — is *this* account entitled (active subscription, trial, grandfathered, …).
- `token` *(recommended)* — a signed entitlement so the verdict is **tamper-resistant** and works
  offline. Without it the plain JSON is trusted (fine to start; add the token before launch).

### Entitlement token format
`HSENT-<base64url(payload)>.<base64url(ed25519 sig)>` where the signature is over the ASCII bytes of
the payload segment, and payload is:
```json
{ "uid": "<supabase user id>", "tier": "pro", "mode": "paid", "exp": 1730000000 }
```
- `exp` = unix seconds (0 = no expiry). The client treats an expired token as not entitled.
- Sign with the **same key as licenses**: `~/.hyprspace-signing/hyprspace-license.pem` (verified by
  the embedded `LICENSE_PUBKEY` in `license.rs`). So no new key to manage.
- Keep `exp` short-ish (e.g. 7 days) and re-issue on each check — that bounds how long a now-expired
  subscriber keeps offline access after you flip to paid.

## Going-paid procedure
1. **Polar:** create a subscription product + a checkout link (Polar is a merchant of record, so it
   handles tax/VAT for you). Pass your Supabase user id as `customer_external_id` when creating the
   checkout so webhook events map back to the account. On Polar webhooks (`subscription.active` /
   `subscription.canceled` / `subscription.revoked`, or the `customer.state_changed` snapshot),
   verify the webhook signature and upsert the user's status into Supabase (e.g. a `subscriptions`
   table keyed by `uid`). Point `SUBSCRIBE_URL` at the Polar checkout link / storefront.
2. **Edge Function `entitlement`:** reads the caller's `uid` from the Supabase JWT, looks up their
   subscription status, and returns the JSON above. While testing, have it return
   `{ "mode": "free", "entitled": true }` — still nobody is gated.
3. **(Recommended) sign tokens:** in the function, mint an `HSENT-…` token for entitled accounts
   (load the private key from a function secret) and include it in `token`.
4. **Flip the switch:** change the function to return `mode: "paid"` and real `entitled` per account.
   Existing installs lock out non-subscribers on their next check (within `RECHECK_MS`, or
   `OFFLINE_GRACE_MS` if they were offline with a cached token).
5. **(Optional) grandfather** early users: in the function, return `entitled: true` for accounts
   created before your cutoff date.

## Behavior & caveats
- **Fail-open by design:** no backend / no session / network error / unverifiable token → entitled.
  This guarantees the free phase works and avoids locking people out on transient errors. The
  tradeoff: a determined user who blocks the network keeps access. If you want fail-*closed* after
  the grace window for paid mode, tighten the `catch` branch in `entitlement.ts`. Don't make it
  fail-closed on every error or an outage locks out paying customers.
- **Client gates are bypassable** by patching the binary — true of every desktop app. The
  account+server model raises the bar; tie real server-side features to entitlement for stronger
  protection. This is separate from the user's *Claude* subscription (that stays the spawn-the-CLI
  path, untouched).
- **Lag:** the flip takes effect on each app's next check (`RECHECK_MS`) — up to `OFFLINE_GRACE_MS`
  for offline users with a cached token. Shorten those for faster enforcement (at the cost of more
  network dependence).
- **⚠️ Supabase RLS must be enforced** regardless — the anon key ships in the binary. See
  [audit/security.md](./audit/security.md).
