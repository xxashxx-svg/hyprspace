// The typed channel itself. In-process it is a pair of unbounded queues, so a message moves
// without being serialized; a headless engine or the phone can later carry the same types over a
// socket without the UI changing.

use futures::channel::mpsc::{UnboundedReceiver, UnboundedSender};

use crate::peer::PeerCommand;
use crate::wire::{Command, Event};

/// The UI's only handle on the engine. Cheap to clone, one per view if it wants.
#[derive(Clone)]
pub struct Client {
    tx: UnboundedSender<Command>,
    via: Option<String>,
}

/// Every event the engine emits, in order. One consumer routes them by session.
pub type Events = UnboundedReceiver<Event>;

impl Client {
    pub fn new(tx: UnboundedSender<Command>) -> Self {
        Self { tx, via: None }
    }

    /// The same channel, with its folder commands sent to a paired computer's folders instead.
    pub fn via(&self, peer: Option<String>) -> Self {
        Self {
            tx: self.tx.clone(),
            via: peer,
        }
    }

    /// Fire and forget. A send after the engine stopped is dropped, which only happens while the
    /// app is quitting.
    pub fn send(&self, cmd: Command) {
        let cmd = match (cmd, &self.via) {
            (Command::Folder(cmd), Some(peer)) => Command::Peer(PeerCommand::Folder {
                peer: peer.clone(),
                cmd,
            }),
            (cmd, _) => cmd,
        };
        let _ = self.tx.unbounded_send(cmd);
    }
}

#[cfg(test)]
mod tests {
    use futures::StreamExt;
    use futures::channel::mpsc;
    use futures::executor::block_on;

    use super::*;
    use crate::wire::SessionId;

    #[test]
    fn sends_in_order_and_survives_a_closed_engine() {
        let (tx, mut rx) = mpsc::unbounded();
        let client = Client::new(tx);
        client.send(Command::Close { id: SessionId(1) });
        client.clone().send(Command::Close { id: SessionId(2) });
        assert_eq!(
            block_on(rx.next()),
            Some(Command::Close { id: SessionId(1) })
        );
        assert_eq!(
            block_on(rx.next()),
            Some(Command::Close { id: SessionId(2) })
        );
        drop(rx);
        client.send(Command::Close { id: SessionId(3) });
    }

    #[test]
    fn a_client_via_a_computer_sends_only_its_folder_commands_there() {
        let (tx, mut rx) = mpsc::unbounded();
        let client = Client::new(tx).via(Some("laptop".into()));
        client.send(Command::Close { id: SessionId(1) });
        client.send(Command::Folder(crate::FolderCommand::Openers));
        assert_eq!(
            block_on(rx.next()),
            Some(Command::Close { id: SessionId(1) })
        );
        assert_eq!(
            block_on(rx.next()),
            Some(Command::Peer(PeerCommand::Folder {
                peer: "laptop".into(),
                cmd: crate::FolderCommand::Openers,
            }))
        );
    }
}
