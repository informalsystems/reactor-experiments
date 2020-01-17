use tokio_serde::SymmetricallyFramed;
use tokio_serde::formats::*; //can we get rid of this?
use tokio_util::codec::{FramedWrite, FramedRead, LengthDelimitedCodec};
use tokio::net::TcpStream;
use tokio::net::tcp::{ReadHalf, WriteHalf};

use crate::address_book::{PeerMessage};
use futures::prelude::*;

//

//
// Framed<Transport, Item, SinkItem, Codec>
// So what is the Transport here?
// it looks like transport is the "input"
type EvCodec = tokio_serde::formats::json::Json<PeerMessage, PeerMessage>;

type EvWriteTransport = FramedWrite<WriteHalf<'static>, LengthDelimitedCodec>;
type EvWriteFramed = SymmetricallyFramed<EvWriteTransport, PeerMessage, EvCodec>;


type EvReadTransport = FramedRead<ReadHalf<'static>, LengthDelimitedCodec>;
type EvReadFramed = SymmetricallyFramed<EvReadTransport, PeerMessage, EvCodec>;

pub fn create_both(stream: TcpStream) -> (EvReadFramed, EvWriteFramed) {
    // this doesn't work as split works with references which will outlive the lifetime of this
    // function scope for sure, we can however 
    let (reader, writer) = stream.split();

    return (create_deserializer(reader), create_serializer(writer))
}

pub fn create_serializer(stream: WriteHalf) -> EvWriteFramed {
    // XXX: maybe these are in the wrong order
    let write_frame = FramedWrite::new(
        stream,
        LengthDelimitedCodec::new());

    let mut serializer = EvWriteFramed::new(
        write_frame, // transport
        // tokio_util::codec::framed_write::FramedWrite<tokio::net::tcp::split::WriteHalf<'_>, tokio_util::codec::length_delimited::LengthDelimitedCodec>
        SymmetricalJson::<PeerMessage>::default(), // Codec
        // sould be tokio_serde::formats::json::Json<address_book::PeerMessage, address_book::PeerMessage>`
    );

    return serializer
}

pub fn create_deserializer(stream: ReadHalf) -> EvReadFramed {
    let read_frame = FramedRead::new(
        stream,
        LengthDelimitedCodec::new());

    let mut deserializer = EvReadFramed::new(
        read_frame, // Transport
        SymmetricalJson::<PeerMessage>::default(), // codec
    );
    // Transport: tokio_util::codec::framed_read::FramedRead<tokio::net::tcp::split::ReadHalf<'_>, tokio_util::codec::length_delimited::LengthDelimitedCodec>
    // Codec: tokio_serde::formats::json::Json<address_book::PeerMessage, address_book::PeerMessage>

    return deserializer
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
