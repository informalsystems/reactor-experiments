//!
//! Tendermint peer-related functionality.
//!
//! Assumes a synchronous, blocking networking model that employs threads to
//! handle peer connectivity.
//!

use bytes::{Bytes, Buf, IntoBuf};
use crossbeam::channel;
use prost::{DecodeError, Message};
use prost::encoding::decode_varint;
use prost_types::Any;
use std::io::{BufReader, BufWriter, Read};
use std::net::{IpAddr, TcpStream};
use std::thread;
use std::str::FromStr;

pub mod protos {
    include!(concat!(env!("OUT_DIR"), "/protos.p2p.rs"));
}

/// ID is a hex-encoded cryptographic address.
pub type ID = String;

#[derive(Debug, Clone, PartialEq)]
pub enum RemotePeerError {
    ChannelFailure(String),
    SendFailed(String),
    NetworkError(String),
    ParseError(DecodeError),
    UnrecognizedType(String),
    TooMuchData,
}

#[derive(Debug, PartialEq, Clone)]
/// The different kinds of messages peers can send to each other.
pub enum PeerMessage {
    RequestAddressBook,
    AddressBook(AddressBookResponse),
}

/// A peer's address, which consists of its cryptographic ID, its IP address and
/// port.
#[derive(Debug, Clone, PartialEq)]
pub struct RemotePeerAddr {
    pub id: ID,
    pub ip: IpAddr,
    pub port: u16,
}

impl From<protos::Addr> for RemotePeerAddr {
    fn from(addr: protos::Addr) -> RemotePeerAddr {
        RemotePeerAddr{
            id: addr.id,
            ip: IpAddr::from_str(addr.ip.as_ref()).unwrap(),
            port: addr.port as u16,
        }
    }
}

/// An address book is simply a list of `PeerAddr` structures.
#[derive(Debug, Clone, PartialEq)]
pub struct AddressBookResponse {
    pub addrs: Vec<RemotePeerAddr>,
}

impl From<protos::AddressBook> for AddressBookResponse {
    fn from(address_book: protos::AddressBook) -> AddressBookResponse {
        AddressBookResponse{
            addrs: address_book.addrs
                .into_iter()
                .map(RemotePeerAddr::from)
                .collect()
        }
    }
}

/// Models a remote peer in a Tendermint network. Encapsulates the state of the
/// peer, and allows us to communicate with that peer.
pub struct RemotePeer {
    // The cryptographic ID of this peer.
    id: ID,

    // Allows us to send peer messages to the sending thread.
    sender_channel: channel::Sender<PeerMessage>,
    // Allows us to receive peer messages from the receiving thread.
    receiver_channel: channel::Receiver<PeerMessage>,
    // We send networking errors to this channel to indicate that at least one of the peer's
    // threads have stopped.
    error_channel: channel::Sender<RemotePeerError>,
}

impl RemotePeer {
    /// Instantiates two threads for handling interaction with this peer: one
    /// for reading from the peer, and one for writing to it.
    pub fn new(
        id: ID,
        stream: &TcpStream,
        error_channel: channel::Sender<RemotePeerError>,
    ) -> std::io::Result<RemotePeer> {
        // To allow for full duplex communication, we need 4 endpoints (2 for
        // each channel)
        // TODO: Should the channels be bounded?
        let (tx_tx, tx_rx) = channel::unbounded::<PeerMessage>();
        let (rx_tx, rx_rx) = channel::unbounded::<PeerMessage>();
        let (peer_id_tx, peer_id_rx) = (id.clone(), id.clone());
        let (send_stream, receive_stream) = (stream.try_clone()?, stream.try_clone()?);
        let (send_errch, receive_errch) = (error_channel.clone(), error_channel.clone());

        // Spawn one thread for sending data
        thread::spawn(move || {
            remote_peer_send_loop(peer_id_tx, send_stream, tx_rx, send_errch)
        });
        // And another thread for receiving data
        thread::spawn(move || {
            remote_peer_receive_loop(peer_id_rx, receive_stream, rx_tx, receive_errch)
        });

        Ok(RemotePeer {
            id,
            sender_channel: tx_tx,
            receiver_channel: rx_rx,
            error_channel,
        })
    }

    /// Attempts to send a message to this peer. Technically, this sends the
    /// message to the peer's network write routine, which will attempt to
    /// serialize the message and send it out to the peer.
    pub fn send(&self, msg: PeerMessage) -> Result<(), RemotePeerError> {
        if let Err(e) = self.sender_channel.send(msg) {
            return Err(RemotePeerError::SendFailed(e.to_string()));
        }
        Ok(())
    }
}

// Takes incoming messages from `rx` and sends them out via the network
// interface for the specified peer.
fn remote_peer_send_loop(
    id: ID,
    stream: TcpStream,
    rx: channel::Receiver<PeerMessage>,
    error_channel: channel::Sender<RemotePeerError>,
) -> Result<(), RemotePeerError> {
    let mut writer = BufWriter::new(stream);
    loop {
        // this will most likely only fail if the channel's been disconnected, which will happen
        // on termination
        let m = rx
            .recv()
            .map_err(|e| {
                let err = RemotePeerError::SendFailed(e.to_string());
                error_channel.try_send(err.clone());
                err
            })?;
    }
}

// Blocks on incoming connections to `writer`
fn remote_peer_receive_loop(
    id: ID,
    stream: TcpStream,
    tx: channel::Sender<PeerMessage>,
    error_channel: channel::Sender<RemotePeerError>,
) -> Result<(), RemotePeerError> {
    const READ_BUF_SIZE: usize = 4096;
    const REMAINDER_BUF_SIZE: usize = 1024 * 1024;
    let mut reader = BufReader::new(stream);
    // we use a 4KB buffer here to read from the input stream
    let mut read_buf: [u8; READ_BUF_SIZE] = [0; READ_BUF_SIZE];
    // unprocessed data left over from the previous read that cannot be
    // interpreted yet because we probably don't have a full message yet
    let mut remainder_buf: [u8; REMAINDER_BUF_SIZE] = [0; REMAINDER_BUF_SIZE];
    let mut remainder_len: usize = 0;
    loop {
        let bytes_read = reader.read(&mut read_buf).map_err(|e| {
            let err = RemotePeerError::NetworkError(e.to_string());
            match error_channel.try_send(err.clone()) {
                Ok(_) => err,
                Err(e) => RemotePeerError::ChannelFailure(e.to_string()),
            }
        })?;
        if bytes_read == 0 {
            continue;
        }
        let mut data = Bytes::from(&remainder_buf[..remainder_len]);
        data.extend_from_slice(&read_buf[..]);
        let mut buf = data.into_buf();
        let parser = PeerStreamParser{buf: &mut buf};
        for result in parser {
            match result {
                // try send this message out to whomever's listening for it
                Ok(msg) => tx.try_send(msg).map_err(|e| RemotePeerError::ChannelFailure(e.to_string()))?,
                // TODO: Should this kill the peer's whole operation? Probably,
                // because we won't know how to interpret any further incoming
                // bytes from the peer.
                Err(e) => return Err(e),
            }
        }
        remainder_len = buf.remaining();
        // if we've exceeded our capacity to hold the remainder, it must mean
        // the peer's trying to push far too much data to us because none of our
        // messages will ever exceed 100kB, let alone 1MB
        if remainder_len > REMAINDER_BUF_SIZE {
            let err = RemotePeerError::TooMuchData;
            return Err(match error_channel.try_send(err.clone()) {
                Ok(_) => err,
                Err(e) => RemotePeerError::ChannelFailure(e.to_string()),
            })
        }
        buf.copy_to_slice(&mut remainder_buf[..]);
    }
}

struct PeerStreamParser<'a, B>
where
    B: Buf
{
    buf: &'a mut B,
}

impl<'a, B> Iterator for PeerStreamParser<'a, B>
where
    B: Buf
{
    type Item = Result<PeerMessage, RemotePeerError>;

    fn next(&mut self) -> Option<Self::Item> {
        // we need at least 10 bytes to decode the varint indicating the length
        // of the next message
        if self.buf.remaining() < 10 {
            return None
        }
        match decode_varint(&mut self.buf) {
            Ok(msg_len) => {
                let raw_msg = self.buf.take(msg_len as usize);
                let msg_encoded = match Any::decode(raw_msg.into_inner()) {
                    Ok(m) => m,
                    Err(e) => return Some(Err(RemotePeerError::ParseError(e))),
                };
                match msg_encoded.type_url.as_ref() {
                    "p2p/RequestAddressBook" => Some(Ok(PeerMessage::RequestAddressBook)),
                    "p2p/AddressBook" => {
                        let msg_buf = Bytes::from(msg_encoded.value).into_buf();
                        let msg_decoded = match protos::AddressBook::decode(msg_buf) {
                            Ok(m) => m,
                            Err(e) => return Some(Err(RemotePeerError::ParseError(e))),
                        };
                        Some(Ok(PeerMessage::AddressBook(AddressBookResponse::from(msg_decoded))))
                    },
                    _ => Some(Err(RemotePeerError::UnrecognizedType(msg_encoded.type_url))),
                }
            },
            Err(e) => Some(Err(RemotePeerError::ParseError(e))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::BytesMut;
    use std::net::TcpListener;
    use std::str::FromStr;
    use prost::Message;
    use prost_types::Any;
    use std::io::Write;
    use std::time::Duration;
    use crossbeam::channel::*;

    struct PeerTestServer {
        addr: String,
        listener: TcpListener,
    }

    impl PeerTestServer {
        fn new(host_addr: &str, port: u16) -> PeerTestServer {
            let addr: String = format!("{}:{}", host_addr, port);
            let listener = TcpListener::bind(addr.clone()).unwrap();
            PeerTestServer{addr, listener}
        }

        fn attach_peer(&self, id: ID) -> (RemotePeer, channel::Receiver<RemotePeerError>) {
            let stream = TcpStream::connect(self.addr.clone()).unwrap();
            let (error_tx, error_rx) = channel::unbounded::<RemotePeerError>();
            let peer = RemotePeer::new(id, &stream, error_tx).unwrap();
            (peer, error_rx)
        }

        fn spawn_single_interaction_thread(&self) -> channel::Receiver<Any> {
            let thread_listener = self.listener.try_clone().unwrap();
            let (tx, rx) = channel::unbounded::<Any>();
            std::thread::spawn(move || {
                // wait for an incoming connection
                let incoming = thread_listener.accept().unwrap().0;
                let mut writer = BufWriter::new(incoming.try_clone().unwrap());
                // send a message to request the peer's address book
                let msg = Any{
                    type_url: String::from("p2p/RequestAddressBook"),
                    value: Vec::new(),
                };
                let mut write_buf = BytesMut::new();
                msg.encode_length_delimited(&mut write_buf).unwrap();
                let write_buf = write_buf.freeze();
                writer.write_all(write_buf.as_ref()).unwrap();

                // now we try to read a response from the peer
                let mut reader = BufReader::new(incoming.try_clone().unwrap());
                // read some data from the incoming connection
                let mut read_buf: [u8; 1024] = [0; 1024];
                let bytes_read = reader.read(&mut read_buf).unwrap();
                let data_buf = Bytes::from(&read_buf[..bytes_read]);
                let msg_encoded = Any::decode_length_delimited(&data_buf).unwrap();
                tx.send(msg_encoded).unwrap();
            });
            rx
        }
    }

    #[test]
    fn test_basic_peer_connectivity() {
        let test_server = PeerTestServer::new("127.0.0.1", 36000);
        let server_rx = test_server.spawn_single_interaction_thread();
        let (_, error_rx) = test_server.attach_peer(ID::from_str("abcd").unwrap());

        // the server will send a RequestAddressBook message when the peer
        // connects, so wait here until we get an AddressBook response
        select! {
            recv(server_rx) -> r => assert_eq!("p2p/AddressBook", r.unwrap().type_url),
            recv(error_rx) -> r => panic!("{:?}", r.unwrap()),
            default(Duration::from_secs(10)) => panic!("timed out waiting for test"),
        }
    }
}

