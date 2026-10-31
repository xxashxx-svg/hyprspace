use hyprspace_proto::Tool;

use super::model::Item;
use super::{Status, TranscriptView, ask, tool};

const LONGEST: usize = 140;

impl TranscriptView {
    pub fn notice(&self, status: Status) -> Option<String> {
        let items = &self.model.items;
        let from = items
            .iter()
            .rposition(|i| matches!(i, Item::User { steer: false, .. }))
            .unwrap_or(0);
        let run = &items[from..];
        let said = match status {
            Status::Waiting => items.iter().find_map(|i| match i {
                Item::Approval {
                    tool,
                    answer: None,
                    expired: false,
                    ..
                } => Some(asks(tool)),
                _ => None,
            }),
            Status::Failed => run.iter().rev().find_map(|i| match i {
                Item::Finished { error: Some(e), .. } | Item::Error(e) => Some(e.clone()),
                _ => None,
            }),
            Status::Done => run
                .iter()
                .rev()
                .find_map(|i| match i {
                    Item::Changes { files, .. } if files.len() == 1 => Some("Edited 1 file".into()),
                    Item::Changes { files, .. } if !files.is_empty() => {
                        Some(format!("Edited {} files", files.len()))
                    }
                    _ => None,
                })
                .or_else(|| Some(self.model.last_reply().replace(['*', '`', '#'], ""))),
            Status::Idle | Status::Working => None,
        }?;
        let line = said.lines().map(str::trim).find(|l| !l.is_empty())?;
        Some(clip(line))
    }
}

fn asks(t: &Tool) -> String {
    match ask::named(t) {
        Some((ask::QUESTION, input)) => ask::questions(input)
            .into_iter()
            .next()
            .map(|q| q.text)
            .unwrap_or_else(|| "It has a question.".into()),
        Some((ask::PLAN, _)) => "It has a plan for you to review.".into(),
        _ => format!("{}?", tool::label(t)),
    }
}

fn clip(line: &str) -> String {
    match line.char_indices().nth(LONGEST) {
        Some((at, _)) => format!("{}...", line[..at].trim_end()),
        None => line.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_long_line_is_clipped_on_a_char() {
        assert_eq!(clip("short"), "short");
        let long = "é".repeat(200);
        let clipped = clip(&long);
        assert!(clipped.ends_with("...") && clipped.chars().count() == LONGEST + 3);
    }
}
