use tokio;
use futures::{
    future::FutureExt, // for `.fuse()`
    select,
};
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use std::collections::HashMap;

use crate::address_book::{PeerMessage, PeerID};
use crate::encoding;
use futures::prelude::*; // XXX: Remove this?
use tokio::prelude::*;

#[derive(Debug)]
pub enum Event {
    Connected(PeerID, TcpStream),
    AddPeer(PeerID),

    FromPeer(PeerID, PeerMessage),
    ToPeer(PeerID, PeerMessage),

    Error(Error),
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
    fn new() -> Dispatcher {
        Dispatcher {
            peers: HashMap::<PeerID, mpsc::Sender<PeerMessage>>::new(),
        }
    }

    pub async fn run(mut self, mut tosend_ch: mpsc::Sender<Event>, mut rcv_ch: mpsc::Receiver<Event>) {
        loop {
            if let Some(event) = rcv_ch.recv().await {
                match event {
                    Event::Connected(peer_id, stream) => {
                        let peer_output = tosend_ch.clone();
                        let (mut peer_sender, mut peer_receiver) = mpsc::channel::<PeerMessage>(0);

                        let thread_peer_id = peer_id.clone();
                        tokio::spawn(async move {
                            run_peer_thread(thread_peer_id, peer_receiver, peer_output, stream);
                        });

                        let index_peer_id = peer_id.clone();
                        self.peers.insert(peer_id.clone(), peer_sender).unwrap();

                        let event_peer_id = peer_id.clone();
                        tosend_ch.send(Event::AddPeer(peer_id)).await.unwrap();
                    },
                    Event::ToPeer(peer_id, message) => {
                        if let Some(peer) = self.peers.get_mut(&peer_id)  {
                            peer.send(message).await.unwrap();
                        } else {
                            tosend_ch.send(Event::Error(Error::PeerNotFound(peer_id))).await.unwrap();
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
    mut received_ch: mpsc::Sender<Event>, // Events received from the socket and sent downstream
    stream: TcpStream) { // socket to read and write to

    let mut encoder  = encoding::create_encoder(stream);
    loop {
        select! {
            msg = encoder.try_next().fuse() => {
                let msg = msg.unwrap().unwrap(); // This will panic on done, instead we should match
                received_ch.send(Event::FromPeer(peer_id.clone(), msg)).await.unwrap();
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
    use super::*;

    // Test connection: passing in a stream
    // test Sending: Does it write to the stream
    // Test Reading: does it receive the message on the other side
    #[test]
    fn test_basic_peer_interaction() {
    }
}
