//!
//! Tendermint peer-related functionality.
//!
//! Assumes a synchronous, blocking networking model that employs threads to
//! handle peer connectivity.
//!

use crossbeam::channel;
use log::info;
use std::io::{BufReader, BufWriter};
use std::net::TcpStream;
use std::thread;

use crate::encoding::{PeerMessageReader, PeerMessageWriter};
use crate::events::{Event, PeerEvent};
use crate::messages::PeerMessage;
use crate::types::ID;

#[derive(Debug)]
/// Models a remote peer in a Tendermint network. Encapsulates the state of the
/// peer, and allows us to communicate with that peer.
pub struct RemotePeer {
    // The cryptographic ID of this peer.
    id: ID,
    stream: TcpStream,

    // Allows us to send peer messages to the sending thread.
    sender_channel: channel::Sender<PeerMessage>,

    // Allows us to communicate with the node (e.g. by passing on incoming
    // messages from this peer).
    node_channel: channel::Sender<Event>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RemotePeerError {
    SendFailed(String),
    ReceiveFailed(String),
    Disconnected,
}

impl RemotePeer {
    /// Instantiates two threads for handling interaction with this peer: one
    /// for reading from the peer, and one for writing to it.
    pub fn new(
        id: ID,
        stream: TcpStream,
        node_channel: channel::Sender<Event>,
    ) -> std::io::Result<RemotePeer> {
        // TODO: Should the channels be bounded?
        let (tx_tx, tx_rx) = channel::unbounded::<PeerMessage>();
        let (peer_id_tx, peer_id_rx) = (id.clone(), id.clone());
        let (send_stream, receive_stream) = (stream.try_clone()?, stream.try_clone()?);
        let (send_evch, receive_evch) = (node_channel.clone(), node_channel.clone());

        // Spawn one thread for sending data
        thread::spawn(move || remote_peer_send_loop(peer_id_tx, send_stream, tx_rx, send_evch));
        // And another thread for receiving data
        thread::spawn(move || remote_peer_receive_loop(peer_id_rx, receive_stream, receive_evch));

        Ok(RemotePeer {
            id,
            stream,
            sender_channel: tx_tx,
            node_channel,
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

impl Drop for RemotePeer {
    fn drop(&mut self) {
        info!("Dropping remote peer: {}", self.id);
    }
}

// Takes incoming messages from `rx` and sends them out via the network
// interface for the specified peer.
fn remote_peer_send_loop(
    id: ID,
    stream: TcpStream,
    rx: channel::Receiver<PeerMessage>,
    node_channel: channel::Sender<Event>,
) -> Result<(), RemotePeerError> {
    let mut writer = BufWriter::new(stream);
    loop {
        // this will most likely only fail if the channel's been disconnected, which will happen
        // on termination
        let msg = rx.recv().map_err(|_| {
            let _ = node_channel.try_send(Event::Peer(PeerEvent::Disconnect(id.clone())));
            RemotePeerError::Disconnected
        })?;
        // try to write the incoming message out to the stream
        // if it fails, we just disconnect the peer
        let _ = writer.write_peer_message(msg).map_err(|e| {
            let _ = node_channel.try_send(Event::Peer(PeerEvent::Disconnect(id.clone())));
            RemotePeerError::SendFailed(e.to_string())
        })?;
    }
}

// Blocks on incoming connections to `writer`
fn remote_peer_receive_loop(
    id: ID,
    stream: TcpStream,
    node_channel: channel::Sender<Event>,
) -> Result<(), RemotePeerError> {
    let mut reader = BufReader::new(stream);
    loop {
        match reader.read_peer_message() {
            Ok(msg) => {
                node_channel
                    .send(Event::Peer(PeerEvent::ReceivedMessage {
                        id: id.clone(),
                        msg,
                    }))
                    .map_err(|e| {
                        let _ =
                            node_channel.try_send(Event::Peer(PeerEvent::Disconnect(id.clone())));
                        RemotePeerError::ReceiveFailed(e.to_string())
                    })?;
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(std::time::Duration::from_millis(100));
            }
            Err(e) => {
                let _ = node_channel.try_send(Event::Peer(PeerEvent::Disconnect(id.clone())));
                return Err(RemotePeerError::ReceiveFailed(e.to_string()));
            }
        }
    }
}
