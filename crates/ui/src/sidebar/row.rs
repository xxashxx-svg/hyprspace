// One thread in the sidebar, laid out like the Tauri app's SessionRow: the agent's mark, model
// and age on top, the title, then where it runs or what it is doing, with a glyph when it needs
// you or has finished. Click to open it, right-click for rename and archive.

use std::time::Duration;

use gpui::{
    Animation, AnimationExt, AnyElement, ClickEvent, Context, FontWeight, IntoElement, MouseButton,
    div, prelude::*, px,
};
use hyprspace_proto::{Thread, ThreadKind};
use hyprspace_theme::MONO;

use super::row_hover;
use crate::assets::{icon, mark};
use crate::colors;
use crate::root::{Action, MenuItems, Rename, Root, Screen, View};
use crate::time::ago;
use crate::transcript::Status;

/// A running count, the way a stopwatch shows it: 45s, 3m 05s, 1h 02m.
fn elapsed(secs: u64) -> String {
    match secs {
        0..60 => format!("{secs}s"),
        60..3600 => format!("{}m {:02}s", secs / 60, secs % 60),
        _ => format!("{}h {:02}m", secs / 3600, secs / 60 % 60),
    }
}

impl Root {
    /// `space` names the space for a thread shown outside it (the Archived group).
    pub(crate) fn thread_row(
        &self,
        t: &Thread,
        space: Option<&str>,
        now: u64,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let id = t.id;
        let status = self.status.get(&id).copied().unwrap_or(Status::Idle);
        let selected = self.screen == Screen::Thread(id);
        let running = match self.views.get(&id) {
            Some(View::Structured(v)) => v.read(cx).elapsed(),
            _ => None,
        };
        let (badge, label) = match t.agent() {
            Some(launch) => {
                let (brand, _) = colors::brand(launch.agent);
                (
                    mark(launch.agent, 13., brand).into_any_element(),
                    self.model_label(launch),
                )
            }
            None => (
                icon("terminal", 12., colors::text3()).into_any_element(),
                "Terminal".to_string(),
            ),
        };
        // an agent running in a terminal says so after its model
        let in_terminal = matches!(t.kind, ThreadKind::Terminal { run: Some(_), .. });
        let title: AnyElement = match &self.rename {
            Some((Rename::Thread(r), input, _)) if *r == id => div()
                .h(px(24.))
                .flex()
                .items_center()
                .px(px(6.))
                .rounded(px(6.))
                .border_1()
                .border_color(colors::accent())
                .bg(colors::bg())
                .text_size(px(13.))
                .text_color(colors::text1())
                .child(input.clone())
                .into_any_element(),
            _ => div()
                .min_w_0()
                .truncate()
                .text_size(px(13.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(colors::text1())
                .child(t.title.clone())
                .into_any_element(),
        };
        let folder = t
            .cwd()
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        let foot: AnyElement = match (status, space) {
            (Status::Waiting, _) => div()
                .text_color(colors::waiting())
                .child("Needs your answer")
                .into_any_element(),
            (Status::Failed, _) => div()
                .text_color(colors::error())
                .child("Stopped with an error")
                .into_any_element(),
            (Status::Working, _) => div()
                .text_color(colors::text2())
                .child("Working")
                .into_any_element(),
            (_, Some(space)) => div().child(format!("in {space}")).into_any_element(),
            _ => div()
                .flex()
                .items_center()
                .gap(px(5.))
                .min_w_0()
                .child(icon("folder", 11., colors::text3()))
                .child(
                    div()
                        .truncate()
                        .font_family(MONO)
                        .text_size(px(10.5))
                        .child(folder),
                )
                .into_any_element(),
        };
        let glyph: Option<AnyElement> = match status {
            Status::Waiting => Some(
                div()
                    .size(px(8.))
                    .rounded_full()
                    .bg(colors::waiting())
                    .with_animation(
                        ("waiting", id),
                        Animation::new(Duration::from_millis(1000)).repeat(),
                        |d, t| d.opacity(0.35 + 0.65 * (t * 2.0 - 1.0).abs()),
                    )
                    .into_any_element(),
            ),
            Status::Done => Some(icon("circle-check", 12., colors::ok()).into_any_element()),
            _ => None,
        };
        let menu: MenuItems = vec![
            ("Rename".into(), Action::Rename(Rename::Thread(id))),
            if t.archived {
                ("Restore".into(), Action::ArchiveThread(id, false))
            } else {
                ("Archive".into(), Action::ArchiveThread(id, true))
            },
            ("Remove".into(), Action::RemoveThread(id)),
        ];
        let right: AnyElement = match running {
            Some(secs) => div()
                .flex_none()
                .font_family(MONO)
                .text_size(px(10.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(colors::busy())
                .child(elapsed(secs))
                .into_any_element(),
            None => div()
                .flex_none()
                .text_size(px(10.5))
                .child(ago(t.created, now))
                .into_any_element(),
        };
        div()
            .id(("thread", id))
            .flex()
            .flex_col()
            .gap(px(3.))
            .pt(px(8.))
            .px(px(8.))
            .pb(px(9.))
            .rounded(px(8.))
            .cursor_pointer()
            .when(status == Status::Waiting, |d| {
                d.bg(colors::waiting().opacity(0.07))
            })
            .when(selected, |d| d.bg(colors::surface3()))
            .when(!selected, |d| d.hover(|s| s.bg(row_hover())))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .min_w_0()
                    .text_size(px(11.))
                    .text_color(colors::text3())
                    .child(
                        div()
                            .flex()
                            .flex_none()
                            .items_center()
                            .justify_center()
                            .size(px(16.))
                            .rounded_full()
                            .when(status == Status::Working, |d| {
                                d.border_1().border_color(colors::busy())
                            })
                            .child(badge),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .items_center()
                            .gap(px(5.))
                            .child(
                                div()
                                    .min_w_0()
                                    .truncate()
                                    .font_family(MONO)
                                    .text_size(px(10.5))
                                    .child(label),
                            )
                            .when(in_terminal, |d| {
                                d.child(icon("terminal", 10., colors::text3()))
                            }),
                    )
                    .child(right),
            )
            .child(title)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .min_w_0()
                    .text_size(px(11.))
                    .text_color(colors::text3())
                    .child(div().flex_1().min_w_0().truncate().child(foot))
                    .children(glyph),
            )
            .on_click(cx.listener(move |r, _: &ClickEvent, window, cx| {
                r.menu = None;
                r.open_thread(id, window, cx)
            }))
            .on_mouse_down(MouseButton::Right, self.context_menu(menu, cx))
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_like_a_stopwatch() {
        assert_eq!(elapsed(45), "45s");
        assert_eq!(elapsed(185), "3m 05s");
        assert_eq!(elapsed(3720), "1h 02m");
    }
}
