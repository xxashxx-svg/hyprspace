/* Every outbound link on the site. */
export const RELEASES = "https://github.com/xxashxx-svg/hyprspace/releases"
export const REPO = "https://github.com/xxashxx-svg/hyprspace"

export const DOWNLOAD_WIN = `${RELEASES}/latest/download/HyprSpace-windows-x64-setup.exe`
export const DOWNLOAD_MAC = `${RELEASES}/latest/download/HyprSpace-macos-aarch64.dmg`
// AppImage rather than the .deb: it's the self-updating build, and it runs on any distro
export const DOWNLOAD_LINUX = `${RELEASES}/latest/download/HyprSpace-linux-x86_64.AppImage`

// The Linux job is written but has never run, so no release carries that asset yet and the URL
// above 404s. Until the first release with Linux artifacts is out, the site offers Linux users the
// source build instead, which does work today. Flip this in the same change that publishes it.
export const LINUX_RELEASED = false
