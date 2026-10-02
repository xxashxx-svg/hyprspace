// Embed the logo in the Windows .exe. GPUI's window class loads icon resource 1 from the running
// module, so this one icon is what the title bar, taskbar, Alt+Tab and Explorer all show.
// macOS takes its icon from the app bundle instead (assets/hyprspace.icns, used when packaging).
fn main() {
    println!("cargo:rerun-if-changed=assets/hyprspace.ico");
    #[cfg(windows)]
    {
        let mut res = winresource::WindowsResource::new();
        res.set_icon_with_id("assets/hyprspace.ico", "1");
        res.compile().expect("embedding the app icon failed");
    }
}
