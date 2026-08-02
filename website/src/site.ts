/* Every outbound link on the site. */
export const RELEASES = "https://github.com/xxashxx-svg/hyprspace/releases"
export const REPO = "https://github.com/xxashxx-svg/hyprspace"

export const DOWNLOAD_WIN = `${RELEASES}/latest/download/HyprSpace-windows-x64-setup.exe`
export const DOWNLOAD_MAC = `${RELEASES}/latest/download/HyprSpace-macos-aarch64.dmg`
// AppImage rather than the .deb: it's the self-updating build, and it runs on any distro
export const DOWNLOAD_LINUX = `${RELEASES}/latest/download/HyprSpace-linux-x86_64.AppImage`

// v0.15.1 carries the AppImage and the .deb, so the download above resolves. Set this back to false
// only if a release ever ships without Linux artifacts, so the site cannot offer a 404.
export const LINUX_RELEASED = true
