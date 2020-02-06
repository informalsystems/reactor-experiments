use tokio;
use futures::select;
use futures::prelude::*;
use tokio::net::{TcpStream, TcpListener};
use tokio::sync::mpsc;

use crate::encoding::Stream;
use crate::address_book::{PeerMessage, Entry};
use crate::seed_node::Event as EEvent;
use log::{info};

#[derive(PartialEq, Eq, Hash, Debug)]
pub enum Event {
    Connect(Entry),
    PeerConnected(Entry, Stream),
    Error(String),
}

pub struct Acceptor {
    entry: Entry,
}

// TODO: Emit a READY event such that simulations can be synchronized

// Acceptor establishes connections involving a handshake process
// Acceptor can receive connection from a socket, on connection
// It will send Hello on the socket
// In response the Peer is expected to say Hello back
// On success, emit a Event::PeerConnected(peer_id, stream)
//
// Accepor can be Told to establish a connection by received a Event::Connect(entry)
// On Event::Connect, setup a new stream to a peer
// on connection, wait for the peer to say hello
// On Hello, respond with hello
// emit a Event::PeerConnected(peer_id, stream)
impl Acceptor {
    pub fn new(entry: Entry) -> Acceptor {
        return Acceptor { entry }
    }

    pub async fn run(self, mut send_ch: mpsc::Sender<EEvent>, mut rcv_ch: mpsc::Receiver<Event>) {
        let addr = format!("{}:{}", self.entry.ip, self.entry.port);

        let mut listener = TcpListener::bind(&addr).await.unwrap();
        info!("[{}] listneing listening on {}", self.entry.id, addr);

        let mut incoming_iter = listener.incoming();
        loop {
            select! {
                stream = incoming_iter.next().fuse() => {
                    let stream = stream.unwrap();
                    match stream {
                        Ok(stream) => {
                            let mut cb = send_ch.clone();
                            let my_id = self.entry.id.clone();

                            let peer_addr = stream.peer_addr().unwrap();
                            info!("[{}] received connection from {}", my_id, peer_addr);
                            let mut encoder = Stream::new(stream).get_framed();

                            tokio::spawn(async move {
                               info!("[{}] saying hello to {}", my_id, peer_addr);
                               let msg = PeerMessage::Hello(my_id.clone());
                               encoder
                                    .send(msg)
                                    .await
                                    .unwrap();

                                match encoder.try_next().await {
                                    Ok(msg) => {
                                        if let Some(PeerMessage::Hello(peer_id)) = msg {
                                           info!("[{}] received hello from {}", my_id, peer_id);
                                           // this need an entry
                                           let entry = Entry::new(peer_id, peer_addr.ip(), peer_addr.port());
                                           let new_stream = Stream::from_framed(encoder);
                                           let o_event: EEvent = EEvent::Acceptor(Event::PeerConnected(entry, new_stream));
                                           cb.send(o_event).await.unwrap();
                                        } else {
                                            panic!("received unknown msg type back");
                                        }
                                    }
                                    Err(err) => {
                                        info!("[{}] error {:?}", my_id, err);
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
                        info!("[{}] received Connect",  self.entry.id);
                        let connect_str = format!("{}:{}", entry.ip, entry.port);
                        info!("[{}] Connecting to: {}", self.entry.id, connect_str);

                        // Establish the connection and handshake
                        let stream = TcpStream::connect(connect_str).await.unwrap();
                        let peer_addr = stream.peer_addr().unwrap();
                        let mut encoder = Stream::new(stream).get_framed();

                        match encoder.try_next().await {
                            Ok(msg) => {
                                if let Some(PeerMessage::Hello(peer_id)) = msg {
                                    info!("[{}] Received Hello from {}",  self.entry.id, peer_id);
                                    let msg = PeerMessage::Hello(self.entry.id.clone());
                                    encoder
                                        .send(msg)
                                        .await
                                        .unwrap();
                                        let entry = Entry::new(peer_id.clone(), peer_addr.ip(), peer_addr.port());
                                        let new_stream = Stream::from_framed(encoder);
                                        let o_event: EEvent = EEvent::Acceptor(Event::PeerConnected(entry, new_stream));
                                        send_ch.send(o_event).await.unwrap();
                                } else {
                                    panic!("received unknown msg type back");
                                }
                            }
                            Err(err) => {
                                info!("[{}] error {:?}", self.entry.id, err);
                                let o_event = EEvent::Acceptor(Event::Error("handshake failed".to_string()));
                                send_ch.send(o_event).await.unwrap();
                            }
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
