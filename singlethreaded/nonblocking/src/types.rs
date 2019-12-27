use std::net::IpAddr;
use std::collections::HashMap;
use std::str::FromStr;
use crossbeam::channel;

#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    AddPeer(Entry),
    PeerAdded(PeerID),
    PeersAdded(PeerList),

    PollTrigger(),
    PollPeers(PeerList),

    PeerEvent(PeerID, PeerMessage),

    Terminate(),
    Terminated(),

    Error(),

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
            id: id,
            ip: ip,
            port: port,
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

    fn handle(&mut self, event: Event) -> Event {
        match event {
            Event::AddPeer(entry) => { // will need PeerInfo
                if self.mapping.contains_key(&entry.id) {
                    return Event::NoOp();
                } else {
                    println!("Adding peer {:?}", entry);
                    self.mapping.insert(entry.id.clone(), entry.clone());
                    return Event::PeerAdded(entry.id);
                }
            },
            Event::PollTrigger() => {
                // TODO: Choose LRU 20
                let requested_peers = self.mapping.keys().map(|x| x.to_string()).collect();
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
                return Event::Terminated();
            },
            _ => {
                return Event::NoOp();
            },
        }
    }

    // This can probably be generalized for all runners
    pub fn run_fsm(mut self, send_ch: channel::Sender<Event>, rcv_ch: channel::Receiver<Event>) {
        'event_loop: loop {
            let output = match rcv_ch.recv() {
                Ok(event) => {
                    println!("Event loop received {:?}", event);
                    self.handle(event)
                },
                _ => {
                    // would most likely make sense to return an error here
                    send_ch.send(Event::Error());
                    break 'event_loop;
                }
            };
            match output {
                Event::Terminated() => {
                    println!("Terminating");
                    send_ch.send(Event::Terminated());
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
        let id = PeerID::from("2");
        let ip_addr = IpAddr::from_str("127.0.0.1").unwrap();
        let port: u16 = 0;
        let entry = Entry::new(id.clone(), ip_addr.clone(), port);
        let sequence = vec![
            (Event::AddPeer(entry), Event::PeerAdded(id.clone())),
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
            assert_eq!(output, expected_output, "expected equality");
        }
    }
    // todo async sequence
}
