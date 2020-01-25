use tokio::task;
use tokio::sync::mpsc;
use crate::address_book::{Event as AddressBookEvent, AddressBook, PeerID, Entry};
use crate::acceptor::Event as AcceptorEvent;
use crate::dispatcher::Event as DispatcherEvent;

enum NodeEvent {
    Connect(PeerID, Entry),
    Connected(PeerID, PeerID) // when one peer connects to another
}

enum Event {
    Node(NodeEvent),
    AddressBook(AddressBookEvent),
    Acceptor(AcceptorEvent),
    Dispatcher(DispatcherEvent),
}

// XXX: Unify this with Entry
#[derive(Debug, Clone)]
pub struct SeedNodeConfig {
    pub id: PeerID,
    pub bind_host: String,
    pub bind_port: u16,
}

pub struct SeedNode {
}


impl SeedNode {
    fn new() -> SeedNode {
        return SeedNode {}
    }

    // We don't need content types on the channels?
    async fn run(events_send: mpsc::channel<Sender>, events_receive: mpsc::channel<Receiver>) {
        let (acceptor_sender, acceptor_receiver) = mpsc::channel::<AcceptorEvent>(0);
        let acceptor = Acceptor::new(); // maybe pass  address and port
        let acceptor_handler = task::spawn(async move {
            acceptor.run(acceptor_receiver, event_send.clone());
        });

        let (dispatcher_sender, dispatcher_receiver) = mpsc::channel::<Dispatcher>(0);
        let dispatcher = Dispatcher::new();
        let dispatcher_handler = task::spawn(async move {
            dispatcher.run(dispatcher_receiver, event_send.clone());
        }

        let (ab_sender, ab_receiver) = mpsc::channel::<Event>(0);
        let address_book = AddressBook::new();
        let ab_handler = task::spawn(async move {
            dispatcher.run(dispatcher_receiver, event_send.clone());
        }

        // The Event translation can be replaced with From trait implementation
        // and the unwraps should bubble up to some kind of reasonable error handling
        while Some(event) = events_receive.recv().await {
            match event {
                Event::NodeEvent(node_event) => {
                    if let NodeEvent::Connect(entry) = node_event {
                        acceptor_sender.send(AcceptorEvent::Connect(entry)).unwrap();
                    }
                },
                Event::AcceptorEvent(acceptor_event) => {
                    if let AcceptorEvent::PeerConnected(peer_id, TcpStream) = acceptor_event {
                        dispatcher_sender.send(DispatcherEvent::PeerConnected(peer_id, TcpStream)).unwrap();
                    }
                },
                Event::DispatcherEvent(dispatcher_event) => {
                    match dispatcher_event {
                        DispatcherEvent::AddPeer(peer_id, entry) => {
                            address_book.send(AddressBookEvent::AddPeer(peer_id, msg)).unwrap();
                        },
                        DispatcherEvent::FromPeer(peer_id, msg) => {
                            address_book.send(AddressBookEvent::FromPeer(peer_id, msg)).unwrap();
                        },
                        _ => {
                        }
                    },
                },
                Event::AddressBookEvent(address_book_event) => {
                    match address_book_event {
                        AddressBookEvent::ToPeer(peer_id, msg) => {
                            dispatcher.send(DispatcherEvent::ToPeer(peer_id, msg)).unwrap();
                        },
                        AddressBookEvent::PeerAdded(added_peer_id) => {
                            let my_id = self.peer_id.clone();
                            event_send.send(NodeEvent::PeerAdded(my_id, added_peer_id)).unwrap();
                        },
                        _ => {
                        }
                    }
                },
                _ => {
                    // hmm?
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_network() {
        let mut node1 = SeedNode::new(SeedNodeConfig {
            id: PeerID::from("1"),
            bind_host: "127.0.0.1".to_string(),
            bind_port: 8902,
        });

        let mut node2 = SeedNode::new(SeedNodeConfig {
            id: PeerID::from("2"),
            bind_host: "127.0.0.1".to_string(),
            bind_port: 8903,
        });

        let (node1_in_send, node1_in_recv) = mpsc::channel<Event>();
        let (node1_out_send, node1_out_recv) = mpsc::channel<Event>();
        task::spawn(move {
            node1.run(node1_in_send, node1_out_send);
        }
        let (node2_in_send, node2_in_recv) = mpsc::channel<Event>();
        let (node2_out_send, node2_out_recv) = mpsc::channel<Event>();
        task::spawn(move {
            node2.run(node2_out_send, node2_in_recv);
        }

        // TODO start timer

        node1.send(Event::Connect(Entry::new(
                    PeerID::from("2"), 
                    "127.0.0.1".to_string(),
                    8903);

        let mut connected = 0;
        let stream = futures::stream::select(node1_out_recv, node2_out_recv);
        while Some(event) = stream.next().await {
            match event {
                // These events should have a include the seed nodes peer ID
                Event::Connected(from_peer_id, to_peer_id) => {
                    println!("Peer {} connected to {}", from_peer_id, to_peer_id);
                    timer.touch = now;
                    connected += 1;
                },
            }
        }
    }
}

