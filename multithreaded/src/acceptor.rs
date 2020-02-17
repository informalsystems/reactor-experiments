use crate::dispatcher::DispatcherMsg;
use crate::encoding;
use crate::tendermint::Entry;
use tokio::net::TcpListener;
use tokio::stream::StreamExt;
use tokio::sync::mpsc;

pub struct Acceptor {
    pub dispatcher_sender: mpsc::UnboundedSender<DispatcherMsg>,
    pub entry: Entry,
}

impl Acceptor {
    pub async fn run(&mut self) {
        let addr = format!("{}:{}", self.entry.ip, self.entry.port);

        let mut listener = TcpListener::bind(&addr)
            .await
            .expect("Failed to bind listener.");
        println!("Listening on {}", addr);

        while let Some(Ok(stream)) = listener.incoming().next().await {
            let encoder = encoding::create_encoder(stream);

            let _ = self
                .dispatcher_sender
                .send(DispatcherMsg::PeerHandshake(encoder, self.entry.id.clone()));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tendermint::PeerId;
    use std::net::IpAddr;
    use std::str::FromStr;
    use std::thread;
    use tokio::io::AsyncWriteExt;
    use tokio::net::TcpStream;
    use tokio::runtime::Runtime;

    #[test]
    fn test_acceptor() {
        let (dispatcher_sender, mut dispatcher_receiver) = mpsc::unbounded_channel();

        let seed_node: PeerId = Default::default();

        let mut acceptor = Acceptor {
            dispatcher_sender: dispatcher_sender.clone(),
            entry: Entry {
                id: seed_node.clone(),
                ip: IpAddr::from_str("127.0.0.1").unwrap(),
                port: 8080,
            },
        };

        thread::spawn(move || {
            let mut rt = Runtime::new().expect("Failed to create runtime.");

            rt.spawn(async move {
                let mut stream = TcpStream::connect("127.0.0.1:8080")
                    .await
                    .expect("Failed to connect stream.");
                let _ = stream.write_all(b"hello world!").await;
            });

            rt.block_on(async move {
                let _ = acceptor.run().await;
            });
        });

        // Assert the acceptor sends the handshake message to the dispatcher.
        loop {
            if let Ok(msg) = dispatcher_receiver.try_recv() {
                match msg {
                    DispatcherMsg::PeerHandshake(_, seed_node_id) => {
                        assert_eq!(seed_node_id, seed_node);
                        break;
                    }
                    _ => continue,
                };
            }
        }
    }
}
