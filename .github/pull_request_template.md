**What changed**


**Why**
<!-- Link the issue if there is one. -->


**How you verified it**
<!-- What you actually ran/clicked. Screenshots for UI changes. -->


**Checklist**
- [ ] `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings` and `cargo test --workspace --locked` pass
- [ ] No version numbers hand-edited (`deploy.ps1` owns the workspace version in `Cargo.toml` and `Cargo.lock`)
- [ ] No hard-coded colors in `crates/ui`: styling uses the tokens in `crates/theme`
- [ ] Diff is scoped to this change (no drive-by reformatting)
