// The bar over the editor's text when something needs the user: the file changed on disk under
// unsaved edits (Reload or Overwrite), a save failed, or the card is closing with unsaved edits
// (Save, Don't save, Cancel).

use gpui::{AnyElement, Context, FontWeight, div, prelude::*, px};
use hyprspace_proto::{Command, FolderCommand};

use super::{Editor, EditorEvent, Notice};
use crate::colors;
use crate::widgets;

impl Editor {
    pub(super) fn notice_bar(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let notice = self.notice.as_ref()?;
        let name = self
            .path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        let (text, color, buttons): (String, _, Vec<AnyElement>) = match notice {
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
            Notice::Unsaved => (
                format!("Save your changes to {name}?"),
                colors::text1(),
                vec![
                    widgets::primary("edit-save-close", "Save")
                        .on_click(cx.listener(|e, _, _, cx| {
                            e.closing = true;
                            e.notice = None;
                            e.save(cx);
                        }))
                        .into_any_element(),
                    widgets::button("edit-discard", "Don't save")
                        .on_click(cx.listener(|e, _, _, cx| {
                            e.notice = None;
                            cx.emit(EditorEvent::Close);
                        }))
                        .into_any_element(),
                    widgets::button("edit-cancel", "Cancel")
                        .on_click(cx.listener(|e, _, _, cx| {
                            e.notice = None;
                            cx.notify();
                        }))
                        .into_any_element(),
                ],
            ),
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
}
