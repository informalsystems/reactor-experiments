# Reactor Experiments

We need to establish a concurrency architecture for building Tendermint in Rust,
but we need to experiment with different approaches to establish which will work
best for us.

This repository will eventually contain several proof-of-concept Rust programs
that will demonstrate different concurrency architectures/approaches.

## Approaches
At present, the approaches under consideration include the following.

### 1. Event-driven single-threaded core with offloading

The first approach assumes a model where a Tendermint node will have a
single-threaded event loop handling and dispatching events in a non-blocking
fashion. All long-running operations that would block need to be offloaded to
separate thread pools dedicated to this long-running work.

In this model, all "reactors" are no longer active entities and only provide
conceptual isolation from the remainder of the node's functionality.

Within this model, there are several possibilities for implementation:

1. Async/await - we could make use of Rust's new
   [async/await](https://rust-lang.github.io/async-book/) functionality to
   implement this model.
2. [Tokio](https://tokio.rs/)-based concurrency, which provides higher-level
   abstraction to handle concurrency.
3. [Crossbeam](https://github.com/crossbeam-rs/crossbeam)-based concurrency,
   which provides much lower-level primitives than Tokio for managing
   concurrency.

### 2. Event-driven single-thread-per-reactor model with offloading

The second approach assumes a model where individual reactors are active
entities, each handling their own internal event loop, all communicating to each
other by way of message passing.

Long-running operations are still offloaded into separate threads.

1. Async/await - we could make use of Rust's new
   [async/await](https://rust-lang.github.io/async-book/) functionality to
   implement this model.
2. [Tokio](https://tokio.rs/)-based concurrency, which provides higher-level
   abstraction to handle concurrency.
3. [Crossbeam](https://github.com/crossbeam-rs/crossbeam)-based concurrency,
   which provides much lower-level primitives than Tokio for managing
   concurrency.
4. [Actix](https://actix.rs/)-based concurrency, which provides very high-level
   patterns for building out distributed systems. Use of Actix facilitates
   inter-reactor communication using higher-level patterns than other
   approaches.

## Experimental Design

We want each experimental implementation to meet the following requirements:

1. Must contain multiple stateful components whose states mutate in complex ways
   according to input events.
2. Some operations when interacting with certain stateful components must block
   for relatively long and non-deterministic periods of time.
3. Non-deterministic input events must be able to trigger the influx of events
   into the system (effectively simulating network calls coming in from external
   entities).
4. Must demonstrate handling of prioritized events, such that high-priority
   events are not blocked by lower-priority events.

## Experimental Evaluation

Each experiment implementation will be judged by way of the following criteria:

1. **Developer ergonomics**: a survey of all team members will be taken where
   they will indicate, by way of Likert scale, their assessment of the
   ergonomics of each approach taken. This should include the apparent
   complexity of the resulting solution, as well as the apparent learning curve
   of a particular framework.
2. **Performance benchmarking**: a finite set of deterministic inputs will be
   supplied to each application to determine how quickly it processes these
   inputs.

An appropriate weighting is to be given to each of the above criteria to be able
to qualitatively and quantitatively assess which approach would be best to use
as a starting point for building Tendermint in Rust.

It is assumed that all experiments must meet the criterion of **determinism**
(at least when controlling for non-deterministic inputs), otherwise they will be
instantly disqualified from consideration.

