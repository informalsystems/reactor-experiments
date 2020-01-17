use crossbeam::channel;
use std::thread;
use crate::address_book::{Event, AddressBook, PeerID};

// One big question is, why verify events instead of state
// * Because events are the public API for asynchronous components
// * Our expectations is that event processing is deterministic, as in, a sequence of events
// provided in order will always produce the same set of events

#[derive(Debug, Clone)]
pub struct SeedNodeConfig {
    pub id: PeerID,
    pub bind_host: String,
    pub bind_port: u16,
}

pub struct SeedNode {
    node_in_sender: channel::Sender<Event>,
    node_in_receiver: channel::Receiver<Event>,
    node_out_sender: channel::Sender<Event>,
    node_out_receiver: channel::Receiver<Event>,
}

impl SeedNode {
    pub fn new(config: SeedNodeConfig) -> SeedNode {
        let (node_in_sender, node_in_receiver) = channel::unbounded::<Event>();
        let (node_out_sender, node_out_receiver) = channel::unbounded::<Event>();
        return SeedNode {
            node_in_sender,
            node_in_receiver,
            node_out_sender,
            node_out_receiver,
        }
    }

    pub fn run(&mut self) {

        let sender = self.node_out_sender.clone();
        let receiver = self.node_in_receiver.clone();

        // start a p2p layer
        // input fsm, output to fsm

        // from connection manager to the address book
        // from the address_book to the connection manager

        let address_book = AddressBook::new();
        let fsm_thread = thread::spawn(move || {
            address_book.run(sender, receiver);
        });

    }

    //pub fn wait(&self) {
    //    // join on each handler
    //    self.handlers
    //}

    fn handle(&mut self, event: Event) {
        println!("Sending event into node");
        self.node_in_sender.send(event);
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

        for event in peer.node_out_receiver.iter() {
            println!("Node output {:?}", event);
            match event {
                Event::Terminated() => {
                    return
                },
                _ => {
                    // do nothing
                }

            }
        }
        // read from the output until closed
    }
}

