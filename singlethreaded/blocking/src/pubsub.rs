//!
//! Pub/sub functionality for internal subscription to node events.
//!

use crate::events::{Event, EventID};
use crate::types::SubscriptionID;
use crossbeam::channel;
use log::debug;
use std::collections::{HashMap, HashSet};

/// A subscription to a particular set of event IDs.
#[derive(Debug, Clone)]
pub struct Subscription {
    pub id: SubscriptionID,
    pub event_ids: HashSet<EventID>,
    pub notify: channel::Sender<Event>,
}

/// Errors relating to subscription management.
#[derive(Debug)]
pub enum SubscriptionError {
    /// The given subscription ID has already been used here.
    IDAlreadyExists,
    /// The event you are attempting to publish has no event ID.
    NoEventIDForEvent,
    /// Failed to publish the event via the notification channel.
    ChannelSendFailed(channel::TrySendError<Event>),
}

impl std::error::Error for SubscriptionError {}

impl std::fmt::Display for SubscriptionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SubscriptionError::IDAlreadyExists => {
                write!(f, "a subscription with the given ID already exists")
            }
            SubscriptionError::NoEventIDForEvent => write!(f, "no event ID for the given event"),
            SubscriptionError::ChannelSendFailed(e) => {
                write!(f, "failed to send event to the given channel: {}", e)
            }
        }
    }
}

/// Designed to provide O(1) lookup time by way of event ID (probably the most
/// frequently called operation), but more linear time for addition/removal of a
/// subscription (relative to the number of event IDs being tracked).
#[derive(Debug, Clone)]
pub struct Subscriptions {
    by_event_id: HashMap<EventID, HashMap<SubscriptionID, channel::Sender<Event>>>,
    subscription_ids: HashSet<SubscriptionID>,
}

impl Subscriptions {
    pub fn new() -> Subscriptions {
        Subscriptions {
            by_event_id: HashMap::new(),
            subscription_ids: HashSet::new(),
        }
    }

    /// Adds a subscription with the given parameters.
    pub fn add(&mut self, subs: Subscription) -> Result<(), SubscriptionError> {
        if self.subscription_ids.contains(&subs.id) {
            return Err(SubscriptionError::IDAlreadyExists);
        }
        for event_id in subs.event_ids {
            if !self.by_event_id.contains_key(&event_id) {
                self.by_event_id.insert(event_id.clone(), HashMap::new());
            }
            // we should definitely have this event ID in here now
            let subs_for_evid = self.by_event_id.get_mut(&event_id).unwrap();
            subs_for_evid.insert(subs.id.clone(), subs.notify.clone());
            debug!("Added {:?} for {:?}", subs.id, event_id);
        }
        self.subscription_ids.insert(subs.id);
        Ok(())
    }

    /// Attempts to publish the given event to all subscribers
    pub fn publish(&self, ev: Event) -> Result<(), SubscriptionError> {
        debug!("Attempting to publish event: {:?}", ev);
        let event_id = ev.id().ok_or(SubscriptionError::NoEventIDForEvent)?;
        let subs_for_evid = match self.by_event_id.get(&event_id) {
            Some(s) => s,
            None => return Ok(()),
        };
        debug!("Publishing event: {:?}", ev);
        // TODO: Should this fail as soon as one channel send fails?
        for notify in subs_for_evid.values() {
            notify
                .try_send(ev.clone())
                .map_err(SubscriptionError::ChannelSendFailed)?;
        }
        Ok(())
    }

    /// Removes subscriptions for the given subscription ID.
    pub fn remove(&mut self, id: SubscriptionID) {
        if !self.subscription_ids.contains(&id) {
            return;
        }
        let event_ids: Vec<EventID> = self.by_event_id.keys().cloned().collect();
        for event_id in event_ids {
            let subs_for_evid = self.by_event_id.get_mut(&event_id).unwrap();
            subs_for_evid.remove(&id);
            if subs_for_evid.is_empty() {
                self.by_event_id.remove(&event_id);
            }
        }
        self.subscription_ids.remove(&id);
    }

    /// Removes the given set of subscriptions.
    pub fn remove_all(&mut self, ids: HashSet<SubscriptionID>) {
        for id in ids {
            self.remove(id);
        }
    }
}
