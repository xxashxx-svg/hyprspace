use gpui::{
    AnyElement, ClickEvent, Context, ObjectFit, PathPromptOptions, StyledImage, div, img,
    prelude::*,
};
use hyprspace_proto::Command;

use super::Root;
use super::controls::group;
use super::index::SIDEBAR_ART;
use crate::zoom::px;
use crate::{colors, widgets};

impl Root {
    pub(super) fn sidebar_art_group(&self, cx: &mut Context<Self>) -> AnyElement {
        let a = &self.state.appearance;
        let preview = a.sidebar_art.clone().map(|path| {
            div()
                .w(px(72.))
                .h(px(48.))
                .rounded(px(6.))
                .overflow_hidden()
                .border_1()
                .border_color(colors::border1())
                .bg(colors::bg())
                .child(img(path).size_full().object_fit(ObjectFit::Cover))
        });
        let background = a.background.as_ref().map(|b| b.source.clone());
        let controls =
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .children(preview)
                .child(
                    widgets::button("sidebar-art-choose", "Choose a picture...")
                        .on_click(cx.listener(|r, _: &ClickEvent, _, cx| r.choose_sidebar_art(cx))),
                )
                .when_some(background, |d, source| {
                    d.child(
                        widgets::button("sidebar-art-background", "Use the background").on_click(
                            cx.listener(move |r, _: &ClickEvent, _, cx| {
                                r.set_sidebar_art(Some(source.clone()), cx)
                            }),
                        ),
                    )
                })
                .when(a.sidebar_art.is_some(), |d| {
                    d.child(widgets::button("sidebar-art-remove", "Remove").on_click(
                        cx.listener(|r, _: &ClickEvent, _, cx| r.set_sidebar_art(None, cx)),
                    ))
                });
        let mut rows = vec![self.item(&SIDEBAR_ART, controls, cx)];
        if let Some(e) = &self.sidebar_art_error {
            rows.push(
                div()
                    .px(px(14.))
                    .py(px(10.))
                    .text_size(px(12.5))
                    .text_color(colors::error())
                    .child(e.clone())
                    .into_any_element(),
            );
        }
        group("Sidebar", rows).into_any_element()
    }

    fn choose_sidebar_art(&mut self, cx: &mut Context<Self>) {
        let rx = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Choose picture".into()),
        });
        cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(paths))) = rx.await else {
                return;
            };
            let Some(source) = paths.into_iter().next() else {
                return;
            };
            let _ = this.update(cx, |r, cx| r.set_sidebar_art(Some(source), cx));
        })
        .detach();
    }

    fn set_sidebar_art(&mut self, source: Option<std::path::PathBuf>, cx: &mut Context<Self>) {
        self.sidebar_art_error = None;
        self.client.send(Command::SetSidebarArt { source });
        cx.notify();
    }

    pub(crate) fn sidebar_art_arrived(
        &mut self,
        result: Result<Option<std::path::PathBuf>, String>,
        cx: &mut Context<Self>,
    ) {
        match result {
            Ok(path) => {
                self.sidebar_art_error = None;
                self.state.appearance.sidebar_art = path;
                self.save();
            }
            Err(e) => self.sidebar_art_error = Some(e),
        }
        cx.notify();
    }
}
