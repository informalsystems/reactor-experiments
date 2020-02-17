use crate::acceptor::Acceptor;
use crate::dispatcher::{Dispatcher, DispatcherMsg, PeerMessage};
use crossbeam::channel::{tick, unbounded, Receiver};
use ipc_channel::ipc::{self, IpcSender};
use ipc_channel::router::ROUTER;
use std::collections::HashMap;
use std::net::IpAddr;
use std::str::FromStr;
use std::thread;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;

#[derive(Deserialize, Serialize)]
pub enum TendermintMsg {
    Exit(IpcSender<()>),
}

#[derive(Deserialize, Serialize)]
pub enum ABCIMsg {
    Exit(IpcSender<()>),
}

#[derive(Clone, Debug)]
pub enum FromDispatcherMsg {
    FromPeer(PeerMessage),
    PeerConnected(PeerId),
    PeerDisconnected(PeerId),
}

#[derive(Debug, Default, Clone, Eq, Hash, PartialEq, Deserialize, Serialize)]
pub struct PeerId(pub String);

/// The address of a peer in the network.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct Entry {
    /// The cryptographic address of this peer.
    pub id: PeerId,
    /// The IPv4/6 address of this peer.
    pub ip: IpAddr,
    /// The port on which the peer is listening for incoming connections.
    pub port: u16,
}

pub struct Tendermint {
    /// Heartbeat
    ticker: Receiver<Instant>,

    /// Receiver of messages from the abci layer.
    tm_receiver: Receiver<TendermintMsg>,

    /// Sender of messages to the abci layer.
    abci_sender: IpcSender<ABCIMsg>,

    /// Id of local node.
    seed_node: PeerId,

    /// Info about network location of other nodes.
    addr_book: HashMap<PeerId, Entry>,

    /// Sender of messages to the dispatcher.
    dispatcher_sender: mpsc::UnboundedSender<DispatcherMsg>,

    /// Receiver of messages from the dispatcher.
    dispatcher_receiver: Receiver<FromDispatcherMsg>,
}

impl Tendermint {
    /// Start a full-node in a thread.
    pub fn start(abci_sender: IpcSender<ABCIMsg>) -> (IpcSender<TendermintMsg>, Acceptor) {
        let (tendermint_ipc_sender, tendermint_ipc_receiver) =
            ipc::channel().expect("ipc channel failure");

        // Route the IPC messages to a crossbeam chan, so it can be included in the select.
        let (tm_sender, tm_receiver) = unbounded();
        ROUTER.add_route(
            tendermint_ipc_receiver.to_opaque(),
            Box::new(move |message| {
                drop(tm_sender.send(message.to().expect("Routing tenderming msg failed")))
            }),
        );

        let (dispatcher_sender, dispatcher_receiver) = mpsc::unbounded_channel();

        let seed_node: PeerId = Default::default();

        let (from_dispatcher_sender, from_dispatcher_receiver) = unbounded();

        let acceptor = Acceptor {
            dispatcher_sender: dispatcher_sender.clone(),
            entry: Entry {
                id: seed_node.clone(),
                ip: IpAddr::from_str("127.0.0.1").unwrap(),
                port: 8083,
            },
        };

        let mut dispatcher = Dispatcher {
            self_sender: dispatcher_sender.clone(),
            receiver: dispatcher_receiver,
            connected_peers: HashMap::new(),
            to_tendermint_sender: from_dispatcher_sender,
        };

        tokio::spawn(async move {
            let _ = dispatcher.run().await;
        });

        let ticker = tick(Duration::from_millis(500));

        let mut tendermint = Tendermint {
            ticker,
            tm_receiver,
            abci_sender,
            seed_node,
            addr_book: HashMap::new(),
            dispatcher_receiver: from_dispatcher_receiver,
            dispatcher_sender,
        };

        thread::spawn(move || {
            while tendermint.run() {
                // running.
            }
        });

        (tendermint_ipc_sender, acceptor)
    }

    fn run(&mut self) -> bool {
        select! {
            recv(self.ticker) -> _ => {
                // Send an addr book request to each peer every 5 sec.
                for (peer_id, _entry) in self.addr_book.iter() {
                    let addr_book_request = PeerMessage::AddressBookRequest(self.seed_node.clone());
                    let msg = DispatcherMsg::ToPeer(peer_id.clone(), addr_book_request);
                    let _ = self.dispatcher_sender.send(msg);
                }
            },
            recv(self.tm_receiver) -> msg => {
                match msg {
                    Ok(TendermintMsg::Exit(confirmation_sender)) => {
                        // Shutdown networking sub-tasks first...
                        let (sender, receiver) = unbounded();
                        let msg = DispatcherMsg::Exit(sender);
                        let _ = self.dispatcher_sender.send(msg);

                        receiver.recv().expect("Failed to shutdown dispatcher");

                        // Notify ABCI we are shutting down.
                        let _ = confirmation_sender.send(());
                        return false;
                    },
                    _ => {},
                }
            },
            recv(self.dispatcher_receiver) -> msg => {
                match msg {
                    Ok(FromDispatcherMsg::FromPeer(msg)) => {
                        match msg {
                            PeerMessage::AddressBookRequest(peer_id) => {
                                let peers = self.addr_book.keys().map(|id| id.clone()).collect();
                                let addr_book_response = PeerMessage::AddressBookResponse(peers);
                                let msg = DispatcherMsg::ToPeer(peer_id, addr_book_response);
                                let _ = self.dispatcher_sender.send(msg);
                            },
                            PeerMessage::Hello(peer_id) => {
                                let hello_msg = PeerMessage::Hello(self.seed_node.clone());
                                let msg = DispatcherMsg::ToPeer(peer_id, hello_msg);
                                let _ = self.dispatcher_sender.send(msg);
                            },
                            _ => {},
                        }
                    },
                    Ok(FromDispatcherMsg::PeerConnected(peer_id)) => {
                        let entry = Entry {
                            id: peer_id.clone(),
                            ip: IpAddr::from_str("127.0.0.1").unwrap(),
                            port: 0,
                        };
                        self.addr_book.insert(peer_id, entry);
                    },
                    Ok(FromDispatcherMsg::PeerDisconnected(peer_id)) => {
                        self.addr_book.remove(&peer_id);
                        let _ = self.dispatcher_sender.send(DispatcherMsg::PeerDisconnected(peer_id));
                    },
                    Err(_) => {
                        // Dispatcher is gone, exit.
                        let (sender, receiver) =
                            ipc::channel().expect("ipc channel failure");
                        let _ = self.abci_sender.send(ABCIMsg::Exit(sender));
                        let _ = receiver.recv();
                        return false;
                    },
                }
            },
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tendermint_shut_down_after_abci_exit() {
        let (abci_sender, abci_receiver) = ipc::channel().expect("ipc channel failure");

        let (tm_sender, tm_receiver) = unbounded();

        let ticker = tick(Duration::from_millis(500));

        let (dispatcher_sender, mut dispatcher_receiver) = mpsc::unbounded_channel();

        let seed_node: PeerId = PeerId(String::from("Seed node"));

        let (_from_dispatcher_sender, from_dispatcher_receiver) = unbounded();

        let mut tendermint = Tendermint {
            ticker,
            tm_receiver,
            abci_sender,
            seed_node,
            addr_book: HashMap::new(),
            dispatcher_receiver: from_dispatcher_receiver,
            dispatcher_sender,
        };

        thread::spawn(move || {
            while tendermint.run() {
                // running.
            }
        });

        // Send a shutdown message.
        let (confirmation_sender, confirmation_receiver) =
            ipc::channel().expect("ipc channel failure");
        let _ = tm_sender.send(TendermintMsg::Exit(confirmation_sender));

        // Assert tendermint shuts-down the dispatcher.
        loop {
            if let Ok(msg) = dispatcher_receiver.try_recv() {
                match msg {
                    DispatcherMsg::Exit(confirmation_sender) => {
                        // Dispatcher sends confirmation to tendermint.
                        let _ = confirmation_sender.send(());
                        break;
                    }
                    _ => continue,
                };
            }
        }

        // Assert we get the confirmation from tendermint.
        match confirmation_receiver.recv() {
            Ok(()) => {}
            _ => panic!("Unexpected reply from tendermint."),
        }

        // Once tendermint drops its channel post-shutdown,
        // we should get an error on the receiver.
        assert!(abci_receiver.recv().is_err());
    }

    #[test]
    fn test_tendermint_shut_down_after_dispatcher_exit() {
        let (abci_sender, abci_receiver) = ipc::channel().expect("ipc channel failure");

        let (_tm_sender, tm_receiver) = unbounded();

        let ticker = tick(Duration::from_millis(500));

        let (dispatcher_sender, _dispatcher_receiver) = mpsc::unbounded_channel();

        let seed_node: PeerId = PeerId(String::from("Seed node"));

        let (from_dispatcher_sender, from_dispatcher_receiver) = unbounded();

        let mut tendermint = Tendermint {
            ticker,
            tm_receiver,
            abci_sender,
            seed_node,
            addr_book: HashMap::new(),
            dispatcher_receiver: from_dispatcher_receiver,
            dispatcher_sender,
        };

        thread::spawn(move || {
            while tendermint.run() {
                // running.
            }
        });

        // Simulate a disconnected dispatcher.
        drop(from_dispatcher_sender);

        // Check tendermint send exit message to abci.
        match abci_receiver.recv() {
            Ok(ABCIMsg::Exit(sender)) => {
                // Send reply, shutting down tendermint.
                let _ = sender.send(());
            }
            _ => panic!("Unexpected abci msg received."),
        }

        // Once tendermint drops its channel post-shutdown,
        // we should get an error on the receiver.
        assert!(abci_receiver.recv().is_err());
    }

    #[test]
    fn test_tendermint_peer_workflow() {
        let (abci_sender, _abci_receiver) = ipc::channel().expect("ipc channel failure");

        let (_tm_sender, tm_receiver) = unbounded();

        let ticker = tick(Duration::from_millis(500));

        let (dispatcher_sender, mut dispatcher_receiver) = mpsc::unbounded_channel();

        let seed_node: PeerId = PeerId(String::from("Seed node"));

        let (from_dispatcher_sender, from_dispatcher_receiver) = unbounded();

        let mut tendermint = Tendermint {
            ticker,
            tm_receiver,
            abci_sender,
            seed_node,
            addr_book: HashMap::new(),
            dispatcher_receiver: from_dispatcher_receiver,
            dispatcher_sender,
        };

        thread::spawn(move || {
            while tendermint.run() {
                // running.
            }
        });

        // Connect a node.
        let peer_1 = PeerId(String::from("Hello, world!"));
        let connected_msg = FromDispatcherMsg::PeerConnected(peer_1.clone());
        let _ = from_dispatcher_sender.send(connected_msg);

        // Send an addr-book request.
        let a_peer = PeerId(String::from("Request"));
        let peer_msg = PeerMessage::AddressBookRequest(a_peer);
        let addr_book_req_msg = FromDispatcherMsg::FromPeer(peer_msg);
        let _ = from_dispatcher_sender.send(addr_book_req_msg.clone());

        // Assert we're getting the other peer in the response.
        loop {
            if let Ok(reply) = dispatcher_receiver.try_recv() {
                let reply = match reply {
                    DispatcherMsg::ToPeer(requesting_peer, reply) => {
                        assert_eq!(requesting_peer, PeerId(String::from("Request")));
                        reply
                    }
                    _ => continue,
                };
                if let PeerMessage::AddressBookResponse(peers) = reply {
                    assert!(peers.contains(&peer_1));
                    break;
                }
            }
        }

        // Assert the node sends an addr-book request to the connected peer.
        loop {
            if let Ok(reply) = dispatcher_receiver.try_recv() {
                let reply = match reply {
                    DispatcherMsg::ToPeer(requesting_peer, reply) => {
                        assert_eq!(requesting_peer, PeerId(String::from("Hello, world!")));
                        reply
                    }
                    _ => continue,
                };
                if let PeerMessage::AddressBookRequest(peer) = reply {
                    assert_eq!(peer, PeerId(String::from("Seed node")));
                    break;
                }
            }
        }

        // Disconnect the one peer that is connected.
        let disconnected_msg = FromDispatcherMsg::PeerDisconnected(peer_1.clone());
        let _ = from_dispatcher_sender.send(disconnected_msg);

        // Send another addr-book request.
        let _ = from_dispatcher_sender.send(addr_book_req_msg);

        // Assert we're getting an empty response.
        loop {
            if let Ok(reply) = dispatcher_receiver.try_recv() {
                let reply = match reply {
                    DispatcherMsg::ToPeer(requesting_peer, reply) => {
                        assert_eq!(requesting_peer, PeerId(String::from("Request")));
                        reply
                    }
                    _ => continue,
                };
                if let PeerMessage::AddressBookResponse(peers) = reply {
                    assert!(peers.is_empty());
                    break;
                }
            }
        }
    }
}
