use tokio_serde::SymmetricallyFramed;
use tokio_serde::formats::*; //can we get rid of this?
use tokio_util::codec::{FramedWrite, FramedRead, LengthDelimitedCodec};
use tokio::net::TcpStream;
use crate::address_book::{PeerMessage};

// Transport: ReadFrame<TcpStream, >?
// Value: PeerMessage
// Codec: SymetricalJson
// What we need try_next and send, where does those methods come from?
// If Item=Result then we can use try_next
type EvFramed = SymmetricallyFramed<TcpStream, PeerMessage, LengthDelimitedCodec>;

pub fn create_both(stream: TcpStream) -> (EvFramed, EvFramed) {

    let (reader, writer) = stream.split();

    return (create_serializer(writer), create_deserializer(reader))
}

pub fn create_serializer(stream: TcpStream) -> EvFramed {
    let write_frame = FramedWrite::new(
        stream,
        LengthDelimitedCodec::new());

    let mut serializer = EvFramed::new(
        stream,
        SymmetricalJson::<PeerMessage>::default(),
    );
}

pub fn create_deserializer(stream: TcpStream) -> EvFramed {
    let read_frame = FramedRead::new(
        stream,
        LengthDelimitedCodec::new());

    let mut deserializer = EvFramed::new(
        read_frame, // Transport
        SymmetricalJson::<PeerMessage>::default(), // codec
    );
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
