use std::net::IpAddr;
use std::collections::HashMap;
use std::str::FromStr;
use crossbeam::channel;

// What is the taxonomy of events?
// Each input event produces an output event
// Events come from someone
// Events can be Sent to someone

#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    AddPeer(PeerID),   // from p2p
    PeerAdded(PeerID), // to dev/null
    PeersAdded(PeerList),

    PollTrigger(), // from ticker
    PollPeers(PeerList), // Goto p2p

    PeerEvent(PeerID, PeerMessage),

    Terminate(), // from OS
    Terminated(), // to seed_node

    NoOp(),
}

#[derive(Debug, Clone, PartialEq)]
pub enum PeerMessage {
    AddressBookRequest(),
    AddressBookResponse(AddressBook),
}

pub type PeerID = String;
pub type PeerList = Vec<PeerID>;

/// The address of a peer in the network.
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    /// The cryptographic address of this peer.
    pub id: PeerID,
    /// The IPv4/6 address of this peer.
    pub ip: IpAddr,
    /// The port on which the peer is listening for incoming connections.
    pub port: u16,
}

impl Entry {
    fn new(id: PeerID, ip: IpAddr, port: u16) -> Entry {
        return Entry {
            id: "".to_string(),
            ip: IpAddr::from_str("127.0.0.1").unwrap(),
            port: 0,
        }
    }

    fn default() -> Entry {
        return Entry {
            id: "".to_string(),
            ip: IpAddr::from_str("127.0.0.1").unwrap(),
            port: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AddressBook {
    mapping: HashMap<PeerID, Entry>,
}

impl AddressBook {
    pub fn new() -> AddressBook {
        return AddressBook {
            mapping: HashMap::new(),
        }
    }

    // We can Take requests
    // we can also issue requests
    // We need to produce responses which can be routed to the peer to peer layer
    fn handle(&mut self, event: Event) -> Event {
        match event {
            Event::AddPeer(peer_id) => { // will need PeerInfo
                // TODO: Add to address_book
                return Event::PeerAdded(peer_id);
            },
            Event::PollTrigger() => {
                // TODO: Choose LRU 20
                let requested_peers = vec![PeerID::from("1")];
                return Event::PollPeers(requested_peers);
            },
            Event::PeerEvent(peer_id, message) => {
                match message {
                   PeerMessage::AddressBookRequest() => {
                       let message = PeerMessage::AddressBookResponse(self.clone());
                        return Event::PeerEvent(peer_id, message);
                    },
                    PeerMessage::AddressBookResponse(address_book) => {
                        // TODO: merge address books and return peers added
                        return Event::NoOp();
                    },
                }
            },
            Event::Terminate() => {
                // cleanup
                return Event::Terminated();
            },
            _ => {
                return Event::NoOp();
            },
        }
    }

    // Consume The address book
    pub fn run_fsm(mut self, rcv_ch: channel::Receiver<Event>, send_ch: channel::Sender<Event>) {
        'event_loop: loop {
            let input = rcv_ch.recv().unwrap();
            // I don't think it's possible to mutable borrow here
            // we need the FSM to be a seperate structure
            println!("Event loop received {:?}", input);
            let output = self.handle(input);
            match output {
                Event::Terminate() => {
                    println!("Terminating");
                    break 'event_loop;
                },
                _ => {
                    println!("Handle output {:?}", output);
                    send_ch.send(output);
                }
            }
        }
    }
}

// What is the best way way to produce an address book literal?
// With From

type Entries = Vec<(PeerID, Entry)>;
impl From<Entries> for AddressBook {
    fn from(entries: Entries) -> Self {
	let mapping: HashMap<PeerID, Entry> = entries.iter().cloned().collect();
	return AddressBook {
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
        let mut address_book = AddressBook::new();
        let sequence = vec![
            (Event::AddPeer(PeerID::from("2")), Event::PeerAdded(PeerID::from("2"))),
            (Event::PollTrigger(), Event::PollPeers(vec![PeerID::from("2")])), 

            // TODO: PollRequest
            /*
            (Event::PeerRequest(PeerID::from("2")), Event::PollResponse(
                    AddressBook::from(
                        vec![(PeerID::from("2"), Entry::default())]))),
            */
        ];

        for (input, expected_output) in sequence.into_iter() {
            let output = address_book.handle(input);
            assert_eq!(output, expected_output);
        }
    }
    // todo async sequence
}
