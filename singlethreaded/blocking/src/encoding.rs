//!
//! This module provides the necessary functionality to facilitate parsing
//! messages from streams and writing them to streams (e.g. TCP streams). This
//! is necessary because Protobuf messages are not inherently length-delimited,
//! and so we need our own wire format to provide length delimiting to incoming
//! and outgoing Protobuf messages.
//!
//! We encode all of our messages as Protobuf `Any` messages, with the types
//! embedded in the body of the message itself. See
//! https://docs.rs/prost-types/0.5.0/prost_types/struct.Any.html for details.
//!
//! Each `Any` message is preceded by a fixed-size message length delimiter (see
//! `LENGTH_DELIMITER_LEN`). Fixed-size delimiters are limiting in terms of
//! size, but make it easier to implement message streaming protocols.
//!

use bytes::BytesMut;
use prost::{DecodeError, EncodeError, Message};
use prost_types::Any;
use std::io::{Error, ErrorKind, Read, Write};

use crate::messages::PeerMessage;
use crate::types;

pub mod protos {
    use crate::encoding::AnyTyped;
    use crate::types;
    use std::net::IpAddr;
    use std::str::FromStr;

    include!(concat!(env!("OUT_DIR"), "/protos.messages.rs"));

    pub const TYPEURL_PEERHELLO: &str = "PeerHello";
    pub const TYPEURL_ADDRESSBOOK: &str = "AddressBook";
    pub const TYPEURL_ADDRESSBOOKREQUEST: &str = "AddressBookRequest";

    impl AnyTyped for PeerHello {
        fn type_url() -> String {
            TYPEURL_PEERHELLO.to_string()
        }
    }

    impl AnyTyped for AddressBook {
        fn type_url() -> String {
            TYPEURL_ADDRESSBOOK.to_string()
        }
    }

    impl AnyTyped for AddressBookRequest {
        fn type_url() -> String {
            TYPEURL_ADDRESSBOOKREQUEST.to_string()
        }
    }

    impl From<PeerHello> for std::io::Result<types::PeerHello> {
        fn from(hello: PeerHello) -> std::io::Result<types::PeerHello> {
            Ok(types::PeerHello { id: hello.id })
        }
    }

    impl From<PeerAddr> for std::io::Result<types::PeerAddr> {
        fn from(peer_addr: PeerAddr) -> std::io::Result<types::PeerAddr> {
            Ok(types::PeerAddr {
                id: peer_addr.id,
                ip: IpAddr::from_str(peer_addr.ip.as_ref())
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?,
                port: peer_addr.port as u16,
            })
        }
    }

    impl From<AddressBook> for std::io::Result<types::AddressBook> {
        fn from(addr_book: AddressBook) -> std::io::Result<types::AddressBook> {
            let mut addrs = Vec::<types::PeerAddr>::new();
            for addr in addr_book.addrs {
                addrs.push(std::io::Result::<types::PeerAddr>::from(addr)?);
            }
            Ok(types::AddressBook { addrs })
        }
    }

    impl From<types::PeerAddr> for PeerAddr {
        fn from(peer_addr: types::PeerAddr) -> PeerAddr {
            PeerAddr {
                id: peer_addr.id,
                ip: peer_addr.ip.to_string(),
                port: peer_addr.port as u32,
            }
        }
    }

    impl From<&types::PeerAddr> for PeerAddr {
        fn from(peer_addr: &types::PeerAddr) -> PeerAddr {
            PeerAddr {
                id: peer_addr.id.clone(),
                ip: peer_addr.ip.to_string(),
                port: peer_addr.port as u32,
            }
        }
    }

    impl From<types::AddressBook> for AddressBook {
        fn from(addr_book: types::AddressBook) -> AddressBook {
            AddressBook {
                addrs: addr_book.addrs.iter().map(PeerAddr::from).collect(),
            }
        }
    }
}

/// We limit message sizes to 10kB at present.
pub const MAX_MESSAGE_LEN: usize = 10 * 1024;

/// The number of bytes in the length delimiter. Must be enough to accommodate
/// `MAX_MESSAGE_LEN`.
pub const LENGTH_DELIMITER_LEN: usize = 2;

/// The `AnyTyped` trait allows us to convert a particular `Message` type
/// to/from a Protobuf `Any` instance. The only method that is required to be
/// implemented is the `type_url` static method, which provides the type's URL
/// for encoding.
pub trait AnyTyped: Sized + Default + Message {
    fn type_url() -> String;

    fn from_protobuf_any(any: Any) -> Result<Self, MessageReaderError> {
        Ok(Self::decode(any.value)?)
    }

    fn to_protobuf_any(&self) -> Result<Any, MessageWriterError> {
        let mut buf = BytesMut::with_capacity(MAX_MESSAGE_LEN);
        self.encode(&mut buf)?;
        Ok(Any {
            type_url: Self::type_url(),
            value: buf.freeze().to_vec(),
        })
    }
}

#[derive(Debug, Clone)]
pub enum MessageReaderError {
    ParseError(DecodeError),
    MessageTooLarge,
    EmptyMessage,
    UnrecognizedType(String),
}

#[derive(Debug, Clone)]
pub enum MessageWriterError {
    EncodingError(EncodeError),
    EmptyMessage,
    MessageTooLarge,
}

impl std::fmt::Display for MessageReaderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MessageReaderError::ParseError(e) => write!(f, "message reader parse error: {}", e),
            MessageReaderError::MessageTooLarge => {
                write!(f, "incoming message exceeded {} bytes", MAX_MESSAGE_LEN)
            }
            MessageReaderError::EmptyMessage => write!(f, "received a null-length message"),
            MessageReaderError::UnrecognizedType(s) => write!(f, "unrecognized type \"{}\"", s),
        }
    }
}

impl std::fmt::Display for MessageWriterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MessageWriterError::EncodingError(e) => write!(f, "failed to encode message: {}", e),
            MessageWriterError::EmptyMessage => write!(f, "cannot send empty messages"),
            MessageWriterError::MessageTooLarge => {
                write!(f, "outgoing message exceeded {} bytes", MAX_MESSAGE_LEN)
            }
        }
    }
}

impl std::error::Error for MessageReaderError {}
impl std::error::Error for MessageWriterError {}

impl From<DecodeError> for MessageReaderError {
    fn from(e: DecodeError) -> Self {
        MessageReaderError::ParseError(e)
    }
}

impl From<MessageReaderError> for Error {
    fn from(e: MessageReaderError) -> Self {
        Error::new(ErrorKind::Other, e)
    }
}

impl From<EncodeError> for MessageWriterError {
    fn from(e: EncodeError) -> Self {
        MessageWriterError::EncodingError(e)
    }
}

impl From<MessageWriterError> for Error {
    fn from(e: MessageWriterError) -> Self {
        Error::new(ErrorKind::Other, e)
    }
}

pub trait PeerMessageReader: AnyReader {
    fn read_peer_message(&mut self) -> std::io::Result<PeerMessage> {
        let msg_encoded = self.read_any()?;
        match msg_encoded.type_url.as_ref() {
            protos::TYPEURL_ADDRESSBOOKREQUEST => Ok(PeerMessage::AddressBookRequest),
            protos::TYPEURL_ADDRESSBOOK => Ok(PeerMessage::AddressBook(std::io::Result::<
                types::AddressBook,
            >::from(
                protos::AddressBook::from_protobuf_any(msg_encoded)?,
            )?)),
            _ => Err(MessageReaderError::UnrecognizedType(msg_encoded.type_url).into()),
        }
    }
}

/// Allows us to read Protobuf `Any` instances from a readable.
pub trait AnyReader: Read {
    /// Attempts to read a length-prefixed `Any` message from the given
    /// readable.
    fn read_any(&mut self) -> std::io::Result<Any> {
        // we attempt to read up to 10 bytes first to extract the varint that
        // represents the message length
        let mut len_buf: [u8; LENGTH_DELIMITER_LEN] = [0; LENGTH_DELIMITER_LEN];
        self.read_exact(&mut len_buf)?;

        let msg_len = decode_length_delimiter(&len_buf);
        if msg_len == 0 {
            return Err(MessageReaderError::EmptyMessage.into());
        }
        if msg_len > MAX_MESSAGE_LEN {
            return Err(MessageReaderError::MessageTooLarge.into());
        }

        let mut msg_buf: Vec<u8> = vec![0; msg_len];
        self.read_exact(&mut msg_buf)?;

        // now try to parse a Protobuf `Any` instance from the message buffer
        Ok(Any::decode(msg_buf)?)
    }
}

// Extend all readables.
impl<T> PeerMessageReader for T where T: AnyReader {}
impl<T> AnyReader for T where T: Read {}

pub trait PeerMessageWriter: AnyWriter {
    fn write_peer_message(&mut self, msg: PeerMessage) -> std::io::Result<()> {
        self.write_any(Result::<Any, MessageWriterError>::from(msg)?)
    }
}

/// Trait that facilitates writing [`Any`][1] messages.
pub trait AnyWriter: Write {
    fn write_any(&mut self, msg: Any) -> std::io::Result<()> {
        let mut buf = BytesMut::with_capacity(MAX_MESSAGE_LEN);
        // serialize the message
        msg.encode(&mut buf)?;
        let msg_len = buf.len();
        // we don't allow zero-length messages
        if msg_len == 0 {
            return Err(MessageWriterError::EmptyMessage.into());
        }
        // we also don't allow messages that are too large
        if msg_len > MAX_MESSAGE_LEN {
            return Err(MessageWriterError::MessageTooLarge.into());
        }
        // write the message length delimiter
        let delim = encode_length_delimiter(msg_len);
        self.write_all(&delim)?;
        // write the message to the output
        Ok(self.write_all(&buf)?)
    }
}

// Extend all writeables.
impl<T> PeerMessageWriter for T where T: AnyWriter {}
impl<T> AnyWriter for T where T: Write {}

fn decode_length_delimiter(buf: &[u8; LENGTH_DELIMITER_LEN]) -> usize {
    let mut usize_buf: [u8; 8] = [0; 8];
    usize_buf[..LENGTH_DELIMITER_LEN].clone_from_slice(&buf[..]);
    usize::from_le_bytes(usize_buf)
}

fn encode_length_delimiter(len: usize) -> [u8; LENGTH_DELIMITER_LEN] {
    let mut delim: [u8; LENGTH_DELIMITER_LEN] = [0; LENGTH_DELIMITER_LEN];
    let encoded = len.to_le_bytes();
    delim.clone_from_slice(&encoded[..LENGTH_DELIMITER_LEN]);
    delim
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use std::net::{IpAddr, Ipv4Addr};

    use crate::types::{AddressBook, PeerAddr, ID};

    #[test]
    fn test_length_delimiter_encoding() {
        let expected: Vec<u8> = vec![0x34, 0x12];
        assert_eq!(expected, encode_length_delimiter(4660));
    }

    #[test]
    fn test_length_delimiter_decoding() {
        let delim: [u8; LENGTH_DELIMITER_LEN] = [0x34, 0x12];
        assert_eq!(4660, decode_length_delimiter(&delim));
    }

    #[test]
    fn test_any_readwrite() {
        let orig_msg = Any {
            type_url: "sometype".to_string(),
            value: vec![0x12, 0x34, 0x56, 0x78],
        };
        let mut buf = Cursor::new(Vec::new());
        buf.write_any(orig_msg.clone()).unwrap();
        buf.set_position(0);
        // read the message back out
        let decoded_msg = buf.read_any().unwrap();
        // check that they're exactly the same
        assert_eq!(orig_msg, decoded_msg);
    }

    #[test]
    fn test_anytyped_peerhello() {
        let msg = protos::PeerHello {
            id: ID::from("abcd"),
        };
        let any = msg.to_protobuf_any().unwrap();
        assert_eq!(protos::TYPEURL_PEERHELLO, any.type_url);
        let msg_decoded = protos::PeerHello::decode(any.clone().value).unwrap();
        assert_eq!(msg, msg_decoded);

        let msg_decoded = protos::PeerHello::from_protobuf_any(any).unwrap();
        assert_eq!(msg, msg_decoded);
    }

    #[test]
    fn test_peer_message_readwrite() {
        let orig_msg = PeerMessage::AddressBook(AddressBook {
            addrs: vec![
                PeerAddr {
                    id: ID::from("abcd"),
                    ip: IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
                    port: 8001,
                },
                PeerAddr {
                    id: ID::from("efgh"),
                    ip: IpAddr::V4(Ipv4Addr::new(192, 168, 1, 10)),
                    port: 8080,
                },
            ],
        });
        let mut buf = Cursor::new(Vec::with_capacity(MAX_MESSAGE_LEN));
        buf.write_peer_message(orig_msg.clone()).unwrap();
        buf.set_position(0);
        let decoded_msg = buf.read_peer_message().unwrap();
        assert_eq!(orig_msg, decoded_msg);
    }
}
