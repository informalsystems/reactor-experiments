use tokio::task;
use tokio::sync::mpsc;
use crate::address_book::{Event as AddressBookEvent, AddressBook, PeerID};
use crate::acceptor::Event as AcceptorEvent;
use crate::dispatcher::Event as DispatcherEvent;

enum NodeEvent {
    Connected(PeerID)
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

    async fn run(events_send: mpsc::channel<Sender>, events_receive: mpsc::channel<Receiver>) {
        // Node we need to alig all the events
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

        let (ab_sender, ab_receiver) = mpsc::channel::<>(0);
        let address_book = AddressBook::new();
        let ab_handler = task::spawn(async move {
            dispatcher.run(dispatcher_receiver, event_send.clone());
        }

        while Some(event) = events_receive.recv().await {
            match event {
                Event::Connect(entry) => {
                    acceptor.send(event);
                },
                Event::Connection(socket) => {
                    dispatcher.send(event);
                },
                Event::Connected(peerID) => {
                    // The peer has connected and is identified
                    fsm.send(event);
                },
                Event::PeerReceive(peerID, Message) => {
                    // message from peer, route the the fsm
                    fsm.send(event);
                },
                Event::PeerSend(peerID, Message) => {
                    dipatch.send(event);
                },
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
                Event::Connected(peer_id) => {
                    timer.touch = now;
                    connected += 1;
                },
            }
        }
    }
}

