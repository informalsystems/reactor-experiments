use tokio;
use futures::select;
use futures::prelude::*;
use tokio::net::{TcpStream, TcpListener};
use tokio::sync::mpsc;
use std::fmt;

use crate::encoding;
use crate::address_book::{PeerMessage, Entry, PeerID};
use crate::seed_node::Event as EEvent;
use log::{info};

pub enum Event {
    Connect(Entry),
    FromPeer(PeerID),
    PeerConnected(PeerID, encoding::MessageFramed),
    Error(String),
}

impl fmt::Display for Event {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "AcceptorEvent display")
    }
}

impl fmt::Debug for Event {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Event::Connect(entry) => {
                return write!(f, "AcceptorEvent::Connect({:?})", entry);
            },
            Event::FromPeer(peer_id) => {
                return write!(f, "AcceptorEvent::FromPeer({:?})", peer_id);
            },
            Event::PeerConnected(peer_id, _) => {
                return write!(f, "AcceptorEvent::PeerConnected({:?}, Stream)", peer_id);
            },
            Event::Error(error_str) => {
                return write!(f, "AcceptorEvent::Error({:?})",error_str);
            },
        }
    }
}

pub struct Acceptor {
    entry: Entry,
}

impl Acceptor {
    pub fn new(entry: Entry) -> Acceptor {
        return Acceptor { entry }
    }

    pub async fn run(self, mut send_ch: mpsc::Sender<EEvent>, mut rcv_ch: mpsc::Receiver<Event>) {
        let addr = format!("{}:{}", self.entry.ip, self.entry.port);

        let mut listener = TcpListener::bind(&addr).await.unwrap();
        // TODO what we need to do is send a ready Ready event to synchronize the simulation 
        info!("Node {} acceptor listneing listening on {}", self.entry.id, addr);

        let mut incoming_iter = listener.incoming();
        loop {
            select! {
                stream = incoming_iter.next().fuse() => {
                    // setup the connection
                    let stream = stream.unwrap();
                    match stream {
                        Ok(stream) => {
                            let mut cb = send_ch.clone();
                            let my_id = self.entry.id.clone();

                            let mut encoder = encoding::create_encoder(stream);

                            tokio::spawn(async move {
                               let msg = PeerMessage::Hello(my_id);
                               encoder
                                    .send(msg)
                                    .await
                                    .unwrap();

                                match encoder.try_next().await {
                                    Ok(Some(PeerMessage::Hello(peer_id))) => {
                                        let o_event: EEvent = EEvent::Acceptor(Event::PeerConnected(peer_id, encoder));
                                        cb.send(o_event).await.unwrap();
                                    },
                                    _ => {
                                        let o_event = EEvent::Acceptor(Event::Error("handshake failed".to_string()));
                                        cb.send(o_event).await.unwrap();
                                    }
                                }
                            });
                        },
                        Err(err) => {
                            println!("acceptor stream not ok, closing");
                            return
                        },
                    }

                },
                event = rcv_ch.recv().fuse() => {
                    if let Some(Event::Connect(entry)) = event {
                        info!("acceptor {} received Connect",  self.entry.id);
                        let connect_str = format!("{}:{}", entry.ip, entry.port);
                        info!("Connecting to: {}", connect_str);
                        let stream = TcpStream::connect(connect_str).await.unwrap();
                        let mut encoder = encoding::create_encoder(stream);
                        let oEvent: EEvent = EEvent::Acceptor(Event::PeerConnected(entry.id, encoder));
                        info!("sent out the event");
                        if let Ok(()) = send_ch.send(oEvent).await {
                            info!("channel send succeed");
                        } else {
                            panic!("couldn't send on channel, weird");
                        }
                    } else {
                        println!("acceptor rcv_ch closed");
                        return
                    }
                },
            }
        }
    }
}
