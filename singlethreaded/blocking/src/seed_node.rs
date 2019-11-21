//!
//! A rough implementation of a seed node for a Tendermint network.
//!

/// A seed node for a Tendermint network. Its only purpose is to crawl other
/// seeds and peers to gather the addresses of peers in the network, and make
/// these addresses available to other peers.
pub struct SeedNode {}
