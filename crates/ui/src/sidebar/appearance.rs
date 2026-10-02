// The Appearance menu at the foot of the sidebar: the six themes and light, dark or system.
// The full settings screen comes later (docs/REWRITE.md step 6); this keeps the look pickable.

use gpui::{
    AnyElement, ClickEvent, Context, Pixels, Point, Window, WindowAppearance, div, prelude::*, px,
};
use hyprspace_proto::state::Scheme;
use hyprspace_theme::{THEMES, build};

use crate::colors::{self, hsla};
use crate::root::Root;
use crate::widgets;

fn dark(scheme: Scheme, system: WindowAppearance) -> bool {
    match scheme {
        Scheme::Dark => true,
        Scheme::Light => false,
        Scheme::System => matches!(
            system,
            WindowAppearance::Dark | WindowAppearance::VibrantDark
        ),
    }
}

impl Root {
    /// Paints the window in the saved theme, on the side the scheme and the system ask for.
    pub(crate) fn apply_theme(&self, window: &mut Window) {
        let a = &self.state.appearance;
        colors::set(&a.theme, dark(a.scheme, window.appearance()));
        window.refresh();
    }

    pub(crate) fn appearance_menu(
        &self,
        at: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let current = self.state.appearance.clone();
        let side = dark(current.scheme, window.appearance());
        let themes = THEMES.iter().enumerate().map(|(i, t)| {
            let id = t.id;
            let swatch = hsla(build(id, side).accent);
            div()
                .id(("theme", i))
                .flex()
                .items_center()
                .gap_2()
                .h(px(28.))
                .px(px(8.))
                .rounded(px(6.))
                .cursor_pointer()
                .hover(|s| s.bg(colors::surface3()).text_color(colors::text1()))
                .when(current.theme == id, |d| d.text_color(colors::text1()))
                .child(div().size(px(12.)).rounded_full().bg(swatch))
                .child(div().flex_1().child(t.name))
                .child(div().text_color(colors::text3()).child(t.blurb))
                .on_click(cx.listener(move |r, _: &ClickEvent, window, cx| {
                    r.state.appearance.theme = id.to_string();
                    r.apply_theme(window);
                    r.save();
                    cx.notify();
                }))
        });
        let schemes = [
            (Scheme::System, "Match the system"),
            (Scheme::Light, "Light"),
            (Scheme::Dark, "Dark"),
        ]
        .into_iter()
        .enumerate()
        .map(|(i, (scheme, label))| {
            widgets::menu_item(("scheme", i), label, None, current.scheme == scheme).on_click(
                cx.listener(move |r, _: &ClickEvent, window, cx| {
                    r.state.appearance.scheme = scheme;
                    r.apply_theme(window);
                    r.save();
                    cx.notify();
                }),
            )
        });
        let close = cx.listener(|r, _: &(), _, cx| {
            r.appearance_at = None;
            cx.notify();
        });
        widgets::popup(
            at,
            widgets::Open::Up,
            window,
            move |w, cx| close(&(), w, cx),
            div()
                .w(px(240.))
                .flex()
                .flex_col()
                .child(widgets::menu_heading("Theme"))
                .children(themes)
                .child(widgets::menu_rule())
                .child(widgets::menu_heading("Mode"))
                .children(schemes),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_scheme_overrides_the_system() {
        assert!(dark(Scheme::Dark, WindowAppearance::Light));
        assert!(!dark(Scheme::Light, WindowAppearance::Dark));
        assert!(dark(Scheme::System, WindowAppearance::Dark));
        assert!(!dark(Scheme::System, WindowAppearance::Light));
    }
}
