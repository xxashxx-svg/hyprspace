import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { emit } from "@tauri-apps/api/event";

// The settings window is defined statically in tauri.conf.json (label "settings", hidden at
// startup). Opening reveals + raises it. The brief always-on-top toggle forces it above the
// maximized main window — plain setFocus doesn't reliably raise a frameless window on Windows.
export async function openSettingsWindow(tab?: string): Promise<void> {
  const w = await WebviewWindow.getByLabel("settings");
  if (!w) return;
  await w.unminimize().catch(() => {});
  await w.center().catch(() => {});
  await w.show().catch(() => {});
  await w.setAlwaysOnTop(true).catch(() => {});
  await w.setFocus().catch(() => {});
  if (tab) await emit("settings:tab", tab);
  setTimeout(() => {
    void w.setAlwaysOnTop(false).catch(() => {});
  }, 700);
}
