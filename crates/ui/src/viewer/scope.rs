use gpui::{AnyElement, ClickEvent, Context, UniformListScrollHandle, div, prelude::*};
use hyprspace_proto::folder::DiffScope;
use hyprspace_proto::{Command, FolderCommand, Pane};

use super::diff::{self, Diff};
use super::{Body, Viewer};
use crate::widgets;
use crate::zoom::px;

impl Viewer {
    pub(super) fn request(&self) {
        let cmd = match &self.pane {
            Some(Pane::Diff { cwd, path }) => FolderCommand::Diff {
                cwd: cwd.clone(),
                path: path.clone(),
            },
            Some(Pane::Changes { cwd, path, .. }) => FolderCommand::Changes {
                cwd: cwd.clone(),
                path: path.clone(),
                scope: self.scope.clone(),
            },
            _ => return,
        };
        self.client.send(Command::Folder(cmd));
    }

    pub(super) fn diff(
        &mut self,
        text: &Result<String, String>,
        path: &str,
        cx: &mut Context<Self>,
    ) {
        self.body = match text {
            Ok(t) if t.trim().is_empty() => Body::Failed("No changes to show.".into()),
            Ok(t) => {
                // keep the scroll when a poll brings the same diff back
                if let Body::Diff(d) = &self.body
                    && d.is(t)
                {
                    return;
                }
                let job = cx.background_spawn({
                    let (text, path) = (t.clone(), path.to_string());
                    async move { diff::colors(&text, &path) }
                });
                let text = t.clone();
                self._colors = Some(cx.spawn(async move |this, cx| {
                    let lines = job.await;
                    let _ = this.update(cx, |v, cx| {
                        if let Body::Diff(d) = &mut v.body
                            && d.is(&text)
                        {
                            d.set_colors(lines);
                            cx.notify();
                        }
                    });
                }));
                Body::Diff(Diff::new(t, path))
            }
            Err(e) => Body::Failed(e.clone().into()),
        };
        cx.notify();
    }

    fn set_scope(&mut self, scope: DiffScope, cx: &mut Context<Self>) {
        self.scope = scope;
        self.body = Body::Loading;
        self._colors = None;
        self.scroll = UniformListScrollHandle::new();
        self.request();
        cx.notify();
    }

    pub(super) fn scopes(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let Some(Pane::Changes { before, after, .. }) = &self.pane else {
            return None;
        };
        let run = DiffScope::Run {
            before: before.clone(),
            after: after.clone(),
        };
        let options = [
            ("This run", run),
            ("Uncommitted", DiffScope::Uncommitted),
            ("Branch", DiffScope::Branch),
        ];
        let branch = match &self.base {
            Some(base) => format!("Everything this branch changed since it left {base}"),
            None => "There's no other branch to compare with".to_string(),
        };
        Some(
            div()
                .flex_none()
                .flex()
                .items_center()
                .px(px(12.))
                .py(px(8.))
                .child(
                    widgets::segments().children(options.into_iter().enumerate().map(
                        |(i, (label, scope))| {
                            let on = self.scope == scope;
                            let off = scope == DiffScope::Branch && self.base.is_none();
                            widgets::segment(("diff-scope", i), None, label, on, false)
                                .when(scope == DiffScope::Branch, |d| {
                                    d.tooltip(widgets::tip_text(branch.clone()))
                                })
                                .when(off, |d| d.opacity(0.5).cursor_default())
                                .when(!off && !on, |d| {
                                    d.on_click(cx.listener(move |v, _: &ClickEvent, _, cx| {
                                        v.set_scope(scope.clone(), cx)
                                    }))
                                })
                        },
                    )),
                )
                .into_any_element(),
        )
    }
}
