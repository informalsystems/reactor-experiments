use tokio;
use futures::select;
use tokio::sync::mpsc;
use std::collections::HashMap;
use futures::prelude::*;
use std::fmt;

use crate::address_book::{PeerMessage, PeerID, Entry};
use crate::encoding;
use crate::seed_node::Event as EEvent;

pub enum Event {
    PeerConnected(PeerID, encoding::MessageFramed),
    AddPeer(PeerID, Entry),

    FromPeer(PeerID, PeerMessage),
    ToPeer(PeerID, PeerMessage),

    Error(Error),
}

impl fmt::Display for Event {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "DispatcherEvent debug")
    }
}

impl fmt::Debug for Event {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Event::PeerConnected(peer_id, _) => {
                return write!(f, "DispatcherEvent::PeerConnected({:?})", peer_id);
            },
            Event::AddPeer(peer_id, entry) => {
                return write!(f, "DispatcherEvent::AddPeer({:?}, {:?})", peer_id, entry);
            },
            Event::FromPeer(peer_id, peer_message) => {
                return write!(f, "DispatcherEvent::FromPeer({:?}, {:?})", peer_id, peer_message);
            },
            Event::ToPeer(peer_id, peer_message) => {
                return write!(f, "DispatcherEvent::ToPeer({:?}, {:?})", peer_id, peer_message);
            },
            Event::Error(error) => {
                return write!(f, "DispatcherEvent::Error({:?})", error);
            },
        }
    }
}

#[derive(Debug)]
pub enum Error {
    PeerNotFound(PeerID)
}

pub struct Dispatcher {
    peers: HashMap<PeerID, mpsc::Sender<PeerMessage>>,
}

//  The input is is every peers socket + the 
impl Dispatcher {
    pub fn new() -> Dispatcher {
        Dispatcher {
            peers: HashMap::<PeerID, mpsc::Sender<PeerMessage>>::new(),
        }
    }

    pub async fn run(mut self, mut tosend_ch: mpsc::Sender<EEvent>, mut rcv_ch: mpsc::Receiver<Event>) {
        loop {
            if let Some(event) = rcv_ch.recv().await {
                // XXX: extrat this into a hande function
                match event {
                    Event::PeerConnected(peer_id, stream) => {
                        let peer_output = tosend_ch.clone();
                        let (mut peer_sender, mut peer_receiver) = mpsc::channel::<PeerMessage>(0);

                        let thread_peer_id = peer_id.clone();
                        tokio::spawn(async move {
                            run_peer_thread(thread_peer_id, peer_receiver, peer_output, stream).await;
                        });

                        let index_peer_id = peer_id.clone();
                        self.peers.insert(peer_id.clone(), peer_sender).unwrap();

                        let event_peer_id = peer_id.clone();
                        // buah we need an Entry here
                        tosend_ch.send(Event::AddPeer(peer_id, Entry::default()).into()).await.unwrap();
                    },
                    Event::ToPeer(peer_id, message) => {
                        if let Some(peer) = self.peers.get_mut(&peer_id)  {
                            peer.send(message).await.unwrap();
                        } else {
                            tosend_ch.send(Event::Error(Error::PeerNotFound(peer_id)).into()).await.unwrap();
                        }
                    },
                    // TODO: Handle Peer Removal
                    _ => {
                        // skip
                    },
                }
            } else {
                println!("Closing dipatcher");
                break;
            }
        }
    }
}

async fn run_peer_thread(
    peer_id: PeerID,
    mut tosend_ch: mpsc::Receiver<PeerMessage>, // Events to write to socket
    mut received_ch: mpsc::Sender<EEvent>, // Events received from the socket and sent downstream
    mut encoder: encoding::MessageFramed) { // socket to read and write to

    loop {
        select! {
            msg = encoder.try_next().fuse() => {
                let msg = msg.unwrap().unwrap(); // This will panic on done, instead we should match
                received_ch.send(Event::FromPeer(peer_id.clone(), msg).into()).await.unwrap();
            },
            potential_peer_message = tosend_ch.recv().fuse() => {
                if let Some(message) = potential_peer_message {
                    // TODO: serialize and write to socket
                } else {
                    break;
                }
            },
        }
    }
}

#[cfg(test)]
mod tests {
    // use super::*;

    // Test connection: passing in a stream
    // test Sending: Does it write to the stream
    // Test Reading: does it receive the message on the other side
    #[test]
    fn test_basic_peer_interaction() {
    }
}
