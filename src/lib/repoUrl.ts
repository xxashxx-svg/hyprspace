/** A git repository link found in text, with the folder name a clone would get. */
export interface RepoRef {
  url: string;
  /** "owner/name" for hosted repos, else the repo name */
  label: string;
  name: string;
}

const HTTP = /^https?:\/\/(?:www\.)?([^/\s]+)\/([^\s?#]+?)(?:\.git)?\/?$/i;
const SSH = /^(?:ssh:\/\/)?git@([^:/\s]+)[:/]([^\s]+?)(?:\.git)?\/?$/i;
const HOSTS = /^(github\.com|gitlab\.com|bitbucket\.org|codeberg\.org|gitea\.com|sr\.ht)$/i;

/** Parse one token as a repository link. Hosted links need owner/name; anything else needs .git. */
export function parseRepoUrl(token: string): RepoRef | null {
  const t = token.trim();
  if (!t) return null;
  const m = HTTP.exec(t) ?? SSH.exec(t);
  if (!m) return null;
  const host = m[1];
  const parts = m[2].split("/").filter(Boolean);
  const hosted = HOSTS.test(host);
  if (hosted ? parts.length !== 2 : !/\.git\/?$/i.test(t)) return null;
  const name = parts[parts.length - 1];
  if (!name) return null;
  return { url: t, label: hosted ? `${parts[0]}/${name}` : name, name };
}

/** Split composer text into the repository link it starts with and the task that follows. */
export function splitRepoPrompt(text: string): { repo: RepoRef; rest: string } | null {
  const trimmed = text.trim();
  const first = trimmed.split(/\s+/)[0] ?? "";
  const repo = parseRepoUrl(first);
  if (!repo) return null;
  return { repo, rest: trimmed.slice(first.length).trim() };
}

