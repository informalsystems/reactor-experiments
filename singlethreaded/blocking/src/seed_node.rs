//!
//! A rough implementation of a seed node for a Tendermint network.
//!

use crate::encoding::{PeerMessageReader, PeerMessageWriter};
use crate::events::{Event, PeerEvent, PubSubRequest};
use crate::messages::PeerMessage;
use crate::pubsub;
use crate::remote_peer::RemotePeer;
use crate::types::{AddressBook, PeerAddr, PeerHello, SubscriptionID, ID};
use crossbeam::channel;
use log::{error, info, debug};
use std::collections::{HashMap, HashSet};
use std::net::{TcpListener, TcpStream};
use std::thread;

#[derive(Debug)]
/// A seed node for a Tendermint network. Its only purpose is to crawl other
/// seeds and peers to gather the addresses of peers in the network, and make
/// these addresses available to other peers.
pub struct SeedNode {
    config: SeedNodeConfig,
    bound_address: Option<String>,
    peers: HashMap<ID, RemotePeer>,
    addr_book: AddressBook,
    sender: channel::Sender<Event>,
    receiver: channel::Receiver<Event>,
    subscriptions: pubsub::Subscriptions,
}

#[derive(Debug, Clone)]
pub struct SeedNodeConfig {
    pub id: ID,
    pub bind_host: String,
    pub bind_port: u16,
}

impl SeedNode {
    /// Creates a new seed node with the given configuration. Immediately
    /// instantiates the server listener thread, but does not instantiate the
    /// event loop (meaning incoming peers could connect, but their connections
    /// won't be handled until the `run` method is called).
    pub fn new(config: SeedNodeConfig) -> std::io::Result<SeedNode> {
        // In this model, our node exclusively receives events via this channel.
        // The transmitting end is cloned and distributed to all other entities
        // that want to emit events, and the `run` method effectively acts as an
        // air traffic controller/router for the entire node.
        let (tx, rx) = channel::unbounded::<Event>();
        let mut seed_node = SeedNode {
            config,
            bound_address: None,
            peers: HashMap::new(),
            addr_book: AddressBook::new(),
            sender: tx,
            receiver: rx,
            subscriptions: pubsub::Subscriptions::new(),
        };
        seed_node.spawn_server_thread()?;
        Ok(seed_node)
    }

    fn spawn_server_thread(&mut self) -> std::io::Result<()> {
        let listener = TcpListener::bind(format!(
            "{}:{}",
            self.config.bind_host, self.config.bind_port
        ))?;
        let server_addr = listener.local_addr()?;
        let bound_address = format!("{}:{}", server_addr.ip(), server_addr.port());
        info!("Server for peer {} bound to: {}", self.config.id, bound_address);
        self.bound_address = Some(bound_address);

        // This is our only way of having the thread communicate with the parent
        // thread.
        let event_tx = self.sender.clone();

        // There is unfortunately no way to kill this thread from the outside,
        // since it's blocking. See https://stackoverflow.com/q/55228629/1156132
        thread::spawn(move || {
            debug!("Spawned listener thread");
            for result in listener.incoming() {
                match result {
                    Ok(stream) => {
                        info!("Received incoming peer connection");
                        match event_tx.try_send(Event::Peer(PeerEvent::Connected(stream))) {
                            Ok(_) => (),
                            Err(channel::TrySendError::Disconnected(_)) => {
                                info!("Channel disconnected. Stopping listener.");
                                return;
                            }
                            Err(e) => error!("Failed to send message to node event loop: {}", e),
                        }
                    }
                    Err(e) => error!("Failed to handle incoming connection: {}", e),
                }
            }
        });
        Ok(())
    }

    /// The primary runtime when running Tendermint purely as a seed node. This
    /// is a blocking routine.
    pub fn run(&mut self) -> std::io::Result<()> {
        debug!("Event loop started for peer: {}", self.config.id);
        'event_loop: loop {
            let ev = self
                .receiver
                .recv()
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
            // publish this event to any potential subscribers
            self.publish_event(ev.clone())?;
            match ev {
                Event::Peer(pe) => self.handle_peer_event(pe)?,
                Event::PubSub(pse) => self.handle_pubsub_event(pse)?,
                Event::Terminate => {
                    info!("Termination signal received for peer: {}", self.config.id);
                    break 'event_loop;
                }
            }
        }

        // cleanly disconnect from all peers
        self.disconnect_all_peers()?;
        Ok(())
    }

    /// Clones the sending end of the event channel for this peer, allowing
    /// sending of events directly to this node.
    pub fn sender(&self) -> channel::Sender<Event> {
        self.sender.clone()
    }

    fn handle_peer_event(&mut self, ev: PeerEvent) -> std::io::Result<()> {
        match ev {
            PeerEvent::Connected(stream) => self.peer_connected(stream),
            PeerEvent::Connect(addr) => self.connect_to_peer(addr),
            PeerEvent::Disconnect(id) => self.disconnect_peer(id),
            _ => Ok(()),
        }
    }

    fn handle_pubsub_event(&mut self, ev: PubSubRequest) -> std::io::Result<()> {
        match ev {
            PubSubRequest::Subscribe(subs) => self.handle_subscribe(subs),
            PubSubRequest::Unsubscribe(ids) => self.handle_unsubscribe(ids),
        }
    }

    fn handle_subscribe(&mut self, subs: pubsub::Subscription) -> std::io::Result<()> {
        info!("Adding {:?} for event types: {:?}", subs.id, subs.event_ids);
        if let Err(e) = self.subscriptions.add(subs) {
            error!("Subscription failed: {}", e);
        }
        Ok(())
    }

    fn handle_unsubscribe(&mut self, ids: HashSet<SubscriptionID>) -> std::io::Result<()> {
        self.subscriptions.remove_all(ids);
        Ok(())
    }

    fn publish_event(&self, ev: Event) -> std::io::Result<()> {
        match self.subscriptions.publish(ev.clone()) {
            Ok(_) => {
                debug!("Published event: {:?}", ev);
                Ok(())
            },
            Err(e) => match e {
                pubsub::SubscriptionError::NoEventIDForEvent => Ok(()),
                _ => Err(std::io::Error::new(std::io::ErrorKind::Other, e)),
            },
        }
    }

    fn peer_connected(&mut self, stream: TcpStream) -> std::io::Result<()> {
        let mut stream = stream.try_clone()?;
        self.read_peer_hello(&mut stream)
    }

    fn read_peer_hello(&mut self, stream: &mut TcpStream) -> std::io::Result<()> {
        // WTF? Rust claims further on in this method that the stream cannot be
        // borrowed as mutable, but it's literally declared as a mutable
        // reference in the function parameters.
        let mut stream = stream;
        // TODO: Implement this as a state machine in a non-blocking fashion
        // we expect the peer to introduce themselves here
        let msg = match stream.read_peer_message() {
            Ok(m) => m,
            Err(e) => {
                error!("Failed to receive hello message from incoming peer: {}", e);
                return Ok(());
            }
        };
        match msg {
            PeerMessage::Hello(hello) => self.try_add_peer(&mut stream, hello)?,
            _ => error!("Incoming peer did not introduce themselves. Dropping connection."),
        }
        Ok(())
    }

    fn try_add_peer(&mut self, stream: &mut TcpStream, hello: PeerHello) -> std::io::Result<()> {
        let mut stream = stream;
        if self.peers.contains_key(&hello.id) {
            error!(
                "We have already seen peer with ID {}. Rejecting incoming connection.",
                hello.id
            );
            return Ok(());
        }
        let peer_sockaddr = match stream.local_addr() {
            Ok(a) => a,
            Err(e) => {
                error!("Failed to obtain socket address of incoming peer: {}", e);
                return Ok(());
            }
        };
        let peer_addr = PeerAddr {
            id: hello.id.clone(),
            ip: peer_sockaddr.ip(),
            port: peer_sockaddr.port(),
        };
        // say hello back to the peer
        self.say_hello_to_peer(&mut stream)?;
        // create a new RemotePeer instance
        let remote_peer =
            match RemotePeer::new(hello.id.clone(), stream.try_clone()?, self.sender.clone()) {
                Ok(p) => p,
                Err(e) => {
                    error!("Failed to instantiate new remote peer: {}", e);
                    return Ok(());
                }
            };
        self.peers.insert(hello.id.clone(), remote_peer);
        // add this peer to our address book
        self.addr_book.addrs.push(peer_addr.clone());
        self.send_event_internally(Event::Peer(PeerEvent::AddedToAddressBook(peer_addr)))?;
        info!("Remote peer connected to {}: {}", self.config.id, hello.id);
        Ok(())
    }

    fn send_event_internally(&self, ev: Event) -> std::io::Result<()> {
        self.sender
            .send(ev)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
    }

    fn connect_to_peer(&mut self, addr: String) -> std::io::Result<()> {
        info!("Attempting to initiate connection to remote peer: {}", addr);
        let mut stream = TcpStream::connect(addr)?;
        // say hello to the peer
        self.say_hello_to_peer(&mut stream)?;
        // now we allow the PeerEvent::Connected event to handle this further
        self.sender
            .send(Event::Peer(PeerEvent::Connected(stream)))
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
    }

    fn say_hello_to_peer(&mut self, stream: &mut TcpStream) -> std::io::Result<()> {
        stream.write_peer_message(PeerMessage::Hello(PeerHello {
            id: self.config.id.clone(),
        }))
    }

    fn disconnect_peer(&mut self, id: ID) -> std::io::Result<()> {
        if !self.peers.contains_key(&id) {
            return Ok(());
        }
        self.peers.remove(&id);
        info!("Disconnected remote peer: {}", id);
        Ok(())
    }

    fn disconnect_all_peers(&mut self) -> std::io::Result<()> {
        let peer_ids: Vec<ID> = self.peers.iter().map(|(k, _)| ID::from(k)).collect();
        for id in peer_ids {
            self.disconnect_peer(id)?;
        }
        Ok(())
    }
}

// Purely test-related functionality for a seed node.
#[cfg(test)]
impl SeedNode {
    pub fn bound_address(&self) -> Option<String> {
        self.bound_address.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::EventID;
    use simple_logger;

    fn test_setup() {
        simple_logger::init_with_level(log::Level::Debug).unwrap();
    }

    // Here we run 3 seed nodes that should talk to each other and discover each
    // other to build up their address books.
    #[test]
    fn test_basic_peer_interaction() {
        test_setup();

        let (join_handle1, tx1, _) = spawn_test_peer(ID::from("1"));
        let (join_handle2, tx2, addr2) = spawn_test_peer(ID::from("2"));
        let (join_handle3, tx3, addr3) = spawn_test_peer(ID::from("3"));
        let (notify_tx1, notify_rx1) = channel::unbounded::<Event>();

        // we want to know from peer 1 when a peer has connected to us
        tx1.send(Event::PubSub(PubSubRequest::Subscribe(
            pubsub::Subscription {
                id: SubscriptionID::new(),
                event_ids: [EventID::PeerAddedToAddressBook].iter().cloned().collect(),
                notify: notify_tx1,
            },
        )))
        .unwrap();

        // tell peer 1 about peers 2 and 3
        tx1.send(Event::Peer(PeerEvent::Connect(addr2))).unwrap();
        tx1.send(Event::Peer(PeerEvent::Connect(addr3))).unwrap();

        let mut received = 0;
        for _ in 0..2 {
            channel::select! {
                recv(notify_rx1) -> r => match r {
                    Ok(ev) => {
                        info!("Got event: {:?}", ev);
                        match ev {
                            Event::Peer(pe) => match pe {
                                PeerEvent::AddedToAddressBook(_) => {},
                                _ => panic!("Expected PeerEvent::AddedToAddressBook event"),
                            }
                            _ => panic!("Expected Event::Peer event"),
                        }
                        received += 1;
                    },
                    Err(e) => {
                        error!("Failed to receive subscription message: {:?}", e);
                    },
                },
                default(std::time::Duration::from_millis(500)) => panic!("timed out waiting for subscription messages"),
            }
        }
        assert_eq!(2, received);

        // terminate all peers
        tx1.send(Event::Terminate).unwrap();
        tx2.send(Event::Terminate).unwrap();
        tx3.send(Event::Terminate).unwrap();

        // wait for peers to terminate
        let _ = join_handle1.join().unwrap();
        let _ = join_handle2.join().unwrap();
        let _ = join_handle3.join().unwrap();
    }

    fn spawn_test_peer(id: ID) -> (
        thread::JoinHandle<std::io::Result<()>>,
        channel::Sender<Event>,
        String,
    ) {
        let mut peer = SeedNode::new(SeedNodeConfig {
            id: id.clone(),
            bind_host: "127.0.0.1".to_string(),
            bind_port: 0,
        })
        .unwrap();
        let tx = peer.sender();
        let addr = peer.bound_address().unwrap();
        (
            thread::spawn(move || {
                info!("Spawning test peer: {}", id);
                let r = peer.run();
                info!("Test peer {} terminated with result: {:?}", id, r);
                r
            }),
            tx,
            addr,
        )
    }
}
