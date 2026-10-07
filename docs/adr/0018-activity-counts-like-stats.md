# ADR 0018: Activity counts tokens the way Claude's /stats does

- Status: Accepted
- Date: 2026-10-07

## Context

Settings, Usage, Activity shows tokens per day, per model and over a period, after Claude Code's
`/stats`. Ash compared it with `/stats` and asked whether it was right. It wasn't: it added in
and out only, read at most 160 transcripts, and took no cache.

Claude writes one reply over several transcript lines (a text block, then each tool call), and
every line repeats the reply's usage. Adding every line counts a reply once per line. That is
what `/stats` and `~/.claude/stats-cache.json` do: on 2026-10-06 the file had 1.97B tokens for
Opus 5.5, adding every line of the transcripts on disk gave 1.54B, and counting each reply once
by its message id gave 0.55B. Counting once is what the API billed.

Claude deletes transcripts after 30 days. Anything older exists only in the stats file, already
counted per line, with a total per model per day and no split.

## Decision

- Count Claude's tokens the way `/stats` does, every line, cache included.
- Every day the stats file covers (through its `lastComputedDate`) takes its total and its
  sessions from the file. The transcripts alone cover the days after it.
- A day whose split doesn't add up to its total (any day the file covers) splits its total the
  way the model splits over all time, from the file's `modelUsage`. Over all time that is exact;
  over 7 or 30 days the split is an estimate, though the totals are not.
- Codex counts each session's `total_token_usage` once, from a year of rollouts.
- The chart's subtitle says the figures are counted as `/stats` counts them.

## Consequences

- Activity agrees with `/stats` on every day the stats file covers, and the period switch never
  joins two ways of counting.
- Claude's figures run about three times what the API billed. Counting each reply once would be
  closer to the bill, but only for the last 30 days, with a cliff where the stats file takes
  over.
- The stats file updates only when someone runs `/stats`; until then the newer days come from
  the transcripts left on disk, which can be missing sessions Claude has already deleted.
