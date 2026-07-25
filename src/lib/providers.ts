// The one list of things a pane can be. Every launcher menu (titlebar New, the pane + picker, the
// Files right-click) derives from this — adding an agent used to mean editing three hand-kept lists
// that drifted apart on icons and ordering.
import { Sparkles, Gem, Bot, SquareCode, Atom, SquareTerminal, Terminal, type LucideIcon } from "lucide-react";
import { claudeCmd, geminiCmd, codexCmd, opencodeCmd, grokCmd, WSL_CMD } from "../actions";
import { isWindows } from "../platform";

export interface ProviderDef {
  id: "claude" | "gemini" | "codex" | "opencode" | "grok" | "wsl" | "terminal";
  label: string;
  icon: LucideIcon;
  /** what gets typed into the pane's shell; undefined = leave it a plain shell */
  cmd: () => string | undefined;
  /** windows-only (wsl) */
  winOnly?: boolean;
}

const ALL: ProviderDef[] = [
  { id: "claude", label: "Claude", icon: Sparkles, cmd: () => claudeCmd() },
  { id: "gemini", label: "Gemini", icon: Gem, cmd: () => geminiCmd() },
  { id: "codex", label: "Codex", icon: Bot, cmd: () => codexCmd() },
  { id: "opencode", label: "OpenCode", icon: SquareCode, cmd: () => opencodeCmd() },
  { id: "grok", label: "Grok", icon: Atom, cmd: () => grokCmd() },
  { id: "wsl", label: "WSL (Linux)", icon: SquareTerminal, cmd: () => WSL_CMD, winOnly: true },
  { id: "terminal", label: "Terminal", icon: Terminal, cmd: () => undefined },
];

/** the providers you can actually launch on this OS, in menu order */
export const PROVIDERS: ProviderDef[] = ALL.filter((p) => !p.winOnly || isWindows);
