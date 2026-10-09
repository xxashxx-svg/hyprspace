// The composer's permission picker, and the names for effort levels that the effort menu shows.

use gpui::{
    AnyElement, ClickEvent, Context, FontWeight, IntoElement, Pixels, Point, Window, div,
    prelude::*, px,
};
use hyprspace_proto::Permission;

use super::Composer;
use crate::assets::icon;
use crate::{colors, widgets};

pub fn permission_label(p: Permission) -> &'static str {
    match p {
        Permission::Plan => "Plan only",
        Permission::Ask => "Ask first",
        Permission::Auto => "Auto edit",
        Permission::Bypass => "Full access",
    }
}

pub fn permission_note(p: Permission) -> &'static str {
    match p {
        Permission::Plan => "Reads and plans. Changes nothing.",
        Permission::Ask => "Asks before edits and commands.",
        Permission::Auto => "Edits files on its own, asks before the rest.",
        Permission::Bypass => "Edits files and runs commands without asking.",
    }
}

/// An effort level's name, from src/lib/models.ts.
pub fn effort_label(level: &str) -> String {
    match level {
        "" => "Default",
        "none" => "None",
        "minimal" => "Minimal",
        "low" => "Low",
        "medium" => "Medium",
        "high" => "High",
        "xhigh" => "Extra high",
        "max" => "Max",
        "ultra" => "Ultra",
        other => other,
    }
    .to_string()
}

/// The permission menu, opened upward from where its chip was clicked.
pub fn permission_menu(
    c: &Composer,
    at: Point<Pixels>,
    window: &mut Window,
    cx: &mut Context<Composer>,
) -> AnyElement {
    let body = permission_rows(c, cx);
    let close = cx.listener(|c, _: &(), _, cx| {
        c.menu = None;
        cx.notify();
    });
    widgets::popup(
        at,
        widgets::Open::Up,
        window,
        move |w, cx| close(&(), w, cx),
        body,
    )
}

/// One row per mode, T3 Code's way: an icon, the mode's name, and a line on what it does.
fn permission_rows(c: &Composer, cx: &mut Context<Composer>) -> AnyElement {
    let modes = [
        (Permission::Plan, "list-checks"),
        (Permission::Ask, "hand"),
        (Permission::Auto, "file-pen-line"),
        (Permission::Bypass, "lock-open"),
    ];
    div()
        .w(px(300.))
        .flex()
        .flex_col()
        .gap(px(1.))
        .children(modes.into_iter().enumerate().map(|(i, (mode, glyph))| {
            let on = c.prefs.permission == mode;
            div()
                .id(("permission", i))
                .flex()
                .gap(px(10.))
                .px(px(10.))
                .py(px(7.))
                .rounded(px(8.))
                .cursor_pointer()
                .when(on, |d| d.bg(colors::ink(0.07)))
                .hover(|s| s.bg(colors::ink(0.05)))
                .child(div().pt(px(2.)).child(icon(glyph, 14., colors::text2())))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(1.))
                        .child(
                            div()
                                .text_size(px(12.5))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(colors::text1())
                                .child(permission_label(mode)),
                        )
                        .child(
                            div()
                                .text_size(px(11.5))
                                .text_color(colors::text3())
                                .child(permission_note(mode)),
                        ),
                )
                .on_click(cx.listener(move |c, _: &ClickEvent, _, cx| {
                    c.prefs.permission = mode;
                    c.menu = None;
                    cx.emit(super::ComposerEvent::Prefs(c.prefs.clone()));
                    cx.notify();
                }))
        }))
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effort_words_match_the_tauri_app() {
        assert_eq!(effort_label("xhigh"), "Extra high");
        assert_eq!(effort_label(""), "Default");
        assert_eq!(effort_label("turbo"), "turbo");
    }
}
