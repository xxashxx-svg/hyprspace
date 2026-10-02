// The stdin lines the Claude harness writes: user messages (with inline images), steers,
// approval answers and the interrupt request. Shapes follow zeron's claude/wire.rs (MIT, see
// THIRD_PARTY_NOTICES.md), checked against claude 2.1.287.

use std::path::{Path, PathBuf};

use base64::Engine as _;
use hyprspace_proto::Prompt;
use serde_json::{Value, json};

/// The API takes inline images up to 5 MB; a bigger or unknown file is named in the text instead,
/// so the agent can still open it with its Read tool.
const MAX_IMAGE: usize = 5 * 1024 * 1024;

fn media_type(path: &Path, bytes: &[u8]) -> Option<&'static str> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase);
    match ext.as_deref() {
        Some("png") => return Some("image/png"),
        Some("jpg" | "jpeg") => return Some("image/jpeg"),
        Some("gif") => return Some("image/gif"),
        Some("webp") => return Some("image/webp"),
        _ => {}
    }
    // pasted screenshots can carry odd names, so the magic bytes decide
    match bytes {
        [0x89, b'P', b'N', b'G', ..] => Some("image/png"),
        [0xFF, 0xD8, 0xFF, ..] => Some("image/jpeg"),
        [b'G', b'I', b'F', b'8', ..] => Some("image/gif"),
        [
            b'R',
            b'I',
            b'F',
            b'F',
            _,
            _,
            _,
            _,
            b'W',
            b'E',
            b'B',
            b'P',
            ..,
        ] => Some("image/webp"),
        _ => None,
    }
}

/// A user message's `content`: the plain text, or the images as base64 blocks then the text.
pub(crate) fn content(prompt: &Prompt) -> Value {
    if prompt.images.is_empty() {
        return Value::String(prompt.text.clone());
    }
    let mut blocks = Vec::new();
    let mut skipped: Vec<&PathBuf> = Vec::new();
    for path in &prompt.images {
        // a few MB at most, read on the session's task
        let bytes = std::fs::read(path).unwrap_or_default();
        match media_type(path, &bytes) {
            Some(media) if !bytes.is_empty() && bytes.len() <= MAX_IMAGE => blocks.push(json!({
                "type": "image",
                "source": {
                    "type": "base64",
                    "media_type": media,
                    "data": base64::engine::general_purpose::STANDARD.encode(&bytes),
                },
            })),
            _ => skipped.push(path),
        }
    }
    let mut text = prompt.text.clone();
    for path in skipped {
        text.push_str(&format!("\n\nAttached file: {}", path.display()));
    }
    blocks.push(json!({ "type": "text", "text": text }));
    Value::Array(blocks)
}

pub(crate) fn user_line(content: Value) -> String {
    json!({
        "type": "user",
        "message": { "role": "user", "content": content },
        "parent_tool_use_id": null,
    })
    .to_string()
}

/// A message sent mid-run. `now` stops streaming text and answers the steer next; it also aborts
/// a running tool, so while one is open the steer goes as `next` and lands after its result.
/// The CLI echoes the line back with this `uuid` (--replay-user-messages) once it takes it.
pub(crate) fn steer_line(content: Value, uuid: &str, now: bool) -> String {
    json!({
        "type": "user",
        "uuid": uuid,
        "priority": if now { "now" } else { "next" },
        "message": { "role": "user", "content": content },
        "parent_tool_use_id": null,
    })
    .to_string()
}

fn control_response(request_id: &str, response: Value) -> String {
    json!({
        "type": "control_response",
        "response": { "subtype": "success", "request_id": request_id, "response": response },
    })
    .to_string()
}

/// The answer to a `can_use_tool` request. Allowing hands the tool's input back unchanged.
pub(crate) fn answer_line(request_id: &str, input: Value, allow: bool) -> String {
    let response = if allow {
        json!({ "behavior": "allow", "updatedInput": input })
    } else {
        json!({ "behavior": "deny", "message": "The user denied this." })
    };
    control_response(request_id, response)
}

pub(crate) fn interrupt_line(request_id: &str) -> String {
    json!({
        "type": "control_request",
        "request_id": request_id,
        "request": { "subtype": "interrupt" },
    })
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(line: &str) -> Value {
        assert!(!line.contains('\n'), "one line per message");
        serde_json::from_str(line).unwrap()
    }

    #[test]
    fn plain_prompts_stay_strings_and_images_go_first() {
        let v = parse(&user_line(content(&Prompt::text("hi\nthere"))));
        assert_eq!(v["type"], "user");
        assert_eq!(v["message"]["content"], "hi\nthere");

        let dir = tempfile::tempdir().unwrap();
        let png = dir.path().join("shot.dat");
        std::fs::write(&png, [0x89, b'P', b'N', b'G', 0, 0]).unwrap();
        let prompt = Prompt {
            text: "what is this?".into(),
            images: vec![png, dir.path().join("missing.png")],
        };
        let v = parse(&user_line(content(&prompt)));
        let blocks = v["message"]["content"].as_array().unwrap();
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0]["source"]["media_type"], "image/png");
        assert_eq!(blocks[0]["source"]["data"], "iVBORwAA");
        let text = blocks[1]["text"].as_str().unwrap();
        assert!(text.starts_with("what is this?"));
        assert!(text.contains("missing.png"), "{text}");
    }

    #[test]
    fn control_lines_match_the_protocol() {
        let v = parse(&steer_line(json!("go"), "u1", false));
        assert_eq!(
            (v["priority"].as_str(), v["uuid"].as_str()),
            (Some("next"), Some("u1"))
        );
        let v = parse(&answer_line("r1", json!({"command": "ls"}), true));
        assert_eq!(v["response"]["request_id"], "r1");
        assert_eq!(v["response"]["response"]["behavior"], "allow");
        assert_eq!(v["response"]["response"]["updatedInput"]["command"], "ls");
        let v = parse(&answer_line("r2", Value::Null, false));
        assert_eq!(v["response"]["response"]["behavior"], "deny");
        let v = parse(&interrupt_line("i1"));
        assert_eq!(v["request"]["subtype"], "interrupt");
    }
}
