// The card that shows beside a thread's row when the pointer rests on it, after T3 Code's, in
// the frame the app's menus and panels share: the thread's title under the agent's mark, then
// its project, folder, branch, agent and effort as labeled lines, and the subagents it has
// running. The row itself only has room for the agent's mark. It floats just past the sidebar's
// edge, level with the row, the way T3 Code's does, so the list under the pointer stays in view.

use std::path::Path;
use std::time::{Duration, Instant};

use gpui::{
    AnyElement, App, Context, FontWeight, IntoElement, SharedString, Window, anchored, deferred,
    div, point, prelude::*, px,
};
use hyprspace_proto::Agent;
use hyprspace_theme::MONO;

use crate::assets::{icon, mark};
use crate::colors;
use crate::root::{Rename, Root};

/// How long the pointer rests on a row before its card shows.
const AFTER: Duration = Duration::from_millis(450);
/// The card's width, and the room its labels take.
const WIDTH: f32 = 300.;
const LABEL: f32 = 58.;
/// A folder longer than this shows its end, which is the part that tells folders apart.
const PATH_CHARS: usize = 30;

#[derive(Clone)]
pub struct RowCard {
    pub title: SharedString,
    pub space: SharedString,
    pub path: SharedString,
    pub branch: Option<SharedString>,
    pub agent: Option<Agent>,
    /// The agent with its model, "Claude Opus 5.5", or "Terminal".
    pub model: SharedString,
    /// The effort it runs at, "High" or "High (default)", for a model that takes one.
    pub effort: Option<SharedString>,
    /// Each subagent running: what it was asked, and how long it has run.
    pub subagents: Vec<(SharedString, SharedString)>,
}

/// Subagents listed before the rest fold into a count.
const SUBS_SHOWN: usize = 6;

/// A folder as the card shows it: the home folder as `~`, and a long one cut at its start.
pub fn tidy(path: &Path) -> String {
    let home = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" });
    let full = path.display().to_string();
    let shown = match home.map(std::path::PathBuf::from) {
        Some(h) if !h.as_os_str().is_empty() && path.starts_with(&h) => {
            let rest = path.strip_prefix(&h).map(|r| r.display().to_string());
            match rest {
                Ok(r) if r.is_empty() => "~".to_string(),
                Ok(r) => format!("~{}{r}", std::path::MAIN_SEPARATOR),
                Err(_) => full,
            }
        }
        _ => full,
    };
    let count = shown.chars().count();
    if count <= PATH_CHARS {
        return shown;
    }
    let tail: String = shown.chars().skip(count - (PATH_CHARS - 3)).collect();
    format!("...{tail}")
}

/// One labeled line: a quiet label in a fixed column, then the value.
fn field(label: &'static str, value: impl IntoElement) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .min_w_0()
        .child(
            div()
                .flex_none()
                .w(px(LABEL))
                .text_size(px(11.5))
                .text_color(colors::text3())
                .child(label),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .items_center()
                .gap(px(6.))
                .child(value),
        )
}

impl RowCard {
    fn render(self) -> AnyElement {
        let agent = match self.agent {
            Some(a) => mark(a, 13., colors::brand(a).0).into_any_element(),
            None => icon("terminal", 13., colors::text3()).into_any_element(),
        };
        let small_agent = match self.agent {
            Some(a) => mark(a, 11., colors::brand(a).0).into_any_element(),
            None => icon("terminal", 11., colors::text3()).into_any_element(),
        };
        let head = div()
            .flex()
            .items_center()
            .gap(px(8.))
            .h(px(38.))
            .px(px(12.))
            .border_b_1()
            .border_color(colors::border1())
            .child(div().flex_none().child(agent))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_size(px(13.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(colors::text1())
                    .child(self.title.clone()),
            );
        let fields = div()
            .flex()
            .flex_col()
            .gap(px(7.))
            .px(px(12.))
            .py(px(10.))
            .child(field(
                "Project",
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .min_w_0()
                    .child(super::row::tag(&self.space, false))
                    .child(div().min_w_0().truncate().child(self.space.clone())),
            ))
            .child(field(
                "Folder",
                div()
                    .min_w_0()
                    .truncate()
                    .font_family(MONO)
                    .text_size(px(11.))
                    .text_color(colors::text2())
                    .child(self.path.clone()),
            ))
            .children(self.branch.clone().map(|b| {
                field(
                    "Branch",
                    div()
                        .flex()
                        .items_center()
                        .gap(px(5.))
                        .min_w_0()
                        .child(icon("git-branch", 11., colors::text3()))
                        .child(
                            div()
                                .min_w_0()
                                .truncate()
                                .font_family(MONO)
                                .text_size(px(11.))
                                .child(b),
                        ),
                )
            }))
            .child(field(
                "Agent",
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .min_w_0()
                    .child(div().flex_none().child(small_agent))
                    .child(div().min_w_0().truncate().child(self.model.clone())),
            ))
            .children(self.effort.clone().map(|e| {
                field(
                    "Effort",
                    div()
                        .flex()
                        .items_center()
                        .gap(px(6.))
                        .min_w_0()
                        .child(icon("gauge", 11., colors::text3()))
                        .child(div().min_w_0().truncate().child(e)),
                )
            }));
        let subs = (!self.subagents.is_empty()).then(|| {
            let more = self.subagents.len().saturating_sub(SUBS_SHOWN);
            div()
                .flex()
                .flex_col()
                .gap(px(6.))
                .px(px(12.))
                .py(px(10.))
                .border_t_1()
                .border_color(colors::border1())
                .child(
                    div()
                        .text_size(px(11.5))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(colors::text3())
                        .child(match self.subagents.len() {
                            1 => "1 subagent running".to_string(),
                            n => format!("{n} subagents running"),
                        }),
                )
                .children(self.subagents.iter().take(SUBS_SHOWN).enumerate().map(
                    |(i, (label, took))| {
                        // the agent's mark with the ring the rows turn while they work
                        let badge = match self.agent {
                            Some(a) => mark(a, 10., colors::brand(a).0).into_any_element(),
                            None => icon("bot", 10., colors::text3()).into_any_element(),
                        };
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .min_w_0()
                            .child(
                                div()
                                    .relative()
                                    .flex()
                                    .flex_none()
                                    .items_center()
                                    .justify_center()
                                    .size(px(16.))
                                    .child(badge)
                                    .child(super::row::ring(("card-sub-ring", i), 16., 2.)),
                            )
                            .child(div().flex_1().min_w_0().truncate().child(label.clone()))
                            .child(
                                div()
                                    .flex_none()
                                    .font_family(MONO)
                                    .text_size(px(10.5))
                                    .text_color(colors::text3())
                                    .child(took.clone()),
                            )
                    },
                ))
                .when(more > 0, |d| {
                    d.child(
                        div()
                            .pl(px(24.))
                            .text_size(px(11.))
                            .text_color(colors::text3())
                            .child(format!("{more} more")),
                    )
                })
        });
        div()
            .w(px(WIDTH))
            .flex()
            .flex_col()
            .overflow_hidden()
            .rounded(px(6.))
            .border_1()
            .border_color(colors::border2())
            .bg(colors::surface2())
            .shadow(colors::shadow())
            .text_size(px(12.))
            .text_color(colors::text1())
            .child(head)
            .child(fields)
            .children(subs)
            .into_any_element()
    }
}

impl Root {
    /// The pointer came onto a thread's row, or left it. The card waits a moment, so sweeping
    /// across the list doesn't flash one card after another.
    pub(crate) fn row_hovered(&mut self, thread: u64, on: bool, cx: &mut Context<Self>) {
        if on {
            self.hover_row = Some((thread, Instant::now()));
            self._hover_timer = Some(cx.spawn(async move |this, cx| {
                cx.background_executor().timer(AFTER).await;
                let _ = this.update(cx, |_, cx| cx.notify());
            }));
        } else if self.hover_row.is_some_and(|(t, _)| t == thread) {
            self.hover_row = None;
        }
        cx.notify();
    }

    /// The card for the row under the pointer, past the sidebar's edge and level with the row.
    /// Not while a menu is open or something is being dragged, and only while the pointer is on
    /// that row. On Windows the pointer can leave for another monitor without any element hearing
    /// of it; GPUI only marks the window unhovered and redraws, so that is checked here.
    pub(crate) fn hover_card(&self, window: &Window, cx: &App) -> Option<AnyElement> {
        let (thread, since) = self.hover_row?;
        if since.elapsed() < AFTER || self.menu.is_some() || cx.has_active_drag() {
            return None;
        }
        // not over a title being typed
        if matches!(&self.rename, Some((Rename::Thread(r), ..)) if *r == thread) {
            return None;
        }
        let bounds = *self.row_bounds.borrow().get(&thread)?;
        if !window.is_window_hovered() || !bounds.contains(&window.mouse_position()) {
            return None;
        }
        let card = self.thread_card(thread, cx)?;
        Some(
            deferred(
                anchored()
                    .position(point(bounds.right() + px(16.), bounds.top()))
                    .snap_to_window_with_margin(px(8.))
                    .child(card.render()),
            )
            .with_priority(1)
            .into_any_element(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn long_folders_keep_their_end() {
        let long = Path::new("/srv/projects/clients/acme/very-long-project-name/app");
        let shown = tidy(long);
        assert!(shown.starts_with("...") && shown.ends_with("project-name/app"));
        assert_eq!(shown.chars().count(), PATH_CHARS);
        assert_eq!(tidy(Path::new("/srv/app")), "/srv/app");
    }
}
