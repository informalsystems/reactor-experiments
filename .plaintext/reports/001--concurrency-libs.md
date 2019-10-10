# Concurrency Approaches/Libraries for Rust

## Executive Summary
TBD

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

* amenability to deterministic,
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
only the most critical source of input and output for Tendermint, it's also the
primary source of non-determinism in the system.

The best possible approach to dealing with I/O is the one that allows for:

* scalability,
* graceful failure handling,
* traceability (speaks to our observability requirement), and
* the ability to simulate I/O deterministically.

### Non-blocking I/O in general
In terms of **scalability**, it generally seems that **non-blocking I/O**
provides the best possible performance. The naive approach of wanting to loop
through many open sockets [simply does not
work](http://www.wangafu.net/~nickm/libevent-book/01_intro.html), and even if it
did work one's CPU usage would be through the roof. The best approach to
implementing non-blocking I/O seems to be OS-specific, where each OS provides
some kind of mechanism to subscribe to certain kinds of I/O-related events, and
then wait (without burning up the CPU) until such events come through.

In terms of libraries and general approaches,
[libevent](https://github.com/libevent/libevent) (C library) seems to be one of
the most mature and popular libraries, and its primary purpose is to abstract
OS-level interfaces that effectively allow for non-blocking I/O. These
lower-level interfaces include:

* [epoll](http://man7.org/linux/man-pages/man7/epoll.7.html) on Linux
* [kqueue](https://man.openbsd.org/kqueue.2) on BSD and
  [Darwin](https://developer.apple.com/library/archive/documentation/Darwin/Conceptual/FSEvents_ProgGuide/KernelQueues/KernelQueues.html)
* [select](https://manpages.debian.org/buster/manpages-dev/select.2.en.html) on
  POSIX systems
* [select](https://docs.microsoft.com/en-ca/windows/win32/api/winsock2/nf-winsock2-select?redirectedfrom=MSDN)
  on Windows
* others (see the [libevent home page](https://libevent.org/) for
  details)

The high-performance web server [Nginx builds its own abstraction layer on top
of these different
APIs](https://github.com/nginx/nginx/tree/master/src/event/modules). [NodeJS
uses libuv under the hood](https://github.com/nodejs/node/tree/master/deps/uv),
which also wraps all of these interfaces.

### Non-blocking I/O in Rust
In the Rust world, Mio [wraps these underlying
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

### Architectural implications of non-blocking I/O
The architecture of wrapping the underlying OS-level interfaces to non-blocking
I/O seems to be ubiquitous. This architecture has two fundamental implications
for the architecture of one's application:

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
application.

