# Reactor Experiments

**Champion: <thane@interchain.io>**

## Problem Statement
We need an appropriate concurrency model for our implementation of Tendermint in
Rust. This concurrency model must satisfy the following criteria:

1. It must allow for **deterministic execution**. The model must, in the long
   run, enable [deterministic
   simulation](https://www.youtube.com/watch?v=4fFDFbi3toc) for Tendermint as a
   whole.
2. It must provide good **developer ergonomics**. This is a qualitative
   measurement that must be an indicator as to how easy it is to read and
   understand code written using the model.
3. It must enable **observability**. It should be easy to "probe" the system to
   be able to diagnose and debug issues, as well as to measure performance.

## Milestones

### 6-week check-in (`6-week-checkin`, due 15 November 2019)
This is a "half-way point" milestone to gauge what's working in the project and
what's not.

#### Deliverables
1. Selection of at least 2 concurrency model options (`options`)
2. Lightweight implementations of at least 2 concurrency models (`2-impl`).
   Depends on:
   * `options`
3. Small demo of work done so far (`check-in-demo`). Depends on:
   * `options`
   * `2-impl`

### 3-month completion (`3-month-completion`, due 20 December 2019)
This is the final milestone for this investigation to showcase the final
decision and way forward for the optimal concurrency model for Tendermint in
Rust.

#### Deliverables
1. Architecture decision record (ADR) for selected concurrency model (`adr`).
   Depends on:
   * `check-in-demo`
2. Presentation showcasing the final design and reasoning for the design
   (`final-demo`). Depends on:
   * `adr`

## Solution Design
For more details on the solution design for this project, please see the
[README](../README.md) for the repository.

