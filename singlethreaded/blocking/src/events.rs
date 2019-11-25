//!
//! All system events originate from the core `Event` type defined here.
//!

use crate::messages::PeerMessage;
use crate::types::{PeerAddr, ID};
use std::net::TcpStream;

/// The core event model for the whole system.
pub enum Event {
    /// All peer-related events.
    Peer(PeerEvent),

    /// A termination signal has been received for the node (e.g. SIGTERM).
    Terminate,
}

/// Events related to a specific peer. We keep the underlying stream type
/// generic to allow for different kinds of streams in future, beyond just TCP
/// streams.
pub enum PeerEvent {
    /// A peer connected to us (or we connected to them).
    Connected(TcpStream),

    /// Part of the system wants us to connect to the given peer.
    Connect(PeerAddr),

    /// We received a message from a peer.
    ReceivedMessage { id: ID, msg: PeerMessage },

    /// Some part of the system needs to send a message to a particular peer.
    SendMessage { id: ID, msg: PeerMessage },

    /// A registered peer was disconnected, or we want to disconnect them.
    Disconnect(ID),
}
