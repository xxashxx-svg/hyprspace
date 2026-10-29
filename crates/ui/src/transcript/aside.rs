use gpui::{
    AnyElement, ClickEvent, Context, InteractiveElement, IntoElement, MouseButton, ParentElement,
    StatefulInteractiveElement, Styled, div,
};
use hyprspace_proto::{Agent, Aside, Command};

use super::TranscriptView;
use crate::assets::icon;
use crate::markdown::{self, Block};
use crate::zoom::px;
use crate::{colors, spinner, widgets};

/// A side question and what it said, which stays out of the transcript and its journal.
pub(super) struct Card {
    question: String,
    answer: String,
    blocks: Vec<Block>,
    ended: Option<Result<(), String>>,
}

impl TranscriptView {
    pub(super) fn ask_aside(
        &mut self,
        question: String,
        images: Vec<std::path::PathBuf>,
        cx: &mut Context<Self>,
    ) {
        let why = if self.launch.agent != Agent::Claude {
            Some("/btw works in Claude threads.")
        } else if self.remote {
            Some("/btw works for threads on this computer.")
        } else if self.launch.resume.is_none() {
            Some("Send a message first. /btw asks about the conversation so far.")
        } else if question.is_empty() {
            Some("Write the question after /btw.")
        } else {
            None
        };
        if let Some(why) = why {
            self.model.note(why);
            self.changed(cx);
            return;
        }
        self.client.send(Command::Aside {
            id: self.id,
            launch: self.launch.clone(),
            question: question.clone(),
            images,
        });
        self.aside = Some(Card {
            question,
            answer: String::new(),
            blocks: Vec::new(),
            ended: None,
        });
        cx.notify();
    }

    pub fn heard_aside(&mut self, aside: Aside, cx: &mut Context<Self>) {
        if aside == Aside::Closed {
            self.aside = None;
            cx.notify();
            return;
        }
        let Some(card) = self.aside.as_mut() else {
            return;
        };
        match aside {
            Aside::Text { text } => {
                card.answer.push_str(&text);
                card.blocks = markdown::parse(&card.answer);
            }
            Aside::Done => card.ended = Some(Ok(())),
            Aside::Failed { message } => card.ended = Some(Err(message)),
            Aside::Asked { .. } | Aside::Closed => {}
        }
        cx.notify();
    }

    pub fn drop_aside(&mut self, cx: &mut Context<Self>) {
        if self.aside.take().is_some() {
            self.client.send(Command::DropAside { id: self.id });
        }
        cx.notify();
    }
}

pub(super) fn card(v: &TranscriptView, cx: &mut Context<TranscriptView>) -> Option<AnyElement> {
    let card = v.aside.as_ref()?;
    let head = div()
        .flex()
        .items_center()
        .gap(px(10.))
        .min_h(px(36.))
        .child(icon("message-square", 14., colors::text3()))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .text_size(px(13.))
                .text_color(colors::text2())
                .child(card.question.clone()),
        )
        .children(
            card.ended
                .is_none()
                .then(|| spinner::ring("aside-live", colors::text3())),
        )
        .child(
            widgets::icon_button("aside-drop", "x", 24.)
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .tooltip(widgets::tip("Dismiss"))
                .on_click(cx.listener(|v, _: &ClickEvent, _, cx| v.drop_aside(cx))),
        );
    let body = match (&card.ended, card.answer.is_empty()) {
        (Some(Err(message)), _) => div()
            .text_size(px(13.))
            .text_color(colors::text3())
            .child(message.clone())
            .into_any_element(),
        (_, true) => div()
            .text_size(px(13.))
            .text_color(colors::text3())
            .child("Answering")
            .into_any_element(),
        (_, false) => markdown::render(&card.blocks, "aside", &[]),
    };
    Some(
        div()
            .mx(px(12.))
            .mb(px(8.))
            .px(px(12.))
            .pb(px(10.))
            .rounded(px(12.))
            .bg(colors::ink(0.035))
            .child(head)
            .child(
                div()
                    .id("aside-body")
                    .max_h(px(260.))
                    .overflow_y_scroll()
                    .child(body),
            )
            .into_any_element(),
    )
}
