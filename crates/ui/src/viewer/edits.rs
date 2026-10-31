use std::cell::RefCell;
use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::Path;
use std::rc::Rc;

use gpui::{AnyElement, Context, FontWeight, IntoElement, div, prelude::*};
use hyprspace_proto::run::FileChange;
use hyprspace_theme::MONO;

use super::diff::{self, COMPACT, Diff};
use crate::colors;
use crate::zoom::px;

const MOST: usize = 400;

thread_local! {
    static SEEN: RefCell<HashMap<u64, Rc<RefCell<Diff>>>> = RefCell::default();
}

fn seen<V: 'static>(path: &str, text: &str, cx: &mut Context<V>) -> Rc<RefCell<Diff>> {
    let mut h = DefaultHasher::new();
    (path, text).hash(&mut h);
    let key = h.finish();
    if let Some(d) = SEEN.with(|s| s.borrow().get(&key).cloned()) {
        return d;
    }
    let d = Rc::new(RefCell::new(Diff::new(text, path)));
    SEEN.with(|s| {
        let mut s = s.borrow_mut();
        if s.len() > 300 {
            s.clear();
        }
        s.insert(key, d.clone());
    });
    if hyprspace_syntax::language(Path::new(path)).is_some() {
        let (path, text) = (path.to_string(), text.to_string());
        let job = cx.background_spawn(async move { diff::colors(&text, &path) });
        cx.spawn(async move |this, cx| {
            let lines = job.await;
            if let Some(d) = SEEN.with(|s| s.borrow().get(&key).cloned()) {
                d.borrow_mut().set_colors(lines);
            }
            let _ = this.update(cx, |_, cx| cx.notify());
        })
        .detach();
    }
    d
}

pub fn edits<V: 'static>(changes: &[FileChange], cx: &mut Context<V>) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap_2()
        .children(changes.iter().map(|c| {
            let d = seen(&c.path, &c.diff, cx);
            let d = d.borrow();
            let (add, del) = d.counts();
            let more = d.len().saturating_sub(MOST);
            div()
                .flex()
                .flex_col()
                .rounded(px(12.))
                .bg(colors::ink(0.035))
                .overflow_hidden()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(6.))
                        .px(px(12.))
                        .py(px(7.))
                        .text_size(px(12.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(colors::text2())
                        .child(crate::assets::icon("file-diff", 12., colors::text3()))
                        .child(div().flex_1().min_w_0().truncate().child(c.path.clone()))
                        .child(
                            div()
                                .flex()
                                .flex_none()
                                .gap(px(6.))
                                .font_family(MONO)
                                .text_size(px(11.))
                                .when(add > 0, |d| {
                                    d.child(
                                        div()
                                            .text_color(colors::diff_add())
                                            .child(format!("+{add}")),
                                    )
                                })
                                .when(del > 0, |d| {
                                    d.child(
                                        div()
                                            .text_color(colors::diff_del())
                                            .child(format!("-{del}")),
                                    )
                                }),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .pb(px(6.))
                        .font_family(MONO)
                        .text_size(px(12.))
                        .line_height(px(17.))
                        .children((0..d.len().min(MOST)).map(|ix| d.draw(ix, &COMPACT)))
                        .when(more > 0, |d| {
                            d.child(
                                div()
                                    .px(px(12.))
                                    .pt(px(4.))
                                    .text_color(colors::text3())
                                    .child(if more == 1 {
                                        "1 more line".to_string()
                                    } else {
                                        format!("{more} more lines")
                                    }),
                            )
                        }),
                )
        }))
        .into_any_element()
}
