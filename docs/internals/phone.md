# The phone

A paired Android phone reaches the desktop app directly, with no server between them. Code:
`crates/proto/src/phone.rs` (the messages), `crates/engine/src/phone/` (the bridge),
`crates/ui/src/phone/` and `ui/src/settings/phone.rs` (the board, the asks, Settings), and
`mobile/` (the app).

## The bridge

- **Off until switched on.** `AppState.phone` holds the switch and the network choice; on
  launch the UI sends `PhoneCommand::Enable` from it. Everywhere binds `0.0.0.0`; Tailscale only
  binds each Tailscale address (100.64.0.0/10) and nothing else.
- **TLS with a pinned certificate.** The engine makes an ECDSA certificate on first use
  (`rcgen`, kept in `phone.json` beside the state). No authority vouches for it, so the phone
  pins its SHA-256 at pairing and refuses any other. rustls uses aws-lc-rs because reqwest
  already builds it.
- **A WebSocket over it**, one JSON message per text frame (`Up` from the phone, `Down` to it).
  A connection has ten seconds to say hello or pair, and drops after a minute without a word;
  the phone pings every twenty seconds.
- **The port** is 47821, or one the OS picks when that is taken. It is saved, so a phone that
  paired once finds it again.
- **Found again after a move.** A restart can leave the computer on another address (the router
  handed out a new one) or the bridge on another port (an installed copy and a dev copy both
  want 47821). The bridge announces itself over mDNS (`_hyprspace._tcp`, with the certificate's
  fingerprint), and a phone that can't reach any saved address looks for that fingerprint,
  saves where it is now and connects, with no new pairing. Tailscale addresses don't move, and
  Tailscale only has no announcement.
- `HYPRSPACE_PHONE_BIND` pins one address, for tests and a test copy on loopback, where Windows
  doesn't ask about the firewall.

## Pairing

- Settings, Phone shows a QR code holding `hyprspace://pair?n=&h=&p=&f=&c=`: the computer's
  name, its addresses (LAN first, then Tailscale, virtual adapters left out), the port, the
  certificate's fingerprint and a 24-character secret. An 8-character code from an alphabet with
  no look-alikes works the same, for typing.
- A code works once, for five minutes, and stops after five wrong tries. Ten failed hellos or
  pairings in a minute shut the door for that minute.
- A paired phone gets a 32-byte token; `phone.json` keeps only its hash. Forget on either side
  ends it. A typed code has no fingerprint to check, so the phone trusts the first certificate and
  shows a security code (eight characters of the fingerprint) to compare with Settings.
- `PROTOCOL` moves when a message changes shape. A phone on another protocol is told which side
  to update, not let in.

## What the phone sees

- **The board** is the sidebar as data, built by the UI (`Root::board`), because only the UI
  knows which session is which thread and what each row says. It goes out when a phone is
  connected and it changed, checked every 400 ms. Titles and the doing line are cut to one short
  line: some thread titles hold a whole pasted prompt, and one board was 350 KB before that.
- **A structured thread** streams its journal: the engine hands a watcher the whole journal under
  the journal's own lock, then every entry as it is recorded, streamed text piece by piece, so the
  phone misses nothing and hears nothing twice. Reading a transcript starts no session.
- **A terminal thread** streams its screen. The desktop's emulator lives in the UI, which the
  engine can't reach, so the engine keeps its own (`mirror.rs`) for each terminal a phone watches,
  started from the last 512 KB the session printed. Watching a terminal whose session isn't
  running asks the UI to start it.

## Terminal frames

The phone keeps every line, scrollback included (2000 lines). A frame says: drop `drop` lines off
the top, cut or pad to `len`, then put each changed line at its index. Lines are numbered from the
oldest kept, so a line keeps its number as output scrolls it into history and only new or changed
lines go out. Once history is full and lines fall off, the mirror finds how far by matching the
old line hashes against the new, checked at several places so a run of blank lines can't fool it.
Any shift that lines up gives the phone the right lines; a good one only saves resending them.
Frames go out at most every 50 ms. A line is runs of `Span` with palette indexes or RGB, so the
phone draws them in the desktop theme's own ANSI colors.

## Sizing a terminal for the phone

Reading a 180-column terminal on a phone means tiny text or sideways scrolling, so the phone asks
for the PTY at its own size (`Up::Fit`). The engine resizes the PTY and its mirror, keeps the size
the desktop last asked for, and ignores the desktop's resizes meanwhile. The desktop shows a strip
saying so; typing there sends `PhoneCommand::Take` and the PTY goes back to the desktop's size.
Leaving the thread or dropping the line gives it back too. The phone re-fits on a size change
only while it still holds the terminal, so its keyboard sliding away doesn't snatch it back from
someone typing at the desktop.

## What the phone does

Everything that changes a thread goes through the UI as a `PhoneEvent::Ask`, done by the same
code as the click: a message to a structured thread goes through the transcript view, so the
desktop shows it too; a new thread starts without taking the desktop's screen; bringing back a
snoozed thread wakes it. Keystrokes for a terminal go straight to the PTY; a paste with line
breaks goes in bracketed when the program asked for that.

## The app

Kotlin and Compose, no JavaScript layer, so scrolling a long transcript or a terminal stays
smooth and the APK is 8.5 MB. It follows the desktop's theme (both sides come with the board) and
fonts (Geist, and the terminal's Nerd Font build).

- `Link` races every address the computer gave, a quarter second apart, and keeps the first that
  answers. It reconnects on its own, sooner when the network comes back or the app comes to the
  front.
- Notifications are off until switched on in the app's Settings. On, a foreground service keeps
  the line open in the background, which is the only way Android lets an app hear from a
  computer without a push server. They come from the board's changes: a thread that starts
  waiting, and if asked, one that finishes or fails. Never while the app is open, and never while
  someone is at the computer: the engine reads how long since the keyboard or mouse was used
  (`GetLastInputInfo`, `CGEventSourceSecondsSinceLastEventType`) and sets the board's `present`
  within two minutes of it.
- Typing in a terminal goes straight in, with no text box (`KeyInput`): a view that poses to the
  keyboard as an editor holding nothing, so whatever the keyboard commits, deletes or presses
  becomes terminal bytes, and a Ctrl key leaves no stray letters. The screen follows its bottom
  line unless the reader scrolled up, so the prompt stays in view when the keyboard opens.
- The QR reader is ML Kit from Google Play services, which fetches its model once; bundling it
  would add 20 MB of native code.
- The wire fixtures in `app/src/test/resources/wire` are written by the proto test
  `the_phone_reads_and_writes_these_exact_lines`. The app's tests read every desktop message and
  write the phone's own byte for byte, so the two sides can't drift apart unnoticed.
