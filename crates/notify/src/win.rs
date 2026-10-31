use std::ffi::c_void;

use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use windows::Win32::Foundation::{COLORREF, HWND, RECT, TRUE};
use windows::Win32::Graphics::Gdi::{
    ANTIALIASED_QUALITY, BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CLIP_DEFAULT_PRECIS, CreateBitmap,
    CreateCompatibleDC, CreateDIBSection, CreateFontW, DEFAULT_CHARSET, DIB_RGB_COLORS, DT_CENTER,
    DT_NOPREFIX, DT_SINGLELINE, DT_VCENTER, DeleteDC, DeleteObject, DrawTextW, FW_BOLD, GdiFlush,
    OUT_DEFAULT_PRECIS, SelectObject, SetBkMode, SetTextColor, TRANSPARENT,
};
use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance};
use windows::Win32::UI::Shell::{ITaskbarList3, TaskbarList};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateIconIndirect, DestroyIcon, FLASHW_TIMERNOFG, FLASHW_TRAY, FLASHWINFO, FlashWindowEx,
    GetSystemMetrics, HICON, ICONINFO, SM_CXSMICON,
};
use windows::core::{PCWSTR, Result, w};

const RED: [f32; 3] = [196., 43., 28.];

pub struct Taskbar(HWND);

impl Taskbar {
    pub fn of(window: &impl HasWindowHandle) -> Option<Self> {
        match window.window_handle().ok()?.as_raw() {
            RawWindowHandle::Win32(h) => Some(Self(HWND(h.hwnd.get() as *mut c_void))),
            _ => None,
        }
    }

    pub fn badge(&self, count: usize) {
        let _ = self.overlay(count);
    }

    pub fn flash(&self) {
        let info = FLASHWINFO {
            cbSize: size_of::<FLASHWINFO>() as u32,
            hwnd: self.0,
            dwFlags: FLASHW_TRAY | FLASHW_TIMERNOFG,
            uCount: 0,
            dwTimeout: 0,
        };
        unsafe {
            let _ = FlashWindowEx(&info);
        }
    }

    fn overlay(&self, count: usize) -> Result<()> {
        unsafe {
            let taskbar: ITaskbarList3 =
                CoCreateInstance(&TaskbarList, None, CLSCTX_INPROC_SERVER)?;
            taskbar.HrInit()?;
            let Some(label) = crate::label(count) else {
                return taskbar.SetOverlayIcon(self.0, HICON::default(), PCWSTR::null());
            };
            let icon = draw(&label)?;
            let words: Vec<u16> = format!("{count} need you")
                .encode_utf16()
                .chain([0])
                .collect();
            let set = taskbar.SetOverlayIcon(self.0, icon, PCWSTR(words.as_ptr()));
            let _ = DestroyIcon(icon);
            set
        }
    }
}

fn draw(label: &str) -> Result<HICON> {
    unsafe {
        let size = GetSystemMetrics(SM_CXSMICON).max(16);
        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: size,
                biHeight: -size,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let dc = CreateCompatibleDC(None);
        let mut bits = std::ptr::null_mut();
        let color = match CreateDIBSection(Some(dc), &info, DIB_RGB_COLORS, &mut bits, None, 0) {
            Ok(color) => color,
            Err(e) => {
                let _ = DeleteDC(dc);
                return Err(e);
            }
        };
        let old = SelectObject(dc, color.into());
        let height = if label.len() > 1 {
            size * 5 / 8
        } else {
            size * 3 / 4
        };
        let font = CreateFontW(
            -height,
            0,
            0,
            0,
            FW_BOLD.0 as i32,
            0,
            0,
            0,
            DEFAULT_CHARSET,
            OUT_DEFAULT_PRECIS,
            CLIP_DEFAULT_PRECIS,
            ANTIALIASED_QUALITY,
            0,
            w!("Segoe UI"),
        );
        let old_font = SelectObject(dc, font.into());
        SetTextColor(dc, COLORREF(0x00FF_FFFF));
        SetBkMode(dc, TRANSPARENT);
        let mut rect = RECT {
            left: 0,
            top: 0,
            right: size,
            bottom: size,
        };
        let mut text: Vec<u16> = label.encode_utf16().collect();
        DrawTextW(
            dc,
            &mut text,
            &mut rect,
            DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX,
        );
        let _ = GdiFlush();
        let side = size as usize;
        paint(
            std::slice::from_raw_parts_mut(bits as *mut u32, side * side),
            side,
        );
        SelectObject(dc, old_font);
        let _ = DeleteObject(font.into());
        SelectObject(dc, old);
        let _ = DeleteDC(dc);
        let mask_bits = vec![0u8; side.div_ceil(16) * 2 * side];
        let mask = CreateBitmap(size, size, 1, 1, Some(mask_bits.as_ptr() as *const c_void));
        let icon = CreateIconIndirect(&ICONINFO {
            fIcon: TRUE,
            xHotspot: 0,
            yHotspot: 0,
            hbmMask: mask,
            hbmColor: color,
        });
        let _ = DeleteObject(mask.into());
        let _ = DeleteObject(color.into());
        icon
    }
}

// GDI text leaves alpha at zero, so the white text's brightness is its coverage
fn paint(pixels: &mut [u32], side: usize) {
    let r = side as f32 / 2.;
    for (i, p) in pixels.iter_mut().enumerate() {
        let x = (i % side) as f32 + 0.5 - r;
        let y = (i / side) as f32 + 0.5 - r;
        let alpha = (r - (x * x + y * y).sqrt() + 0.5).clamp(0., 1.);
        let text = (*p & 0xff) as f32 / 255.;
        let [red, green, blue] = RED.map(|c| (c + (255. - c) * text) as u32);
        *p = (((alpha * 255.) as u32) << 24) | (red << 16) | (green << 8) | blue;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_badge_is_a_red_circle_with_white_text() {
        let side = 16;
        let mut pixels = vec![0u32; side * side];
        pixels[8 * side + 8] = 0x00FF_FFFF;
        paint(&mut pixels, side);
        assert_eq!(pixels[0] >> 24, 0, "a corner is clear");
        assert_eq!(pixels[4 * side + 8], 0xFFC4_2B1C, "inside is opaque red");
        assert_eq!(pixels[8 * side + 8], 0xFFFF_FFFF, "text is white");
    }
}
