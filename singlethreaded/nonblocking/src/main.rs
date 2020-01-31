// mod seed_node;
#![warn(rust_2018_idioms)]
#![recursion_limit="1024"]
use tokio;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use std::error::Error;


mod seed_node;
mod address_book;
mod acceptor;
mod encoding;
mod dispatcher;

// Why don't we have to start the runtime here?
// can  move this into an node.run function?
//
// What will the architecture of this look like?
// we need a seed node
// each seed node will register a handler function
//  * handler function is probably just this loop
//
//  In the old version we have seed_node.spawn_server_thread
//  - this thread will listen on the socket
//  And we also have seed_node.run
//      - which listens on the eventbus
//  these two will be async functions
#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    println!("Hello from singlethreaded-nonblocking");

    let addr = "127.0.0.1:8080".to_string();

    let mut listener = TcpListener::bind(&addr).await?;
    println!("Listening on {}", addr);

    // the event loop
    loop {
        let (mut socket, _) = listener.accept().await?;

        // spawn per connection handler
        tokio::spawn(async move {
            let mut buf = [0; 1024];

            // read from the socket
            loop {
                let n = socket
                    .read(&mut buf)
                    .await
                    .expect("failed to read data from socket");

                if n == 0 {
                    return;
                }

                socket
                    .write_all(&buf[0..n])
                    .await
                    .expect("failed to write data to socket")
            }
        });
    }
}
