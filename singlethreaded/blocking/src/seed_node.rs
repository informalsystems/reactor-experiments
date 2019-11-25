//!
//! A rough implementation of a seed node for a Tendermint network.
//!

use crate::encoding::PeerMessageReader;
use crate::events::{Event, PeerEvent};
use crate::messages::PeerMessage;
use crate::remote_peer::RemotePeer;
use crate::types::{AddressBook, PeerAddr, PeerHello, ID};
use crossbeam::channel;
use log::{error, info};
use std::collections::HashMap;
use std::net::{TcpListener, TcpStream};
use std::thread;

#[derive(Debug)]
/// A seed node for a Tendermint network. Its only purpose is to crawl other
/// seeds and peers to gather the addresses of peers in the network, and make
/// these addresses available to other peers.
pub struct SeedNode {
    config: SeedNodeConfig,
    peers: HashMap<ID, RemotePeer>,
    addr_book: AddressBook,
    sender: channel::Sender<Event>,
    receiver: channel::Receiver<Event>,
}

#[derive(Debug, Clone)]
pub struct SeedNodeConfig {
    pub bind_host: String,
    pub bind_port: u16,
}

enum InternalEvent {
    Terminate,
}

impl SeedNode {
    /// Creates a new seed node with the given configuration.
    pub fn new(config: SeedNodeConfig) -> SeedNode {
        // In this model, our node exclusively receives events via this channel.
        // The transmitting end is cloned and distributed to all other entities
        // that want to emit events, and the `run` method effectively acts as an
        // air traffic controller/router for the entire node.
        let (tx, rx) = channel::unbounded::<Event>();
        SeedNode {
            config,
            peers: HashMap::new(),
            addr_book: AddressBook::new(),
            sender: tx,
            receiver: rx,
        }
    }

    /// The primary runtime when running Tendermint purely as a seed node.
    pub fn run(&mut self) -> std::io::Result<()> {
        let listener = TcpListener::bind(format!(
            "{}:{}",
            self.config.bind_host, self.config.bind_port
        ))?;
        listener.set_nonblocking(true)?;
        let event_tx = self.sender.clone();
        let (server_tx, server_rx) = channel::unbounded::<InternalEvent>();

        let server_handle = thread::spawn(move || {
            info!("Spawned listener thread");
            for result in listener.incoming() {
                if let Ok(ev) = server_rx.try_recv() {
                    match ev {
                        InternalEvent::Terminate => {
                            info!("Termination signal received. Stopping listener.");
                            return;
                        }
                    }
                }

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
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        // In more advanced implementations, this would use some
                        // kind of OS-specific mechanism to sleep until there is
                        // an incoming connection, thereby reducing CPU usage.
                        thread::sleep(std::time::Duration::from_millis(100));
                    }
                    Err(e) => error!("Failed to handle incoming connection: {}", e),
                }
            }
        });

        'event_loop: loop {
            let ev = self
                .receiver
                .recv()
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
            match ev {
                Event::Peer(pe) => self.handle_peer_event(pe)?,
                Event::Terminate => break 'event_loop,
            }
        }

        // signal to terminate the server thread
        server_tx
            .send(InternalEvent::Terminate)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

        // disconnect from all peers
        self.disconnect_all_peers()?;

        // wait until the server thread terminates
        server_handle.join().map_err(|_| {
            std::io::Error::new(std::io::ErrorKind::Other, "failed to join server thread")
        })?;
        Ok(())
    }

    fn handle_peer_event(&mut self, ev: PeerEvent) -> std::io::Result<()> {
        match ev {
            PeerEvent::Connected(stream) => self.peer_connected(stream),
            PeerEvent::Disconnect(id) => self.disconnect_peer(id),
            _ => Ok(()),
        }
    }

    fn peer_connected(&mut self, stream: TcpStream) -> std::io::Result<()> {
        let mut stream = stream.try_clone()?;
        // TODO: Implement timeout here
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
        self.addr_book.addrs.push(peer_addr);
        info!("Remote peer connected: {}", hello.id);
        Ok(())
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

