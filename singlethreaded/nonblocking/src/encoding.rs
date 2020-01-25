use futures::prelude::*;
use tokio::prelude::*;

use tokio_serde::{SymmetricallyFramed, formats::SymmetricalJson};
use tokio_util::codec::{Framed, LengthDelimitedCodec};
use tokio::net::TcpStream;

use crate::address_book::{PeerMessage};

type TcpFrame = Framed<TcpStream, LengthDelimitedCodec>;
type JsonFrame = SymmetricallyFramed<TcpFrame, PeerMessage, SymmetricalJson<PeerMessage>>;

pub fn create_encoder(stream: TcpStream) -> JsonFrame {
    let tcp_frame = TcpFrame::new(
        stream,
        LengthDelimitedCodec::new());

    let mut json_frame = JsonFrame::new(
        tcp_frame,
        SymmetricalJson::<PeerMessage>::default(),
    );

    return json_frame;
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