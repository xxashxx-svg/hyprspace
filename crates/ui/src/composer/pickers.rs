// The composer's permission picker, and the names and notes for effort levels that the model
// menu shows.

use gpui::{
    AnyElement, ClickEvent, Context, IntoElement, Pixels, Point, Window, div, prelude::*, px,
};
use hyprspace_proto::Permission;

use super::Composer;
use crate::widgets;

pub fn permission_label(p: Permission) -> &'static str {
    match p {
        Permission::Plan => "Plan only",
        Permission::Ask => "Ask first",
        Permission::Auto => "Auto edit",
        Permission::Bypass => "Full access",
    }
}

fn permission_note(p: Permission) -> &'static str {
    match p {
        Permission::Plan => "Reads and plans. Changes nothing.",
        Permission::Ask => "Asks before edits and commands.",
        Permission::Auto => "Edits files on its own, asks before the rest.",
        Permission::Bypass => "Never asks. Only for folders you trust.",
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

/// One line on what a level does.
pub fn effort_note(level: &str) -> &'static str {
    match level {
        "" => "The CLI picks",
        "none" => "No extra thinking",
        "minimal" => "Fastest, barely thinks",
        "low" => "Quick answers",
        "medium" => "Balanced",
        "high" => "Thinks longer",
        "xhigh" => "Thinks much longer",
        "max" => "Everything it has",
        "ultra" => "Max, plus it delegates to sub-agents",
        _ => "Thinks harder",
    }
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

fn permission_rows(c: &Composer, cx: &mut Context<Composer>) -> AnyElement {
    let modes = [
        Permission::Plan,
        Permission::Ask,
        Permission::Auto,
        Permission::Bypass,
    ];
    div()
        .w(px(280.))
        .flex()
        .flex_col()
        .child(widgets::menu_heading("Permission"))
        .children(modes.into_iter().enumerate().map(|(i, mode)| {
            widgets::menu_item(
                ("permission", i),
                permission_label(mode),
                Some(permission_note(mode).into()),
                c.prefs.permission == mode,
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
        assert_eq!(effort_note("max"), "Everything it has");
    }
}
