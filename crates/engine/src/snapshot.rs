// The screenshot shortcut: Ctrl+Alt+S anywhere captures the window in front, such as Roblox Studio,
// and the UI attaches it to the thread on screen. The shortcut is the system's, held by a thread
// of its own while switched on, so it works while another app has the keyboard. Windows only.

use std::sync::Mutex;

use futures::channel::mpsc::UnboundedSender;
use hyprspace_proto::Event;

/// The hotkey thread's id while the shortcut is on, for telling it to stop.
static HELD: Mutex<Option<u32>> = Mutex::new(None);

pub fn set(on: bool, tx: UnboundedSender<Event>) {
    let mut held = HELD.lock().unwrap_or_else(|e| e.into_inner());
    match (on, *held) {
        (true, None) => *held = imp::hold(tx),
        (false, Some(thread)) => {
            imp::release(thread);
            *held = None;
        }
        _ => {}
    }
}

#[cfg(windows)]
mod imp {
    use std::ffi::c_void;
    use std::path::PathBuf;

    use futures::channel::mpsc::UnboundedSender;
    use hyprspace_proto::Event;

    #[repr(C)]
    #[derive(Default)]
    struct Rect {
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
    }

    #[repr(C)]
    #[derive(Default)]
    struct Msg {
        hwnd: isize,
        message: u32,
        wparam: usize,
        lparam: isize,
        time: u32,
        x: i32,
        y: i32,
        private: u32,
    }

    #[repr(C)]
    struct BitmapInfo {
        size: u32,
        width: i32,
        height: i32,
        planes: u16,
        bits: u16,
        compression: u32,
        image_size: u32,
        x_ppm: i32,
        y_ppm: i32,
        used: u32,
        important: u32,
        colors: [u32; 3],
    }

    #[link(name = "user32")]
    unsafe extern "system" {
        fn RegisterHotKey(hwnd: isize, id: i32, modifiers: u32, key: u32) -> i32;
        fn UnregisterHotKey(hwnd: isize, id: i32) -> i32;
        fn GetMessageW(msg: *mut Msg, hwnd: isize, min: u32, max: u32) -> i32;
        fn PostThreadMessageW(thread: u32, msg: u32, wparam: usize, lparam: isize) -> i32;
        fn GetForegroundWindow() -> isize;
        fn GetWindowRect(hwnd: isize, rect: *mut Rect) -> i32;
        fn GetWindowTextW(hwnd: isize, text: *mut u16, max: i32) -> i32;
        fn GetDC(hwnd: isize) -> isize;
        fn ReleaseDC(hwnd: isize, dc: isize) -> i32;
    }

    #[link(name = "gdi32")]
    unsafe extern "system" {
        fn CreateCompatibleDC(dc: isize) -> isize;
        fn CreateCompatibleBitmap(dc: isize, w: i32, h: i32) -> isize;
        fn SelectObject(dc: isize, obj: isize) -> isize;
        fn BitBlt(
            dc: isize,
            x: i32,
            y: i32,
            w: i32,
            h: i32,
            src: isize,
            sx: i32,
            sy: i32,
            rop: u32,
        ) -> i32;
        fn GetDIBits(
            dc: isize,
            bmp: isize,
            start: u32,
            lines: u32,
            bits: *mut c_void,
            info: *mut BitmapInfo,
            usage: u32,
        ) -> i32;
        fn DeleteObject(obj: isize) -> i32;
        fn DeleteDC(dc: isize) -> i32;
    }

    #[link(name = "dwmapi")]
    unsafe extern "system" {
        fn DwmGetWindowAttribute(hwnd: isize, attr: u32, value: *mut c_void, size: u32) -> i32;
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetCurrentThreadId() -> u32;
    }

    const MOD_ALT: u32 = 0x1;
    const MOD_CONTROL: u32 = 0x2;
    const MOD_NOREPEAT: u32 = 0x4000;
    const WM_HOTKEY: u32 = 0x0312;
    const WM_QUIT: u32 = 0x0012;
    const EXTENDED_FRAME_BOUNDS: u32 = 9;

    pub fn hold(tx: UnboundedSender<Event>) -> Option<u32> {
        let (id_tx, id_rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            // SAFETY: plain calls on this thread's own message queue
            let held = unsafe {
                let _ = id_tx.send(GetCurrentThreadId());
                RegisterHotKey(0, 1, MOD_CONTROL | MOD_ALT | MOD_NOREPEAT, b'S' as u32) != 0
            };
            if !held {
                let _ = tx.unbounded_send(Event::Snapshot(Err(
                    "Another app already uses Ctrl+Alt+S.".into(),
                )));
                return;
            }
            let mut msg = Msg::default();
            // SAFETY: `msg` is a MSG the call fills in
            while unsafe { GetMessageW(&mut msg, 0, 0, 0) } > 0 {
                if msg.message == WM_HOTKEY {
                    let _ = tx.unbounded_send(Event::Snapshot(capture()));
                }
            }
            // SAFETY: the hotkey this thread registered above
            unsafe { UnregisterHotKey(0, 1) };
        });
        id_rx.recv().ok()
    }

    fn save(rgba: Vec<u8>, w: u32, h: u32) -> Result<PathBuf, String> {
        let dir = std::env::temp_dir().join("hyprspace-images");
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_millis());
        let path = dir.join(format!("snapshot-{ms}.png"));
        let img = image::RgbaImage::from_raw(w, h, rgba).ok_or("The capture came out empty.")?;
        img.save(&path).map_err(|e| e.to_string())?;
        Ok(path)
    }

    pub fn release(thread: u32) {
        // SAFETY: a quit message to the hotkey thread's queue, which ends its loop
        unsafe { PostThreadMessageW(thread, WM_QUIT, 0, 0) };
    }

    fn capture() -> Result<(PathBuf, String), String> {
        // SAFETY: Win32 calls on the window in front; every handle made here is freed below
        unsafe {
            let hwnd = GetForegroundWindow();
            if hwnd == 0 {
                return Err("No window is in front to capture.".into());
            }
            let mut r = Rect::default();
            // the frame without its invisible resize border, which GetWindowRect includes
            let size = std::mem::size_of::<Rect>() as u32;
            let framed = DwmGetWindowAttribute(
                hwnd,
                EXTENDED_FRAME_BOUNDS,
                (&mut r as *mut Rect).cast(),
                size,
            ) == 0;
            if !framed {
                GetWindowRect(hwnd, &mut r);
            }
            let (w, h) = (r.right - r.left, r.bottom - r.top);
            if w <= 0 || h <= 0 {
                return Err("The window in front has no size.".into());
            }
            let mut title = vec![0u16; 512];
            let n = GetWindowTextW(hwnd, title.as_mut_ptr(), title.len() as i32);
            let title = String::from_utf16_lossy(&title[..n.max(0) as usize]);

            let screen = GetDC(0);
            let mem = CreateCompatibleDC(screen);
            let bmp = CreateCompatibleBitmap(screen, w, h);
            let old = SelectObject(mem, bmp);
            BitBlt(
                mem,
                0,
                0,
                w,
                h,
                screen,
                r.left,
                r.top,
                0x00CC_0020 | 0x4000_0000,
            );
            let mut info = BitmapInfo {
                size: 40,
                width: w,
                height: -h,
                planes: 1,
                bits: 32,
                compression: 0,
                image_size: 0,
                x_ppm: 0,
                y_ppm: 0,
                used: 0,
                important: 0,
                colors: [0; 3],
            };
            let mut px = vec![0u8; (w * h * 4) as usize];
            let lines = GetDIBits(mem, bmp, 0, h as u32, px.as_mut_ptr().cast(), &mut info, 0);
            SelectObject(mem, old);
            DeleteObject(bmp);
            DeleteDC(mem);
            ReleaseDC(0, screen);
            if lines == 0 {
                return Err("Windows wouldn't hand over the window's pixels.".into());
            }
            for p in px.as_chunks_mut::<4>().0 {
                p.swap(0, 2);
                p[3] = 255;
            }
            save(px, w as u32, h as u32).map(|path| (path, title))
        }
    }
}

#[cfg(not(windows))]
mod imp {
    use futures::channel::mpsc::UnboundedSender;
    use hyprspace_proto::Event;

    pub fn hold(_: UnboundedSender<Event>) -> Option<u32> {
        None
    }

    pub fn release(_: u32) {}
}
