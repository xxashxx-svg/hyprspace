// The file behind Claude Code's `[Image #N]` marker in a terminal, for the hover preview. Ported
// from the Tauri app's src-tauri/src/lib.rs (claude_image_path), which learned these rules the
// hard way.
//
// Up to CLI 2.1.272 a pasted image was written to ~/.claude/image-cache/<conversation>/<N>.<ext>.
// Newer CLIs keep it only as base64 inside the conversation's transcript, so the image is read
// back out of the .jsonl and saved under ~/.hyprspace/image-cache, where the next look finds it.

use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use base64::Engine as _;
use serde_json::Value;

use crate::sessions::claude_project_dir;

/// Transcripts run to hundreds of MB; a marker that matters is near the end.
const TAIL: u64 = 64 * 1024 * 1024;
/// How many lines holding the marker and an image are parsed before giving up.
const PARSES: usize = 32;

/// The image file for marker `n` in the conversation `conversation` held in `cwd`, or None.
pub fn find(home: &Path, cwd: &Path, conversation: Option<&str>, n: u32) -> Option<PathBuf> {
    if let Some(p) = old_cache(home, cwd, conversation, n) {
        return Some(p);
    }
    let transcript = transcript(home, cwd, conversation)?;
    let stem = transcript.file_stem()?.to_str()?.to_string();
    from_transcript(home, &stem, n, &transcript)
}

/// The conversation's transcript. The thread pins its own id, so that file is the right one;
/// only guess (the newest in the folder) when it isn't there, e.g. after a /clear forked it.
fn transcript(home: &Path, cwd: &Path, conversation: Option<&str>) -> Option<PathBuf> {
    let dir = claude_project_dir(home, cwd);
    if let Some(c) = conversation {
        let own = dir.join(format!("{c}.jsonl"));
        if own.is_file() {
            return Some(own);
        }
    }
    std::fs::read_dir(&dir)
        .ok()?
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "jsonl"))
        .filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path())))
        .max_by_key(|(m, _)| *m)
        .map(|(_, p)| p)
}

/// Older CLIs' own cache: ~/.claude/image-cache/<conversation>/<n>.<ext>.
fn old_cache(home: &Path, cwd: &Path, conversation: Option<&str>, n: u32) -> Option<PathBuf> {
    let cache = home.join(".claude").join("image-cache");
    let dir = match conversation.map(|c| cache.join(c)).filter(|d| d.is_dir()) {
        Some(d) => d,
        None => cache.join(transcript(home, cwd, conversation)?.file_stem()?),
    };
    let want = n.to_string();
    std::fs::read_dir(&dir).ok()?.flatten().find_map(|e| {
        let p = e.path();
        let named = p.file_stem().and_then(|s| s.to_str()) == Some(want.as_str());
        // an empty file is one Claude is still writing
        let written = e.metadata().is_ok_and(|m| m.is_file() && m.len() > 0);
        (named && written).then_some(p)
    })
}

/// The marker's image decoded out of the transcript, or the last decode while it still holds. A
/// saved file is good until the transcript changes: after /clear the markers restart at 1, so an
/// older file is thrown away rather than trusted.
fn from_transcript(home: &Path, stem: &str, n: u32, transcript: &Path) -> Option<PathBuf> {
    let dir = home.join(".hyprspace").join("image-cache").join(stem);
    let written = std::fs::metadata(transcript).ok()?.modified().ok()?;
    let want = n.to_string();
    if let Ok(rd) = std::fs::read_dir(&dir) {
        for e in rd.flatten() {
            let p = e.path();
            if p.file_stem().and_then(|s| s.to_str()) != Some(want.as_str()) {
                continue;
            }
            let meta = e.metadata().ok();
            let fresh = meta
                .as_ref()
                .and_then(|m| m.modified().ok())
                .is_some_and(|m| m >= written);
            if fresh && meta.is_some_and(|m| m.len() > 0) {
                return Some(p);
            }
            let _ = std::fs::remove_file(&p);
        }
    }
    let (bytes, ext) = image_bytes(transcript, n)?;
    std::fs::create_dir_all(&dir).ok()?;
    let out = dir.join(format!("{n}.{ext}"));
    std::fs::write(&out, bytes).ok()?;
    Some(out)
}

/// Only the transcript's tail is read, and only lines holding the marker and an image are parsed.
/// Newest first: the same message is written more than once, and the marker also turns up where
/// no image is attached (an agent quoting it), which just doesn't match.
fn image_bytes(transcript: &Path, n: u32) -> Option<(Vec<u8>, &'static str)> {
    let mut f = std::fs::File::open(transcript).ok()?;
    let len = f.metadata().ok()?.len();
    let from = len.saturating_sub(TAIL);
    f.seek(SeekFrom::Start(from)).ok()?;
    let mut buf = Vec::with_capacity((len - from) as usize);
    f.read_to_end(&mut buf).ok()?;
    if from > 0 {
        // the first line is cut in half and would never parse
        let cut = buf.iter().position(|&b| b == b'\n')?;
        buf.drain(..=cut);
    }
    let needle = format!("[Image #{n}]").into_bytes();
    let hits: Vec<usize> = buf
        .windows(needle.len())
        .enumerate()
        .filter(|(_, w)| *w == needle.as_slice())
        .map(|(i, _)| i)
        .collect();
    let mut parsed = 0;
    for &at in hits.iter().rev() {
        let start = buf[..at]
            .iter()
            .rposition(|&b| b == b'\n')
            .map_or(0, |i| i + 1);
        let end = buf[at..]
            .iter()
            .position(|&b| b == b'\n')
            .map_or(buf.len(), |i| at + i);
        let line = &buf[start..end];
        // most lines with the marker are an agent quoting it, with nothing attached
        if !line.windows(7).any(|w| w == b"\"image\"") {
            continue;
        }
        parsed += 1;
        if parsed > PARSES {
            break;
        }
        if let Some(hit) = image_for(line, n) {
            return Some(hit);
        }
    }
    None
}

/// Markers and images pair up by position inside one message: the first marker in the text is
/// the first attachment. By position, not number: deleting a paste leaves a gap in the numbers
/// while the attachments stay in step.
fn image_for(line: &[u8], n: u32) -> Option<(Vec<u8>, &'static str)> {
    let v: Value = serde_json::from_slice(line).ok()?;
    let parts = v.get("message")?.get("content")?.as_array()?;
    let mut marks: Vec<u32> = Vec::new();
    let mut images: Vec<&Value> = Vec::new();
    for p in parts {
        match p.get("type").and_then(Value::as_str) {
            Some("text") => marks.extend(markers(p.get("text")?.as_str()?)),
            Some("image") => images.push(p.get("source")?),
            _ => {}
        }
    }
    if marks.len() != images.len() {
        return None;
    }
    let src = images[marks.iter().position(|m| *m == n)?];
    if src.get("type").and_then(Value::as_str) != Some("base64") {
        return None;
    }
    let ext = match src.get("media_type").and_then(Value::as_str) {
        Some("image/jpeg") => "jpg",
        Some("image/gif") => "gif",
        Some("image/webp") => "webp",
        _ => "png",
    };
    let data = src.get("data")?.as_str()?;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(data)
        .ok()?;
    Some((bytes, ext))
}

/// The numbers of every `[Image #N]` in `text`, in order.
pub fn markers(text: &str) -> Vec<u32> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(i) = rest.find("[Image #") {
        let after = &rest[i + "[Image #".len()..];
        let Some(close) = after.find(']') else {
            break;
        };
        if let Ok(n) = after[..close].parse() {
            out.push(n);
        }
        rest = &after[close + 1..];
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn b64(bytes: &[u8]) -> String {
        base64::engine::general_purpose::STANDARD.encode(bytes)
    }

    /// A home with one conversation in `cwd` whose transcript holds `lines`.
    fn home_with(cwd: &Path, id: &str, lines: &[Value]) -> tempfile::TempDir {
        let home = tempfile::tempdir().unwrap();
        let dir = claude_project_dir(home.path(), cwd);
        std::fs::create_dir_all(&dir).unwrap();
        let text: Vec<String> = lines.iter().map(Value::to_string).collect();
        std::fs::write(dir.join(format!("{id}.jsonl")), text.join("\n")).unwrap();
        home
    }

    fn message(text: &str, images: &[(&str, &[u8])]) -> Value {
        let mut content = vec![json!({ "type": "text", "text": text })];
        for (media, bytes) in images {
            content.push(json!({ "type": "image",
                "source": { "type": "base64", "media_type": media, "data": b64(bytes) } }));
        }
        json!({ "type": "user", "message": { "role": "user", "content": content } })
    }

    #[test]
    fn markers_read_in_order_and_skip_junk() {
        assert_eq!(
            markers("a [Image #3] b [Image #x] [Image #12]"),
            vec![3, 12]
        );
        assert!(markers("[Image #4").is_empty());
    }

    #[test]
    fn a_marker_reads_its_own_image_out_of_the_transcript_and_saves_it() {
        let cwd = Path::new("/w/game");
        let home = home_with(
            cwd,
            "c1",
            &[
                message(
                    "look [Image #1] and [Image #3]",
                    &[("image/png", b"one"), ("image/jpeg", b"three")],
                ),
                // an agent quoting the marker, with nothing attached
                json!({ "type": "assistant", "message": { "content": [
                    { "type": "text", "text": "I see [Image #3]" } ] } }),
            ],
        );
        let three = find(home.path(), cwd, Some("c1"), 3).unwrap();
        assert_eq!(three.extension().unwrap(), "jpg");
        assert_eq!(std::fs::read(&three).unwrap(), b"three");
        assert!(three.starts_with(home.path().join(".hyprspace").join("image-cache")));
        // the second look finds the saved file
        assert_eq!(find(home.path(), cwd, Some("c1"), 3), Some(three));
        assert_eq!(find(home.path(), cwd, Some("c1"), 2), None);
    }

    #[test]
    fn the_old_cache_wins_and_a_missing_conversation_falls_back_to_the_newest() {
        let cwd = Path::new("/w/game");
        let home = home_with(
            cwd,
            "c1",
            &[message("[Image #1]", &[("image/png", b"new")])],
        );
        let old = home.path().join(".claude").join("image-cache").join("c1");
        std::fs::create_dir_all(&old).unwrap();
        std::fs::write(old.join("1.png"), b"old").unwrap();
        assert_eq!(
            find(home.path(), cwd, Some("c1"), 1),
            Some(old.join("1.png"))
        );
        // an id with no transcript of its own reads the folder's newest
        let got = find(home.path(), cwd, Some("gone"), 1).unwrap();
        assert_eq!(std::fs::read(got).unwrap(), b"old");
    }
}
