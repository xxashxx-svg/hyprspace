//! `cargo run -p hyprspace-update --example verify -- <file>`: checks `<file>` against
//! `<file>.sig` with the updater key the app ships. Release CI runs it right after signing, so a
//! secret that drifted from the app's key fails the build instead of every user's update.

use anyhow::Context as _;

fn main() -> anyhow::Result<()> {
    let file = std::env::args()
        .nth(1)
        .context("usage: verify <file> (its signature is <file>.sig)")?;
    let bytes = std::fs::read(&file).with_context(|| format!("reading {file}"))?;
    let sig = std::fs::read_to_string(format!("{file}.sig"))
        .with_context(|| format!("reading {file}.sig"))?;
    hyprspace_update::verify(&bytes, &sig)?;
    println!("{file}: the signature matches the app's updater key");
    Ok(())
}
