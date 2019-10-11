# Concurrency Approaches/Libraries for Rust

## Executive Summary
Different approaches to concurrency (especially as they relate to Rust
development) are discussed here, along with the architectural implications of
choosing blocking (multi-threaded) versus non-blocking I/O approaches.  It
appears as though there is only really one major library that facilitates
cross-platform non-blocking (also multi-threaded) I/O in Rust
([Mio](https://github.com/tokio-rs/mio/), on which Tokio is built). It is
recommended, for the sake of time and to consider all options as broadly as
possible, that we attempt to build 4 experiments to showcase (1) the impact of
having a single global event loop for a Tendermint node versus each reactor
having its own event loop, and (2) both blocking and non-blocking I/O approaches
for each of the possible node architectures.

## Related Tasks
* [00001](../tasks/00001--research-potential-options.md)

## Overview

In order to implement an appropriate concurrency model in Rust, we need to
evaluate the different libraries/frameworks available to us for this purpose.
At the time of creating this task, the following options were known and
available:

* Plain Rust with its own standard library concurrency (pre-async/await)
* Rust with async/await support
* Plain Rust with bindings to [libevent](https://github.com/libevent/libevent)
* [Crossbeam](https://github.com/crossbeam-rs/crossbeam)
* [Tokio](https://tokio.rs/)
* [Mio](https://github.com/tokio-rs/mio/) - used by Tokio under the hood, but
  provides lower-level APIs
* [Actix](https://actix.rs/) - a full actor framework

The question is, which of these frameworks/approaches will be most suitable for
our purposes?

In line with the [project goals](../README.md), we need to assess these
different libraries in terms of:

* amenability to deterministic simulation,
* developer ergonomics, and
* observability.

Ideally, whichever solution we choose should additionally:

* be stable, mature and well-tested, and
* have good community support.

If we are choosing an approach as the foundation for building Tendermint in
Rust, we need it to be a solid one.

## Background

### Dealing with I/O
We are most interested in the best possible approach to dealing with I/O, and
when we say I/O we mean specifically socket and filesystem access. I/O is not
only the most critical source of data for Tendermint's correct operation, it's
also the primary source of non-determinism in the system.

The best possible approach to dealing with I/O is the one that allows for:

* no blocking of critical application functions,
* scalability,
* graceful failure handling,
* traceability (speaks to our observability requirement),
* the ability to simulate I/O deterministically, and
* permissive use in terms of licensing.

### Blocking I/O
When the quantity of simultaneous I/O operations is relatively small, blocking
I/O implemented using thread pools (to prevent blocking of critical application
functionality) is quite simple, and the developer ergonomics thereof are great.
Such systems become quite easy to reason about, as long as the threading model
does not become too complex.

What is meant by "relatively small"? This seems to be system resource-dependent.
It could imply a few thousand simultaneous I/O operations on some systems, but
on more constrained hardware it might mean only a few hundred, or even fewer.

### Non-blocking I/O
In terms of **scalability**, it generally seems that **non-blocking I/O**
provides the best possible performance. Looking at how non-blocking I/O is
implemented in many popular systems, the best approach to doing so seems to be
OS-specific. Each OS seems to provide some kind of mechanism to subscribe to
certain kinds of I/O-related events, and then wait (without burning up the CPU)
until such events come through.

To showcase a concrete example of what seems to be the most popular approach to
non-blocking I/O, take a look at
[libevent](https://github.com/libevent/libevent) (C library). Its primary
purpose is to abstract OS-level interfaces that effectively allow for
non-blocking I/O. These lower-level interfaces include:

* [epoll](http://man7.org/linux/man-pages/man7/epoll.7.html) on Linux
* [kqueue](https://man.openbsd.org/kqueue.2) on BSD and
  [Darwin](https://developer.apple.com/library/archive/documentation/Darwin/Conceptual/FSEvents_ProgGuide/KernelQueues/KernelQueues.html)
* [select](https://manpages.debian.org/buster/manpages-dev/select.2.en.html) on
  POSIX systems
* [select](https://docs.microsoft.com/en-ca/windows/win32/api/winsock2/nf-winsock2-select?redirectedfrom=MSDN)
  on Windows
* others (see the [libevent home page](https://libevent.org/) for
  details)

Other examples of popular systems making use of this approach:

* The high-performance web server [Nginx builds its own abstraction layer on top
  of these different
  APIs](https://github.com/nginx/nginx/tree/master/src/event/modules).
* [NodeJS uses libuv under the
  hood](https://github.com/nodejs/node/tree/master/deps/uv), which also wraps
  all of these OS-specific interfaces.

For further, more detailed information on non-blocking I/O in general, see [this
chapter from the libevent
documentation](http://www.wangafu.net/~nickm/libevent-book/01_intro.html).

### Non-blocking I/O in Rust
In the Rust world, [Mio also wraps these underlying
APIs](https://github.com/tokio-rs/mio/tree/master/src/sys). Tokio uses Mio under
the hood, and Actix uses Tokio under the hood, so we can consider all both Tokio
and Actix to just be higher and higher levels of abstraction built on top of
Mio. Even other web servers such as [Hyper](https://hyper.rs/) [make use of
Tokio under the hood](https://github.com/hyperium/hyper/blob/master/Cargo.toml).

In Rust's [async/await
proposal](https://github.com/rust-lang/rfcs/blob/master/text/2394-async_await.md),
they make mention of a `futures::block_on()` method, where this seems to take
the place of the Tokio or Mio event loops. The latest alpha releases of Tokio at
the time of this writing, however, make use of the async/await syntax under the
hood while still using Tokio's underlying event loop.

### Concurrency in Rust
While I/O is one of the biggest influences in terms of concurrency architecture,
we know that the Rust-based implementation of Tendermint will most likely need
to deal with threads and possibly thread pool management. In Go, this was not a
real concern since Go's channels effectively abstracted these concerns away.

When it comes to managing threads and/or thread pools, as well as concurrent
data structures, the following libraries should be considered:

1. [Rust's built-in mpsc library](https://doc.rust-lang.org/std/sync/mpsc/) -
   multiple producer, single consumer channels for communicating between
   threads.
2. [Crossbeam](https://github.com/crossbeam-rs/crossbeam) - provides multiple
   producer, multiple consumer channels, and more data structures for dealing
   with concurrency in Rust.
3. [Rayon](https://github.com/rayon-rs/rayon) - A data parallelism library for
   Rust. This seems like it could be useful for implementing map/reduce-style
   patterns. Rayon provides its own thread pool implementation.
4. [rust-threadpool](https://github.com/rust-threadpool/rust-threadpool) - A
   simple thread pool implementation.

## Architectural Implications

The following sections deal with the implications of choosing the blocking
versus non-blocking I/O-based architecture.

### Implications of blocking I/O
Assuming multi-threading via the use of (possibly configurable) thread pools,
blocking I/O usage provides a very simple way to architect applications. The
chief concerns here are:

1. strict adherence to a message passing architecture (as per Go's philosophy of
   "share state by communicating, instead of communicating by sharing state"),
2. where thread pools are necessary, and
3. the intelligent management of thread pools.

### Implications of non-blocking I/O
The architecture of wrapping the underlying OS-level interfaces to non-blocking
I/O seems to be ubiquitous. In *addition* to the concerns regarding blocking
I/O, this architecture has two fundamental implications for the architecture of
one's application:

1. You will be architecting your application as a collection of callbacks that
   respond to I/O events (of course, these callbacks can be composed in complex
   ways if necessary).
2. The central executor of your application is usually some kind of **event
   loop** or similar execution environment that receives incoming events using
   an OS-specific approach, and calls the relevant callbacks to handle those
   events.

This complex callback composition even further necessitates our requirement for
traceability/observability throughout the system. It also impacts the way that
we think about deterministic simulation, as we cannot necessarily control (or
easily mock out) the underlying event loop that would effectively run our
application. This does not mean that either blocking or non-blocking I/O
approaches are better suited for such deterministic simulation - it just means
that the way we architect the application for blocking and non-blocking I/O may
be slightly different.

### Migrating from blocking to non-blocking I/O
Due to the complexities involved in both approaches, a question naturally
arises: how difficult would it be, at some later stage when conditions change or
the selected approach results in irreconcilable performance issues, to migrate
from the one approach to the other?

If the application is architected in the right way, it should not require a lot
of effort to migrate from one approach to the other.

What does such an architecture look like though?

This is the question that this **Reactor Experiments** project will hopefully
answer. We know intuitively that the architecture most amenable to such
migration would be one that clearly separates the business logic (the Tendermint
core protocols) from the I/O. Incidentally, this is also the type of
architecture that lends itself to deterministic simulation.

## Recommendations

### Architecture
After reviewing some popular approaches to dealing with concurrency in general,
and specifically using Rust, it would appear as though the best approach in the
**Reactor Experiments** project would be to build the following versions of an
application:

1. Single-threaded core event loop, implemented using
   1. blocking I/O, and
   2. non-blocking I/O.
2. One thread per reactor (and one thread for the Tendermint node), implemented
   using:
   1. blocking I/O, and
   2. non-blocking I/O.

Options 1 and 2 above will allow us to showcase the differences between two
high-level architectural approaches to building a Tendermint node, and we will
be able to see the effects of both blocking and non-blocking I/O architectures
for each of those high-level architectures. This will also start giving us a
practical sense of what it would take to migrate from a blocking to a
non-blocking I/O-based architecture in future, should we choose to go down that
route.

### Libraries recommended for experimentation

Common libraries to assist in multi-threading:

* [Crossbeam](https://github.com/crossbeam-rs/crossbeam)
* Either [rust-threadpool](https://github.com/rust-threadpool/rust-threadpool)
  or our own custom thread pool management solution (if building such a solution
  is simple enough)

Blocking I/O:

* Use Rust's standard libraries for I/O with whichever thread pool
  implementation we decide on

Non-blocking I/O:

* [Tokio](https://tokio.rs/), but the pre-async/await version of Tokio

