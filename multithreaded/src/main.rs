#[macro_use]
extern crate crossbeam;
#[macro_use]
extern crate serde;

mod acceptor;
mod concurrent_addr_book;
mod dispatcher;
mod encoding;
mod tendermint;

use crate::tendermint::Tendermint;
use ipc_channel::ipc;

#[tokio::main(basic_scheduler)]
async fn main() {
    println!("Hello from multithreaded!");

    // let's assume we get an IPC sender from the abci layer.
    let (abci_sender, _abci_receiver) = ipc::channel().expect("ipc channel failure");
    let (_tendermint_sender, mut acceptor) = Tendermint::start(abci_sender);

    // Run until exit.
    let _ = acceptor.run().await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dispatcher::*;
    use crate::encoding;
    use crate::tendermint::*;
    use crossbeam::channel::unbounded;
    use futures_util::sink::SinkExt;
    use futures_util::stream::StreamExt;
    use ipc_channel::ipc::IpcSender;
    use std::thread;
    use tokio::net::TcpStream;
    use tokio::runtime::Runtime;

    #[test]
    fn test_everything() {
        let (abci_sender, _abci_receiver) = ipc::channel().expect("ipc channel failure");

        enum TestResult {
            Failure,
            Success,
            Chan(IpcSender<TendermintMsg>),
        }

        // A channel which the task representing the other node,
        // can use to give feedback to the main test thread.
        let (result_sender, result_receiver) = unbounded();
        let result_sender_clone = result_sender.clone();

        thread::spawn(move || {
            let mut rt = Runtime::new().unwrap();

            // Start the "other node".
            rt.spawn(async move {
                let stream = TcpStream::connect("127.0.0.1:8083")
                    .await
                    .expect("Failed to connect stream.");
                let mut encoder = encoding::create_encoder(stream);

                // First, wait for the hello message from the dispatcher.
                let msg = encoder.next().await;
                match msg {
                    Some(Ok(PeerMessage::Hello(seed_node_id))) => {
                        assert_eq!(seed_node_id, Default::default());
                    }
                    _ => {
                        // Unexpected message from the dispatcher, fail the test.
                        let _ = result_sender.send(TestResult::Failure);
                    }
                }
                // Next, we say hello back.
                let msg = PeerMessage::Hello(PeerId(String::from("Connected")));
                encoder.send(msg).await.unwrap();

                // Finally, wait for an addr-book request.
                let msg = encoder.next().await;
                match msg {
                    Some(Ok(PeerMessage::AddressBookRequest(seed_node_id))) => {
                        assert_eq!(seed_node_id, Default::default());
                    }
                    _ => {
                        // Unexpected message from the dispatcher, fail the test.
                        let _ = result_sender.send(TestResult::Failure);
                    }
                }

                // Tell the main thread test successful so far.
                let _ = result_sender.send(TestResult::Success);
            });

            // Start tendermint and the acceptor.
            rt.block_on(async move {
                let (tendermint_sender, mut acceptor) = Tendermint::start(abci_sender);
                let _ = result_sender_clone.send(TestResult::Chan(tendermint_sender));
                let _ = acceptor.run().await;
            });
        });

        // A way to access the sender to tendermint.
        let mut tendermint_sender: Option<IpcSender<TendermintMsg>> = None;

        // Assert the async test is successful.
        loop {
            select! {
                recv(result_receiver) -> msg => {
                    match msg {
                         Ok(TestResult::Failure) => {
                             panic!("Integration test failed.");
                         },
                         Ok(TestResult::Success) => {
                             // Shut down tendermint.
                             let sender = tendermint_sender.take().expect("Failed to receive sender to tendermint.");
                             let (exit_sender, exit_receiver) = ipc::channel().expect("ipc channel failure");
                             let _ = sender.send(TendermintMsg::Exit(exit_sender));
                             let _ = exit_receiver.recv().expect("Failed to get confirmation of shutdown.");
                             break;
                         },
                         Ok(TestResult::Chan(sender)) => {
                             tendermint_sender = Some(sender);
                         }
                         Err(_) => {
                             panic!("Integration test async part panicked.");
                         }
                    }
                }
            }
        }
    }
}
