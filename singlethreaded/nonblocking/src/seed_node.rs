/// We want to develop a non-blocking version of the peer exchange protocol.
/// The peer exchange protocol or PEX is a protocol in which peers accumulated an address book of
/// other peers through a gossip process. Peers will periodically poll peers in it's owned address
/// book in order to discover what other peers are avaiable in the network.
///
/// Non blocking in this case means that no activity being performed by one peer will impede or
/// block the operations of another peer.
///
/// In addition to functional requirements, inline with the experimental framework outlined in
/// https://github.com/interchainio/reactor-experiments/blob/master/.plaintext/README.md, we would
/// like to explore specific architectural constraints.
///
/// 1. Seperation of concerns into components
///     1.1 Each component exists as it's own Finite state machine
///         1.1.2 State mutation is serialized and driven by events
///     1.2 Each components is encapsulated and has no knowledge of any other component
/// 2. Components are composable.
///     2.1 Components can coordinate by intra-component coordination, driven exclusively by events
/// 3. Component composition is done centrally in a routing table
///
/// The goal of these constraints is to explore the ergonomics of code which can
/// A. Be simulated deterministically
/// B. Trace the path of simulated events through the components
use futures::prelude::*;
use std::net::IpAddr;
use std::str::FromStr;
use tokio::sync::mpsc;
use crate::address_book::{Event as AddressBookEvent, AddressBook, PeerID, Entry};
use crate::acceptor::{Acceptor, Event as AcceptorEvent};
use crate::dispatcher::{Dispatcher, Event as DispatcherEvent};
use log::{info};

#[derive(Hash, PartialEq, Eq, Debug, Clone)]
enum NodeEvent {
    Connect(PeerID, Entry),
    Connected(PeerID, PeerID) // when one peer connects to another
}

#[derive(Debug, PartialEq, Eq, Hash)]
pub enum Event {
    Node(NodeEvent),
    AddressBook(AddressBookEvent),
    Acceptor(AcceptorEvent),
    Dispatcher(DispatcherEvent),
}

impl From<AcceptorEvent> for Event {
    fn from(item: AcceptorEvent) -> Self {
        Event::Acceptor(item)
    }
}

// Can we convert Channel types?
impl From<DispatcherEvent> for Event {
    fn from(item: DispatcherEvent) -> Self {
        Event::Dispatcher(item)
    }
}

impl From<AddressBookEvent> for Event {
    fn from(item: AddressBookEvent) -> Self {
        Event::AddressBook(item)
    }
}

impl From<NodeEvent> for Event {
    fn from(item: NodeEvent) -> Self {
        Event::Node(item)
    }
}


// We should be able to use futures::sink::SinkExt
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

    // Conversino problem
    // Can we convert a Sender to a different type of sender?
    async fn run(self,
        mut events_out_send: mpsc::Sender<Event>,
        mut events_in_send: mpsc::Sender<Event>,
        mut events_receive: mpsc::Receiver<Event>) {
        // The ergonomics here can be improved by changing run to a start
        // and then returning the channels needed for interaction
        let (mut acceptor_sender, acceptor_receiver) = mpsc::channel::<AcceptorEvent>(1);
        let acceptor = Acceptor::new(self.entry.clone());
        let acceptor_output_sender = events_in_send.clone();
        let acceptor_handler = tokio::spawn(async move {
            acceptor.run(acceptor_output_sender, acceptor_receiver).await;
        });

        let (mut dispatcher_sender, dispatcher_receiver) = mpsc::channel::<DispatcherEvent>(1);
        let dispatcher = Dispatcher::new(self.entry.id.clone());
        let dispatcher_output_sender = events_in_send.clone();
        let dispatcher_handler = tokio::spawn(async move {
            dispatcher.run(dispatcher_output_sender, dispatcher_receiver).await;
        });

        let (mut ab_sender, ab_receiver) = mpsc::channel::<AddressBookEvent>(1);
        let address_book = AddressBook::new(self.entry.id.clone());
        let ab_output_sender = events_in_send.clone();
        let ab_handler = tokio::spawn(async move {
            address_book.run(ab_output_sender, ab_receiver).await;
        });

        info!("Node run: {:?}", self.entry);
        while let Some(event) = events_receive.recv().await {
           info!("[{}] node received: {:?}", self.entry.id, event);
            match event {
                Event::Node(node_event) => {
                    if let NodeEvent::Connect(peer_id, entry) = node_event {
                        let o_event = AcceptorEvent::Connect(entry);
                        acceptor_sender.send(o_event).await.unwrap();
                    }
                },
                Event::Acceptor(acceptor_event) => {
                    if let AcceptorEvent::PeerConnected(entry, stream) = acceptor_event {
                        let o_event = DispatcherEvent::PeerConnected(entry, stream);
                        dispatcher_sender.send(o_event).await.unwrap();
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
                            info!("emitting peer connected");
                            let my_id = self.entry.id.clone();
                            events_out_send.send(NodeEvent::Connected(my_id, added_peer_id).into()).await.unwrap();
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
    use simple_logger;

    fn test_setup() {
        simple_logger::init_with_level(log::Level::Debug).unwrap();
    }

    #[test]
    fn test_network() {
        test_setup();
        use tokio::runtime::Runtime;
        let mut rt = Runtime::new().unwrap();

        let ip_addr = IpAddr::from_str("127.0.0.1").unwrap();
        let node1 = SeedNode::new(Entry::new(
            PeerID::from("1"),
            ip_addr.clone(),
            8902
        ));

        let node2 = SeedNode::new(Entry::new(
            PeerID::from("2"),
            ip_addr.clone(),
            8903
        ));

        // so what we really need is to clone the main loop sender such that the internal
        // components can route events to itself
        let (mut node1_in_send, node1_in_recv) = mpsc::channel::<Event>(1);
        let (node1_out_send, node1_out_recv) = mpsc::channel::<Event>(1);
        let node1_in_send2 = node1_in_send.clone();

        let (node2_in_send, node2_in_recv) = mpsc::channel::<Event>(1);
        let (node2_out_send, node2_out_recv) = mpsc::channel::<Event>(1);
        let node2_in_send2 = node1_in_send.clone();

        // start node 1
        rt.spawn(async move {
            node1.run(node1_out_send,node1_in_send2, node1_in_recv).await;
        });

        // start node 2
        rt.spawn(async move {
            node2.run(node2_out_send, node2_in_send2, node2_in_recv).await;
        });

        rt.block_on(node1_in_send.send(Event::Node(NodeEvent::Connect(PeerID::from("2"), Entry::new(
                    PeerID::from("2"),
                    ip_addr.clone(),
                    8903))))).unwrap();

        let mut connected:i32 = 0;
        let mut stream = futures::stream::select(node1_out_recv, node2_out_recv);

        while let Some(event) = rt.block_on(stream.next()) {
            info!("Test Stream received; {:?}", event);
            if let Event::Node(NodeEvent::Connected(from_peer_id, to_peer_id)) = event {
                info!("Peer {} connected to {}", from_peer_id, to_peer_id);
                //timer.touch = now;
                connected += 1;
                if connected == 2 {
                    info!("All peers connected");
                    return
                }
            }
        }
    }
}

