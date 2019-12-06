//!
//! All system events originate from the core `Event` type defined here.
//!

use crate::messages::PeerMessage;
use crate::pubsub::Subscription;
use crate::types::{PeerAddr, SubscriptionID, ID};
use std::collections::HashSet;
use std::net::TcpStream;

/// The core event model for the whole system.
#[derive(Debug, Clone)]
pub enum Event {
    /// All peer-related events.
    Peer(PeerEvent),

    /// Events relating to the node's internal pub/sub mechanism.
    PubSub(PubSubRequest),

    /// A termination signal has been received for the node (e.g. SIGTERM).
    Terminate,
}

impl Event {
    /// Returns the `EventID` associated with this particular kind of event, if
    /// any. If it returns `None`, this means you cannot subscribe to it.
    pub fn id(&self) -> Option<EventID> {
        match self {
            Event::Peer(e) => e.id(),
            // It's dangerous to allow people to subscribe to subscription
            // events (infinite loops), and there's no potential use case for it
            // at present.
            Event::PubSub(_) => None,
            Event::Terminate => Some(EventID::Terminate),
        }
    }
}

/// Events related to a specific peer. We keep the underlying stream type
/// generic to allow for different kinds of streams in future, beyond just TCP
/// streams.
#[derive(Debug)]
pub enum PeerEvent {
    /// A peer connected to us (or we connected to them).
    Connected(TcpStream),

    /// Part of the system wants us to connect to the given peer. The assumption
    /// is that this string contains a `host:port` string.
    Connect(String),

    /// A peer has been added to our address book.
    AddedToAddressBook(PeerAddr),

    /// We received a message from a peer.
    ReceivedMessage { id: ID, msg: PeerMessage },

    /// Some part of the system needs to send a message to a particular peer.
    SendMessage { id: ID, msg: PeerMessage },

    /// A registered peer was disconnected, or we want to disconnect them.
    Disconnect(ID),
}

impl Clone for PeerEvent {
    fn clone(&self) -> PeerEvent {
        match self {
            PeerEvent::Connected(stream) => PeerEvent::Connected(stream.try_clone().unwrap()),
            PeerEvent::Connect(addr) => PeerEvent::Connect(addr.clone()),
            PeerEvent::AddedToAddressBook(addr) => PeerEvent::AddedToAddressBook(addr.clone()),
            PeerEvent::ReceivedMessage { id, msg } => PeerEvent::ReceivedMessage {
                id: id.clone(),
                msg: msg.clone(),
            },
            PeerEvent::SendMessage { id, msg } => PeerEvent::SendMessage {
                id: id.clone(),
                msg: msg.clone(),
            },
            PeerEvent::Disconnect(id) => PeerEvent::Disconnect(id.clone()),
        }
    }
}

impl PeerEvent {
    pub fn id(&self) -> Option<EventID> {
        match self {
            PeerEvent::Connected(_) => Some(EventID::PeerConnected),
            PeerEvent::Connect(_) => Some(EventID::PeerConnect),
            PeerEvent::AddedToAddressBook(_) => Some(EventID::PeerAddedToAddressBook),
            PeerEvent::ReceivedMessage { .. } => Some(EventID::PeerReceivedMessage),
            PeerEvent::SendMessage { .. } => Some(EventID::PeerSendMessage),
            PeerEvent::Disconnect(_) => Some(EventID::PeerDisconnect),
        }
    }
}

/// Allows internal entities to subscribe to different kinds of events that
/// occur within a node.
#[derive(Debug, Clone)]
pub enum PubSubRequest {
    /// Sent when an internal entity wants to subscribe to a particular kind of
    /// event.
    Subscribe(Subscription),

    /// Sent when an internal entity wants to terminate one or more
    /// subscriptions.
    Unsubscribe(HashSet<SubscriptionID>),
}

/// The events to which one can subscribe.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum EventID {
    PeerConnected,
    PeerConnect,
    PeerAddedToAddressBook,
    PeerReceivedMessage,
    PeerSendMessage,
    PeerDisconnect,
    Terminate,
}
