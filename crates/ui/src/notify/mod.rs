mod rule;
mod toast;

use std::collections::HashSet;
use std::time::Duration;

use gpui::{Context, SharedString, Subscription, SystemNotification, Task, Window};
use hyprspace_notify::Taskbar;

use crate::root::{Root, Screen, View};
use crate::transcript::Status;
use rule::{Kind, Reach};

const SHOW_FOR: Duration = Duration::from_secs(6);

pub(crate) struct Toast {
    thread: u64,
    kind: Kind,
    serial: u64,
}

#[derive(Default)]
pub(crate) struct Notices {
    focused: bool,
    taskbar: Option<Taskbar>,
    badge: Option<usize>,
    posted: HashSet<u64>,
    toast: Option<Toast>,
    serial: u64,
    _timer: Option<Task<()>>,
}

impl Notices {
    pub fn new(window: &Window) -> Self {
        Self {
            focused: window.is_window_active(),
            taskbar: Taskbar::of(window),
            ..Self::default()
        }
    }
}

fn tag(thread: u64) -> SharedString {
    format!("thread-{thread}").into()
}

impl Root {
    pub(crate) fn notify_hooks(window: &mut Window, cx: &mut Context<Self>) -> Subscription {
        let root = cx.entity().downgrade();
        let handle = window.window_handle();
        cx.on_system_notification_response(move |response, cx| {
            let Some(thread) = response
                .tag
                .strip_prefix("thread-")
                .and_then(|t| t.parse::<u64>().ok())
            else {
                return;
            };
            cx.activate(true);
            let root = root.clone();
            let _ = handle.update(cx, |_, window, cx| {
                window.activate_window();
                let _ = root.update(cx, |r, cx| {
                    if r.state.thread(thread).is_some() {
                        r.open_thread(thread, window, cx);
                    }
                });
            });
        });
        cx.observe_window_activation(window, |r, window, cx| {
            r.notices.focused = window.is_window_active();
            if let (true, Screen::Thread(id)) = (r.notices.focused, r.screen) {
                r.notice_gone(id, cx);
            }
            // Windows drops an overlay set before the taskbar button existed
            r.notices.badge = None;
            r.sync_badge();
        })
    }

    pub(crate) fn alert(
        &mut self,
        thread: u64,
        before: Option<Status>,
        now: Status,
        cx: &mut Context<Self>,
    ) {
        if now == Status::Working {
            self.notice_gone(thread, cx);
        }
        let Some(kind) = Kind::of(before, now).filter(|k| k.on(&self.state.notify)) else {
            return;
        };
        match rule::reach(self.notices.focused, self.screen == Screen::Thread(thread)) {
            Reach::Nothing => {}
            Reach::Toast => self.show_toast(thread, kind, cx),
            Reach::Os => self.post(thread, kind, cx),
        }
    }

    fn post(&mut self, thread: u64, kind: Kind, cx: &mut Context<Self>) {
        let Some((space, t)) = self.state.thread(thread) else {
            return;
        };
        let title = if t.title.is_empty() {
            space.name.clone()
        } else {
            t.title.clone()
        };
        let detail = match self.views.get(&thread) {
            Some(View::Structured(v)) => v.read(cx).notice(kind.status()),
            _ => self
                .activity
                .get(&thread)
                .and_then(|a| a.doing.clone())
                .filter(|_| kind == Kind::Waiting),
        };
        cx.show_system_notification(SystemNotification {
            tag: tag(thread),
            title: title.into(),
            body: kind.body(detail).into(),
            actions: Vec::new(),
        });
        self.notices.posted.insert(thread);
        if let Some(t) = &self.notices.taskbar {
            t.flash();
        }
    }

    fn show_toast(&mut self, thread: u64, kind: Kind, cx: &mut Context<Self>) {
        self.notices.serial += 1;
        self.notices.toast = Some(Toast {
            thread,
            kind,
            serial: self.notices.serial,
        });
        self.notices._timer = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(SHOW_FOR).await;
            let _ = this.update(cx, |r, cx| {
                r.notices.toast = None;
                cx.notify();
            });
        }));
        cx.notify();
    }

    pub(crate) fn notice_gone(&mut self, thread: u64, cx: &mut Context<Self>) {
        if self.notices.posted.remove(&thread) {
            cx.dismiss_system_notification(&tag(thread));
        }
        if self
            .notices
            .toast
            .as_ref()
            .is_some_and(|t| t.thread == thread)
        {
            self.notices.toast = None;
            self.notices._timer = None;
            cx.notify();
        }
    }

    pub(crate) fn sync_badge(&mut self) {
        let count = if self.state.notify.on {
            rule::need_you(&self.unseen, &self.status)
        } else {
            0
        };
        if self.notices.badge != Some(count) {
            self.notices.badge = Some(count);
            if let Some(t) = &self.notices.taskbar {
                t.badge(count);
            }
        }
    }
}
