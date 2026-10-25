// What a click on a link or a named file does. Markdown is drawn without a view, so the
// transcript drawing it sets its handler here for the rows it builds.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use gpui::{App, Window};

pub type Open = Rc<dyn Fn(&str, &mut Window, &mut App)>;
pub type File = Rc<dyn Fn(&str) -> Option<PathBuf>>;
pub type Show = Rc<dyn Fn(PathBuf, &mut Window, &mut App)>;

pub struct Links {
    /// Opens what a link points at.
    pub open: Open,
    /// The local file a link or a path in code names, if there is one.
    pub file: File,
    /// Shows a picture full size.
    pub show: Show,
}

thread_local! {
    static LINKS: RefCell<Option<Rc<Links>>> = const { RefCell::new(None) };
}

pub(super) fn current() -> Option<Rc<Links>> {
    LINKS.with(|l| l.borrow().clone())
}

/// Sets `links` for the markdown drawn until the guard drops.
pub fn scope(links: Option<Rc<Links>>) -> Scope {
    Scope(LINKS.with(|l| l.replace(links)))
}

pub struct Scope(Option<Rc<Links>>);

impl Drop for Scope {
    fn drop(&mut self) {
        let before = self.0.take();
        LINKS.with(|l| *l.borrow_mut() = before);
    }
}
