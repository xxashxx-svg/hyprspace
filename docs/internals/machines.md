# Other computers

A thread can run on another computer you own. Its CLI, its folder and its session live there;
this computer shows it in the sidebar and drives it as if it were local. Code:
`crates/proto/src/peer.rs` (the UI and engine messages), `crates/engine/src/peers/` (the
connections), `crates/engine/src/phone/` (the host's side, shared with the phone) and
`crates/ui/src/machines/` (the computers in Settings, the sidebar and the composer).

## The shape

- **Direct, or through a relay that can't read it.** The computer that runs a thread is its
  host. It runs the same bridge a phone pairs with (docs/internals/phone.md): TLS under a
  certificate the client pins, a WebSocket over it, pairing by code, a revocable token per
  device. A computer reaches another on the local network or over Tailscale, the way the phone
  does. With the host's network set to Anywhere, it can also dial the host by an iroh key
  (`phone/far.rs`): iroh punches through both routers when it can and goes through n0's public
  relays when it can't. The bridge's TLS runs inside that stream, so a relay only sees which keys
  talk and encrypted bytes. There is no account and no server of ours.
- **The key comes from the certificate.** The iroh key is derived from the bridge's TLS key, so
  it lasts as long as pairings do and needs no storage of its own. The pairing link carries it as
  `i`, and every welcome repeats it, so computers paired before Anywhere was on learn it on their
  next connection. A client dials the local addresses first and the key 750 ms later, and the
  first that answers wins. The lookup can miss a host for a few seconds after it starts
  listening, so the dial retries until its 15 second limit.
- **The host owns the thread.** It lives in the host's state and sidebar and in its journal, and
  the host's UI does what the other computer asks the same way a click would (`Ask`), so both
  screens show it. A thread started from here on the host appears in the host's sidebar too.
- **The client mirrors.** Each connected host's board (its sidebar as data) becomes runtime-only
  spaces and threads in this computer's sidebar, marked with the host's name. They take ids from
  a range of their own and are never saved here. While a host is out of reach its threads stay
  listed with no status, and the engine keeps redialing; when it is unpaired they go away.

## Sessions are routed, not reimplemented

The transcript and terminal views talk to the engine about a `SessionId`, as they do for local
threads. Before a remote thread's view opens, the UI binds its session to the host and thread
(`PeerCommand::Bind`), and the engine forwards what the view sends:

| The view sends | The engine sends the host |
|---|---|
| `LoadJournal` | `Watch`: the journal, then each entry as the host records it |
| `OpenStructured`, `Send` | `Ask::Send`, after uploading attached images |
| `Interrupt`, `Approve` | `Ask::Interrupt`, `Ask::Approve` |
| `OpenTerminal` | `Attach`: the last 512 KB the terminal printed, then its output as it comes |
| `WriteTerminal`, `ResizeTerminal` | the bytes, `Size` |

Answers come back as the events a local session gives (`Journal`, `Run`, `TerminalOutput`), so
the views draw a remote thread with the same code and at the same speed. A transcript drops the
host's copy of a prompt it already showed when it sent it.

## Starting a thread there

The composer's computer chip (shown once a computer is paired) picks where the next thread runs.
Picking a host moves the composer to the space with the same name there, or opens the folder
browser over the host's folders. A folder that isn't a space on the host yet gets a placeholder
space here until the first thread starts in it; `Ask::New` names the folder, the host makes the
space, and its next board replaces the placeholder. The start carries a request id, the host
answers `Started` with the new thread, and this computer opens it once the board has it.
Attached images are uploaded first and passed as host paths.

## Pairing

Settings, Computers lists the hosts this computer uses (with Forget), takes a pairing link or a
code typed for a host found on the network, and on the host side shows the code to give out and
the computers that may connect (with Revoke). Hosts are looked for on the network only while that
page is open.

## Terminals

A computer gets the terminal's raw output, not the phone's line frames, so its own emulator
keeps the cursor, mouse modes, colors and scrollback exactly as the program drew them. Output
travels in binary frames compressed with one deflate stream per connection; a redrawing TUI
repeats itself, and the shared window makes it about ten times smaller. Keystrokes go up as
binary frames, uncompressed, since they are a few bytes each.

The PTY can only have one size. The computer that types in a terminal last gets it at its own
size, and the host shows the strip the phone uses ("Sized for Laptop. Typing here gives it
back.").

## Files and git

The dock's file tree and git tab and the file viewer send `FolderCommand`s. For a remote thread
they go to the host (`PeerCommand::Folder`), whose engine answers them with the same code it uses
for its own UI, and the answers come back tagged with the host so a path that exists on both
computers can't land in the wrong view. Opening a folder in another app is local only.

## Trust

A paired computer can do what a paired phone can, which is what you can do at the host: start
agents with any permission, read and write files in its folders, commit and push. Revoke it on
the host to end that. Pairing, tokens and the pinned certificate work as they do for a phone.
