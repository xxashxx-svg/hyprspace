// The saved values a Settings row can put back the way a new install has them, read off
// `AppState::default()`, so a reset can never drift from the real defaults.

use hyprspace_proto::Agent;
use hyprspace_proto::state::{AppState, Appearance, Pick};

use super::appearance::{MAX_SIZE, MIN_SIZE, line_height};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Field {
    Permission,
    Settle,
    Titles,
    Structured,
    Working,
    Theme,
    Scheme,
    Glass,
    UiFont,
    Diff,
    Animations,
    TermFont,
    TermSize,
    LineHeight,
    Pick(Agent),
}

fn size(a: &Appearance) -> f32 {
    a.terminal_font_size.clamp(MIN_SIZE, MAX_SIZE)
}

impl Field {
    pub(super) fn changed(self, s: &AppState) -> bool {
        !self.same(s, &AppState::default())
    }

    pub(super) fn reset(self, s: &mut AppState) {
        let fresh = AppState::default();
        let (from, to) = (&fresh.appearance, &mut s.appearance);
        match self {
            Field::Permission => s.composer.permission = fresh.composer.permission,
            Field::Settle => s.settle_after = fresh.settle_after,
            Field::Titles => s.titles = fresh.titles,
            Field::Structured => s.composer.structured = fresh.composer.structured,
            Field::Working => s.working_shelf = fresh.working_shelf,
            Field::Theme => to.theme = from.theme.clone(),
            Field::Scheme => to.scheme = from.scheme,
            Field::Glass => to.glass = from.glass,
            Field::UiFont => to.ui_font = from.ui_font.clone(),
            Field::Diff => to.diff_colors = from.diff_colors,
            Field::Animations => to.animations = from.animations,
            Field::TermFont => to.terminal_font = from.terminal_font.clone(),
            Field::TermSize => to.terminal_font_size = from.terminal_font_size,
            Field::LineHeight => to.terminal_line_height = from.terminal_line_height,
            Field::Pick(agent) => {
                let fast = s.composer.pick(agent).fast;
                let pick = fresh.composer.pick(agent);
                s.composer.set_pick(Pick { fast, ..pick });
            }
        }
    }

    fn same(self, a: &AppState, b: &AppState) -> bool {
        let (x, y) = (&a.appearance, &b.appearance);
        match self {
            Field::Permission => a.composer.permission == b.composer.permission,
            Field::Settle => a.settle_after == b.settle_after,
            Field::Titles => a.titles == b.titles,
            Field::Structured => a.composer.structured == b.composer.structured,
            Field::Working => a.working_shelf == b.working_shelf,
            Field::Theme => x.theme == y.theme,
            Field::Scheme => x.scheme == y.scheme,
            Field::Glass => x.glass == y.glass,
            Field::UiFont => x.ui_font == y.ui_font,
            Field::Diff => x.diff_colors == y.diff_colors,
            Field::Animations => x.animations == y.animations,
            Field::TermFont => x.terminal_font.trim() == y.terminal_font.trim(),
            Field::TermSize => size(x) == size(y),
            Field::LineHeight => {
                line_height(x.terminal_line_height) == line_height(y.terminal_line_height)
            }
            Field::Pick(agent) => {
                let (p, q) = (a.composer.pick(agent), b.composer.pick(agent));
                p.model == q.model && p.effort == q.effort
            }
        }
    }

    pub(super) fn name(self) -> &'static str {
        match self {
            Field::Permission => "Permission",
            Field::Settle => "Settle untouched threads",
            Field::Titles => "Name new threads",
            Field::Structured => "Structured threads",
            Field::Working => "Working section",
            Field::Theme => "Theme",
            Field::Scheme => "Light or dark",
            Field::Glass => "Glass",
            Field::UiFont => "Interface font",
            Field::Diff => "Diff colors",
            Field::Animations => "Animations",
            Field::TermFont => "Terminal font",
            Field::TermSize => "Terminal font size",
            Field::LineHeight => "Terminal line height",
            Field::Pick(agent) => agent.name(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hyprspace_proto::state::Scheme;

    const ALL: [Field; 16] = [
        Field::Permission,
        Field::Settle,
        Field::Titles,
        Field::Structured,
        Field::Working,
        Field::Theme,
        Field::Scheme,
        Field::Glass,
        Field::UiFont,
        Field::Diff,
        Field::Animations,
        Field::TermFont,
        Field::TermSize,
        Field::LineHeight,
        Field::Pick(Agent::Claude),
        Field::Pick(Agent::Codex),
    ];

    #[test]
    fn a_fresh_state_has_nothing_to_reset() {
        let s = AppState::default();
        assert!(ALL.iter().all(|f| !f.changed(&s)));
    }

    #[test]
    fn a_reset_puts_back_only_its_own_field() {
        let mut s = AppState::default();
        s.appearance.terminal_font_size = 15.;
        s.appearance.scheme = Scheme::Light;
        s.titles = false;
        assert!(Field::TermSize.changed(&s));
        Field::TermSize.reset(&mut s);
        assert!(!Field::TermSize.changed(&s));
        assert!(Field::Scheme.changed(&s));
        assert!(Field::Titles.changed(&s));
    }

    #[test]
    fn a_pick_reset_keeps_fast_mode() {
        let mut s = AppState::default();
        s.composer.set_pick(Pick {
            agent: Agent::Claude,
            model: "claude-opus-5-5".into(),
            effort: "high".into(),
            fast: true,
        });
        assert!(Field::Pick(Agent::Claude).changed(&s));
        assert!(!Field::Pick(Agent::Codex).changed(&s));
        Field::Pick(Agent::Claude).reset(&mut s);
        let pick = s.composer.pick(Agent::Claude);
        assert!(pick.model.is_empty() && pick.effort.is_empty() && pick.fast);
    }

    #[test]
    fn a_line_height_saved_before_it_existed_is_the_default() {
        let mut s = AppState::default();
        s.appearance.terminal_line_height = 0.;
        assert!(!Field::LineHeight.changed(&s));
    }
}
