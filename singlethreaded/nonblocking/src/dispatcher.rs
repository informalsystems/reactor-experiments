use tokio;
use futures::{
    future::FutureExt, // for `.fuse()`
    pin_mut,
    select,
};
use tokio::net::TcpStream;
use std::collections::HashMap;

use crate::address_book::{PeerMessage, Event, PeerID, Error};
use crate::encoding;
use tokio::sync::mpsc;
//use futures::prelude::*;

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
                    Event::Connection(peer_id, stream) => {
                        let peer_output = tosend_ch.clone();
                        let (mut peer_sender, mut peer_receiver) = mpsc::channel::<PeerMessage>(0);

                        let thread_peer_id = peer_id.clone();
                        tokio::spawn(async move {
                            run_peer_thread(thread_peer_id, peer_receiver, peer_output, stream);
                        });

                        let index_peer_id = peer_id.clone();
                        self.peers.insert(peer_id.clone(), peer_sender).unwrap();

                        let event_peer_id = peer_id.clone();
                        tosend_ch.send(Event::Connected(peer_id)).await.unwrap();
                    },
                    Event::ToPeer(peer_id, message) => {
                        if let Some(peer) = self.peers.get_mut(&peer_id)  {
                            peer.send(message).await.unwrap();
                        } else {
                            tosend_ch.send(Event::Error(Error::PeerNotFound())).await.unwrap();
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
    received_ch: mpsc::Sender<Event>, // Events received from the socket and sent downstream
    stream: TcpStream) { // socket to read and write to

    let encoder  = encoding::create_encoder(stream);
    loop {
        select! {
            //msg = received_deserializer.try_next().fuse() => {
            //    let msg = msg.unwrap(); // This will panic on done, instead we should match
            //    received_ch.send(Event::FromPeer(peer_id.clone(), msg)).await.unwrap();
            //},
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
