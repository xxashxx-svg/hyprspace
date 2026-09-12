// What each agent CLI can be launched as: the models it takes, the effort levels each model
// understands, and how those turn into command-line flags.
//
// Sources, checked against the installed CLIs:
// - Claude Code: `claude --help` lists `--model` and `--effort low|medium|high|xhigh|max`.
// - Codex: `-m <model>` and `-c model_reasoning_effort=<level>`. Its model list and the levels each
//   model supports come from `~/.codex/models_cache.json`, which the CLI keeps fresh (see
//   stores/providers.ts). The static list below is the fallback when that file is missing.
// - Gemini, OpenCode, Grok: `-m` / `--model` as documented; no effort setting known.
// Every agent also accepts a custom model id, because vendors ship models faster than this file.

import type { ClaudePermission, CodexMode } from "../stores/settings";

export type ProviderId = "claude" | "gemini" | "codex" | "opencode" | "grok" | "wsl" | "terminal";

// How much an agent may do without asking, in the provider's own words. These are the names their
// own CLIs use, so what the app says matches what you see once the session is running:
// - Claude prints "accept edits on", "plan mode on" and "bypass permissions on" in its footer.
// - Codex calls the presets Read Only / Agent / Full Access, and `--dangerously-bypass-approvals-
//   and-sandbox` (alias `--yolo`) shows as "permissions: YOLO mode" in its session header.
// One list each, used by every screen that offers the choice — they drifted into three different
// vocabularies when each screen kept its own copy.
export const CLAUDE_PERMISSIONS: { value: ClaudePermission; label: string }[] = [
  { value: "default", label: "Ask each time" },
  { value: "acceptEdits", label: "Accept edits" },
  { value: "plan", label: "Plan mode" },
  { value: "bypass", label: "Bypass permissions" },
];

export const CODEX_MODES: { value: CodexMode; label: string }[] = [
  { value: "default", label: "Ask each time" },
  { value: "auto", label: "Agent" },
  { value: "bypass", label: "YOLO" },
];

export const AGENT_IDS: ProviderId[] = ["claude", "codex", "gemini", "opencode", "grok"];

export interface ModelOption {
  id: string; // "" means "let the CLI pick"
  label: string;
  note?: string;
  /** effort levels this model supports, lowest first; falls back to the catalog's list */
  efforts?: string[];
  /** what each level means, in the vendor's words */
  effortNotes?: Record<string, string>;
  defaultEffort?: string;
}

export interface AgentCatalog {
  id: ProviderId;
  models: ModelOption[];
  efforts: string[]; // the levels the CLI takes when a model does not say otherwise
  customModel: boolean; // accepts a free-text model id
  modelHint?: string; // placeholder for the custom model box
}

const DEFAULT: ModelOption = { id: "", label: "Default", note: "Whatever the CLI is set to" };

const CLAUDE_EFFORTS = ["low", "medium", "high", "xhigh", "max"];
const CODEX_EFFORTS = ["low", "medium", "high", "xhigh", "max"];

export const CATALOG: Record<ProviderId, AgentCatalog> = {
  claude: {
    id: "claude",
    models: [
      DEFAULT,
      { id: "claude-fable-5-1", label: "Fable 5.1", note: "Most capable" },
      { id: "claude-opus-5", label: "Opus 5" },
      { id: "claude-sonnet-5", label: "Sonnet 5" },
      { id: "claude-haiku-4-5-20251001", label: "Haiku 4.5", note: "Fastest" },
    ],
    efforts: CLAUDE_EFFORTS,
    customModel: true,
    modelHint: "claude-...",
  },
  codex: {
    id: "codex",
    models: [
      DEFAULT,
      { id: "gpt-6-astra", label: "GPT-6 Astra", efforts: [...CODEX_EFFORTS, "ultra"] },
      { id: "gpt-5.6-sol", label: "GPT-5.6 Sol", efforts: [...CODEX_EFFORTS, "ultra"] },
      { id: "gpt-5.6-terra", label: "GPT-5.6 Terra", efforts: [...CODEX_EFFORTS, "ultra"] },
      { id: "gpt-5.6-luna", label: "GPT-5.6 Luna" },
      { id: "gpt-5.5", label: "GPT-5.5", efforts: ["low", "medium", "high", "xhigh"] },
    ],
    efforts: CODEX_EFFORTS,
    customModel: true,
    modelHint: "gpt-...",
  },
  gemini: {
    id: "gemini",
    models: [
      DEFAULT,
      { id: "gemini-3-pro", label: "Gemini 3 Pro" },
      { id: "gemini-3-flash", label: "Gemini 3 Flash" },
      { id: "gemini-2.5-pro", label: "Gemini 2.5 Pro" },
      { id: "gemini-2.5-flash", label: "Gemini 2.5 Flash" },
    ],
    efforts: [],
    customModel: true,
    modelHint: "gemini-...",
  },
  opencode: {
    id: "opencode",
    models: [DEFAULT],
    efforts: [],
    customModel: true,
    modelHint: "provider/model",
  },
  grok: {
    id: "grok",
    models: [DEFAULT],
    efforts: [],
    customModel: true,
    modelHint: "grok-...",
  },
  wsl: { id: "wsl", models: [], efforts: [], customModel: false },
  terminal: { id: "terminal", models: [], efforts: [], customModel: false },
};

export const EFFORT_LABEL: Record<string, string> = {
  none: "None",
  minimal: "Minimal",
  low: "Low",
  medium: "Medium",
  high: "High",
  xhigh: "Extra high",
  max: "Max",
  ultra: "Ultra",
};

export const EFFORT_NOTE: Record<string, string> = {
  "": "The CLI picks",
  none: "No extra thinking",
  minimal: "Fastest, barely thinks",
  low: "Quick answers",
  medium: "Balanced",
  high: "Thinks longer",
  xhigh: "Thinks much longer",
  max: "Everything it has",
  ultra: "Max, plus it delegates to sub-agents",
};

/** Human label for a model id, falling back to the id itself for custom entries. */
export function modelLabel(provider: ProviderId, id: string, catalog: AgentCatalog = CATALOG[provider]): string {
  if (!id) return "Default";
  return catalog.models.find((m) => m.id === id)?.label ?? id;
}

/** The effort levels a launch can pick: the model's own list, else the CLI's. */
export function effortsFor(provider: ProviderId, modelId: string, catalog: AgentCatalog = CATALOG[provider]): string[] {
  const m = catalog.models.find((x) => x.id === modelId);
  return m?.efforts ?? catalog.efforts;
}

/** One line on what a level means for this model, the vendor's wording when it has one. */
export function effortNote(provider: ProviderId, modelId: string, level: string, catalog: AgentCatalog = CATALOG[provider]): string {
  const m = catalog.models.find((x) => x.id === modelId);
  return m?.effortNotes?.[level] ?? EFFORT_NOTE[level] ?? "Thinks harder";
}

const quote = (s: string) => (/[\s"]/.test(s) ? `"${s.replace(/"/g, '\\"')}"` : s);

/** Extra command-line flags that select a model and effort for one launch. */
export function modelFlags(provider: ProviderId, model: string, effort: string): string[] {
  const flags: string[] = [];
  switch (provider) {
    case "claude":
      if (model) flags.push("--model", quote(model));
      if (effort) flags.push("--effort", effort);
      break;
    case "codex":
      if (model) flags.push("-m", quote(model));
      if (effort) flags.push("-c", quote(`model_reasoning_effort="${effort}"`));
      break;
    case "gemini":
      if (model) flags.push("-m", quote(model));
      break;
    case "opencode":
    case "grok":
      if (model) flags.push("--model", quote(model));
      break;
    default:
      break;
  }
  return flags;
}

/** Environment variables that carry settings the CLI has no flag for. None today; kept so a
 *  pane's spawn path has one place to ask. */
export function modelEnv(_provider: ProviderId, _effort: string): Record<string, string> {
  return {};
}
