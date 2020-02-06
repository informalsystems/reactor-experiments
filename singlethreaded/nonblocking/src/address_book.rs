use std::net::IpAddr;
use std::collections::HashMap;
use std::str::FromStr;
use tokio::sync::mpsc;
use serde::{Deserialize, Serialize};
use log::info;
use std::hash::{Hash, Hasher};

use crate::seed_node::Event as EEvent;


#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Event {
    AddPeer(Entry),
    // RemovePeer
    PeerAdded(PeerID),

    PollTrigger(),
    PollPeers(PeerList),

    ToPeer(PeerID, PeerMessage),
    FromPeer(PeerID, PeerMessage),

    Terminate(),
    Terminated(),

    NoOp(),
    Modified(),
}

// We need to make this an actual type
type Mapping = HashMap<PeerID, Entry>;

#[derive(Hash, Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum PeerMessage {
    Hello(PeerID),
    AddressBookRequest(),
    AddressBookResponse(AddressBook),
}

pub type PeerID = String;
pub type PeerList = Vec<PeerID>;

/// The address of a peer in the network.
#[derive(Hash, Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct Entry {
    /// The cryptographic address of this peer.
    pub id: PeerID,
    /// The IPv4/6 address of this peer.
    pub ip: IpAddr,
    /// The port on which the peer is listening for incoming connections.
    pub port: u16,
}

impl Entry {
    pub fn new(id: PeerID, ip: IpAddr, port: u16) -> Entry {
        return Entry {
            id: id,
            ip: ip,
            port: port,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AddressBook {
    id: PeerID,
    mapping: HashMap<PeerID, Entry>,
}

// XXX: use a stupid hash which will cause collisions
impl Hash for AddressBook {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // XXX: Dummy
        "foobar".to_string().hash(state);
    }
}

impl AddressBook {
    pub fn new(peer_id: PeerID) -> AddressBook {
        return AddressBook {
            id: peer_id,
            mapping: HashMap::new(),
        }
    }

    fn handle(&mut self, event: Event) -> Event {
        match event {
            Event::AddPeer(entry) => { // will need PeerInfo
                if self.mapping.contains_key(&entry.id) {
                    return Event::NoOp();
                } else {
                    info!("[{}] adding peer {:?}", self.id, entry);
                    self.mapping.insert(entry.id.clone(), entry.clone());
                    return Event::PeerAdded(entry.id);
                }
            },
            Event::PollTrigger() => {
                // TODO: Choose LRU 20
                let requested_peers = self.mapping.keys().map(|x| x.to_string()).collect();
                return Event::PollPeers(requested_peers);
            },
            Event::FromPeer(peer_id, message) => {
                match message {
                    PeerMessage::AddressBookRequest() => {
                       let message = PeerMessage::AddressBookResponse(self.clone());
                       return Event::ToPeer(peer_id, message);
                    },
                    PeerMessage::AddressBookResponse(other) => {
                        if self.mapping == other.mapping {
                            return Event::NoOp();
                        } else {
                            self.mapping.extend(other.mapping);
                            return Event::Modified(); // XXX: Produce diff
                        }
                    },
                    _ => {
                        info!("unprocessed message from peer");
                        return Event::NoOp()
                    },
                }
            },
            Event::Terminate() => {
                return Event::Terminated();
            },
            _ => {
                info!("Missed event: {:?}", event);
                return Event::NoOp();
            },
        }
    }

    // This can probably be generalized for all runners
    pub async fn run(mut self, mut send_ch: mpsc::Sender<EEvent>, mut rcv_ch: mpsc::Receiver<Event>) {
        while let Some(event) = rcv_ch.recv().await {
            send_ch.send(self.handle(event).into()).await.unwrap();
        }
    }
}

type Entries = Vec<(PeerID, Entry)>;
impl From<Entries> for AddressBook {
    fn from(entries: Entries) -> Self {
        let mapping: HashMap<PeerID, Entry> = entries.iter().cloned().collect();
        return AddressBook {
            id: PeerID::from(""),
            mapping,
        }
    }
}

// Deterministic Sequences
// To test deterministic operations of a finite state machine
// Define a sequence composed of steps
// Each step must be triggered in order
// Each step matches an event,
// On match, send the next event in the sequence
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fsm() {
        let mut address_book = AddressBook::new(PeerID::from("2"));

        let id = PeerID::from("2");
        let ip_addr = IpAddr::from_str("127.0.0.1").unwrap();
        let port: u16 = 0;
        let peer_1_entry = Entry::new(PeerID::from("1"), ip_addr.clone(), port);
        let peer_2_entry = Entry::new(PeerID::from("2"), ip_addr.clone(), port);
        let peer_3_entry = Entry::new(PeerID::from("3"), ip_addr.clone(), port);

        let peer_2_mapping: Mapping = [(PeerID::from("3"), peer_3_entry.clone())].iter().cloned().collect();
        let address_book_2 = AddressBook {
            id: PeerID::from("2"),
            mapping: peer_2_mapping,
        };

        let mapping_1 = [
            (PeerID::from("2"), peer_2_entry.clone()),
            (PeerID::from("3"), peer_3_entry.clone())
        ].iter().cloned().collect();
        let address_book_1 = AddressBook {
            id: PeerID::from("2"),
            mapping: mapping_1,
        };


        let sequence = vec![
            // System adds peer
            (Event::AddPeer(peer_2_entry.clone()),
                Event::PeerAdded(PeerID::from("2"))),

            // Adding again should do nothing
            (Event::AddPeer(peer_2_entry.clone()),
                Event::NoOp()),

            // System triggers a polling operation
            (Event::PollTrigger(), Event::PollPeers(vec![PeerID::from("2")])),

            // Peer:2 responds with an address Book containing peer 3
            (Event::FromPeer(PeerID::from("2"),  PeerMessage::AddressBookResponse(address_book_2)),
                Event::Modified()),

            // peer 2 then asks peer:1 for address book which contains peer 3
            (Event::FromPeer(PeerID::from("2"), PeerMessage::AddressBookRequest()),
                Event::ToPeer(id.clone(), PeerMessage::AddressBookResponse(address_book_1)))
        ];

        for (input, expected_output) in sequence.into_iter() {
            let output = address_book.handle(input);
            assert_eq!(output, expected_output, "expected equality");
        }
    }
    // todo async sequence
}
