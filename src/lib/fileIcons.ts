// Per-file-type icons for the Files tree, matched on exact filename first, then extension.
// Deliberately name-based (not OS file associations) so it's identical on every platform and for
// remote/worktree paths. Lucide only — no icon font, no extra asset.
import {
  File,
  FileArchive,
  FileAudio,
  FileBox,
  FileChartColumn,
  FileCheck,
  FileCode,
  FileCog,
  FileDiff,
  FileImage,
  FileJson,
  FileKey,
  FileLock,
  FileSliders,
  FileSpreadsheet,
  FileTerminal,
  FileText,
  FileType,
  FileVideo,
  Database,
  type LucideIcon,
} from "lucide-react";

// exact filenames win over extensions — package.json shouldn't just be "some json"
const BY_NAME: Record<string, LucideIcon> = {
  "package.json": FileBox,
  "package-lock.json": FileLock,
  "bun.lockb": FileLock,
  "yarn.lock": FileLock,
  "pnpm-lock.yaml": FileLock,
  "cargo.toml": FileBox,
  "cargo.lock": FileLock,
  "go.mod": FileBox,
  "go.sum": FileLock,
  "requirements.txt": FileBox,
  "pyproject.toml": FileBox,
  dockerfile: FileCog,
  "docker-compose.yml": FileCog,
  "docker-compose.yaml": FileCog,
  makefile: FileTerminal,
  ".gitignore": FileSliders,
  ".gitattributes": FileSliders,
  ".editorconfig": FileSliders,
  ".npmrc": FileSliders,
  ".nvmrc": FileSliders,
  license: FileKey,
  "license.md": FileKey,
  "readme.md": FileText,
  "changelog.md": FileText,
  "claude.md": FileText,
  "tsconfig.json": FileSliders,
  "vite.config.ts": FileCog,
  "tauri.conf.json": FileCog,
};

const BY_EXT: Record<string, LucideIcon> = {
  // code
  ts: FileCode, tsx: FileCode, js: FileCode, jsx: FileCode, mjs: FileCode, cjs: FileCode,
  rs: FileCode, go: FileCode, py: FileCode, rb: FileCode, php: FileCode, java: FileCode,
  kt: FileCode, swift: FileCode, c: FileCode, h: FileCode, cpp: FileCode, hpp: FileCode,
  cs: FileCode, lua: FileCode, dart: FileCode, zig: FileCode, ex: FileCode, exs: FileCode,
  vue: FileCode, svelte: FileCode, astro: FileCode, html: FileCode, htm: FileCode,
  // styles
  css: FileType, scss: FileType, sass: FileType, less: FileType,
  // data / config
  json: FileJson, jsonc: FileJson, json5: FileJson,
  yml: FileSliders, yaml: FileSliders, toml: FileSliders, ini: FileSliders, conf: FileSliders,
  xml: FileCode, plist: FileSliders,
  csv: FileSpreadsheet, tsv: FileSpreadsheet, xlsx: FileSpreadsheet, xls: FileSpreadsheet,
  db: Database, sqlite: Database, sqlite3: Database, sql: Database,
  // docs
  md: FileText, mdx: FileText, txt: FileText, rst: FileText, pdf: FileText, doc: FileText, docx: FileText,
  // shell
  sh: FileTerminal, bash: FileTerminal, zsh: FileTerminal, fish: FileTerminal,
  ps1: FileTerminal, psm1: FileTerminal, bat: FileTerminal, cmd: FileTerminal,
  // media
  png: FileImage, jpg: FileImage, jpeg: FileImage, gif: FileImage, webp: FileImage,
  bmp: FileImage, svg: FileImage, ico: FileImage, avif: FileImage,
  mp4: FileVideo, mov: FileVideo, webm: FileVideo, mkv: FileVideo, avi: FileVideo,
  mp3: FileAudio, wav: FileAudio, flac: FileAudio, ogg: FileAudio, m4a: FileAudio,
  // archives / binaries
  zip: FileArchive, gz: FileArchive, tgz: FileArchive, rar: FileArchive, "7z": FileArchive,
  tar: FileArchive, xz: FileArchive, bz2: FileArchive,
  exe: FileCog, dll: FileCog, so: FileCog, dylib: FileCog, wasm: FileCog,
  // misc
  patch: FileDiff, diff: FileDiff,
  pem: FileKey, key: FileKey, crt: FileKey, cer: FileKey,
  lock: FileLock,
  log: FileChartColumn,
  test: FileCheck,
};

export function fileIcon(name: string): LucideIcon {
  const lower = name.toLowerCase();
  const exact = BY_NAME[lower];
  if (exact) return exact;
  // prefix rules — .env, .env.local, Dockerfile.dev, Makefile.win …
  if (lower === ".env" || lower.startsWith(".env.")) return FileLock;
  if (lower.startsWith("dockerfile")) return FileCog;
  if (lower.startsWith("makefile")) return FileTerminal;
  // compound archive extensions
  if (lower.endsWith(".tar.gz") || lower.endsWith(".tar.bz2") || lower.endsWith(".tar.xz")) return FileArchive;
  const ext = lower.includes(".") ? lower.slice(lower.lastIndexOf(".") + 1) : "";
  return BY_EXT[ext] ?? File;
}
