// quick OS sniff for UI that differs by platform (macOS uses native traffic lights;
// Windows uses our own custom window controls).
export const isMac = navigator.userAgent.includes("Mac");
export const isWindows = navigator.userAgent.includes("Windows");
