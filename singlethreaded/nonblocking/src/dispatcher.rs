use tokio;
// use tokio::io::{AsyncReadExt, AsyncWriteExt};
use futures::{
    future::FutureExt, // for `.fuse()`
    pin_mut,
    select,
};
use tokio::net::TcpStream;
use std::collections::HashMap;

use crate::address_book::{Event, PeerID, Error};
use crate::encoding;
use tokio::sync::mpsc;
use futures::prelude::*;

pub struct Dispatcher {
    peers: HashMap<PeerID, mpsc::Sender<Event>>,
}

//  The input is is every peers socket + the 
impl Dispatcher {
    fn new() -> Dispatcher {
        Dispatcher {
            peers: HashMap::new(),
        }
    }

    pub async fn run(mut self, tosend_ch: mpsc::Sender<Event>, rcv_ch: mpsc::Receiver<Event>) {
        loop {
            if let Some(event) = rcv_ch.recv().await {
                match event {
                    Event::Connection(peer_id, stream) => {
                        let peer_output = tosend_ch.clone();
                        let (mut peer_sender, mut peer_receiver) = mpsc::channel::<Event>(0);

                        tokio::spawn(async move {
                            run_peer_thread(peer_id.clone(), peer_receiver, peer_output, stream);
                        });

                        self.peers.insert(peer_id.clone(), peer_sender).unwrap();
                        tosend_ch.send(Event::Connected(peer_id)).await.unwrap();
                    },
                    Event::ToPeer(peer_id, message) => {
                        if let Some(peer) = self.peers.get_mut(&peer_id)  {
                            peer.send(event).await.unwrap();
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
    tosend_ch: mpsc::Receiver<Event>, // Events to write to socket
    received_ch: mpsc::Sender<Event>, // Events received from the socket and sent downstream
    stream: TcpStream) { // socket to read and write to

    let (tosend_serializer, received_deserializer) = encoding::create_both(stream);
    loop {
        select! {
            //msg = received_deserializer.try_next().fuse() => {
            //    let msg = msg.unwrap(); // This will panic on done, instead we should match
            //    received_ch.send(Event::FromPeer(peer_id.clone(), msg)).await.unwrap();
            //},
            Some(event) = tosend_ch.recv().fuse() => {
                match event {
                    Event::ToPeer(peer_d, message) => {
                        //tosend_serializer.send(message).await.unwrap();
                    },
                    _ => {
                        panic!("dunno how to send this");
                    }
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
