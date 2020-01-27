use futures::prelude::*;
use tokio::prelude::*;
use tokio::task;
use tokio::sync::mpsc;
use crate::address_book::{Event as AddressBookEvent, AddressBook, PeerID, Entry};
use crate::acceptor::{Acceptor, Event as AcceptorEvent};
use crate::dispatcher::{Dispatcher, Event as DispatcherEvent};

#[derive(Debug, Clone)]
enum NodeEvent {
    Connect(PeerID, Entry),
    Connected(PeerID, PeerID) // when one peer connects to another
}

#[derive(Debug)]
enum Event {
    Node(NodeEvent),
    AddressBook(AddressBookEvent),
    Acceptor(AcceptorEvent),
    Dispatcher(DispatcherEvent),
}

impl From<AcceptorEvent> for Event {
    fn from(item: AcceptorEvent) -> Self {
        Event::AcceptorEvent(item)
    }
}

// Can we convert Channel types?

impl From<DispatcherEvent> for Event {
    fn from(item: DispatcherEvent) -> Self {
        Event::DispatcherEvent(item)
    }
}

impl From<AcceptorEvent> for Event {
    fn from(item: AcceptorEvent) -> Self {
        Event::AcceptorEvent(item)
    }
}

impl From<AddressBookEvent> for Event {
    fn from(item: AddressBookEvent) -> Self {
        Event::AddressBook(item)
    }
}


pub struct SeedNode {
    entry: Entry,
}

// we need some FROM impl for Event -> _Event
impl SeedNode {
    pub fn new(entry: Entry) -> SeedNode {
        return SeedNode {entry}
    }

    // Given how we are being diligent about the deployment of tasks as threads, it may be even
    // simpler to avoid the tokio runtime and use threads with channels.

    // Intead of using a single queue of events, and matching on the event type of. It might be
    // simpler and better in terms of quality of service, to have seperate queues per component and
    // then select between queues in the central routing table.
    //
    // Additionally, if we were to select on queues instead of on message types, one of the queues
    // could be a channel which is populated periodically as a timer in order to coorinate
    // timeouts.

    // XXX: we need to implement the FROM trait to facilitate conversion between component specific events
    async fn run(self, events_send: mpsc::Sender<Event>, events_receive: mpsc::Receiver<Event>) {
        // The ergonomics here can be improved by changing run to a start
        // which runs it's threads and returns the sender
        let (acceptor_sender, acceptor_receiver) = mpsc::channel::<AcceptorEvent>(0);
        let acceptor = Acceptor::new(self.entry.clone()); // maybe pass  address and port
        let acceptor_handler = tokio::spawn(async move {
            // how can we map sender
            acceptor.run(events_send.clone(), acceptor_receiver);
        });

        let (dispatcher_sender, dispatcher_receiver) = mpsc::channel::<DispatcherEvent>(0);
        let dispatcher = Dispatcher::new();
        let dispatcher_handler = tokio::spawn(async move {
            dispatcher.run(events_send.clone(), dispatcher_receiver);
        });

        let (ab_sender, ab_receiver) = mpsc::channel::<AddressBookEvent>(0);
        let address_book = AddressBook::new();
        let ab_handler = tokio::spawn(async move {
            address_book.run(events_send.clone(), ab_receiver);
        });

        // The Event translation can be replaced with From trait implementation
        // and the unwraps should bubble up to some kind of reasonable error handling
        while let Some(event) = events_receive.recv().await {
            match event {
                Event::Node(node_event) => {
                    if let NodeEvent::Connect(peer_id, entry) = node_event {
                        acceptor_sender.send(AcceptorEvent::Connect(entry)).await.unwrap();
                    }
                },
                Event::Acceptor(acceptor_event) => {
                    if let AcceptorEvent::PeerConnected(peer_id, stream) = acceptor_event {
                        dispatcher_sender.send(DispatcherEvent::PeerConnected(peer_id, framed)).await.unwrap();
                    }
                },
                Event::Dispatcher(dispatcher_event) => {
                    match dispatcher_event {
                        DispatcherEvent::AddPeer(peer_id, entry) => {
                            ab_sender.send(AddressBookEvent::AddPeer(entry)).await.unwrap();
                        },
                        DispatcherEvent::FromPeer(peer_id, msg) => {
                            ab_sender.send(AddressBookEvent::FromPeer(peer_id, msg)).await.unwrap();
                        },
                        _ => {
                        },
                    }
                },
                Event::AddressBook(address_book_event) => {
                    match address_book_event {
                        AddressBookEvent::ToPeer(peer_id, msg) => {
                            dispatcher_sender.send(DispatcherEvent::ToPeer(peer_id, msg)).await.unwrap();
                        },
                        AddressBookEvent::PeerAdded(added_peer_id) => {
                            let my_id = self.entry.id.clone();
                            events_send.send(NodeEvent::Connected(my_id, added_peer_id)).await.unwrap();
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
        let mut node1 = SeedNode::new(Entry::new(
            PeerID::from("1"),
            "127.0.0.1".to_string(),
            8902
        ));

        let mut node2 = SeedNode::new(Entry::new(
            PeerID::from("2"),
            "127.0.0.1".to_string(),
            8903
        ));

        let (node1_in_send, node1_in_recv) = mpsc::channel::<Event>();
        let (node1_out_send, node1_out_recv) = mpsc::channel::<Event>();
        task::spawn(async move {
            node1.run(node1_in_send, node1_out_send);
        });
        let (node2_in_send, node2_in_recv) = mpsc::channel::<Event>();
        let (node2_out_send, node2_out_recv) = mpsc::channel::<Event>();
        task::spawn(async move {
            node2.run(node2_out_send, node2_in_recv);
        });

        // TODO start timer

        // can't do this if we consume node1
        node1.send(NodeEvent::Connect(Entry::new(
                    PeerID::from("2"),
                    "127.0.0.1".to_string(),
                    8903)));

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

