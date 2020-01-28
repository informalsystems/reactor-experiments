### Message-passing that works(in Rust).

We can model concurrent components as threads that run handling one message at a time. The message can be the result of blocking on a `recv` of a single receiver, or by selecting on multiple ones.

A component therefore runs something like:

1. Block on a (select of) channel(s).
2. Handle the message received.
3. Back to 1.

Such algorithm is usually referred to as an "event-loop", or a "message pump".

The component/thread then owns data representing its internal state. When a message is handled, the data is read and/or written to, in order to produce the desired side effect.

A component can also own one, or multiple, sender(s) to other component. Therefore, when a message is handled, not only can the local data be manipulated, one or several message(s) can also be sent on one or several channel(s) the component owns.

Bounded channels can be used when necessary, in other places, unbounded channels are preffered since they remove the risk of deadlock.

Message sends are non-blocking when using an unbounded channel, and an eventual reply, if one is expected, can be received through the regular event-loop of the component. So a component sending a message to another component, will usually not block on the reply. Instead, the event-loop keeps running, allowing the component to do other work while "not waiting" for an eventual reply as a message(if a reply is expected at all).

If necessary, a component can also block on a reply, preventing it's own event-loop from running.

```rust
let (sender, receiver) = channel();
let _ = another_sender.send(Msg::Exit(sender));
// Wait for confirmation of exit by the other component.
let _ = receiver.recv();
```

#### How does a component run?

In order to build some intuition about how components "run", it is important to realize that they represent a linear state-machine only with respect to the messages they receive, and the state transition that occur when the message is handled. A message-send by another component will not affect the state of the receiving component, until the message has been handled.

One should therefore avoid writing logic where the timing of receipt is important. Sending a message should be considered appending an action to a queue, that will eventually be handled by the other component. The sender cannot make assumptions about when that will happen, since it's unknown how many messages are in front of it in the queue, or how much work handling those messages will represent for the receiving component.

Logic can only be deterministic from the perspective of receiving messages, never sending them.

Logic can therefore only be deterministic as a function of these things:

1. The state of the component when the message is received,
2. The action that the component will undertake as a way of handling the message,
3. The potential state transition, and sending of messages to other components, that will occur as a result.

So one could for example only formulate as assumption in the form of: "If component A is in a given state, and it handles a given message, then the following will happen when that message is handled.".

The exception to this rule is when a component sends multiple messages to another component, using the same channel. In such a case, the sending component can rely on the fact that messages will eventually be received in the order that they were sent. However, when they will be received, and how many and what type of messages will be received first, should never be used to build assumptions.

```rust
let (sender, receiver) = channel();
let _ = another_sender.send(Msg::Finish(work));
let _ = another_sender.send(Msg::Exit(sender));
// Wait for confirmation of exit by the other component.
let _ = receiver.recv();
```

So in the above example, once could indeed make the assumption that the component receiving the messages, will indeed first receive `Finish`, followed by `Exit`.

In conclusion, one should think of a component as something that "runs" handling one message at a time. You can send messages to it, and those will be eventually handled(if the component doesn't shut down first), but you can't make much assumptions from the sending side of things.

#### What are the benefits?

Since the state of a component is local to it, the internal logic of the component is effectively single-threaded and sequential. So one can write the logic from a single-threaded perspective, where local state is manipulated, and interaction with other parallel components occur via "send and forget" type of message-passing.

If you need to stop while waiting from a result from a parallel computation, that can be done via a blocking send/recv flow as seen above. If you find yourself needing to do lots of those, you should look into consolidating those components into one, since they obviously depend a lot on each other for synchronous operations.

#### Can you build a larger system in this way?

If you can only write logic from the perspective of receiving messages, and should consider sending a messages as appending an action to an opaque queue, how can you build a large concurrent system consisting of deterministic workflows?

Since each component can be deterministic from the perspective of its message-handling, and the state transitions and message-sends that occur as a result, a larger system of components can also be deterministic with regards to workflows that respect the basic constraints of message-passing.

It is important to note that it is not when a component handles a message, or what messages will be handled first or after, that is deterministic.

It is rather that:
1. given one, or many, queue(s) of messages(created by sending messages to one or multiple components),
2. A sequence of state-transition is known to eventually occur,
3. And each of these state-transitions can be paired by adding more messages to other queues, resulting eventually in other state-transitions/queueing of messages to other parts of the system.

Quite simply, if component A receives a set of messages, this will result in local state transitions, and perhaps the sending of messages to component B, or C. Since each B or C have their own dedicated message queue, we cannot make assumptions about the relationship between both queues. However we can assume those messages will eventually be handled, resulting in local changes and perhaps more messages added to more queues.

Eventually, if we write logic that avoids pitfalls of assuming queues will somehow synchronize between each other when they do not, the system as a whole, or at least a given workflow from beginning to completion on the system, can be seen as simply one large deterministic queue of operations that will eventually be completed.

#### How to express relationships between components?
Relationships between components are expressed by sharing senders to component that you want to receive messages from. A sender can be cloned to multiple components, representing a "one to many" relationship where one component expect many different components to send it messages on the same channel. Or many "one to one" relationships can be build by using different channels.
Those relationships can be long- or short-lived. Long-lived relationship are typically represented by channels whose receiver a component includes in its event-loop, while shorter lived relationship will see channels shared only to get a single, or limited number or, response(s) and typicall not added to the select of an event-loop.

One can then build a topology of components based on their relationships and expressed by how they share senders to each other. Hierarchies of components can also be built, where one component owns several sub-components, who do not or do own senders to components outside of this hierarchy. Sometimes, the "main" component owns a sender to another component, and clones for sharing with one or several of its sub-components, in which case they will have a direct link to another "main" component. However note this doesn't mean that other "main" component needs to be aware of the internal structure of another component. From the perspective of the other component, any messages is simply coming from the component with whom a sender was shared, whether it was sent internally via a sub-component or not.

Senders are essentially shared between component to support the variety of workflow they need to perform.

#### An example: concurrent address book.

An example can be found at `multithreaded/src/concurrent_addr_book.rs`. The goal of the example is to show how a system of component can reach a kind of "eventual determinism", simply by ensuring each component is expressed as a sequence of predictable state-transition, even when we add some randomness to the topology of the components.

An `AddrBook` owns some local state, a map of peer Ids to senders to that peer, and periodically send requests for peers to its known peers. When receiving such a request, it randomly selects a potential peer and forwards a channel to the requestor, who will then perform a handshake with the unknown peer.

So, even though effectively a different random topology of nodes is created on each test, they eventually all connect to each other, simply because that behavior is encoded in their internal message-handling behavior.

The logic of the code is entirely written from the perspective of one such `AddrBook`, running one message at a time, mutating local state, and non-blockingly sending messages on channels.