use gpui::{
    AnyElement, ClickEvent, Context, MouseButton, MouseMoveEvent, ObjectFit, PathPromptOptions,
    SharedString, StyledImage, Window, div, img, prelude::*,
};
use hyprspace_proto::Command;
use hyprspace_proto::state::Effect;

use super::Root;
use super::controls::group;
use super::index::{EFFECT, GLASS, PICTURE, PICTURE_ZOOM, WALLPAPERS};
use crate::zoom::px;
use crate::{colors, widgets};

const MIN_ZOOM: f32 = 1.0;
const MAX_ZOOM: f32 = 3.0;

impl Root {
    pub(super) fn background_group(&self, cx: &mut Context<Self>) -> AnyElement {
        let a = &self.state.appearance;
        let busy = self.background_busy;
        let picture =
            match &a.background {
                Some(b) => {
                    let (x, y) = (b.x, b.y);
                    div()
                        .flex()
                        .items_center()
                        .gap(px(10.))
                        .child(
                            div()
                                .id("background-preview")
                                .w(px(96.))
                                .h(px(54.))
                                .flex_none()
                                .rounded(px(6.))
                                .overflow_hidden()
                                .border_1()
                                .border_color(colors::border2())
                                .cursor_grab()
                                .child(
                                    img(b.shown.clone())
                                        .size_full()
                                        .object_fit(ObjectFit::Cover),
                                )
                                .tooltip(widgets::tip("Drag to choose which part stays in view"))
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(move |r, e: &gpui::MouseDownEvent, _, _| {
                                        r.background_drag = Some((e.position, x, y));
                                    }),
                                )
                                .on_mouse_move(cx.listener(|r, e: &MouseMoveEvent, _, cx| {
                                    r.drag_background(e, cx);
                                }))
                                .on_mouse_up(
                                    MouseButton::Left,
                                    cx.listener(|r, _: &gpui::MouseUpEvent, _, _| {
                                        if r.background_drag.take().is_some() {
                                            r.save();
                                        }
                                    }),
                                ),
                        )
                        .child(
                            div()
                                .max_w(px(160.))
                                .truncate()
                                .text_size(px(12.5))
                                .text_color(colors::text2())
                                .child(b.name.clone()),
                        )
                        .child(widgets::button("background-replace", "Replace").on_click(
                            cx.listener(|r, _: &ClickEvent, _, cx| r.choose_background(cx)),
                        ))
                        .child(widgets::button("background-remove", "Remove").on_click(
                            cx.listener(|r, _: &ClickEvent, window, cx| {
                                r.state.appearance.background = None;
                                r.apply_theme(window);
                                r.save();
                                cx.notify();
                            }),
                        ))
                        .into_any_element()
                }
                None => widgets::button("background-choose", "Choose picture")
                    .on_click(cx.listener(|r, _: &ClickEvent, _, cx| r.choose_background(cx)))
                    .into_any_element(),
            };
        let zoom = a.background.as_ref().map_or(1.0, |b| b.zoom);
        let shown = a.background.is_some();
        let zoom_control = super::appearance::stepper(
            format!("{:.0}%", zoom * 100.),
            self.zoom_step(
                "background-zoom-out",
                "minus",
                shown && zoom > MIN_ZOOM,
                -0.25,
                cx,
            ),
            self.zoom_step(
                "background-zoom-in",
                "plus",
                shown && zoom < MAX_ZOOM,
                0.25,
                cx,
            ),
        );
        let effect = a.effect;
        let effects =
            widgets::segments().children(Effect::ALL.into_iter().enumerate().map(|(i, e)| {
                widgets::segment(("effect", i), None, e.label(), effect == e, false)
                    .on_click(cx.listener(move |r, _: &ClickEvent, _, cx| r.set_effect(e, cx)))
            }));
        let folder = a.wallpapers.as_ref().map(|f| {
            f.file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| f.display().to_string())
        });
        let folder_control = div()
            .flex()
            .items_center()
            .gap(px(8.))
            .children(folder.map(|name| {
                div()
                    .max_w(px(160.))
                    .truncate()
                    .text_size(px(12.5))
                    .text_color(colors::text2())
                    .child(name)
            }))
            .child(
                widgets::button("wallpapers-choose", "Choose folder")
                    .on_click(cx.listener(|r, _: &ClickEvent, _, cx| r.choose_wallpapers(cx))),
            )
            .child(
                widgets::button("wallpapers-shuffle", "Shuffle")
                    .tooltip(widgets::tip("Show another picture"))
                    .on_click(cx.listener(|r, _: &ClickEvent, _, cx| r.shuffle_background(cx))),
            );
        let glass = a.glass;
        let glass_control = widgets::switch("glass", glass).on_click(cx.listener(
            move |r, _: &ClickEvent, window, cx| {
                r.state.appearance.glass = !glass;
                r.apply_theme(window);
                r.save();
                cx.notify();
            },
        ));
        let current = a.background.as_ref().map(|b| b.name.as_str());
        let tiles = crate::wallpaper::BUILT_IN.iter().enumerate().map(|(i, w)| {
            let on = current == Some(w.title);
            div()
                .id(("built-in", i))
                .h(px(58.))
                .rounded(px(6.))
                .overflow_hidden()
                .border_2()
                .border_color(if on {
                    colors::accent()
                } else {
                    gpui::transparent_black()
                })
                .cursor_pointer()
                .hover(|d| d.border_color(colors::border2()))
                .child(
                    img(SharedString::from(crate::wallpaper::thumb_path(w.slug)))
                        .size_full()
                        .rounded(px(6.))
                        .object_fit(ObjectFit::Cover),
                )
                .tooltip(widgets::tip(w.title))
                .on_click(cx.listener(move |r, _: &ClickEvent, _, cx| r.use_built_in(i, cx)))
        });
        let gallery = div()
            .flex()
            .flex_col()
            .gap(px(10.))
            .px(px(14.))
            .py(px(12.))
            .child(
                div()
                    .text_size(px(13.))
                    .text_color(colors::text1())
                    .child("Built in"),
            )
            .child(div().grid().grid_cols(6).gap(px(8.)).children(tiles))
            .into_any_element();
        let mut rows = vec![
            self.item(&PICTURE, picture, cx),
            gallery,
            self.item(&PICTURE_ZOOM, zoom_control, cx),
            self.item(&EFFECT, effects, cx),
            self.item(&WALLPAPERS, folder_control, cx),
            self.item(&GLASS, glass_control, cx),
        ];
        if busy || self.background_error.is_some() {
            let (text, color) = match &self.background_error {
                Some(e) => (e.clone(), colors::error()),
                None => ("Preparing the picture".to_string(), colors::text3()),
            };
            rows.push(
                div()
                    .px(px(14.))
                    .py(px(10.))
                    .text_size(px(12.5))
                    .text_color(color)
                    .child(text)
                    .into_any_element(),
            );
        }
        group("Background", rows).into_any_element()
    }

    fn zoom_step(
        &self,
        id: &'static str,
        glyph: &str,
        enabled: bool,
        by: f32,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        super::controls::step(id, glyph, enabled)
            .when(enabled, |d| {
                d.on_click(cx.listener(move |r, _: &ClickEvent, window, cx| {
                    if let Some(b) = r.state.appearance.background.as_mut() {
                        b.zoom = (b.zoom + by).clamp(MIN_ZOOM, MAX_ZOOM);
                        r.apply_theme(window);
                        r.save();
                        cx.notify();
                    }
                }))
            })
            .into_any_element()
    }

    fn drag_background(&mut self, e: &MouseMoveEvent, cx: &mut Context<Self>) {
        let Some((from, x, y)) = self.background_drag else {
            return;
        };
        if e.pressed_button != Some(MouseButton::Left) {
            self.background_drag = None;
            return;
        }
        let Some(b) = self.state.appearance.background.as_mut() else {
            return;
        };
        let d = e.position - from;
        b.x = (x - f32::from(d.x) / 96.).clamp(0., 1.);
        b.y = (y - f32::from(d.y) / 54.).clamp(0., 1.);
        crate::wallpaper::set(Some(b.clone()));
        cx.notify();
    }

    fn choose_background(&mut self, cx: &mut Context<Self>) {
        let rx = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Choose picture".into()),
        });
        let effect = self.state.appearance.effect;
        cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(paths))) = rx.await else {
                return;
            };
            let Some(source) = paths.into_iter().next() else {
                return;
            };
            let _ = this.update(cx, |r, cx| {
                if r.background_busy {
                    return;
                }
                r.background_busy = true;
                r.background_error = None;
                r.client.send(Command::SetBackground { source, effect });
                cx.notify();
            });
        })
        .detach();
    }

    fn choose_wallpapers(&mut self, cx: &mut Context<Self>) {
        let rx = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Choose folder".into()),
        });
        cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(paths))) = rx.await else {
                return;
            };
            let Some(folder) = paths.into_iter().next() else {
                return;
            };
            let _ = this.update(cx, |r, cx| {
                r.state.appearance.wallpapers = Some(folder);
                r.save();
                r.shuffle_background(cx);
            });
        })
        .detach();
    }

    pub(crate) fn shuffle_background(&mut self, cx: &mut Context<Self>) {
        if self.background_busy {
            return;
        }
        let skip = self
            .state
            .appearance
            .background
            .as_ref()
            .map(|b| b.name.clone());
        let Some(folder) = self.state.appearance.wallpapers.clone() else {
            let all = crate::wallpaper::BUILT_IN;
            let others: Vec<usize> = (0..all.len())
                .filter(|&i| skip.as_deref() != Some(all[i].title))
                .collect();
            let seed = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.subsec_nanos() as usize);
            if let Some(&i) = others.get(seed % others.len().max(1)) {
                self.use_built_in(i, cx);
            }
            return;
        };
        self.background_busy = true;
        self.background_error = None;
        self.client.send(Command::Shuffle {
            folder,
            skip,
            effect: self.state.appearance.effect,
        });
        cx.notify();
    }

    fn use_built_in(&mut self, ix: usize, cx: &mut Context<Self>) {
        if self.background_busy {
            return;
        }
        let w = &crate::wallpaper::BUILT_IN[ix];
        self.background_busy = true;
        self.background_error = None;
        self.client.send(Command::SetBuiltIn {
            name: w.title.to_string(),
            bytes: w.picture.to_vec(),
            effect: self.state.appearance.effect,
        });
        cx.notify();
    }

    fn set_effect(&mut self, effect: Effect, cx: &mut Context<Self>) {
        if self.background_busy {
            return;
        }
        self.state.appearance.effect = effect;
        self.save();
        if let Some(background) = self.state.appearance.background.clone() {
            self.background_busy = true;
            self.background_error = None;
            self.client.send(Command::Restyle { background, effect });
        }
        cx.notify();
    }

    pub(crate) fn background_arrived(
        &mut self,
        result: Result<hyprspace_proto::state::Background, String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.background_busy = false;
        match result {
            Ok(b) => {
                self.background_error = None;
                self.state.appearance.background = Some(b);
                self.apply_theme(window);
                self.save();
            }
            Err(e) => self.background_error = Some(e),
        }
        cx.notify();
    }
}
