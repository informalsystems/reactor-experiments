use crossbeam::channel::{tick, unbounded, Receiver, Sender};
use rand::{thread_rng, Rng};
use std::collections::HashMap;
use std::thread;
use std::time::{Duration, Instant};

enum PeerMsg {
    Request(Sender<PeerMsg>),
    Response(Sender<PeerMsg>),
    Ping(Sender<PeerMsg>),
    Pong(PeerInfo),
    Exit,
}

struct PeerInfo {
    id: PeerId,
    sender: Sender<PeerMsg>,
}

#[derive(Debug, Default, Clone, Eq, Hash, PartialEq)]
pub struct PeerId(pub u32);

struct AddrBook {
    peer_id: PeerId,
    ticker: Receiver<Instant>,
    receiver: Receiver<PeerMsg>,
    sender: Sender<PeerMsg>,
    known_peers: HashMap<PeerId, Sender<PeerMsg>>,
    done_sender: Option<Sender<()>>,
}

impl AddrBook {
    pub fn start(
        peer_id: PeerId,
        receiver: Receiver<PeerMsg>,
        sender: Sender<PeerMsg>,
        seed_node: (PeerId, Sender<PeerMsg>),
        done_sender: Sender<()>,
    ) {
        let _ = seed_node.1.send(PeerMsg::Request(sender.clone()));
        let mut known_peers = HashMap::new();
        known_peers.insert(seed_node.0, seed_node.1);

        let ticker = tick(Duration::from_millis(50));

        let mut addr_book = AddrBook {
            peer_id,
            ticker,
            receiver,
            sender,
            known_peers,
            done_sender: Some(done_sender),
        };

        thread::spawn(move || {
            while addr_book.run() {
                // running.
            }
        });
    }

    fn run(&mut self) -> bool {
        select! {
            recv(self.ticker) -> _ => {
                for peer_sender in self.known_peers.values() {
                    let _ = peer_sender.send(PeerMsg::Request(self.sender.clone()));
                }
            },
            recv(self.receiver) -> msg => {
                let msg = msg.unwrap();
                match msg {
                    PeerMsg::Request(response_sender) => {
                        let mut rng = thread_rng();
                        let peer_id =  PeerId(rng.gen_range(1, 10));
                        if let Some(sender) = self.known_peers.get(&peer_id) {
                            let _ = response_sender.send(PeerMsg::Response(sender.clone()));
                        }
                    },
                    PeerMsg::Response(sender) => {
                        let _ = sender.send(PeerMsg::Ping(self.sender.clone()));
                    },
                    PeerMsg::Ping(sender) => {
                        let info = PeerInfo {
                            id: self.peer_id.clone(),
                            sender: self.sender.clone(),
                        };
                        let _ = sender.send(PeerMsg::Pong(info));
                    },
                    PeerMsg::Pong(info) => {
                        self.known_peers.insert(info.id, info.sender);
                        if self.known_peers.len() == 10 {
                            // Send when done, only once.
                            if let Some(sender) = self.done_sender.take() {
                                let _ = sender.send(());
                            }
                        }
                    },
                    PeerMsg::Exit => {
                        return false;
                    }
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
    fn test_addr_book() {
        let mut peers: HashMap<PeerId, Sender<PeerMsg>> = HashMap::new();

        let seed_node = PeerId(0);
        let (seed_node_sender, seed_node_receiver) = unbounded();
        let (done_sender, done_receiver) = unbounded();

        for i in 1..10 {
            let node: PeerId = PeerId(i);

            let (peer_sender, peer_receiver) = unbounded();

            peers.insert(node.clone(), peer_sender.clone());

            AddrBook::start(
                node,
                peer_receiver,
                peer_sender,
                (seed_node.clone(), seed_node_sender.clone()),
                done_sender.clone(),
            );
        }

        let mut done_count = 0;

        loop {
            select! {
                recv(seed_node_receiver) -> msg => {
                    match msg {
                        Ok(PeerMsg::Request(response_sender)) => {
                            let mut rng = thread_rng();
                            let peer_id =  PeerId(rng.gen_range(1, 10));
                            let sender = peers.get(&peer_id).unwrap();
                            let _ = response_sender.send(PeerMsg::Response(sender.clone()));
                        },
                        _ => {}
                    }
                },
                recv(done_receiver) -> _ => {
                    done_count += 1;
                    if done_count == 9 {
                        for sender in peers.values() {
                            let _ = sender.send(PeerMsg::Exit);
                        }
                        break;
                    }
                }
            }
        }
    }
}
