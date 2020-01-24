use tokio;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use crate::encoding;
use crate::address_book::{PeerMessage, Entry};

#[derive(Debug)]
pub enum Event {
    Connect(Entry),
    FromPeer(PeerID),
    PeerConnected(PeerID, TcpStream)
}

pub type Acceptor {
    peer_id: PeerID;
}

impl Acceptor {
    fn new(peer_id: PeerID) -> Acceptor {
        return Acceptor { peer_id }
    }

    pub async fn run(mut self, send_ch: channel::Sender<Event>, rcv_ch: channel::Receiver<Event>) {
        let addr = "127.0.0.1:8080".to_string();

        let mut listener = TcpListener::bind(&addr).await;
        println!("Listening on {}", addr);


        // TODO: Receve channel to alow acceptor to be told who to connect to
        while let Some(stream) = listener.incomming().try_next().await.unwrap() {
            let cb = send_ch.clone();
            let my_id = self.peer_id.clone();

            let encoder = encoding::create_encoders(stream);

            tokio::task(async move {
                // First we say hello
                let msg = PeerMessage::Hello(PeerHello { id: my_id });
               encoder
                    .send(msg)
                    .await
                    .unwrap();

                if let Some(msg) = deserialized.try_next().await {
                    match PeerMessage::Hello(peer_id) => {
                        cb.send(Event::PeerConnected(stream, peer_id)).unwrap();
                    },
                    _ => {
                        // TODO error handling
                    }
                };
            });
        }
    }
}
