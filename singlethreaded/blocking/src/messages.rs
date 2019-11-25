//!
//! All messages used in/by the system.
//!

use crate::encoding::{protos, AnyTyped, MessageWriterError};
use crate::types;
use prost_types::Any;

/// A message from one peer to another.
#[derive(Debug, Clone, PartialEq)]
pub enum PeerMessage {
    Hello(types::PeerHello),
    AddressBookRequest,
    AddressBook(types::AddressBook),
}

impl From<PeerMessage> for Result<Any, MessageWriterError> {
    fn from(msg: PeerMessage) -> Result<Any, MessageWriterError> {
        match msg {
            PeerMessage::Hello(hello) => protos::PeerHello { id: hello.id }.to_protobuf_any(),
            PeerMessage::AddressBookRequest => protos::AddressBookRequest {}.to_protobuf_any(),
            PeerMessage::AddressBook(addr_book) => {
                protos::AddressBook::from(addr_book).to_protobuf_any()
            }
        }
    }
}
