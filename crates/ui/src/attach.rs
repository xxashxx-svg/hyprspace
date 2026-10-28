// Images that go in with a prompt: pasted ones are written to a temp file (the CLIs take image
// paths), dropped ones are used where they are. Shared by the composer and the transcript, with
// how a dropped file's path is typed, which the terminal shares too.

use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use gpui::{
    Anchor, AnyElement, App, Asset, Context, ElementId, Image, ImageFormat, IntoElement, ObjectFit,
    Pixels, Point, RenderImage, Window, anchored, deferred, div, img, point, prelude::*,
};
pub use hyprspace_media::is_video;

use crate::colors;
use crate::zoom::px;

const IMAGE_EXT: &[&str] = &["png", "jpg", "jpeg", "gif", "webp", "bmp"];

pub fn is_image(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| IMAGE_EXT.contains(&e.to_ascii_lowercase().as_str()))
}

/// An image or a video, which a click opens over the window.
pub fn previews(path: &Path) -> bool {
    is_image(path) || is_video(path)
}

pub fn render_image(f: hyprspace_media::Frame) -> Option<Arc<RenderImage>> {
    let buffer = image::RgbaImage::from_raw(f.width, f.height, f.bgra)?;
    Some(Arc::new(RenderImage::new(vec![image::Frame::new(buffer)])))
}

struct VideoThumb;

impl Asset for VideoThumb {
    type Source = PathBuf;
    type Output = Option<Arc<RenderImage>>;

    // an async fn would hold on to the App, and the cache wants a 'static future
    #[allow(clippy::manual_async_fn)]
    fn load(path: PathBuf, _: &mut App) -> impl Future<Output = Self::Output> + Send + 'static {
        async move { hyprspace_media::thumbnail(&path, (480, 480)).and_then(render_image) }
    }
}

fn video_thumb(path: &Path, size: f32) -> AnyElement {
    let source = path.to_path_buf();
    div()
        .relative()
        .size(px(size))
        .rounded_md()
        .overflow_hidden()
        .bg(colors::ink(0.08))
        .border_1()
        .border_color(colors::border2())
        .child(
            img(move |window: &mut Window, cx: &mut App| {
                window
                    .use_asset::<VideoThumb>(&source, cx)
                    .flatten()
                    .map(Ok)
            })
            .size_full()
            .object_fit(ObjectFit::Cover),
        )
        .child(
            div()
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .size(px((size * 0.3).clamp(18., 34.)))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded_full()
                        .bg(colors::surface3())
                        .border_1()
                        .border_color(colors::border2())
                        .child(crate::assets::icon(
                            "play",
                            (size * 0.13).clamp(9., 15.),
                            colors::text1(),
                        )),
                ),
        )
        .into_any_element()
}

/// What gets typed for a file: its path, quoted when it holds a space (Windows profile folders
/// often do) or anything a shell reads as syntax, like zsh's parentheses in "shot(1).png", so an
/// agent reads it as one argument, then a space to keep typing after. Forward slashes on
/// Windows: the CLIs take them, and a bash in a terminal won't eat them.
pub fn path_text(path: &Path) -> String {
    let mut p = path.display().to_string();
    if cfg!(windows) {
        p = p.replace('\\', "/");
    }
    let plain = |c: char| c.is_alphanumeric() || "/._-:~+@,=".contains(c);
    if !p.chars().all(plain) {
        format!("\"{p}\" ")
    } else {
        format!("{p} ")
    }
}

/// Paths dropped on a prompt box: the files, to go in with the message, and the folders' paths
/// to type in, as a terminal types a dropped folder.
pub fn split_drop(paths: &[PathBuf]) -> (Vec<PathBuf>, String) {
    let files = paths.iter().filter(|p| !p.is_dir()).cloned().collect();
    let text = paths
        .iter()
        .filter(|p| p.is_dir())
        .map(|p| path_text(p))
        .collect();
    (files, text)
}

/// Writes a pasted image to `hyprspace-images` in the temp folder and returns its path.
pub fn save(image: &Image) -> std::io::Result<PathBuf> {
    static COUNT: AtomicU64 = AtomicU64::new(0);
    let dir = std::env::temp_dir().join("hyprspace-images");
    std::fs::create_dir_all(&dir)?;
    let ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let n = COUNT.fetch_add(1, Ordering::Relaxed);
    // the agents read png, jpeg, gif and webp; a Windows screenshot often reaches the clipboard
    // only as a bitmap, so that one becomes a png
    let (bytes, ext) = match image.format {
        ImageFormat::Bmp => (to_png(&image.bytes)?, "png"),
        f => (image.bytes.clone(), f.extension()),
    };
    let path = dir.join(format!("paste-{ms}-{n}.{ext}"));
    std::fs::write(&path, bytes)?;
    Ok(path)
}

pub fn save_later<T: 'static>(
    images: Vec<Image>,
    cx: &mut Context<T>,
    done: impl FnOnce(&mut T, Vec<PathBuf>, &mut Context<T>) + 'static,
) {
    // a screenshot arrives as a bitmap, and turning it into a png froze the window for a moment
    let saving =
        cx.background_spawn(async move { images.iter().filter_map(|i| save(i).ok()).collect() });
    cx.spawn(async move |this, cx| {
        let saved = saving.await;
        let _ = this.update(cx, |this, cx| done(this, saved, cx));
    })
    .detach();
}

/// Fast compression and no filter search: the file is read once by the agent, and the default
/// encoder takes a second or two on a screenshot (the Tauri app measured it).
fn to_png(bmp: &[u8]) -> std::io::Result<Vec<u8>> {
    use image::ImageEncoder;
    use image::codecs::png::{CompressionType, FilterType, PngEncoder};
    let img = image::load_from_memory_with_format(bmp, image::ImageFormat::Bmp)
        .map_err(std::io::Error::other)?
        .into_rgba8();
    let mut out = Vec::new();
    PngEncoder::new_with_quality(&mut out, CompressionType::Fast, FilterType::NoFilter)
        .write_image(
            img.as_raw(),
            img.width(),
            img.height(),
            image::ExtendedColorType::Rgba8,
        )
        .map_err(std::io::Error::other)?;
    Ok(out)
}

/// A thumbnail of an attached image, or a tile with the name of any other file.
pub fn thumb(path: &Path, size: f32) -> AnyElement {
    if is_video(path) {
        return video_thumb(path, size);
    }
    if is_image(path) {
        // Cover scales past the box, and an img paints outside its bounds unless a parent clips
        return div()
            .size(px(size))
            .flex_none()
            .rounded_md()
            .overflow_hidden()
            .border_1()
            .border_color(colors::border2())
            .child(
                img(path.to_path_buf())
                    .size_full()
                    .object_fit(ObjectFit::Cover),
            )
            .into_any_element();
    }
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_uppercase())
        .unwrap_or_else(|| "FILE".into());
    div()
        .size(px(size))
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(px(3.))
        .px(px(5.))
        .rounded_md()
        .bg(colors::ink(0.05))
        .border_1()
        .border_color(colors::border2())
        .child(crate::assets::icon(
            "file-text",
            (size * 0.28).clamp(14., 24.),
            colors::text2(),
        ))
        .child(
            div()
                .text_size(px(9.5))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(colors::text2())
                .child(ext),
        )
        .when(size >= 80., |d| {
            d.child(
                div()
                    .w_full()
                    .text_size(px(10.5))
                    .text_color(colors::text3())
                    .text_center()
                    .truncate()
                    .child(name.clone()),
            )
        })
        .id(ElementId::Name(format!("file-{name}").into()))
        .tooltip(crate::widgets::tip_text(name))
        .into_any_element()
}

static HELD: AtomicBool = AtomicBool::new(false);

pub fn held() -> bool {
    HELD.load(Ordering::Relaxed)
}

pub fn hold(on: bool) -> bool {
    HELD.swap(on, Ordering::Relaxed) != on
}

/// What a paste types at the cursor, so the prompt can point at the nth image in the tray.
pub fn marker(word: &str, n: usize) -> String {
    format!("[{word} {n}]")
}

const WORDS: [&str; 4] = ["Image", "Video", "Thread", "File"];

pub fn word(path: &Path) -> &'static str {
    if path
        .parent()
        .and_then(|p| p.file_name())
        .is_some_and(|n| n == hyprspace_proto::wire::THREAD_EXPORTS)
    {
        "Thread"
    } else if is_video(path) {
        "Video"
    } else if is_image(path) {
        "Image"
    } else {
        "File"
    }
}

/// Types a marker at the cursor for each of `count` new images, numbered from `first`.
pub fn mark(
    input: &gpui::Entity<crate::input::TextInput>,
    first: usize,
    words: &[&str],
    cx: &mut App,
) {
    input.update(cx, |i, cx| {
        let before = i.text()[..i.cursor_offset()].chars().next_back();
        let lead = if before.is_some_and(|c| !c.is_whitespace()) {
            " "
        } else {
            ""
        };
        let tags: String = words
            .iter()
            .enumerate()
            .map(|(i, w)| marker(w, first + i) + " ")
            .collect();
        i.insert(&format!("{lead}{tags}"), cx);
    });
}

/// Takes the tray image at `ix` out of the text in `input`, counting later markers down.
pub fn unmark(input: &gpui::Entity<crate::input::TextInput>, ix: usize, cx: &mut App) {
    input.update(cx, |i, cx| {
        let text = renumber(i.text(), ix);
        if text != i.text() {
            i.set_text(text, cx);
        }
    });
}

/// Takes out of `images` the ones whose markers an edit removed, counting the markers left down.
pub fn drop_unmarked(
    images: &mut Vec<PathBuf>,
    gone: &[usize],
    input: &gpui::Entity<crate::input::TextInput>,
    cx: &mut App,
) {
    let mut gone = gone.to_vec();
    gone.sort_unstable_by(|a, b| b.cmp(a));
    gone.dedup();
    for n in gone {
        if (1..=images.len()).contains(&n) {
            images.remove(n - 1);
            unmark(input, n - 1, cx);
        }
    }
}

/// Where `text` names a tray image, and which one, counting from 1.
pub fn markers(text: &str) -> Vec<(Range<usize>, usize)> {
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(at) = text[from..].find('[') {
        let start = from + at;
        from = start + 1;
        let Some(word) = WORDS
            .iter()
            .find(|w| text[from..].starts_with(*w) && text[from + w.len()..].starts_with(' '))
        else {
            continue;
        };
        let head = from + word.len() + 1;
        let rest = &text[head..];
        let len = rest
            .find(|c: char| !c.is_ascii_digit())
            .unwrap_or(rest.len());
        if len > 0
            && rest[len..].starts_with(']')
            && let Ok(n) = rest[..len].parse()
        {
            out.push((start..head + len + 1, n));
        }
    }
    out
}

/// `text` once the tray's image at `ix` is removed: its markers go and the later ones count down.
pub fn renumber(text: &str, ix: usize) -> String {
    let gone = ix + 1;
    let mut out = String::with_capacity(text.len());
    let mut last = 0;
    for (range, n) in markers(text) {
        out.push_str(&text[last..range.start]);
        last = range.end;
        if n == gone {
            if text[last..].starts_with(' ') {
                last += 1;
            }
        } else if n > gone {
            let word = text[range.start + 1..].split(' ').next().unwrap_or("Image");
            out.push_str(&marker(word, n - 1));
        } else {
            out.push_str(&text[range]);
        }
    }
    out.push_str(&text[last..]);
    out
}

/// The tray image a marker names, floated just over the marker while the pointer rests on it.
pub fn peek(images: &[PathBuf], at: Option<(usize, Point<Pixels>)>) -> Option<AnyElement> {
    let (n, p) = at?;
    let path = images.get(n.checked_sub(1)?)?;
    let picture = if is_image(path) {
        img(path.clone())
            .w(px(280.))
            .h(px(180.))
            .rounded(px(6.))
            .object_fit(ObjectFit::Contain)
            .into_any_element()
    } else {
        thumb(path, 120.)
    };
    let card = div()
        .p(px(4.))
        .rounded(px(9.))
        .border_1()
        .border_color(colors::border2())
        .bg(colors::surface2())
        .shadow(colors::shadow())
        .child(picture);
    let at = point(p.x, p.y - px(6.));
    Some(
        deferred(
            anchored()
                .anchor(Anchor::BottomLeft)
                .position(at)
                .child(card),
        )
        .with_priority(2)
        .into_any_element(),
    )
}

/// The tray of images waiting to be sent, each with a remove button. `remove` gets the index.
pub fn tray(
    id: &str,
    images: &[PathBuf],
    open: impl Fn(&PathBuf, &mut gpui::Window, &mut gpui::App) + 'static,
    remove: impl Fn(&usize, &mut gpui::Window, &mut gpui::App) + 'static,
) -> impl IntoElement {
    let open = std::rc::Rc::new(open);
    let remove = std::rc::Rc::new(remove);
    div()
        .flex()
        .flex_wrap()
        .gap_2()
        .children(images.iter().enumerate().map(|(ix, path)| {
            let (open, remove, shown) = (open.clone(), remove.clone(), path.clone());
            let preview = div()
                .id(ElementId::Name(format!("{id}-{ix}").into()))
                .child(thumb(path, 56.))
                .when(previews(path), |d| {
                    d.cursor_pointer()
                        .on_click(move |_, w, cx| open(&shown, w, cx))
                });
            div().relative().child(preview).child(
                div()
                    .id(ElementId::Name(format!("{id}-x-{ix}").into()))
                    .absolute()
                    .top(px(-8.))
                    .right(px(-8.))
                    .size(px(20.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded_full()
                    .bg(colors::surface3())
                    .border_1()
                    .border_color(colors::border2())
                    .hover(|s| s.border_color(colors::text3()))
                    .cursor_pointer()
                    .occlude()
                    .child(crate::assets::icon("x", 11., colors::text1()))
                    .on_click(move |_, w, cx| remove(&ix, w, cx)),
            )
        }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removing_an_image_drops_its_marker_and_counts_the_rest_down() {
        let text = "see [Image 1] and [Video 2] then [Video 3], not [Image x] or [Image 3";
        assert_eq!(
            markers(text).iter().map(|(_, n)| *n).collect::<Vec<_>>(),
            [1, 2, 3]
        );
        assert_eq!(
            renumber(text, 1),
            "see [Image 1] and then [Video 2], not [Image x] or [Image 3"
        );
        assert!(markers("[Videos 1] [Video1]").is_empty());
        assert_eq!(renumber("[Image 1] hi", 0), "hi");
    }

    #[test]
    fn paths_stay_one_argument() {
        assert_eq!(path_text(Path::new("/tmp/a.png")), "/tmp/a.png ");
        assert_eq!(
            path_text(Path::new("C:/Users/First Last/a.png")),
            "\"C:/Users/First Last/a.png\" "
        );
        assert_eq!(
            path_text(Path::new("/tmp/shot(1).png")),
            "\"/tmp/shot(1).png\" "
        );
    }

    #[test]
    fn a_drop_attaches_files_and_types_folders() {
        let dir = std::env::temp_dir().join(format!("hs-drop-{}", std::process::id()));
        let folder = dir.join("src");
        std::fs::create_dir_all(&folder).unwrap();
        let paths = [dir.join("a.png"), folder.clone(), dir.join("b.rs")];
        let (files, text) = split_drop(&paths);
        assert_eq!(files, [dir.join("a.png"), dir.join("b.rs")]);
        assert_eq!(text, path_text(&folder));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_pasted_bitmap_is_saved_as_png() {
        let mut bmp = Vec::new();
        image::RgbaImage::from_pixel(3, 2, image::Rgba([10, 20, 30, 255]))
            .write_to(&mut std::io::Cursor::new(&mut bmp), image::ImageFormat::Bmp)
            .unwrap();
        let path = save(&Image::from_bytes(ImageFormat::Bmp, bmp)).unwrap();
        assert_eq!(path.extension().unwrap(), "png");
        let back = image::open(&path).unwrap();
        assert_eq!((back.width(), back.height()), (3, 2));
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn knows_images_by_extension() {
        assert!(is_image(Path::new("C:/a/Shot.PNG")));
        assert!(is_image(Path::new("b.webp")));
        assert!(!is_image(Path::new("notes.txt")));
        assert!(!is_image(Path::new("png")));
    }
}
