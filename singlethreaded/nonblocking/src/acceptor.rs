use tokio;
use futures::prelude::*;
use tokio::prelude::*;
use tokio::net::TcpListener;
use tokio::sync::mpsc;

use crate::encoding;
use crate::address_book::{PeerMessage, Entry, PeerID};

#[derive(Debug)]
pub enum Event {
    Connect(Entry),
    FromPeer(PeerID),
    PeerConnected(PeerID, encoding::MessageFramed)
}

pub struct Acceptor {
    entry: Entry,
}

// Would it be possible to make event handling of this component synchronous and deterministic.
// Every async function can be made synchronous using .await, however is it deterministic?
// As in do we know the precise order in which await will return?
// Would it be possible to model Handle functions as async functions which can be sequenced
// I think the handle function could be async and sequence of asynchronous events could be composed
// via combinators.
// And so the routing table should compose a compbinator which could be used in deterministic
// testing.
impl Acceptor {
    fn new(entry: Entry) -> Acceptor {
        return Acceptor { entry }
    }

    pub async fn run(mut self, send_ch: mpsc::Sender<Event>, rcv_ch: mpsc::Receiver<Event>) {
        let addr = format!("{}:{}", self.entry.ip, self.entry.port);

        let mut listener = TcpListener::bind(&addr).await.unwrap();
        println!("Listening on {}", addr);

        // What's the difference between Option and Result
        // Option has Some or None
        // Error is Ok or Error
        // TODO: Receve channel to alow acceptor to be told who to connect to
        // What the fuck is incomming
        // incomming returns Option<Result>
        while let Some(Ok(stream)) = listener.incoming().next().await {
            let cb = send_ch.clone();
            let my_id = self.entry.id.clone();

            let encoder = encoding::create_encoder(stream);

            tokio::task(async move {
                // First we say hello
               let msg = PeerMessage::Hello(my_id);
               encoder
                    .send(msg)
                    .await
                    .unwrap();

                if let Some(msg) = encoder.try_next().await.unwrap() {
                    if let PeerMessage::Hello(peer_id) = msg {
                        cb.send(Event::PeerConnected(peer_id, encoder)).await.unwrap();
                    }
                };
            });
        }
    }
}
