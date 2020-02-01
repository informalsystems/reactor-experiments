use tokio;
use futures::select;
use futures::prelude::*;
use tokio::net::{TcpStream, TcpListener};
use tokio::sync::mpsc;

use crate::encoding;
use crate::address_book::{PeerMessage, Entry, PeerID};
use crate::seed_node::Event as EEvent;

pub enum Event {
    Connect(Entry),
    FromPeer(PeerID),
    PeerConnected(PeerID, encoding::MessageFramed)
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
        println!("Listening on {}", addr);

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
                                // First we say hello
                               let msg = PeerMessage::Hello(my_id);
                               encoder
                                    .send(msg)
                                    .await
                                    .unwrap();

                                // XXX: Peer disconnected
                                if let Some(msg) = encoder.try_next().await.unwrap() {
                                    if let PeerMessage::Hello(peer_id) = msg {
                                        let oEvent: EEvent = EEvent::Acceptor(Event::PeerConnected(peer_id, encoder));
                                        cb.send(oEvent).await;
                                    }
                                };
                            });
                        },
                        Err(err) => {
                            println!("acceptor stream not ok, closing");
                            return
                        },
                    }

                },
                event = rcv_ch.recv().fuse() => {
                    println!("accepted received event");
                    if let Some(Event::Connect(entry)) = event {
                        let connect_str = format!("{}:{}", entry.ip, entry.port);
                        println!("Connecting to: {}", connect_str);
                        let stream = TcpStream::connect(connect_str).await.unwrap();
                        let mut encoder = encoding::create_encoder(stream);
                        let oEvent: EEvent = EEvent::Acceptor(Event::PeerConnected(entry.id, encoder));
                        println!("sent out the event");
                        if let Ok(()) = send_ch.send(oEvent).await {
                            println!("channel send succeed");
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
