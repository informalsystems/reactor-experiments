use tokio;
use futures::select;
use tokio::sync::mpsc;
use std::collections::HashMap;
use futures::prelude::*;
use log::info;

use crate::address_book::{PeerMessage, PeerID, Entry};
use crate::encoding::Stream;
use crate::seed_node::Event as EEvent;

#[derive(PartialEq, Eq, Hash, Debug)]
pub enum Event {
    PeerConnected(Entry, Stream),
    AddPeer(PeerID, Entry),

    FromPeer(PeerID, PeerMessage),
    ToPeer(PeerID, PeerMessage),

    Error(Error),
}


#[derive(PartialEq, Eq, Hash, Debug)]
pub enum Error {
    PeerNotFound(PeerID)
}

pub struct Dispatcher {
    id: PeerID,
    peers: HashMap<PeerID, mpsc::Sender<PeerMessage>>,
}

//  The input is is every peers socket + the 
impl Dispatcher {
    pub fn new(peer_id: PeerID) -> Dispatcher {
        Dispatcher {
            id: peer_id,
            peers: HashMap::<PeerID, mpsc::Sender<PeerMessage>>::new(),
        }
    }

    pub async fn run(mut self, mut tosend_ch: mpsc::Sender<EEvent>, mut rcv_ch: mpsc::Receiver<Event>) {
        loop {
            if let Some(event) = rcv_ch.recv().await {
                info!("[{}] received event {:?}", self.id, event);
                match event {
                    Event::PeerConnected(entry, stream) => {
                        let peer_output = tosend_ch.clone();
                        let (peer_sender, peer_receiver) = mpsc::channel::<PeerMessage>(1);

                        let thread_peer_id = entry.id.clone();
                        let my_id = self.id.clone();
                        tokio::spawn(async move {
                            run_peer_thread(my_id, thread_peer_id, peer_receiver, peer_output, stream).await;
                        });

                        // why would this panic?
                        self.peers.insert(entry.id.clone(), peer_sender);

                        let event_peer_id = entry.id.clone();
                        tosend_ch.send(Event::AddPeer(event_peer_id, entry).into()).await.unwrap();
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
                info!("[{}] Closing dipatcher", self.id);
                break;
            }
        }
    }
}

async fn run_peer_thread(
    my_id: PeerID,
    peer_id: PeerID,
    mut tosend_ch: mpsc::Receiver<PeerMessage>, // Events to write to socket
    mut received_ch: mpsc::Sender<EEvent>, // Events received from the socket and sent downstream
    stream: Stream) { // socket to read and write to

    let mut encoder = stream.get_framed();
    loop {
        select! {
            msg = encoder.next().fuse() => {
                // Here we are receiving Ok(None) From the Peer
                info!("[{}] peer thread {} received {:?}", my_id, peer_id, msg);
                if let Some(Ok(msg)) = msg {
                    received_ch.send(Event::FromPeer(peer_id.clone(), msg).into()).await.unwrap();
                } else {
                    info!("[{}] dispatcher {} thread done", my_id, peer_id);
                    return
                }
            },
            potential_peer_message = tosend_ch.recv().fuse() => {
                if let Some(message) = potential_peer_message {
                    info!("[{}] Sending message {:?}", my_id, message);
                    // TODO: serialize and write to socket
                } else {
                    // XXX: why is this being closed?
                    info!("[{}] sender {} tosend_ch done", my_id, peer_id);
                    break;
                }
            },
        }
    }
}
