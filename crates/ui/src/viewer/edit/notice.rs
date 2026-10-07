// What the editor asks of the user. A bar over the text when the file changed on disk under
// unsaved edits (Reload or Overwrite) or a save failed; a dialog over the editor when the card is
// closing with unsaved edits (Save changes, Discard or Cancel; Enter saves and Esc cancels).

use gpui::{AnyElement, Context, FontWeight, MouseButton, div, prelude::*, px};
use hyprspace_proto::{Command, FolderCommand};

use super::{Editor, EditorEvent, Notice};
use crate::assets::icon;
use crate::colors;
use crate::widgets;

impl Editor {
    /// Asked to close with unsaved edits: the dialog is up and holds the keyboard.
    pub(super) fn asking(&self) -> bool {
        matches!(self.notice, Some(Notice::Unsaved))
    }

    /// The dialog's answers.
    pub(super) fn save_and_close(&mut self, cx: &mut Context<Self>) {
        self.closing = true;
        self.notice = None;
        self.save(cx);
    }

    pub(super) fn discard(&mut self, cx: &mut Context<Self>) {
        self.notice = None;
        cx.emit(EditorEvent::Close);
    }

    pub(super) fn keep_editing(&mut self, cx: &mut Context<Self>) {
        self.notice = None;
        cx.notify();
    }

    pub(super) fn notice_bar(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let (text, color, buttons): (String, _, Vec<AnyElement>) = match self.notice.as_ref()? {
            Notice::Changed => (
                "This file changed on disk since you opened it.".into(),
                colors::waiting(),
                vec![
                    widgets::button("edit-reload", "Reload")
                        .tooltip(widgets::tip(
                            "Drop your edits and show the file as it is now",
                        ))
                        .on_click(cx.listener(|e, _, _, cx| {
                            e.force = true;
                            e.notice = None;
                            e.client.send(Command::Folder(FolderCommand::ReadFile {
                                path: e.path.clone(),
                            }));
                            cx.notify();
                        }))
                        .into_any_element(),
                    widgets::button("edit-overwrite", "Overwrite")
                        .tooltip(widgets::tip("Save your edits over the change"))
                        .on_click(cx.listener(|e, _, _, cx| {
                            e.notice = None;
                            e.write(None, cx);
                        }))
                        .into_any_element(),
                ],
            ),
            Notice::Failed(why) => (
                format!("Couldn't save. {why}"),
                colors::error(),
                vec![
                    widgets::button("edit-dismiss", "Dismiss")
                        .on_click(cx.listener(|e, _, _, cx| {
                            e.notice = None;
                            cx.notify();
                        }))
                        .into_any_element(),
                ],
            ),
            Notice::Unsaved => return None,
        };
        Some(
            div()
                .flex_none()
                .flex()
                .items_center()
                .gap(px(6.))
                .h(px(40.))
                .px(px(12.))
                .border_b_1()
                .border_color(colors::border1())
                .bg(colors::surface1())
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .text_size(px(12.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(color)
                        .child(text),
                )
                .children(buttons)
                .into_any_element(),
        )
    }

    /// The close dialog, over the whole editor on a dimmed backdrop. A click beside it cancels.
    pub(super) fn unsaved_dialog(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if !self.asking() {
            return None;
        }
        let name = self
            .path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        let (glyph, tint) = crate::dock::kinds::file_icon(&name);
        let scrim = colors::hsla(colors::theme().shadow);
        let card = div()
            .w(px(400.))
            .flex()
            .flex_col()
            .gap(px(18.))
            .p(px(20.))
            .rounded(px(12.))
            .border_1()
            .border_color(colors::border2())
            .bg(colors::surface2())
            .shadow(colors::shadow())
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(
                div()
                    .flex()
                    .gap(px(14.))
                    .child(
                        div()
                            .flex_none()
                            .flex()
                            .items_center()
                            .justify_center()
                            .size(px(36.))
                            .rounded(px(9.))
                            .bg(colors::ink(0.06))
                            .child(icon(glyph, 18., tint)),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .gap(px(4.))
                            .child(
                                div()
                                    .text_size(px(14.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(colors::text1())
                                    .child(format!("Save changes to {name}?")),
                            )
                            .child(
                                div()
                                    .text_size(px(12.5))
                                    .line_height(px(18.))
                                    .text_color(colors::text2())
                                    .child("If you discard them, your edits are gone for good."),
                            ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(
                        widgets::button("edit-discard", "Discard")
                            .text_color(colors::error())
                            .on_click(cx.listener(|e, _, _, cx| e.discard(cx))),
                    )
                    .child(div().flex_1())
                    .child(
                        widgets::button("edit-cancel", "Cancel")
                            .tooltip(widgets::tip("Esc"))
                            .on_click(cx.listener(|e, _, _, cx| e.keep_editing(cx))),
                    )
                    .child(
                        widgets::primary("edit-save-close", "Save changes")
                            .tooltip(widgets::tip("Enter"))
                            .on_click(cx.listener(|e, _, _, cx| e.save_and_close(cx))),
                    ),
            );
        Some(
            div()
                .id("edit-unsaved")
                .absolute()
                .inset_0()
                .occlude()
                .flex()
                .items_center()
                .justify_center()
                .bg(scrim.opacity(0.6))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|e, _, _, cx| {
                        cx.stop_propagation();
                        e.keep_editing(cx);
                    }),
                )
                .child(crate::slide::rise_in(card, "edit-unsaved-in"))
                .into_any_element(),
        )
    }
}
