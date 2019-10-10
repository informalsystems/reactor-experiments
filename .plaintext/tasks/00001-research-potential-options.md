# Research regarding potential concurrency library options

| Owner       | <thane@interchain.io> |
| Due         | 18 November 2019      |
| Uncertainty | High                  |

## Description
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

## Outputs
* A report describing which of the above options are to be considered for
  implementation of the different application concurrency models
* More tasks for implementing the options selected in the report

## Milestones
* `6-week-checkin`:`options`

## Tags
* research
* concurrency
* rust
* tendermint

