// Settings, Usage, Activity: what each agent has done, from its own files on this machine.

use gpui::{AnyElement, Div, FontWeight, Hsla, IntoElement, div, prelude::*, px, relative};
use hyprspace_proto::usage::{ModelUsage, ProviderUsage};

use super::page::*;
use super::{Limits, PROVIDERS, brand};
use crate::assets::provider_mark;
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

/// The summary over Activity: three figures split by hairlines.
fn strip(
    tokens: String,
    sessions: String,
    across: String,
    signed: usize,
    marks: Vec<(&str, bool)>,
) -> AnyElement {
    let stat = |label: &str, figure: AnyElement, foot: AnyElement| {
        div()
            .flex_1()
            .flex_basis(px(0.))
            .min_w_0()
            .flex()
            .flex_col()
            .gap(px(3.))
            .px(px(18.))
            .pt(px(14.))
            .pb(px(15.))
            .child(
                div()
                    .text_size(px(12.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(colors::text2())
                    .child(label.to_string()),
            )
            .child(figure)
            .child(foot)
    };
    let figure = |text: String| {
        div()
            .text_size(px(26.))
            .line_height(px(30.))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(colors::text1())
            .child(text)
            .into_any_element()
    };
    let foot = |text: String| {
        div()
            .text_size(px(11.5))
            .text_color(colors::text3())
            .child(text)
            .into_any_element()
    };
    let logos = div()
        .flex()
        .gap(px(7.))
        .mt(px(3.))
        .children(marks.into_iter().filter_map(|(id, live)| {
            let tint = if live { brand(id) } else { colors::text3() };
            provider_mark(id, 15., tint).map(|m| div().when(!live, |d| d.opacity(0.35)).child(m))
        }))
        .into_any_element();
    let of = div()
        .flex()
        .items_end()
        .gap(px(5.))
        .child(figure(signed.to_string()))
        .child(
            div()
                .pb(px(4.))
                .text_size(px(14.))
                .text_color(colors::text3())
                .child(format!("of {}", PROVIDERS.len())),
        )
        .into_any_element();
    div()
        .flex()
        .rounded(px(12.))
        .border_1()
        .border_color(colors::border1())
        .bg(colors::surface2())
        .child(stat(
            "Tokens",
            figure(tokens),
            foot("In and out, recent".into()),
        ))
        .child(
            stat("Sessions", figure(sessions), foot(across))
                .border_l_1()
                .border_color(colors::border1()),
        )
        .child(
            stat("Signed in", of, logos)
                .border_l_1()
                .border_color(colors::border1()),
        )
        .into_any_element()
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

fn block_head(left: &str, right: String) -> Div {
    div()
        .flex()
        .justify_between()
        .gap(px(12.))
        .mb(px(10.))
        .text_size(px(12.5))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(colors::text2())
        .child(left.to_string())
        .child(
            div()
                .font_weight(FontWeight::NORMAL)
                .text_color(colors::text3())
                .child(right),
        )
}

fn models(list: &[ModelUsage], tint: Hsla) -> AnyElement {
    let real = |m: &ModelUsage| m.input_tokens + m.output_tokens;
    let mut shown: Vec<&ModelUsage> = list.iter().collect();
    shown.sort_by_key(|m| std::cmp::Reverse(real(m)));
    shown.truncate(6);
    let max = shown.iter().map(|m| real(m)).max().unwrap_or(1).max(1);
    div()
        .px(px(16.))
        .pt(px(14.))
        .pb(px(16.))
        .child(block_head("By model", "in and out".into()))
        .children(shown.into_iter().map(|m| {
            div()
                .flex()
                .items_center()
                .gap(px(10.))
                .py(px(3.))
                .text_size(px(12.5))
                .child(
                    div()
                        .flex_none()
                        .w(px(110.))
                        .truncate()
                        .text_color(colors::text2())
                        .child(pretty_model(&m.model)),
                )
                .child(
                    div()
                        .flex_1()
                        .h(px(6.))
                        .rounded(px(3.))
                        .bg(colors::ink(0.05))
                        .overflow_hidden()
                        .child(
                            div()
                                .h_full()
                                .w(relative(real(m) as f32 / max as f32))
                                .bg(tint),
                        ),
                )
                .child(
                    div()
                        .flex_none()
                        .w(px(56.))
                        .flex()
                        .justify_end()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(colors::text1())
                        .child(short(real(m))),
                )
        }))
        .into_any_element()
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
    if !u.models.is_empty() {
        items.push(models(&u.models, tint));
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
    /// The summary, then `chart` (tokens per day), then a card per agent.
    pub(super) fn activity_view(&self, chart: Option<AnyElement>) -> Vec<AnyElement> {
        let data: Vec<&ProviderUsage> = PROVIDERS
            .iter()
            .filter_map(|(id, _)| self.local.get(*id))
            .collect();
        let on: Vec<&&ProviderUsage> = data.iter().filter(|u| u.signed_in).collect();
        let off: Vec<&&ProviderUsage> = data.iter().filter(|u| !u.signed_in).collect();
        let mut out = Vec::new();
        if !on.is_empty() {
            let tokens: u64 = on.iter().map(|u| u.input_tokens + u.output_tokens).sum();
            let sessions: u64 = on.iter().map(|u| u.sessions).sum();
            let agents = if on.len() == 1 { "agent" } else { "agents" };
            out.push(strip(
                short(tokens),
                grouped(sessions),
                format!("Across {} {agents}", on.len()),
                on.len(),
                PROVIDERS
                    .iter()
                    .map(|(id, _)| (*id, self.local.get(*id).is_some_and(|u| u.signed_in)))
                    .collect(),
            ));
        }
        out.extend(chart);
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
