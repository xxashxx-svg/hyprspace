// Images that go in with a prompt: pasted ones are written to a temp file (the CLIs take image
// paths), dropped ones are used where they are. Shared by the composer and the transcript.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use gpui::{
    AnyElement, ElementId, Image, ImageFormat, IntoElement, ObjectFit, div, img, prelude::*, px,
};

use crate::colors;

const IMAGE_EXT: &[&str] = &["png", "jpg", "jpeg", "gif", "webp", "bmp"];

pub fn is_image(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| IMAGE_EXT.contains(&e.to_ascii_lowercase().as_str()))
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

/// A thumbnail of an attached image.
pub fn thumb(path: &Path, size: f32) -> AnyElement {
    img(path.to_path_buf())
        .size(px(size))
        .object_fit(ObjectFit::Cover)
        .rounded_md()
        .border_1()
        .border_color(colors::border2())
        .into_any_element()
}

/// The tray of images waiting to be sent, each with a remove button. `remove` gets the index.
pub fn tray(
    id: &str,
    images: &[PathBuf],
    remove: impl Fn(&usize, &mut gpui::Window, &mut gpui::App) + 'static,
) -> impl IntoElement {
    let remove = std::rc::Rc::new(remove);
    div()
        .flex()
        .flex_wrap()
        .gap_2()
        .children(images.iter().enumerate().map(|(ix, path)| {
            let remove = remove.clone();
            div().relative().child(thumb(path, 56.)).child(
                div()
                    .id(ElementId::Name(format!("{id}-x-{ix}").into()))
                    .absolute()
                    .top(px(-6.))
                    .right(px(-6.))
                    .size(px(16.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded_full()
                    .bg(colors::surface3())
                    .border_1()
                    .border_color(colors::border2())
                    .text_xs()
                    .cursor_pointer()
                    .child("×")
                    .on_click(move |_, w, cx| remove(&ix, w, cx)),
            )
        }))
}

#[cfg(test)]
mod tests {
    use super::*;

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
