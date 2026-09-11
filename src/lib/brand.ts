// provider → brand mark, shared by the sidebar session rows and the home composer
import claudeLogo from "../assets/brand/claude.svg";
import geminiLogo from "../assets/brand/gemini.svg";
import openaiLogo from "../assets/brand/openai.svg";
import opencodeLogo from "../assets/brand/opencode.svg";
import grokLogo from "../assets/brand/grok.svg";
import linuxLogo from "../assets/brand/linux.svg";

export const PROVIDER_LOGO: Record<string, string> = {
  claude: claudeLogo,
  codex: openaiLogo, // codex = openai
  gemini: geminiLogo,
  opencode: opencodeLogo,
  grok: grokLogo,
  wsl: linuxLogo,
};

// each vendor's signature color, for tinting controls that belong to that agent.
// a "2" entry is the second stop of a gradient where the brand has one.
export const PROVIDER_COLOR: Record<string, string> = {
  claude: "#d97757",
  claude2: "#e8a07e",
  codex: "#10a37f",
  codex2: "#5ed3b3",
  gemini: "#4c8bf5",
  gemini2: "#9b72cb",
  opencode: "#8f8f8f",
  opencode2: "#e5e5e5",
  grok: "#8a8a8a",
  grok2: "#f2f2f2",
  wsl: "#e95420",
  wsl2: "#f5a06b",
  terminal: "#6b7280",
  terminal2: "#a3aab5",
};

export const PROVIDER_DESC: Record<string, string> = {
  claude: "Anthropic's coding agent",
  codex: "OpenAI's Codex CLI",
  gemini: "Google's Gemini CLI",
  opencode: "SST's open-source agent",
  grok: "xAI's Grok Build CLI",
  wsl: "Linux shell",
  terminal: "Plain shell",
};

export const PROVIDER_NAME: Record<string, string> = {
  claude: "Claude",
  codex: "Codex",
  gemini: "Gemini",
  opencode: "OpenCode",
  grok: "Grok",
  wsl: "WSL",
  terminal: "Terminal",
  image: "Image",
  editor: "Editor",
  media: "Media",
  diff: "Diff",
};
