//!
//! Global types for a Tendermint node. Does not include messages. For messages,
//! see `messages.rs`.
//!

use std::net::IpAddr;
use std::sync::atomic::{AtomicU64, Ordering};

/// A node/peer ID is a hex-encoded cryptographic address.
pub type ID = String;

/// The initial message a peer sends when connecting to us.
#[derive(Debug, Clone, PartialEq)]
pub struct PeerHello {
    pub id: ID,
}

/// The address of a peer in the network.
#[derive(Debug, Clone, PartialEq)]
pub struct PeerAddr {
    /// The cryptographic address of this peer.
    pub id: ID,
    /// The IPv4/6 address of this peer.
    pub ip: IpAddr,
    /// The port on which the peer is listening for incoming connections.
    pub port: u16,
}

/// An address book containing the addresses of zero or more peers.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct AddressBook {
    pub addrs: Vec<PeerAddr>,
}

impl AddressBook {
    pub fn new() -> Self {
        AddressBook { addrs: Vec::new() }
    }
}

/// We define a subscription ID as a monotonically increasing unsigned integer
/// here.
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct SubscriptionID(u64);

// We use a global atomic subscription counter here. With 64-bit values, we
// shouldn't run out during the lifetime of our node.
static SUBSCRIPTION_COUNTER: AtomicU64 = AtomicU64::new(0);

impl Default for SubscriptionID {
    fn default() -> SubscriptionID {
        SubscriptionID(SUBSCRIPTION_COUNTER.fetch_add(1, Ordering::SeqCst))
    }
}

impl SubscriptionID {
    pub fn new() -> SubscriptionID {
        SubscriptionID::default()
    }
}
