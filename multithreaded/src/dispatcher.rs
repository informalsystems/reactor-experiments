use crate::encoding;
use crate::tendermint::{FromDispatcherMsg, PeerId};
use crossbeam::channel::Sender;
use futures_util::future::FutureExt;
use futures_util::sink::SinkExt;
use futures_util::stream::StreamExt;
use std::collections::{HashMap, HashSet};
use tokio::sync::mpsc;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum PeerMessage {
    Hello(PeerId),
    AddressBookRequest(PeerId),
    AddressBookResponse(HashSet<PeerId>),
}

pub enum DispatcherMsg {
    ToPeer(PeerId, PeerMessage),
    PeerHandshake(encoding::MessageFramed, PeerId),
    HandshakeCompleted(encoding::MessageFramed, PeerId),
    PeerDisconnected(PeerId),
    Exit(Sender<()>),
}

pub struct Dispatcher {
    pub self_sender: mpsc::UnboundedSender<DispatcherMsg>,
    pub receiver: mpsc::UnboundedReceiver<DispatcherMsg>,
    pub connected_peers: HashMap<PeerId, mpsc::Sender<PeerMessage>>,
    pub to_tendermint_sender: Sender<FromDispatcherMsg>,
}

impl Dispatcher {
    pub async fn run(&mut self) {
        while let Some(msg) = self.receiver.recv().await {
            match msg {
                DispatcherMsg::PeerHandshake(mut encoder, my_id) => {
                    let self_sender = self.self_sender.clone();

                    // Spawn a handshake task
                    tokio::spawn(async move {
                        // First we say hello
                        let msg = PeerMessage::Hello(my_id);
                        encoder.send(msg).await.unwrap();

                        // Loop until we receive Hello back.
                        while let Ok(msg) = encoder.next().await.unwrap() {
                            if let PeerMessage::Hello(peer_id) = msg {
                                let _ = self_sender
                                    .send(DispatcherMsg::HandshakeCompleted(encoder, peer_id));
                                break;
                            }
                        }
                    });
                }
                DispatcherMsg::ToPeer(peer_id, msg) => {
                    let chan = self
                        .connected_peers
                        .get_mut(&peer_id)
                        .expect("Attempt to send msg to disconnected peer.");
                    let _ = chan.send(msg).await;
                }
                DispatcherMsg::PeerDisconnected(peer_id) => {
                    self.connected_peers.remove(&peer_id);
                }
                DispatcherMsg::HandshakeCompleted(encoder, peer_id) => {
                    self.handle_new_connected_peer(encoder, peer_id);
                }
                DispatcherMsg::Exit(confirmation_sender) => {
                    // Note: this will result in dropping our map of senders per connected peers,
                    // which will result in the exit of the corresponding spawned tasks.
                    let _ = confirmation_sender.send(());
                    break;
                }
            }
        }
    }

    /// Handle a new peer connected after a successful handshake.
    fn handle_new_connected_peer(&mut self, mut encoder: encoding::MessageFramed, peer_id: PeerId) {
        let (tx, mut rx) = mpsc::channel(1);
        self.connected_peers.insert(peer_id.clone(), tx);

        let _ = self
            .to_tendermint_sender
            .send(FromDispatcherMsg::PeerConnected(peer_id.clone()));

        let to_tendermint_sender = self.to_tendermint_sender.clone();

        // Spawn a per-peer sub-dispatcher.
        tokio::spawn(async move {
            use futures::select;

            enum PeerEvent {
                FromPeer(PeerMessage),
                ToPeer(PeerMessage),
            }

            loop {
                let event = select! {
                    to_peer = rx.recv().fuse() => {
                        match to_peer {
                            Some(msg) => PeerEvent::ToPeer(msg),
                            // Exit.
                            None => break,
                        }
                    },
                    from_peer = encoder.next().fuse() => {
                        match from_peer {
                           Some(Ok(msg)) => {
                               PeerEvent::FromPeer(msg)
                           },
                           _ => {
                               let _ = to_tendermint_sender
                                   .send(FromDispatcherMsg::PeerDisconnected(peer_id.clone()));
                               // Exit.
                               break;
                           }
                        }
                    },
                };
                match event {
                    PeerEvent::FromPeer(msg) => {
                        match msg {
                            PeerMessage::AddressBookRequest(peer_id) => {
                                let msg = PeerMessage::AddressBookRequest(peer_id);
                                let _ = to_tendermint_sender.send(FromDispatcherMsg::FromPeer(msg));
                            }
                            _ => {
                                // TODO: handle other messages.
                            }
                        }
                    }
                    PeerEvent::ToPeer(msg) => {
                        encoder
                            .send(msg)
                            .await
                            .expect("Failed to send PeerMessage.");
                    }
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::acceptor::Acceptor;
    use crate::encoding;
    use crate::tendermint::{Entry, PeerId};
    use crossbeam::channel::unbounded;
    use std::net::IpAddr;
    use std::str::FromStr;
    use std::thread;
    use tokio::net::TcpStream;
    use tokio::runtime::Runtime;

    #[test]
    fn test_dispatcher() {
        let (dispatcher_sender, dispatcher_receiver) = mpsc::unbounded_channel();
        let (from_dispatcher_sender, from_dispatcher_receiver) = unbounded();

        let seed_node: PeerId = Default::default();

        let mut acceptor = Acceptor {
            dispatcher_sender: dispatcher_sender.clone(),
            entry: Entry {
                id: seed_node.clone(),
                ip: IpAddr::from_str("127.0.0.1").unwrap(),
                port: 8081,
            },
        };

        let mut dispatcher = Dispatcher {
            self_sender: dispatcher_sender.clone(),
            receiver: dispatcher_receiver,
            connected_peers: HashMap::new(),
            to_tendermint_sender: from_dispatcher_sender.clone(),
        };

        // A channel which the task representing the other node,
        // can use to fail the test if the handshake fails.
        let (failure_sender, failure_receiver) = unbounded();

        thread::spawn(move || {
            let mut rt = Runtime::new().unwrap();

            // Start the acceptor.
            rt.spawn(async move {
                let _ = acceptor.run().await;
            });

            // Start the equivalent of another node to complete the handshake.
            rt.spawn(async move {
                let stream = TcpStream::connect("127.0.0.1:8081")
                    .await
                    .expect("Failed to connect stream.");
                let mut encoder = encoding::create_encoder(stream);

                // First, wait for the hello message from the dispatcher.
                let msg = encoder.next().await;
                match msg {
                    Some(Ok(PeerMessage::Hello(seed_node_id))) => {
                        assert_eq!(seed_node_id, Default::default());
                    }
                    _ => {
                        // Unexpected messag from the dispatcher, fail the test.
                        let _ = failure_sender.send(());
                    }
                }

                // Next, we say hello back.
                let msg = PeerMessage::Hello(PeerId(String::from("Connected")));
                encoder.send(msg).await.unwrap();
            });

            // Start the dispatcher.
            rt.block_on(async move {
                let _ = dispatcher.run().await;
            });
        });

        // Assert tendermint receives the peer connected message.
        select! {
            recv(from_dispatcher_receiver) -> msg => {
                match msg {
                    Ok(FromDispatcherMsg::PeerConnected(peer_id)) => {
                        assert_eq!(peer_id, PeerId(String::from("Connected")));
                    },
                    _ => panic!("Unpexted message from the dispatcher."),
                }
            },
            recv(failure_receiver) -> msg => {
                if let Ok(()) = msg {
                    panic!("Dispatcher test failed.");
                }
            }
        }
    }
}
