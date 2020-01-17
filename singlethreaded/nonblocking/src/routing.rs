use crate::acceptor::Acceptor;
use crate::dispatcher::Dispatch;
use crossbeam::channel;
use tokio::task;

fn routing() {
    let (events_send, events_receive) = channel::unbounded<Event>();

    let (acceptor_sender, acceptor_receiver) = channel::bounded::<Event>(0);
    let acceptor = Acceptor::new(); // maybe pass  address and port
    let acceptor_handler = task::spawn(async move {
        acceptor.run(acceptor_receiver, event_send.clone());
    });

    let (dispatcher_sender, dispatcher_receiver) = channel::bounded::<Event>(0);
    let dispatcher = Dispatcher::new();
    let dispatcher_handler = task::spawn(async move {
        dispatcher.run(dispatcher_receiver, event_send.clone());
    }

    let (ab_sender, ab_receiver) = channel::bounded::<Event>(0);
    let address_book = AddressBook::new();
    let ab_handler = task::spawn(async move {
        dispatcher.run(dispatcher_receiver, event_send.clone());
    }

    for event in events_receive.iter() {
        match event {
            Event::Connection(socket) => {
                // Establish a new peer
                dispatcher.send(event);
            },
            Event::Connected(peerID) => {
                // The peer has connected and is identified
                fsm.send(event);
            },
            Event::PeerReceive(peerID, Message) => {
                // message from peer, route the the fsm
                fsm.send(event);
            },
            Event::PeerSend(peerID, Message) => {
                dipatch.send(event);
            },
            // TODO: Errors?
            // XXX: Do we need to do this?
            Event::Terminate() => {
                acceptor_sender.send(event.clone());
                break;
            },
        }
    }
}
