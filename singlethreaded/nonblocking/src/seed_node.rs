use crate::types::{AddressBook, PeerID, Event};
use crossbeam::channel;
use std::thread;

#[derive(Debug, Clone)]
pub struct SeedNodeConfig {
    pub id: PeerID,
    pub bind_host: String,
    pub bind_port: u16,
}

pub struct SeedNode {
    sender: channel::Sender<Event>,
    receiver: channel::Receiver<Event>,
}

impl SeedNode {
    // How do you do error handling here?
    pub fn new(config: SeedNodeConfig) -> SeedNode {
        // create an address book
        let (sender, receiver) = channel::unbounded::<Event>();
        return SeedNode {
            sender: sender,
            receiver: receiver,
        }
    }

    pub fn run(&mut self) {
        let fsm_send = self.sender.clone();
        let fsm_rcv = self.receiver.clone();

        let address_book = AddressBook::new();
        let fsm_thread = thread::spawn(move || {
            address_book.run_fsm(fsm_rcv, fsm_send);
        });

    }

    fn handle(&mut self, event: Event) {
        self.sender.send(event);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node() {
        let mut peer = SeedNode::new(SeedNodeConfig {
            id: PeerID::from("1"),
            bind_host: "127.0.0.1".to_string(),
            bind_port: 0,
        });

        peer.run();
        peer.handle(Event::Terminate());
    }
}

