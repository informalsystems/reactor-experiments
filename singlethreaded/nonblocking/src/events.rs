use crate::address_book::{PeerID, Error};

// Dictionary of events for each component
#[derive(Debug)]
pub enum Event {
    AddPeer(Entry),
    PeerAdded(PeerID),

    Connection(PeerID, TcpStream),
    Connected(PeerID),

    PollTrigger(),
    PollPeers(PeerList),

    ToPeer(PeerID, PeerMessage),
    FromPeer(PeerID, PeerMessage),

    Terminate(),
    Terminated(),

    Error(Error),

    NoOp(),
    Modified(),
}
