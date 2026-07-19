**What changed**


**Why**
<!-- Link the issue if there is one. -->


**How you verified it**
<!-- What you actually ran/clicked. Screenshots for UI changes. -->


**Checklist**
- [ ] `npx tsc --noEmit` passes
- [ ] `cargo check` passes in `src-tauri/` (if you touched Rust)
- [ ] No version numbers hand-edited (`tauri.conf.json` / `package.json` / `Cargo.toml` — `deploy.ps1` owns those)
- [ ] No Tailwind / CSS-in-JS added to `src/` — styling uses the tokens in `src/styles/tokens.css`
- [ ] Diff is scoped to this change (no drive-by reformatting)
