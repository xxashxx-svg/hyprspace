// Settings, Usage, Activity: what each agent has done, from its own files on this machine.

use gpui::{AnyElement, FontWeight, Hsla, IntoElement, div, prelude::*, px, relative};
use hyprspace_proto::usage::ProviderUsage;

use super::page::*;
use super::{Limits, PROVIDERS, brand};
use crate::colors;

/// "claude-opus-4-8" is "Opus 4.8", "claude-3-5-sonnet-20241022" is "Sonnet 3.5", and
/// "gpt-5.6-terra" is "GPT-5.6 Terra". Other ids pass through.
pub(super) fn pretty_model(id: &str) -> String {
    if let Some(rest) = id.strip_prefix("gpt-") {
        let mut parts = rest.split('-');
        let version = parts.next().unwrap_or_default();
        let words: Vec<String> = parts
            .map(|w| {
                let mut c = w.chars();
                c.next().map_or_else(String::new, |f| {
                    f.to_uppercase().collect::<String>() + c.as_str()
                })
            })
            .collect();
        return [format!("GPT-{version}")]
            .into_iter()
            .chain(words)
            .collect::<Vec<_>>()
            .join(" ");
    }
    let Some(rest) = id.strip_prefix("claude-") else {
        return id.to_string();
    };
    let parts: Vec<&str> = rest
        .split('-')
        .filter(|p| !(p.len() == 8 && p.chars().all(|c| c.is_ascii_digit())))
        .collect();
    let digits = |p: &str| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit());
    let words: Vec<String> = parts
        .iter()
        .filter(|p| !digits(p))
        .map(|w| {
            let mut c = w.chars();
            c.next().map_or_else(String::new, |f| {
                f.to_uppercase().collect::<String>() + c.as_str()
            })
        })
        .collect();
    let nums: Vec<&str> = parts.iter().copied().filter(|p| digits(p)).collect();
    let name = if words.is_empty() {
        id.to_string()
    } else {
        words.join(" ")
    };
    if nums.is_empty() {
        name
    } else {
        format!("{name} {}", nums.join("."))
    }
}

fn skeleton(id: &str, label: &str) -> AnyElement {
    let reading = if id == "claude" {
        "Reading its history. A long one takes a while."
    } else {
        "Reading local files"
    };
    let bar = |h: f32| div().h(px(h)).rounded(px(9.)).bg(colors::ink(0.06));
    card(id, label, None, Some(reading.into()))
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(10.))
                .p(px(16.))
                .child(bar(48.))
                .child(bar(34.)),
        )
        .into_any_element()
}

/// Tokens count input and output only, the way Claude's own /usage does. Cache reads dwarf real
/// work and cost a fraction as much, so they get their own line instead of swamping the bar.
fn tokens(u: &ProviderUsage, tint: Hsla) -> AnyElement {
    let real = u.input_tokens + u.output_tokens;
    let in_pct = if real > 0 {
        u.input_tokens as f32 / real as f32
    } else {
        0.0
    };
    let mut d = div().flex().flex_col().gap(px(10.)).p(px(16.)).child(
        div()
            .flex()
            .items_end()
            .justify_between()
            .gap(px(12.))
            .child(big(short(real), "tokens", colors::text1()))
            .children(u.tokens_window.clone().map(dim)),
    );
    if real > 0 {
        let swatch = |c: Hsla| div().size(px(8.)).rounded(px(2.)).bg(c);
        let out = tint.opacity(0.42);
        d = d
            .child(
                div()
                    .flex()
                    .gap(px(2.))
                    .h(px(6.))
                    .rounded(px(3.))
                    .overflow_hidden()
                    .bg(colors::ink(0.05))
                    .when(u.input_tokens > 0, |d| {
                        d.child(div().h_full().min_w(px(3.)).w(relative(in_pct)).bg(tint))
                    })
                    .when(u.output_tokens > 0, |d| {
                        d.child(div().h_full().min_w(px(3.)).flex_1().bg(out))
                    }),
            )
            .child(
                div()
                    .flex()
                    .gap(px(14.))
                    .text_size(px(11.5))
                    .text_color(colors::text3())
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .child(swatch(tint))
                            .child(format!("{} in", short(u.input_tokens))),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .child(swatch(out))
                            .child(format!("{} out", short(u.output_tokens))),
                    )
                    .when(u.cache_tokens > 0, |d| {
                        d.child(
                            div()
                                .ml_auto()
                                .opacity(0.8)
                                .child(format!("+ {} read from cache", short(u.cache_tokens))),
                        )
                    }),
            );
    }
    d.into_any_element()
}

fn activity_card(u: &ProviderUsage) -> AnyElement {
    let tint = brand(&u.id);
    let counts: Vec<(&str, u64)> = [
        ("Sessions", u.sessions),
        ("Messages", u.messages),
        ("Tool calls", u.tool_calls),
        ("Active days", u.active_days),
    ]
    .into_iter()
    .filter(|c| c.1 > 0)
    .collect();
    let mut items: Vec<AnyElement> = Vec::new();
    if u.total_tokens > 0 {
        items.push(tokens(u, tint));
    }
    if !counts.is_empty() {
        items.push(
            div()
                .flex()
                .flex_wrap()
                .pt(px(12.))
                .pb(px(14.))
                .children(counts.into_iter().enumerate().map(|(i, (label, n))| {
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(1.))
                        .pl(px(16.))
                        .pr(px(18.))
                        .when(i > 0, |d| d.border_l_1().border_color(colors::border1()))
                        .child(
                            div()
                                .text_size(px(11.5))
                                .text_color(colors::text3())
                                .child(label),
                        )
                        .child(
                            div()
                                .text_size(px(17.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(colors::text1())
                                .child(grouped(n)),
                        )
                }))
                .into_any_element(),
        );
    }
    card(
        &u.id,
        &u.label,
        u.plan.clone(),
        u.account.as_deref().map(masked),
    )
    .child(rows(items))
    .children(u.note.clone().map(note))
    .into_any_element()
}

impl Limits {
    /// `top` (the overview, tokens per day and the models), then a card per agent.
    pub(super) fn activity_view(&self, top: Vec<AnyElement>) -> Vec<AnyElement> {
        let data: Vec<&ProviderUsage> = PROVIDERS
            .iter()
            .filter_map(|(id, _)| self.local.get(*id))
            .collect();
        let off: Vec<&&ProviderUsage> = data.iter().filter(|u| !u.signed_in).collect();
        let mut out = top;
        for (id, label) in PROVIDERS {
            match self.local.get(id) {
                Some(u) if u.signed_in => out.push(activity_card(u)),
                None if self.pending.contains(id) => out.push(skeleton(id, label)),
                _ => {}
            }
        }
        if !off.is_empty() {
            let names: Vec<&str> = off.iter().map(|u| u.label.as_str()).collect();
            out.push(foot_text(&format!(
                "Not signed in on this machine: {}.",
                names.join(", ")
            )));
        }
        if !data.is_empty() {
            out.push(foot_text(
                "Everything here comes from each tool's own files on this machine. No network calls, no tokens spent.",
            ));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_ids_read_like_their_names() {
        assert_eq!(pretty_model("claude-opus-4-8"), "Opus 4.8");
        assert_eq!(pretty_model("claude-haiku-4-5-20251001"), "Haiku 4.5");
        assert_eq!(pretty_model("claude-3-5-sonnet-20241022"), "Sonnet 3.5");
        assert_eq!(pretty_model("gpt-6-luna"), "GPT-6 Luna");
        assert_eq!(pretty_model("gpt-5.6-terra"), "GPT-5.6 Terra");
        assert_eq!(pretty_model("gpt-5.5"), "GPT-5.5");
        assert_eq!(pretty_model("codex"), "codex");
    }
}
